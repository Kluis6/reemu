//! Cache local em disco das capas (`cover://localhost/<rom_id>`) e das mídias
//! por jogo (`media://localhost/<game_uuid>/<kind>/<n>`).
//!
//! Capas ficam por ROM em `<dados>/covers/<rom_id>.png` (capa editada, capa
//! escrapeada de `game_metadata` ou fallback da libretro).
//! Hero, logo, tela de título e capturas de tela (0..7) ficam por jogo em
//! `<dados>/media/<game_uuid>/`, compartilhados entre dumps do mesmo jogo e
//! servidos offline depois da primeira abertura. Vídeos não entram no cache, e
//! apenas URLs `https://` sem credenciais vindas da resposta guardada são aceitas.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use domain::library::RomRepository;
use domain::metadata::MetadataRepository;
use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::http;
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::catalog::answer::GameAnswer;
use crate::commands::{rom_title, AppState};

pub const MAX_SCREENSHOTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Hero,
    Logo,
    Title,
    Screenshot(u8),
}

impl MediaKind {
    pub fn parse_request_path(raw_path: &str) -> Option<(&str, Self)> {
        let trimmed = raw_path.trim_matches('/');
        let (game_uuid, rest) = trimmed.split_once('/')?;
        if !is_safe_id(game_uuid) {
            return None;
        }
        let kind = match rest {
            "hero" | "hero/0" => Self::Hero,
            "logo" | "logo/0" => Self::Logo,
            "title" | "title/0" => Self::Title,
            _ => {
                let idx_str = rest.strip_prefix("screenshot/")?;
                let idx: u8 = idx_str.parse().ok()?;
                if usize::from(idx) >= MAX_SCREENSHOTS {
                    return None;
                }
                Self::Screenshot(idx)
            }
        };
        Some((game_uuid, kind))
    }

    fn file_name(self) -> String {
        match self {
            Self::Hero => "hero".into(),
            Self::Logo => "logo".into(),
            Self::Title => "title".into(),
            Self::Screenshot(n) => format!("screenshot_{n}"),
        }
    }

    #[cfg(test)]
    pub fn as_url_suffix(self) -> String {
        match self {
            Self::Hero => "hero/0".into(),
            Self::Logo => "logo/0".into(),
            Self::Title => "title/0".into(),
            Self::Screenshot(n) => format!("screenshot/{n}"),
        }
    }
}

fn is_safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
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

fn cache_path(covers_dir: &Path, rom_id: &str) -> PathBuf {
    covers_dir.join(format!("{}.png", sanitize(rom_id)))
}

fn media_cache_path(media_dir: &Path, game_uuid: &str, kind: MediaKind) -> PathBuf {
    media_dir.join(sanitize(game_uuid)).join(kind.file_name())
}

/// Chamar sempre que a `cover_url` escrapeada de uma ROM mudar: descarta o
/// cache antigo para a próxima leitura buscar a capa nova.
pub fn invalidate(covers_dir: &Path, rom_id: &str) {
    let _ = std::fs::remove_file(cache_path(covers_dir, rom_id));
}

/// Remove mídias cacheadas por `game_uuid` para forçar releitura após enriquecimento.
pub fn invalidate_game_media(media_dir: &Path, game_uuid: &str) {
    if is_safe_id(game_uuid) {
        let _ = std::fs::remove_dir_all(media_dir.join(sanitize(game_uuid)));
    }
}

/// O cache de capas na pasta `covers_dir`, para a crate `scraping`.
pub struct CoverDir(pub PathBuf);

impl scraping::CoverCache for CoverDir {
    fn invalidate(&self, rom_id: &str) {
        invalidate(&self.0, rom_id);
    }
}

fn not_found() -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(404)
        .body(Vec::new())
        .unwrap()
}

fn artwork_response(content_type: &str, bytes: Vec<u8>) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(200)
        .header("Content-Type", content_type)
        .header("X-Content-Type-Options", "nosniff")
        .header("Cache-Control", "no-cache")
        .body(bytes)
        .unwrap()
}

const MAX_ARTWORK_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn uploaded_artwork(url: &str) -> Option<(&str, Vec<u8>)> {
    let (mime, data) = url.strip_prefix("data:")?.split_once(";base64,")?;
    if data.len() > MAX_ARTWORK_BYTES.div_ceil(3) * 4 {
        return None;
    }
    let bytes = STANDARD.decode(data).ok()?;
    (bytes.len() <= MAX_ARTWORK_BYTES && artwork_mime(&bytes) == Some(mime))
        .then_some((mime, bytes))
}

fn artwork_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else {
        None
    }
}

pub(crate) fn read_artwork_file(path: &Path) -> Result<String, crate::app_error::AppError> {
    let invalid = || {
        crate::app_error::AppError::new(
            crate::app_error::ErrorCode::Corrupt,
            "unsupported artwork file",
        )
    };
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() || meta.len() > MAX_ARTWORK_BYTES as u64 {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_ARTWORK_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let mime = artwork_mime(&bytes)
        .filter(|_| bytes.len() <= MAX_ARTWORK_BYTES)
        .ok_or_else(invalid)?;
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

fn scheme_url(scheme: &str, path: &str) -> String {
    let clean = path.trim_start_matches('/');
    if cfg!(windows) {
        format!("http://{scheme}.localhost/{clean}")
    } else {
        format!("{scheme}://localhost/{clean}")
    }
}

/// URL da capa de uma ROM para `<img>`. No Windows o WebView2 intercepta
/// `http://<esquema>.localhost/`; nos demais sistemas, `<esquema>://localhost/`.
pub fn cover_url(rom_id: &str) -> String {
    scheme_url("cover", rom_id)
}

/// URL da capa com sufixo `?v=<fnv1a>` quando há metadados/edição, evitando que
/// o WebView mantenha em memória um `404` ou uma capa anterior do mesmo `rom_id`.
pub fn cover_url_versioned(rom_id: &str, version_seed: Option<&str>) -> String {
    let base = cover_url(rom_id);
    let Some(seed) = version_seed.map(str::trim).filter(|s| !s.is_empty()) else {
        return base;
    };
    let mut hash: u32 = 0x811c_9dc5;
    for b in seed.bytes() {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{base}?v={hash:08x}")
}

#[cfg(test)]
pub fn media_url(game_uuid: &str, kind: MediaKind) -> String {
    scheme_url("media", &format!("{game_uuid}/{}", kind.as_url_suffix()))
}

/// Registra `cover://` e `media://` no builder antes de `.setup()`/`.run()`.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .register_asynchronous_uri_scheme_protocol(
            "cover",
            |ctx: UriSchemeContext<'_, R>, request, responder: UriSchemeResponder| {
                let app = ctx.app_handle().clone();
                let rom_id = request.uri().path().trim_start_matches('/').to_string();
                tauri::async_runtime::spawn(async move {
                    responder.respond(resolve_cover(&app, &rom_id).await);
                });
            },
        )
        .register_asynchronous_uri_scheme_protocol(
            "media",
            |ctx: UriSchemeContext<'_, R>, request, responder: UriSchemeResponder| {
                let app = ctx.app_handle().clone();
                let raw_path = request.uri().path().to_string();
                tauri::async_runtime::spawn(async move {
                    responder.respond(resolve_media_request(&app, &raw_path).await);
                });
            },
        )
}

async fn http_fetch_bytes(url: String) -> Option<Vec<u8>> {
    let resp = net::http::client().get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.bytes().await.ok().map(|b| b.to_vec())
}

async fn serve_cached_or_fetch<F, Fut>(
    cache_file: &Path,
    url: Option<String>,
    fetch: F,
) -> http::Response<Vec<u8>>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Option<Vec<u8>>>,
{
    if let Ok(bytes) = std::fs::read(cache_file) {
        let mime = artwork_mime(&bytes).unwrap_or("image/png");
        return artwork_response(mime, bytes);
    }
    let Some(url) = url else {
        return not_found();
    };
    let Some(bytes) = fetch(url).await else {
        return not_found();
    };
    let Some(mime) = artwork_mime(&bytes) else {
        return not_found();
    };
    if let Some(parent) = cache_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(cache_file, &bytes) {
        log::warn!("cache de imagem: falha ao gravar ({e})");
    }
    artwork_response(mime, bytes)
}

async fn resolve_cover<R: Runtime>(
    app: &tauri::AppHandle<R>,
    rom_id: &str,
) -> http::Response<Vec<u8>> {
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

    let metadata = db::MetadataRepo::new(pool);
    let edited_cover = metadata.cover_override(rom_id).await.ok().flatten();
    if let Some(url) = edited_cover
        .as_deref()
        .filter(|url| url.starts_with("data:"))
    {
        return uploaded_artwork(url)
            .map(|(mime, bytes)| artwork_response(mime, bytes))
            .unwrap_or_else(not_found);
    }

    let path = cache_path(&state.covers_dir, rom_id);
    // Se já estiver em disco, serve antes mesmo de montar URL.
    if let Ok(bytes) = std::fs::read(&path) {
        let mime = artwork_mime(&bytes).unwrap_or("image/png");
        return artwork_response(mime, bytes);
    }

    let scraped_cover = match edited_cover {
        Some(edited) => net::http::https_url(&edited),
        None => {
            resolve_rom_cover_source_url(metadata.get_metadata(rom_id).await.ok().flatten(), &rom)
        }
    };
    let url = scraped_cover.or_else(|| {
        let title = rom_title(&rom);
        library_scan::libretro_boxart_url(&rom.system_id, &title)
    });

    serve_cached_or_fetch(&path, url, http_fetch_bytes).await
}

pub(crate) fn resolve_rom_cover_source_url(
    meta: Option<domain::metadata::GameMetadata>,
    rom: &domain::library::Rom,
) -> Option<String> {
    let m = meta?;
    let detected = scraping::rom_name::detect_rom_region(
        &rom.file_path,
        rom.ident_name.as_deref(),
        rom.serial.as_deref(),
    );
    if let Some(gk) = m.details.gamekave.as_ref() {
        let reg = gk.rom_region.as_deref().or(detected);
        if let Some(regional) = scraping::rom_name::pick_regional_cover(
            &gk.regional_covers,
            reg,
            m.cover_url.as_deref(),
        ) {
            if let Some(valid) = net::http::https_url(regional) {
                return Some(valid);
            }
        }
    }
    m.cover_url.as_deref().and_then(net::http::https_url)
}

async fn resolve_media_request<R: Runtime>(
    app: &tauri::AppHandle<R>,
    raw_path: &str,
) -> http::Response<Vec<u8>> {
    let state = app.state::<AppState>();
    let Some(pool) = state.db.clone() else {
        return not_found();
    };
    resolve_media_with(&pool, &state.media_dir, raw_path, http_fetch_bytes).await
}

async fn media_source_url(pool: &db::Db, game_uuid: &str, kind: MediaKind) -> Option<String> {
    if let Ok(Some(res)) = db::RomResolutionRepo::new(pool.clone())
        .by_game_uuid(game_uuid)
        .await
    {
        if let Some(answer) = res
            .answer_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<GameAnswer>(raw).ok())
        {
            let candidate = match kind {
                MediaKind::Hero => answer.media.hero,
                MediaKind::Logo => answer.media.logo,
                MediaKind::Title => answer.media.title_screen,
                MediaKind::Screenshot(n) => answer.media.screenshots.get(usize::from(n)).cloned(),
            };
            if let Some(valid) = candidate.as_deref().and_then(net::http::https_url) {
                return Some(valid);
            }
        }
    }

    let meta = db::MetadataRepo::new(pool.clone())
        .get_by_game_uuid(game_uuid)
        .await
        .ok()
        .flatten()?;
    let candidate = match kind {
        MediaKind::Hero => meta.details.fanart_url,
        MediaKind::Logo => meta.details.logo_url,
        MediaKind::Title => meta.details.title_screen_url,
        MediaKind::Screenshot(n) => meta
            .details
            .gamekave
            .and_then(|gk| gk.screenshots.get(usize::from(n)).cloned())
            .or_else(|| (n == 0).then_some(meta.details.screenshot_url).flatten()),
    };
    candidate.as_deref().and_then(net::http::https_url)
}

pub(crate) async fn resolve_media_with<F, Fut>(
    pool: &db::Db,
    media_dir: &Path,
    raw_path: &str,
    fetch: F,
) -> http::Response<Vec<u8>>
where
    F: FnOnce(String) -> Fut,
    Fut: Future<Output = Option<Vec<u8>>>,
{
    let Some((game_uuid, kind)) = MediaKind::parse_request_path(raw_path) else {
        return not_found();
    };
    let path = media_cache_path(media_dir, game_uuid, kind);
    if let Ok(bytes) = std::fs::read(&path) {
        let mime = artwork_mime(&bytes).unwrap_or("image/png");
        return artwork_response(mime, bytes);
    }
    let url = media_source_url(pool, game_uuid, kind).await;
    serve_cached_or_fetch(&path, url, fetch).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::answer::{metadata_from_answer, Media};
    use domain::library::Rom;
    use domain::metadata::{ResolutionState, RomResolution};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    const TINY_PNG: &[u8] = b"\x89PNG\r\n\x1a\ncatalog-media-bytes";

    async fn seed_rom_and_answer(pool: &db::Db, rom_id: &str, game_uuid: &str, media: Media) {
        db::RomsRepo::new(pool.clone())
            .add(&Rom {
                id: rom_id.into(),
                file_path: format!("/roms/snes/{rom_id}.sfc"),
                system_id: "snes".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let answer = GameAnswer {
            game_id: game_uuid.into(),
            title: "Chrono Trigger".into(),
            media,
            ..Default::default()
        };
        let answer_json = serde_json::to_string(&answer).unwrap();
        db::RomResolutionRepo::new(pool.clone())
            .put(&RomResolution {
                rom_id: rom_id.into(),
                state: ResolutionState::Found,
                game_uuid: Some(game_uuid.into()),
                matched_by: Some("sha1".into()),
                reason: None,
                retry_after: None,
                media_pending: false,
                answer_json: Some(answer_json),
                resolved_at: 1_700_000_000,
            })
            .await
            .unwrap();
        let meta = metadata_from_answer(rom_id, &answer);
        db::MetadataRepo::new(pool.clone())
            .upsert_metadata(&meta)
            .await
            .unwrap();
    }

    #[test]
    fn uploaded_artwork_checks_type_and_length() {
        let png = b"\x89PNG\r\n\x1a\n";
        let payload = STANDARD.encode(png);
        let url = format!("data:image/png;base64,{payload}");
        assert_eq!(uploaded_artwork(&url), Some(("image/png", png.to_vec())));
        assert!(uploaded_artwork(&format!("data:image/gif;base64,{payload}")).is_none());
        assert!(uploaded_artwork(&format!("data:image/svg+xml;base64,{payload}")).is_none());
        assert!(uploaded_artwork("data:image/png;base64,invalid!").is_none());
        let mut large = png.to_vec();
        large.resize(4 * 1024 * 1024 + 1, 0);
        let url = format!("data:image/png;base64,{}", STANDARD.encode(large));
        assert!(uploaded_artwork(&url).is_none());
    }

    #[test]
    fn cache_path_is_stable_per_rom() {
        let dir = Path::new("/data/covers");
        assert_eq!(
            cache_path(dir, "rom-1"),
            cache_path(dir, "rom-1"),
            "mesmo rom_id sempre bate no mesmo arquivo"
        );
        assert_ne!(cache_path(dir, "rom-1"), cache_path(dir, "rom-2"));
    }

    #[test]
    fn cache_path_sanitizes_unsafe_characters() {
        let dir = Path::new("/data/covers");
        let p = cache_path(dir, "../../etc");
        assert_eq!(p, dir.join(".._.._etc.png"));
    }

    #[tokio::test]
    async fn media_is_fetched_once_and_served_from_disk() {
        let server = MockServer::start().await;
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        Mock::given(method("GET"))
            .and(path("/ct/hero.png"))
            .respond_with(move |_: &Request| {
                hits2.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_bytes(TINY_PNG)
            })
            .mount(&server)
            .await;

        let pool = db::connect_in_memory().await.unwrap();
        let media_dir = tempfile::tempdir().unwrap();
        let uuid = "01950f10-aaaa-7bbb-8ccc-111122223333";
        seed_rom_and_answer(
            &pool,
            "rom1",
            uuid,
            Media {
                hero: Some("https://cdn.alvanista.com/ct/hero.png".into()),
                ..Default::default()
            },
        )
        .await;

        let mock_base = server.uri();
        let fetch_from_mock = |url: String| {
            let rewritten = url.replace("https://cdn.alvanista.com", &mock_base);
            async move {
                let resp = reqwest::Client::new().get(rewritten).send().await.ok()?;
                resp.status()
                    .is_success()
                    .then_some(resp.bytes().await.ok()?.to_vec())
            }
        };

        let req_path = format!("/{uuid}/hero/0");
        let first = resolve_media_with(&pool, media_dir.path(), &req_path, fetch_from_mock).await;
        assert_eq!(first.status(), 200);
        assert_eq!(first.body().as_slice(), TINY_PNG);
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // Servidor desligado (modo offline): a 2ª leitura sai do disco sem tocar na rede.
        drop(server);
        let second = resolve_media_with(&pool, media_dir.path(), &req_path, |_| async {
            panic!("não deveria chamar a rede quando já está em disco");
        })
        .await;
        assert_eq!(second.status(), 200);
        assert_eq!(second.body().as_slice(), TINY_PNG);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn two_roms_of_one_game_share_media() {
        let server = MockServer::start().await;
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        Mock::given(method("GET"))
            .and(path("/ct/shot0.png"))
            .respond_with(move |_: &Request| {
                hits2.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(200).set_body_bytes(TINY_PNG)
            })
            .mount(&server)
            .await;

        let pool = db::connect_in_memory().await.unwrap();
        let media_dir = tempfile::tempdir().unwrap();
        let uuid = "01950f10-aaaa-7bbb-8ccc-444455556666";
        let media = Media {
            screenshots: vec!["https://cdn.alvanista.com/ct/shot0.png".into()],
            ..Default::default()
        };
        seed_rom_and_answer(&pool, "rom_usa", uuid, media.clone()).await;
        seed_rom_and_answer(&pool, "rom_jpn", uuid, media).await;

        assert_eq!(
            media_url(uuid, MediaKind::Screenshot(0)),
            media_url(uuid, MediaKind::Screenshot(0))
        );

        let mock_base = server.uri();
        let make_fetcher = || {
            let base = mock_base.clone();
            move |url: String| async move {
                let rewritten = url.replace("https://cdn.alvanista.com", &base);
                let resp = reqwest::Client::new().get(rewritten).send().await.ok()?;
                resp.status()
                    .is_success()
                    .then_some(resp.bytes().await.ok()?.to_vec())
            }
        };

        let req_path = format!("/{uuid}/screenshot/0");
        let r1 = resolve_media_with(&pool, media_dir.path(), &req_path, make_fetcher()).await;
        let r2 = resolve_media_with(&pool, media_dir.path(), &req_path, make_fetcher()).await;
        assert_eq!(r1.status(), 200);
        assert_eq!(r2.status(), 200);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn non_https_url_is_refused() {
        let pool = db::connect_in_memory().await.unwrap();
        let media_dir = tempfile::tempdir().unwrap();
        let uuid = "01950f10-aaaa-7bbb-8ccc-777788889999";

        db::RomsRepo::new(pool.clone())
            .add(&Rom {
                id: "rom_bad".into(),
                file_path: "/roms/snes/bad.sfc".into(),
                system_id: "snes".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        db::RomResolutionRepo::new(pool.clone())
            .put(&RomResolution {
                rom_id: "rom_bad".into(),
                state: ResolutionState::Found,
                game_uuid: Some(uuid.into()),
                matched_by: Some("crc32".into()),
                reason: None,
                retry_after: None,
                media_pending: false,
                answer_json: Some(
                    serde_json::json!({
                        "game_id": uuid,
                        "title": "Bad URLs",
                        "media": {
                            "hero": "http://insecure.example/hero.png",
                            "logo": "file:///etc/passwd",
                            "title_screen": "https://user:secret@cdn.example/title.png",
                            "screenshots": ["ftp://files.example/s0.png"]
                        }
                    })
                    .to_string(),
                ),
                resolved_at: 100,
            })
            .await
            .unwrap();

        let called = Arc::new(AtomicUsize::new(0));
        for req_path in [
            format!("/{uuid}/hero/0"),
            format!("/{uuid}/logo/0"),
            format!("/{uuid}/title/0"),
            format!("/{uuid}/screenshot/0"),
            format!("/{uuid}/screenshot/8"),
            "/../etc/hero/0".to_owned(),
        ] {
            let c2 = called.clone();
            let resp =
                resolve_media_with(&pool, media_dir.path(), &req_path, move |_| async move {
                    c2.fetch_add(1, Ordering::SeqCst);
                    Some(TINY_PNG.to_vec())
                })
                .await;
            assert_eq!(resp.status(), 404, "deveria recusar {req_path}");
        }
        assert_eq!(called.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn two_regional_roms_of_same_game_serve_distinct_regional_covers() {
        let mut regional_covers = std::collections::BTreeMap::new();
        regional_covers.insert("JP".into(), "https://cdn.gamekave.net/smw/jp.webp".into());
        regional_covers.insert("EU".into(), "https://cdn.gamekave.net/smw/eu.webp".into());
        let answer = GameAnswer {
            game_id: "01950f10-aaaa-7bbb-8ccc-999900001111".into(),
            title: "Super Mario World".into(),
            media: Media {
                cover: Some("https://cdn.gamekave.net/smw/na.webp".into()),
                regional_covers,
                ..Default::default()
            },
            ..Default::default()
        };
        let base_meta = metadata_from_answer("shared", &answer);

        let rom_jp = Rom {
            id: "smw_jp".into(),
            file_path: "/roms/snes/Super Mario World (Japan).sfc".into(),
            system_id: "snes".into(),
            ..Default::default()
        };
        let rom_eu = Rom {
            id: "smw_eu".into(),
            file_path: "/roms/snes/Super Mario World (Europe).sfc".into(),
            system_id: "snes".into(),
            ..Default::default()
        };
        let rom_us = Rom {
            id: "smw_us".into(),
            file_path: "/roms/snes/Super Mario World (USA).sfc".into(),
            system_id: "snes".into(),
            ..Default::default()
        };

        assert_eq!(
            resolve_rom_cover_source_url(Some(base_meta.clone()), &rom_jp).as_deref(),
            Some("https://cdn.gamekave.net/smw/jp.webp")
        );
        assert_eq!(
            resolve_rom_cover_source_url(Some(base_meta.clone()), &rom_eu).as_deref(),
            Some("https://cdn.gamekave.net/smw/eu.webp")
        );
        assert_eq!(
            resolve_rom_cover_source_url(Some(base_meta), &rom_us).as_deref(),
            Some("https://cdn.gamekave.net/smw/na.webp")
        );
    }
}
