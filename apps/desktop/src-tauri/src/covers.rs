//! Cache local das capas de jogo, servido pelo protocolo custom
//! `cover://`. Antes, `list_roms` mandava direto a URL de
//! `thumbnails.libretro.com` no `boxart` e o `<img>` buscava da rede toda
//! vez — sem internet, sem capa. Agora `boxart` aponta pra
//! `cover://localhost/<rom_id>`: este protocolo confere se já tem a capa
//! em `<dados>/covers/<rom_id>.png`; se sim, serve do disco (funciona
//! offline); se não, baixa e grava no cache — só precisa de rede na
//! primeira vez que aquele jogo aparece na tela.
//!
//! Fonte, em ordem de prioridade: a `cover_url` escrapeada (ScreenScraper,
//! `game_metadata` — só existe se o jogo já foi escrapeado) senão a URL
//! padrão da libretro (`library_scan::libretro_boxart_url`). Quando um
//! scraping (automático ou aceito manualmente) grava uma `cover_url` nova
//! pra um rom, `invalidate()` descarta o cache antigo — a próxima leitura
//! busca a capa escrapeada em vez de continuar servindo a padrão já salva.
//!
//! Falha (sem cobertura da libretro pro sistema, sem rede na 1ª tentativa,
//! 404) responde com HTTP 404 e NADA fica gravado — o `<img onError>` do
//! frontend cai no placeholder de iniciais, e tenta de novo normalmente na
//! próxima vez que o card renderizar (ex.: reabrir o app já com rede).

use domain::library::RomRepository;
use domain::metadata::MetadataRepository;
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

fn cache_path(covers_dir: &std::path::Path, rom_id: &str) -> std::path::PathBuf {
    covers_dir.join(format!("{}.png", sanitize(rom_id)))
}

/// Chamar sempre que a `cover_url` escrapeada de um rom mudar (scraping
/// automático ou `resolve_pending_match` com `accept: true`) — descarta o
/// cache antigo pra próxima leitura buscar a capa certa (escrapeada, ou a
/// padrão da libretro se o scraping não trouxe capa).
pub fn invalidate(covers_dir: &std::path::Path, rom_id: &str) {
    let _ = std::fs::remove_file(cache_path(covers_dir, rom_id));
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
            let rom_id = request.uri().path().trim_start_matches('/').to_string();
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
    let Ok(Some(rom)) = db::RomsRepo::new(pool.clone()).get(rom_id).await else {
        return not_found();
    };

    let path = cache_path(&state.covers_dir, rom_id);
    if let Ok(bytes) = std::fs::read(&path) {
        return ok_png(bytes);
    }

    // Prioridade: capa escrapeada (se o jogo já passou pelo scraping e
    // trouxe uma) — senão a URL padrão calculada pela convenção da libretro.
    let scraped_cover = db::MetadataRepo::new(pool)
        .get_metadata(rom_id)
        .await
        .ok()
        .flatten()
        .and_then(|m| m.cover_url)
        .filter(|u| !u.is_empty());
    let url = match scraped_cover {
        Some(u) => u,
        None => {
            let title = rom_title(&rom);
            match library_scan::libretro_boxart_url(&rom.system_id, &title) {
                Some(u) => u,
                None => return not_found(),
            }
        }
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
    fn cache_path_is_stable_per_rom() {
        let dir = std::path::Path::new("/data/covers");
        assert_eq!(
            cache_path(dir, "rom-1"),
            cache_path(dir, "rom-1"),
            "mesmo rom_id sempre bate no mesmo arquivo"
        );
        assert_ne!(cache_path(dir, "rom-1"), cache_path(dir, "rom-2"));
    }

    #[test]
    fn cache_path_sanitizes_unsafe_characters() {
        let dir = std::path::Path::new("/data/covers");
        let p = cache_path(dir, "../../etc");
        assert_eq!(p, dir.join(".._.._etc.png"));
    }
}
