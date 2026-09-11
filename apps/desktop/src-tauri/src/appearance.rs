//! Papel de parede da tela inicial (opcional, um por instalação) — mesmo
//! padrão do avatar do perfil (`profile.rs`): a imagem escolhida vai pra
//! `<dados>/appearance/wallpaper.<ext>` e é servida por IPC; a presença do
//! arquivo É o estado (sem flag "ligado/desligado" separada no banco).
//!
//! Fica só no filesystem, sem tabela — não tem nenhum outro dado associado
//! (diferente do perfil, que tem nome/bio). `AnimatedBackground` desenha
//! esta imagem como a camada mais no fundo, ATRÁS das manchas de cor do tema
//! (que continuam por cima, translúcidas).

use std::path::{Path, PathBuf};

use tauri::State;

use crate::commands::AppState;

const WALLPAPER_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const MAX_BYTES: u64 = 20 * 1024 * 1024;

fn wallpaper_file(dir: &Path) -> Option<PathBuf> {
    WALLPAPER_EXTS
        .iter()
        .map(|e| dir.join(format!("wallpaper.{e}")))
        .find(|p| p.is_file())
}

fn ext_of(path: &Path) -> Option<String> {
    let e = path.extension()?.to_str()?.to_ascii_lowercase();
    WALLPAPER_EXTS.contains(&e.as_str()).then_some(e)
}

/// Bytes do papel de parede escolhido (vazio se não há nenhum). Frontend
/// embrulha num `Blob` → `blob:` URL, igual ao avatar.
#[tauri::command]
pub async fn read_wallpaper(state: State<'_, AppState>) -> Result<tauri::ipc::Response, String> {
    let bytes = wallpaper_file(&state.appearance_dir)
        .and_then(|p| std::fs::read(&p).ok())
        .unwrap_or_default();
    Ok(tauri::ipc::Response::new(bytes))
}

/// Copia `src_path` (imagem escolhida pelo usuário) pra
/// `<dados>/appearance/wallpaper.<ext>`, removendo qualquer papel de parede
/// anterior.
#[tauri::command]
pub async fn set_wallpaper_file(
    state: State<'_, AppState>,
    src_path: String,
) -> Result<(), String> {
    let src = PathBuf::from(&src_path);
    let ext = ext_of(&src).ok_or("formato não suportado (use PNG, JPG ou WEBP)")?;
    let meta = std::fs::metadata(&src).map_err(|e| e.to_string())?;
    if meta.len() > MAX_BYTES {
        return Err("imagem grande demais (máx. 20 MB)".into());
    }
    std::fs::create_dir_all(&state.appearance_dir).map_err(|e| e.to_string())?;
    for e in WALLPAPER_EXTS {
        let _ = std::fs::remove_file(state.appearance_dir.join(format!("wallpaper.{e}")));
    }
    let dest = state.appearance_dir.join(format!("wallpaper.{ext}"));
    std::fs::copy(&src, &dest).map_err(|e| format!("copiar papel de parede: {e}"))?;
    log::info!("papel de parede salvo em {}", dest.display());
    Ok(())
}

/// Remove o papel de parede escolhido (volta a ser só a cor do tema).
#[tauri::command]
pub async fn clear_wallpaper(state: State<'_, AppState>) -> Result<(), String> {
    for e in WALLPAPER_EXTS {
        let _ = std::fs::remove_file(state.appearance_dir.join(format!("wallpaper.{e}")));
    }
    Ok(())
}
