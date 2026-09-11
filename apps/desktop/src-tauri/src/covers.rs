//! Cache local das capas de jogo, servido pelo protocolo custom
//! `cover://`. Antes, `list_roms` mandava direto a URL de
//! `thumbnails.libretro.com` no `boxart` e o `<img>` buscava da rede toda
//! vez — sem internet, sem capa. Agora `boxart` aponta pra
//! `cover://localhost/<rom_id>`: este protocolo confere se já tem a capa
//! em `<dados>/covers/<system_id>/<rom_id>.png`; se sim, serve do disco
//! (funciona offline); se não, baixa da libretro, grava no cache e serve —
//! só precisa de rede na primeira vez que aquele jogo aparece na tela.
//!
//! Falha (sem cobertura da libretro pro sistema, sem rede na 1ª tentativa,
//! 404) responde com HTTP 404 e NADA fica gravado — o `<img onError>` do
//! frontend cai no placeholder de iniciais, e tenta de novo normalmente na
//! próxima vez que o card renderizar (ex.: reabrir o app já com rede).

use domain::library::RomRepository;
use tauri::http;
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::commands::{rom_title, AppState};

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

fn cache_path(covers_dir: &std::path::Path, system_id: &str, rom_id: &str) -> std::path::PathBuf {
    covers_dir
        .join(sanitize(system_id))
        .join(format!("{}.png", sanitize(rom_id)))
}

fn not_found() -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(404)
        .body(Vec::new())
        .unwrap()
}

fn ok_png(bytes: Vec<u8>) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(200)
        .header("Content-Type", "image/png")
        .header("Cache-Control", "no-cache")
        .body(bytes)
        .unwrap()
}

/// Registra `cover://` no builder. Chamar antes de `.setup()`/`.run()` —
/// o handler só roda depois, quando o `AppState` já está gerenciado.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(
        "cover",
        |ctx: UriSchemeContext<'_, R>, request, responder: UriSchemeResponder| {
            let app = ctx.app_handle().clone();
            let rom_id = request
                .uri()
                .path()
                .trim_start_matches('/')
                .to_string();
            tauri::async_runtime::spawn(async move {
                responder.respond(resolve(&app, &rom_id).await);
            });
        },
    )
}

async fn resolve<R: Runtime>(app: &tauri::AppHandle<R>, rom_id: &str) -> http::Response<Vec<u8>> {
    if rom_id.is_empty() {
        return not_found();
    }
    let state = app.state::<AppState>();
    let Some(pool) = state.db.clone() else {
        return not_found();
    };
    let Ok(Some(rom)) = db::RomsRepo::new(pool).get(rom_id).await else {
        return not_found();
    };
    let path = cache_path(&state.covers_dir, &rom.system_id, rom_id);
    if let Ok(bytes) = std::fs::read(&path) {
        return ok_png(bytes);
    }

    let title = rom_title(&rom);
    let Some(url) = library_scan::libretro_boxart_url(&rom.system_id, &title) else {
        return not_found();
    };
    let Ok(resp) = reqwest::get(&url).await else {
        return not_found();
    };
    if !resp.status().is_success() {
        return not_found();
    }
    let Ok(bytes) = resp.bytes().await else {
        return not_found();
    };
    let bytes = bytes.to_vec();

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, &bytes) {
        log::warn!("cache de capa: não gravou {} ({e})", path.display());
    }
    ok_png(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_is_stable_and_scoped_by_system() {
        let dir = std::path::Path::new("/data/covers");
        let a = cache_path(dir, "megadrive", "rom-1");
        let b = cache_path(dir, "snes", "rom-1");
        assert_eq!(a, dir.join("megadrive").join("rom-1.png"));
        assert_ne!(a, b, "mesmo rom_id em sistemas diferentes não pode colidir");
    }

    #[test]
    fn cache_path_sanitizes_unsafe_characters() {
        let dir = std::path::Path::new("/data/covers");
        let p = cache_path(dir, "mega/drive", "../../etc");
        assert_eq!(p, dir.join("mega_drive").join(".._.._etc.png"));
    }
}
