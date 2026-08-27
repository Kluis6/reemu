//! Orquestração de save state (etapa 08): serializa → grava o arquivo em
//! disco → registra a metadata via `SaveStateRepository`; no load valida que
//! o `core_id` bate (states não são portáveis entre cores).
//!
//! O binário do state vai pra disco (pode ser MB); o banco só guarda o
//! `file_path` + metadata. Thumbnail ainda não (continua a etapa 08).

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

/// Grava `state_bytes` e registra a metadata. Se `slot` já estava ocupado,
/// o state anterior nele é apagado (arquivo + registro).
pub async fn save<R: SaveStateRepository + ?Sized>(
    repo: &R,
    save_dir: &Path,
    rom_id: &str,
    core_id: &str,
    slot: Option<u32>,
    state_bytes: &[u8],
) -> Result<SaveStateMetadata, SaveError> {
    std::fs::create_dir_all(save_dir)?;

    if let Some(s) = slot {
        if let Some(old) = repo.find_state_in_slot(rom_id, core_id, s).await? {
            let _ = std::fs::remove_file(&old.file_path);
            repo.delete_state(&old.id).await?;
        }
    }

    let path = state_path(save_dir, rom_id, core_id, slot);
    std::fs::write(&path, state_bytes)?;

    let meta = SaveStateMetadata {
        id: uuid::Uuid::new_v4().to_string(),
        rom_id: rom_id.to_string(),
        core_id: core_id.to_string(),
        slot,
        file_path: path.to_string_lossy().into_owned(),
        thumbnail_path: None,
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
    if meta.core_id != running {
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
    }
    repo.delete_state(state_id).await?;
    Ok(())
}
