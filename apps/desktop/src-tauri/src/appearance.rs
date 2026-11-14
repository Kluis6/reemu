//! Papel de parede da tela inicial (opcional, um por instalação) — mesmo
//! padrão do avatar do perfil (`profile.rs`): a imagem escolhida vai pra
//! `<dados>/appearance/wallpaper.<ext>` e é servida por IPC; a presença do
//! arquivo É o estado (sem flag "ligado/desligado" separada no banco).
//!
//! Fica só no filesystem, sem tabela — não tem nenhum outro dado associado
//! (diferente do perfil, que tem nome/bio). `AnimatedBackground` desenha
//! esta imagem como a camada mais no fundo, ATRÁS das manchas de cor do tema
//! (que continuam por cima, translúcidas).
//!
//! Ou um dos papéis de parede embutidos em vídeo: aí só o id fica em
//! `wallpaper.preset` (os vídeos vêm com o app, em `public/wallpapers/`).

use std::path::{Path, PathBuf};

use tauri::State;

use crate::app_error::{AppError, ErrorCode};
use crate::commands::AppState;

/// Imagens que a pessoa pode escolher do disco (`wallpaper.<ext>`).
const WALLPAPER_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];
const MAX_BYTES: u64 = 20 * 1024 * 1024;
/// Papel de parede embutido escolhido: só o id ("1".."6"). Os vídeos vêm com
/// o app (`packages/app-desktop/public/wallpapers/<id>.webm`, gerados por
/// `scripts/wallpaper-video/render.py`); aqui fica só qual deles.
const PRESET_FILE: &str = "wallpaper.preset";
const PRESET_IDS: &[&str] = &["1", "2", "3", "4", "5", "6"];

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

/// Tira qualquer papel de parede: a imagem, o embutido e o `wallpaper.svg`
/// de um build anterior (os embutidos foram SVG antes de virar vídeo).
fn remove_all(dir: &Path) {
    for e in WALLPAPER_EXTS.iter().chain(&["svg"]) {
        let _ = std::fs::remove_file(dir.join(format!("wallpaper.{e}")));
    }
    let _ = std::fs::remove_file(dir.join(PRESET_FILE));
}

fn write_preset(dir: &Path, id: &str) -> Result<(), AppError> {
    if !PRESET_IDS.contains(&id) {
        return Err(AppError::new(
            ErrorCode::NotFound,
            "papel de parede desconhecido",
        ));
    }
    std::fs::create_dir_all(dir)?;
    remove_all(dir);
    std::fs::write(dir.join(PRESET_FILE), id)?;
    Ok(())
}

fn read_preset(dir: &Path) -> Option<String> {
    let id = std::fs::read_to_string(dir.join(PRESET_FILE)).ok()?;
    let id = id.trim();
    PRESET_IDS.contains(&id).then(|| id.to_string())
}

/// Bytes da imagem escolhida (vazio se não há nenhuma). Frontend
/// embrulha num `Blob` → `blob:` URL, igual ao avatar.
#[tauri::command]
pub async fn read_wallpaper(state: State<'_, AppState>) -> Result<tauri::ipc::Response, AppError> {
    let bytes = wallpaper_file(&state.appearance_dir)
        .and_then(|p| std::fs::read(&p).ok())
        .unwrap_or_default();
    Ok(tauri::ipc::Response::new(bytes))
}

/// Copia `src_path` (imagem escolhida pelo usuário) pra
/// `<dados>/appearance/wallpaper.<ext>`, no lugar do que houver (inclusive
/// um embutido).
#[tauri::command]
pub async fn set_wallpaper_file(
    state: State<'_, AppState>,
    src_path: String,
) -> Result<(), AppError> {
    let src = PathBuf::from(&src_path);
    let ext = ext_of(&src).ok_or("formato não suportado (use PNG, JPG ou WEBP)")?;
    let meta = std::fs::metadata(&src)?;
    if meta.len() > MAX_BYTES {
        return Err("imagem grande demais (máx. 20 MB)".into());
    }
    std::fs::create_dir_all(&state.appearance_dir)?;
    remove_all(&state.appearance_dir);
    let dest = state.appearance_dir.join(format!("wallpaper.{ext}"));
    std::fs::copy(&src, &dest).map_err(|e| {
        let mut err = AppError::from(e);
        err.message = format!("copiar papel de parede: {}", err.message);
        err
    })?;
    log::info!("papel de parede salvo em {}", dest.display());
    Ok(())
}

/// Escolhe um dos papéis de parede embutidos (em vídeo), no lugar do que
/// houver.
#[tauri::command]
pub async fn set_wallpaper_preset(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    write_preset(&state.appearance_dir, &id)?;
    log::info!("papel de parede embutido: {id}");
    Ok(())
}

/// Qual papel de parede embutido está escolhido (`None` = nenhum ou imagem).
#[tauri::command]
pub fn get_wallpaper_preset(state: State<'_, AppState>) -> Option<String> {
    read_preset(&state.appearance_dir)
}

/// Remove o papel de parede escolhido (volta a ser só a cor do tema).
#[tauri::command]
pub async fn clear_wallpaper(state: State<'_, AppState>) -> Result<(), AppError> {
    remove_all(&state.appearance_dir);
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

fn write_theme(dir: &Path, json: &str) -> Result<(), AppError> {
    if json.len() > THEME_MAX_BYTES {
        return Err("tema grande demais".into());
    }
    serde_json::from_str::<serde_json::Value>(json)
        .map_err(|e| AppError::new(ErrorCode::Corrupt, format!("tema inválido: {e}")))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{THEME_FILE}.tmp"));
    std::fs::write(&tmp, json)?;
    Ok(std::fs::rename(&tmp, dir.join(THEME_FILE))?)
}

/// JSON da escolha de tema salva (vazio se nunca escolheu).
#[tauri::command]
pub fn get_theme_selection(state: State<'_, AppState>) -> String {
    read_theme(&state.appearance_dir)
}

#[tauri::command]
pub fn set_theme_selection(state: State<'_, AppState>, json: String) -> Result<(), AppError> {
    write_theme(&state.appearance_dir, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embutido_so_ids_conhecidos_e_substitui_a_imagem() {
        let dir = std::env::temp_dir().join(format!("reemu-wall-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("wallpaper.png"), b"png").unwrap();
        std::fs::write(dir.join("wallpaper.svg"), b"<svg/>").unwrap();

        write_preset(&dir, "6").unwrap();
        assert_eq!(read_preset(&dir).as_deref(), Some("6"));
        assert!(wallpaper_file(&dir).is_none(), "a imagem sai");
        assert!(!dir.join("wallpaper.svg").exists(), "o SVG antigo sai");

        assert!(write_preset(&dir, "7").is_err());
        assert!(write_preset(&dir, "../x").is_err());
        std::fs::write(dir.join(PRESET_FILE), "../../etc").unwrap();
        assert_eq!(read_preset(&dir), None, "conteúdo estranho não vale");

        remove_all(&dir);
        assert_eq!(read_preset(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

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
