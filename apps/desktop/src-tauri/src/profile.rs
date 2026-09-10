//! Perfil local (um por instalação) — comandos Tauri sobre
//! `domain::profile::ProfileRepository` + a imagem de avatar que o usuário
//! escolheu, guardada em `<dados>/profile/avatar.<ext>` e servida por IPC
//! (mesmo padrão do thumbnail de save state).

use std::path::{Path, PathBuf};

use tauri::State;

use crate::commands::AppState;

const AVATAR_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDto {
    pub name: String,
    pub bio: Option<String>,
    /// `"preset:1"`..`"preset:5"` ou `"file"`.
    pub avatar: String,
    pub onboarded: bool,
}

fn avatar_file(profile_dir: &Path) -> Option<PathBuf> {
    AVATAR_EXTS
        .iter()
        .map(|e| profile_dir.join(format!("avatar.{e}")))
        .find(|p| p.is_file())
}

fn ext_of(path: &Path) -> Option<String> {
    let e = path.extension()?.to_str()?.to_ascii_lowercase();
    AVATAR_EXTS.contains(&e.as_str()).then_some(e)
}

/// Perfil atual. Sem banco → devolve um perfil "já onboardado" pra não
/// prender o usuário no fluxo de 1ª execução num estado já quebrado.
#[tauri::command]
pub async fn get_profile(state: State<'_, AppState>) -> Result<ProfileDto, String> {
    use domain::profile::ProfileRepository;
    let Some(pool) = state.db.clone() else {
        log::warn!("get_profile sem banco — devolvendo perfil padrão");
        return Ok(ProfileDto {
            name: "Jogador".into(),
            bio: None,
            avatar: "preset:1".into(),
            onboarded: true,
        });
    };
    let p = db::ProfileRepo::new(pool)
        .get()
        .await
        .map_err(|e| e.to_string())?;
    Ok(ProfileDto {
        name: p.name,
        bio: p.bio,
        avatar: p.avatar,
        onboarded: p.onboarded,
    })
}

/// Bytes da imagem de avatar do usuário (vazio se ele usa um preset ou ainda
/// não escolheu). Frontend embrulha num `Blob` → `blob:` URL.
#[tauri::command]
pub async fn read_profile_avatar(
    state: State<'_, AppState>,
) -> Result<tauri::ipc::Response, String> {
    let bytes = avatar_file(&state.profile_dir)
        .and_then(|p| std::fs::read(&p).ok())
        .unwrap_or_default();
    Ok(tauri::ipc::Response::new(bytes))
}

/// Salva nome/bio/avatar e marca o onboarding como concluído. `name` vazio é
/// recusado. `avatar` = `"preset:N"` (1..5) ou `"file"` (a imagem já tem que
/// ter sido copiada via `set_profile_avatar_file`).
#[tauri::command]
pub async fn set_profile(
    state: State<'_, AppState>,
    name: String,
    bio: Option<String>,
    avatar: String,
) -> Result<(), String> {
    use domain::profile::{Profile, ProfileRepository};
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("o nome não pode ficar vazio".into());
    }
    if name.chars().count() > 40 {
        return Err("nome muito longo (máx. 40)".into());
    }
    let ok_avatar = avatar == "file"
        || avatar
            .strip_prefix("preset:")
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=5).contains(&n));
    if !ok_avatar {
        return Err(format!("avatar inválido: '{avatar}'"));
    }
    if avatar == "file" && avatar_file(&state.profile_dir).is_none() {
        return Err("nenhuma imagem de avatar foi carregada".into());
    }
    let bio = bio
        .map(|b| b.trim().to_string())
        .filter(|b| !b.is_empty())
        .map(|b| b.chars().take(280).collect::<String>());

    let pool = state
        .db
        .clone()
        .ok_or("sem banco — não dá pra salvar o perfil")?;
    db::ProfileRepo::new(pool)
        .update(&Profile {
            name,
            bio,
            avatar,
            onboarded: true,
        })
        .await
        .map_err(|e| e.to_string())
}

/// Copia `src_path` (imagem escolhida pelo usuário) pra
/// `<dados>/profile/avatar.<ext>`, removendo qualquer avatar anterior.
#[tauri::command]
pub async fn set_profile_avatar_file(
    state: State<'_, AppState>,
    src_path: String,
) -> Result<(), String> {
    let src = PathBuf::from(&src_path);
    let ext = ext_of(&src).ok_or("formato não suportado (use PNG, JPG, WEBP ou GIF)")?;
    let meta = std::fs::metadata(&src).map_err(|e| e.to_string())?;
    if meta.len() > 8 * 1024 * 1024 {
        return Err("imagem grande demais (máx. 8 MB)".into());
    }
    std::fs::create_dir_all(&state.profile_dir).map_err(|e| e.to_string())?;
    for e in AVATAR_EXTS {
        let _ = std::fs::remove_file(state.profile_dir.join(format!("avatar.{e}")));
    }
    let dest = state.profile_dir.join(format!("avatar.{ext}"));
    std::fs::copy(&src, &dest).map_err(|e| format!("copiar avatar: {e}"))?;
    log::info!("avatar do perfil salvo em {}", dest.display());
    Ok(())
}
