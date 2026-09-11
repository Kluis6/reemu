//! Orquestração de save state (etapa 08): serializa → grava o arquivo em
//! disco → registra a metadata via `SaveStateRepository`; no load valida que
//! o `core_id` bate (states não são portáveis entre cores).
//!
//! O binário do state vai pra disco (pode ser MB); o banco só guarda o
//! `file_path` + metadata. O thumbnail (PNG já codificado pelo caller) é
//! gravado ao lado, com a mesma stem + `.png`.

use domain::save_state::{SaveStateMetadata, SaveStateRepository};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Repo(#[from] domain::error::RepoError),
    #[error("save state não encontrado")]
    NotFound,
    #[error("save state é do core '{state}', mas o core carregado é '{running}'")]
    CoreMismatch { state: String, running: String },
    #[error("nenhum core carregado")]
    NoCore,
}

/// Chave de compatibilidade de save state. Algumas builds só trocam o
/// renderer do MESMO motor de emulação (ex.: `mednafen_psx_libretro` vs
/// `mednafen_psx_hw_libretro` — o Beetle PSX "normal" e o "HW"/Vulkan são o
/// mesmo `libretro/beetle-psx-libretro`, compilado com/sem HAVE_HW; o
/// `retro_serialize`/`retro_unserialize` é idêntico, só o vídeo muda). Pra
/// esses, o state de um carrega no outro — normaliza os dois pro mesmo nome
/// de base. Qualquer outro core_id passa direto (sem par conhecido).
fn save_family(core_id: &str) -> std::borrow::Cow<'_, str> {
    match core_id.strip_suffix("_hw_libretro") {
        Some(base) => std::borrow::Cow::Owned(format!("{base}_libretro")),
        None => std::borrow::Cow::Borrowed(core_id),
    }
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn state_path(dir: &Path, rom_id: &str, core_id: &str, slot: Option<u32>) -> PathBuf {
    let slot = slot.map_or_else(|| "quick".to_string(), |s| format!("slot{s}"));
    dir.join(format!(
        "{}.state",
        sanitize(&format!("{rom_id}__{core_id}__{slot}"))
    ))
}

/// Grava `state_bytes` (+ `thumbnail_png` ao lado, se dado) e registra a
/// metadata. Se `slot` já estava ocupado, o state anterior nele é apagado
/// (arquivo + thumbnail + registro).
pub async fn save<R: SaveStateRepository + ?Sized>(
    repo: &R,
    save_dir: &Path,
    rom_id: &str,
    core_id: &str,
    slot: Option<u32>,
    state_bytes: &[u8],
    thumbnail_png: Option<&[u8]>,
) -> Result<SaveStateMetadata, SaveError> {
    std::fs::create_dir_all(save_dir)?;

    if let Some(s) = slot {
        if let Some(old) = repo.find_state_in_slot(rom_id, core_id, s).await? {
            let _ = std::fs::remove_file(&old.file_path);
            if let Some(t) = &old.thumbnail_path {
                let _ = std::fs::remove_file(t);
            }
            repo.delete_state(&old.id).await?;
        }
    }

    let path = state_path(save_dir, rom_id, core_id, slot);
    std::fs::write(&path, state_bytes)?;

    let thumbnail_path = match thumbnail_png {
        Some(png) => {
            let tp = path.with_extension("png");
            std::fs::write(&tp, png)?;
            Some(tp.to_string_lossy().into_owned())
        }
        None => None,
    };

    let meta = SaveStateMetadata {
        id: uuid::Uuid::new_v4().to_string(),
        rom_id: rom_id.to_string(),
        core_id: core_id.to_string(),
        slot,
        file_path: path.to_string_lossy().into_owned(),
        thumbnail_path,
        created_at: now_unix(),
        play_time_at_save: None,
    };
    repo.record_state(&meta).await?;
    Ok(meta)
}

/// Lê os bytes de um save state, validando o core. Devolve pro caller passar
/// pro `retro_unserialize` (via `EmuSession::restore_state`).
pub async fn load_bytes<R: SaveStateRepository + ?Sized>(
    repo: &R,
    state_id: &str,
    running_core: Option<&str>,
) -> Result<SaveStateMetadata, SaveError> {
    let meta = repo.get_state(state_id).await?.ok_or(SaveError::NotFound)?;
    let running = running_core.ok_or(SaveError::NoCore)?;
    if save_family(&meta.core_id) != save_family(running) {
        return Err(SaveError::CoreMismatch {
            state: meta.core_id,
            running: running.to_string(),
        });
    }
    Ok(meta)
}

pub async fn list<R: SaveStateRepository + ?Sized>(
    repo: &R,
    rom_id: &str,
) -> Result<Vec<SaveStateMetadata>, SaveError> {
    Ok(repo.list_states_for_rom(rom_id).await?)
}

pub async fn delete<R: SaveStateRepository + ?Sized>(
    repo: &R,
    state_id: &str,
) -> Result<(), SaveError> {
    if let Some(meta) = repo.get_state(state_id).await? {
        let _ = std::fs::remove_file(&meta.file_path);
        if let Some(t) = &meta.thumbnail_path {
            let _ = std::fs::remove_file(t);
        }
    }
    repo.delete_state(state_id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::save_family;

    #[test]
    fn hw_and_sw_beetle_psx_share_a_family() {
        assert_eq!(
            save_family("mednafen_psx_libretro"),
            save_family("mednafen_psx_hw_libretro")
        );
    }

    #[test]
    fn unrelated_cores_keep_their_own_family() {
        assert_ne!(save_family("mesen"), save_family("nestopia"));
        assert_ne!(
            save_family("mednafen_psx_libretro"),
            save_family("mednafen_saturn_libretro")
        );
    }

    #[test]
    fn core_without_hw_sibling_is_its_own_family() {
        assert_eq!(save_family("mesen"), "mesen");
    }
}
