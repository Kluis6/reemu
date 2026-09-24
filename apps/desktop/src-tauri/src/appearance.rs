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

// --- tema de cor --------------------------------------------------------
//
// A escolha de tema (`useThemeStore` no frontend) morava só no
// `localStorage`, que é POR ORIGEM: dev (`http://127.0.0.1:1420`), produção
// no Linux (`tauri://localhost`) e no Windows (`http://tauri.localhost`) têm
// cada um o seu — o tema escolhido "sumia" entre eles. Fica aqui a fonte da
// verdade; o `localStorage` segue como cache síncrono (o app abre já com o
// tema certo, sem piscar o padrão). O Rust só guarda o JSON: quem valida o
// formato é o frontend (`isSelection`), que conhece os temas.

const THEME_FILE: &str = "theme.json";
const THEME_MAX_BYTES: usize = 4 * 1024;

fn read_theme(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(THEME_FILE)).unwrap_or_default()
}

fn write_theme(dir: &Path, json: &str) -> Result<(), String> {
    if json.len() > THEME_MAX_BYTES {
        return Err("tema grande demais".into());
    }
    serde_json::from_str::<serde_json::Value>(json).map_err(|e| format!("tema inválido: {e}"))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{THEME_FILE}.tmp"));
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join(THEME_FILE)).map_err(|e| e.to_string())
}

/// JSON da escolha de tema salva (vazio se nunca escolheu).
#[tauri::command]
pub fn get_theme_selection(state: State<'_, AppState>) -> String {
    read_theme(&state.appearance_dir)
}

#[tauri::command]
pub fn set_theme_selection(state: State<'_, AppState>, json: String) -> Result<(), String> {
    write_theme(&state.appearance_dir, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_roundtrip_and_validation() {
        let dir = std::env::temp_dir().join(format!("reemu-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(read_theme(&dir), "", "sem arquivo = vazio");

        let json = r#"{"selection":{"kind":"preset","id":"alto-contraste"}}"#;
        write_theme(&dir, json).unwrap();
        assert_eq!(read_theme(&dir), json);

        assert!(write_theme(&dir, "{nao-e-json").is_err());
        assert!(write_theme(&dir, &"x".repeat(THEME_MAX_BYTES + 1)).is_err());
        assert_eq!(
            read_theme(&dir),
            json,
            "escrita recusada não estraga o salvo"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
