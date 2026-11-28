//! RetroAchievements no processo principal (`reemu-desktop`):
//! - cliente HTTP único (`reqwest`) com `User-Agent: ReEmu/<versão> rcheevos/<versão>`
//!   compartilhado entre login, página do jogo e proxy IPC do `reemu-core-host`;
//! - comandos Tauri `ra_status`, `ra_login`, `ra_logout`, `ra_set_preferences` e
//!   `ra_game_achievements`;
//! - espelho em memória do jogo rodando agora (`LiveGameAchievements`), atualizado
//!   pelos eventos do core para refletir desbloqueios na hora e bloquear cheats /
//!   carregamento de save state durante sessões Hardcore.

use crate::app_error::{AppError, ErrorCode};
use crate::commands::AppState;
use crate::credentials::{self, OsKeyring, RaConfig};
use achievements::{
    console::{RC_CONSOLE_ARCADE, RC_CONSOLE_ARDUBOY, RC_CONSOLE_MS_DOS},
    console_id_for_core, console_id_for_system, hash_buffer, is_disc_console, is_disc_like_path,
    is_incompatible_game_id, is_warning_achievement_id, resolve_console_and_hash,
    AchievementClient, AchievementEntry, AchievementEvent, AchievementKind, AsyncOutcome,
    GameSummary, LeaderboardFormat, LeaderboardInfo, LeaderboardState, OutboundHttpRequest,
    RaError, RaLoadedSnapshot, RaSessionConfig,
};
use domain::library::RomRepository as _;
use emu_session::RaHostMessage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{
    http, AppHandle, Emitter, Manager, Runtime, State, UriSchemeContext, UriSchemeResponder,
};

const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
/// Limite oficial `MAX_BUFFER_SIZE` do `rcheevos` (`rc_hash_internal.h`: 64 MiB).
const RA_MAX_ROM_BUFFER: usize = 64 * 1024 * 1024;
const RA_DISK_CACHE_FILE: &str = "ra_hashes.json";
const RA_DISK_CACHE_MAX_ENTRIES: usize = 8192;
const RA_BADGES_DIR_NAME: &str = "ra-badges";
const RA_VIEWS_SUBDIR: &str = "views";
const RA_MAX_BADGE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskHashEntry {
    size: u64,
    modified_secs: u64,
    console_id: u32,
    hash: String,
}

fn file_signature(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some((meta.len(), modified))
}

fn app_hash_cache_file(app: &AppHandle) -> Option<PathBuf> {
    std::env::var_os("REEMU_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| app.path().app_data_dir().ok())
        .map(|d| d.join(RA_DISK_CACHE_FILE))
}

fn app_badges_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    std::env::var_os("REEMU_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| app.path().app_data_dir().ok())
        .map(|d| d.join(RA_BADGES_DIR_NAME))
}

fn sanitize_badge_stem(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return None;
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return None;
    }
    Some(trimmed.to_owned())
}

fn badge_cache_filename_from_url(raw_url: &str) -> Option<(String, String)> {
    let parsed = reqwest::Url::parse(raw_url.trim()).ok()?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    if !matches!(
        host.as_str(),
        "media.retroachievements.org" | "i.retroachievements.org"
    ) {
        return None;
    }
    let path = parsed.path();
    let lower_path = path.to_ascii_lowercase();
    if !lower_path.ends_with(".png") {
        return None;
    }
    let without_ext = &path[..path.len() - 4];
    if let Some(stem_raw) = without_ext.strip_prefix("/Badge/") {
        let safe = sanitize_badge_stem(stem_raw)?;
        return Some((format!("{safe}.png"), parsed.into()));
    }
    if let Some(stem_raw) = without_ext.strip_prefix("/Images/") {
        let safe = sanitize_badge_stem(stem_raw)?;
        return Some((format!("game_{safe}.png"), parsed.into()));
    }
    None
}

fn badge_cache_path(badges_dir: &Path, filename: &str) -> Option<PathBuf> {
    let trimmed = filename.trim();
    if trimmed.len() <= 4 || !trimmed.to_ascii_lowercase().ends_with(".png") {
        return None;
    }
    let stem = sanitize_badge_stem(&trimmed[..trimmed.len() - 4])?;
    Some(badges_dir.join(format!("{stem}.png")))
}

fn rabadge_url(filename: &str) -> String {
    let clean = filename.trim_start_matches('/');
    if cfg!(windows) {
        format!("http://rabadge.localhost/{clean}")
    } else {
        format!("rabadge://localhost/{clean}")
    }
}

fn badge_image_mime(bytes: &[u8]) -> Option<&'static str> {
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

fn is_valid_cached_badge_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() == 0 || meta.len() > RA_MAX_BADGE_BYTES as u64 {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 12];
    let Ok(n) = f.read(&mut header) else {
        return false;
    };
    badge_image_mime(&header[..n]).is_some()
}

fn resolve_badge_url_for_ui(
    badges_dir: Option<&Path>,
    raw_url: Option<String>,
    pending_downloads: &mut Vec<(PathBuf, String)>,
) -> Option<String> {
    let url = raw_url?;
    if url.starts_with("http://rabadge.localhost/") || url.starts_with("rabadge://localhost/") {
        return Some(url);
    }
    let (Some(dir), Some((fname, canonical_https))) =
        (badges_dir, badge_cache_filename_from_url(&url))
    else {
        return Some(url);
    };
    let Some(disk_path) = badge_cache_path(dir, &fname) else {
        return Some(url);
    };
    if is_valid_cached_badge_file(&disk_path) {
        return Some(rabadge_url(&fname));
    }
    if !pending_downloads.iter().any(|(p, _)| p == &disk_path) {
        pending_downloads.push((disk_path, canonical_https));
    }
    Some(url)
}

fn resolve_rabadge_response(badges_dir: Option<&Path>, raw_path: &str) -> http::Response<Vec<u8>> {
    let not_found = || {
        http::Response::builder()
            .status(404)
            .body(Vec::new())
            .unwrap()
    };
    let Some(dir) = badges_dir else {
        return not_found();
    };
    let clean = raw_path.trim_matches('/');
    let Some(path) = badge_cache_path(dir, clean) else {
        return not_found();
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        return not_found();
    };
    if !meta.is_file() || meta.len() > RA_MAX_BADGE_BYTES as u64 {
        return not_found();
    }
    let mut bytes = Vec::new();
    let Ok(mut file) = std::fs::File::open(&path) else {
        return not_found();
    };
    if file
        .by_ref()
        .take(RA_MAX_BADGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > RA_MAX_BADGE_BYTES
    {
        return not_found();
    }
    let Some(mime) = badge_image_mime(&bytes) else {
        return not_found();
    };
    http::Response::builder()
        .status(200)
        .header("Content-Type", mime)
        .header("X-Content-Type-Options", "nosniff")
        .header("Cache-Control", "public, max-age=86400")
        .body(bytes)
        .unwrap()
}

/// Registra o protocolo local `rabadge://` para emblemas cacheados em `<dados>/ra-badges/`.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(
        "rabadge",
        |ctx: UriSchemeContext<'_, R>, request, responder: UriSchemeResponder| {
            let dir = app_badges_dir(ctx.app_handle());
            let raw_path = request.uri().path().to_string();
            tauri::async_runtime::spawn_blocking(move || {
                responder.respond(resolve_rabadge_response(dir.as_deref(), &raw_path));
            });
        },
    )
}

fn view_cache_file(badges_dir: &Path, rom_hash: &str) -> Option<PathBuf> {
    let safe = sanitize_badge_stem(rom_hash)?;
    Some(
        badges_dir
            .join(RA_VIEWS_SUBDIR)
            .join(format!("{safe}.json")),
    )
}

fn store_cached_game_view(badges_dir: Option<&Path>, rom_hash: Option<&str>, view: &RaGameViewDto) {
    if view.status != "loaded" {
        return;
    }
    let (Some(dir), Some(hash)) = (badges_dir, rom_hash) else {
        return;
    };
    let Some(path) = view_cache_file(dir, hash) else {
        return;
    };
    let mut snapshot = view.clone();
    snapshot.playing_live = false;
    snapshot.rich_presence = None;
    snapshot.active_trackers.clear();
    snapshot.active_challenges.clear();
    let Ok(bytes) = serde_json::to_vec(&snapshot) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, bytes);
}

fn load_cached_game_view(
    badges_dir: Option<&Path>,
    rom_hash: &str,
) -> Option<(RaGameViewDto, Vec<(PathBuf, String)>)> {
    let dir = badges_dir?;
    let path = view_cache_file(dir, rom_hash)?;
    let bytes = std::fs::read(path).ok()?;
    let mut view: RaGameViewDto = serde_json::from_slice(&bytes).ok()?;
    if view.status != "loaded" {
        return None;
    }
    view.playing_live = false;
    view.rich_presence = None;
    view.active_trackers.clear();
    view.active_challenges.clear();
    let mut pending = Vec::new();
    rewrite_view_badge_urls(&mut view, Some(dir), &mut pending);
    Some((view, pending))
}

fn rewrite_view_badge_urls(
    view: &mut RaGameViewDto,
    badges_dir: Option<&Path>,
    pending: &mut Vec<(PathBuf, String)>,
) {
    view.badge_url = resolve_badge_url_for_ui(badges_dir, view.badge_url.take(), pending);
    for ach in &mut view.achievements {
        ach.badge_url = resolve_badge_url_for_ui(badges_dir, ach.badge_url.take(), pending);
        ach.badge_locked_url =
            resolve_badge_url_for_ui(badges_dir, ach.badge_locked_url.take(), pending);
    }
    for ch in &mut view.active_challenges {
        ch.badge_url = resolve_badge_url_for_ui(badges_dir, ch.badge_url.take(), pending);
    }
}

fn can_attempt_ra(system_id: &str, preferred_console: Option<u32>, path: &Path) -> bool {
    preferred_console.is_some()
        || system_id == "disc"
        || system_id.is_empty()
        || (system_id != "laserdisc" && is_disc_like_path(path))
}

/// Calcula `(console_id, hash)` oficial do `rcheevos` para `path`:
/// - reaproveita `extracted_hint` quando o loader ja extraiu um `.zip`/`.7z` para disco;
/// - extrai em memoria ROMs de cartucho ate 64 MiB quando estao em `.zip`/`.7z`;
/// - delega imagens de disco (`.iso`, `.cue`/`.bin`, `.chd`, `.cso`, `.gcm`, `.gcz`,
///   `.gdi`, `.pbp`, `.m3u` e discos em `.zip`/`.7z`) e `system_id == "disc"` ao
///   resolvedor `resolve_console_and_hash`.
fn compute_rom_console_and_hash(
    preferred_console: Option<u32>,
    system_id: &str,
    path: &Path,
    extracted_hint: Option<&Path>,
) -> Option<(u32, String)> {
    let container_hashed_directly = matches!(
        preferred_console,
        Some(RC_CONSOLE_ARCADE | RC_CONSOLE_MS_DOS | RC_CONSOLE_ARDUBOY)
    );

    if !container_hashed_directly {
        if let Some(extracted) = extracted_hint.filter(|p| p.is_file()) {
            if let Some(res) = resolve_console_and_hash(preferred_console, extracted) {
                return Some(res);
            }
        }
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    if library_scan::is_supported_archive(ext) && !container_hashed_directly {
        if let Some(cid) = preferred_console.filter(|c| !is_disc_console(*c)) {
            let folder_hint = (!system_id.is_empty() && system_id != "disc").then_some(system_id);
            if let Some(archived) = library_scan::peek_archive(path, folder_hint) {
                if !is_disc_like_path(Path::new(&archived.entry)) {
                    if let Ok(bytes) = library_scan::read_archive_entry_capped(
                        path,
                        &archived.entry,
                        RA_MAX_ROM_BUFFER,
                    ) {
                        if let Some(h) = hash_buffer(cid, &bytes) {
                            return Some((cid, h));
                        }
                    }
                }
            }
        }
    }

    resolve_console_and_hash(preferred_console, path)
}

async fn resolve_rom_console_and_hash(
    ra_state: &RaState,
    cache_file: Option<&Path>,
    preferred_console: Option<u32>,
    rom_id: &str,
    system_id: &str,
    file_path: &str,
    extracted_hint: Option<&Path>,
) -> Option<(u32, String)> {
    if let Some(cached) = ra_state.cached_console_and_hash(cache_file, rom_id, file_path) {
        return Some(cached);
    }
    let sys = system_id.to_owned();
    let path_owned = file_path.to_owned();
    let hint_owned = extracted_hint.map(Path::to_path_buf);
    let computed = tauri::async_runtime::spawn_blocking(move || {
        compute_rom_console_and_hash(
            preferred_console,
            &sys,
            Path::new(&path_owned),
            hint_owned.as_deref(),
        )
    })
    .await
    .ok()
    .flatten()?;
    ra_state.store_console_and_hash(
        cache_file,
        rom_id,
        file_path,
        computed.0,
        computed.1.clone(),
    );
    Some(computed)
}

pub struct RaState {
    http: Mutex<Option<reqwest::Client>>,
    /// `(console_id, hash)` RA por `(rom_id, file_path)` para nao reler a midia ao reabrir a aba.
    rom_hashes: Mutex<HashMap<(String, String), (u32, String)>>,
    disk_hashes_loaded: AtomicBool,
    disk_hashes: Mutex<HashMap<String, DiskHashEntry>>,
    /// `rom_id` da partida atualmente em execucao (para anexar ao `LiveGameAchievements`).
    active_rom_id: Mutex<Option<String>>,
    /// Hash RA da partida aberta agora, para gravar cache offline em `ra-badges/views/`.
    active_rom_hash: Mutex<Option<String>>,
    /// `true` enquanto o jogo aberto iniciou uma sessao de conquistas em modo Hardcore.
    hardcore_active: AtomicBool,
    /// Estado vivo da partida aberta agora.
    live: Mutex<Option<LiveGameAchievements>>,
    /// Emblemas sendo baixados em segundo plano (evita downloads duplicados simultaneos).
    in_flight_badges: Arc<Mutex<HashSet<PathBuf>>>,
    badge_dl_sem: Arc<tokio::sync::Semaphore>,
}

impl Default for RaState {
    fn default() -> Self {
        Self {
            http: Mutex::new(None),
            rom_hashes: Mutex::new(HashMap::new()),
            disk_hashes_loaded: AtomicBool::new(false),
            disk_hashes: Mutex::new(HashMap::new()),
            active_rom_id: Mutex::new(None),
            active_rom_hash: Mutex::new(None),
            hardcore_active: AtomicBool::new(false),
            live: Mutex::new(None),
            in_flight_badges: Arc::new(Mutex::new(HashSet::new())),
            badge_dl_sem: Arc::new(tokio::sync::Semaphore::new(6)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LiveGameAchievements {
    pub rom_id: Option<String>,
    pub rom_hash: Option<String>,
    pub game_id: u32,
    pub title: String,
    pub badge_url: Option<String>,
    pub hardcore: bool,
    pub hardcore_downgraded: bool,
    pub incompatible_version: bool,
    pub summary: GameSummary,
    pub achievements: Vec<AchievementEntry>,
    pub leaderboards: Vec<LeaderboardInfo>,
    pub active_trackers: Vec<RaTrackerDto>,
    pub active_challenges: Vec<RaChallengeDto>,
    pub rich_presence: Option<String>,
}

fn host_system_clause() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    }
}

impl RaState {
    pub fn http_client(&self) -> reqwest::Client {
        let mut guard = self.http.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = guard.as_ref() {
            return c.clone();
        }
        let mut tmp = AchievementClient::new();
        let ua = format!(
            "ReEmu/{} ({} {}) {}",
            env!("CARGO_PKG_VERSION"),
            host_system_clause(),
            std::env::consts::ARCH,
            tmp.user_agent_clause()
        );
        let client = reqwest::Client::builder()
            .user_agent(ua)
            .timeout(HTTP_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        *guard = Some(client.clone());
        client
    }

    pub fn is_hardcore_active(&self) -> bool {
        self.hardcore_active.load(Ordering::Acquire)
    }

    pub fn set_hardcore_active(&self, active: bool) {
        self.hardcore_active.store(active, Ordering::Release);
    }

    pub fn set_active_rom(&self, rom_id: Option<String>) {
        *self.active_rom_id.lock().unwrap_or_else(|p| p.into_inner()) = rom_id;
    }

    pub fn active_rom(&self) -> Option<String> {
        self.active_rom_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn set_active_rom_hash(&self, rom_hash: Option<String>) {
        *self
            .active_rom_hash
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = rom_hash;
    }

    pub fn active_rom_hash(&self) -> Option<String> {
        self.active_rom_hash
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn queue_badge_downloads(&self, jobs: Vec<(PathBuf, String)>) {
        if jobs.is_empty() {
            return;
        }
        let http = self.http_client();
        for (disk_path, https_url) in jobs {
            {
                let mut guard = self
                    .in_flight_badges
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                if guard.contains(&disk_path) {
                    continue;
                }
                guard.insert(disk_path.clone());
            }
            let http_c = http.clone();
            let sem = Arc::clone(&self.badge_dl_sem);
            let in_flight = Arc::clone(&self.in_flight_badges);
            tauri::async_runtime::spawn(async move {
                if let Ok(_permit) = sem.acquire().await {
                    if !is_valid_cached_badge_file(&disk_path) {
                        if let Ok(resp) = http_c.get(&https_url).send().await {
                            if resp.status().is_success() {
                                if let Ok(bytes) = resp.bytes().await {
                                    if bytes.len() <= RA_MAX_BADGE_BYTES
                                        && badge_image_mime(&bytes).is_some()
                                    {
                                        if let Some(parent) = disk_path.parent() {
                                            let _ = std::fs::create_dir_all(parent);
                                        }
                                        let tmp_path = disk_path.with_extension("png.part");
                                        if std::fs::write(&tmp_path, &bytes).is_ok()
                                            && std::fs::rename(&tmp_path, &disk_path).is_err()
                                        {
                                            let _ = std::fs::write(&disk_path, &bytes);
                                            let _ = std::fs::remove_file(&tmp_path);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                in_flight
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&disk_path);
            });
        }
    }

    pub fn set_live(&self, snapshot: Option<LiveGameAchievements>) {
        *self.live.lock().unwrap_or_else(|p| p.into_inner()) = snapshot;
    }

    pub fn mark_live_unlocked(&self, achievement_id: u32, hardcore: bool) {
        let mut guard = self.live.lock().unwrap_or_else(|p| p.into_inner());
        let Some(live) = guard.as_mut() else {
            return;
        };
        live.mark_unlocked(achievement_id, hardcore);
    }

    pub fn live_for(&self, rom_id: &str) -> Option<LiveGameAchievements> {
        self.live
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|l| l.rom_id.as_deref() == Some(rom_id))
            .cloned()
    }

    fn ensure_disk_cache_loaded(&self, cache_file: &Path) {
        if self.disk_hashes_loaded.swap(true, Ordering::AcqRel) {
            return;
        }
        let Ok(bytes) = std::fs::read(cache_file) else {
            return;
        };
        let Ok(parsed) = serde_json::from_slice::<HashMap<String, DiskHashEntry>>(&bytes) else {
            return;
        };
        *self.disk_hashes.lock().unwrap_or_else(|p| p.into_inner()) = parsed;
    }

    fn cached_console_and_hash(
        &self,
        cache_file: Option<&Path>,
        rom_id: &str,
        file_path: &str,
    ) -> Option<(u32, String)> {
        if let Some(hit) = self
            .rom_hashes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(rom_id.to_owned(), file_path.to_owned()))
            .cloned()
        {
            return Some(hit);
        }

        let cache_path = cache_file?;
        self.ensure_disk_cache_loaded(cache_path);
        let (size, modified_secs) = file_signature(Path::new(file_path))?;
        let entry = self
            .disk_hashes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(file_path)
            .filter(|e| e.size == size && e.modified_secs == modified_secs)
            .cloned()?;
        let out = (entry.console_id, entry.hash);
        self.rom_hashes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert((rom_id.to_owned(), file_path.to_owned()), out.clone());
        Some(out)
    }

    fn store_console_and_hash(
        &self,
        cache_file: Option<&Path>,
        rom_id: &str,
        file_path: &str,
        console_id: u32,
        hash: String,
    ) {
        self.rom_hashes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                (rom_id.to_owned(), file_path.to_owned()),
                (console_id, hash.clone()),
            );

        let Some(cache_path) = cache_file else {
            return;
        };
        let Some((size, modified_secs)) = file_signature(Path::new(file_path)) else {
            return;
        };
        self.ensure_disk_cache_loaded(cache_path);
        let serialized = {
            let mut guard = self.disk_hashes.lock().unwrap_or_else(|p| p.into_inner());
            if guard.len() >= RA_DISK_CACHE_MAX_ENTRIES && !guard.contains_key(file_path) {
                guard.clear();
            }
            guard.insert(
                file_path.to_owned(),
                DiskHashEntry {
                    size,
                    modified_secs,
                    console_id,
                    hash,
                },
            );
            serde_json::to_vec(&*guard).ok()
        };
        if let Some(bytes) = serialized {
            if let Some(parent) = cache_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(cache_path, bytes);
        }
    }
}

/// Executa um pedido HTTP construído pelo `rcheevos` (`dorequest.php` ou mídia).
/// Usado tanto pelos comandos deste módulo quanto pelo proxy IPC do core.
pub async fn execute_rcheevos_http(
    http: &reqwest::Client,
    req: &OutboundHttpRequest,
) -> (i32, Vec<u8>) {
    let builder = if let Some(post) = &req.post_data {
        let ct = req
            .content_type
            .as_deref()
            .unwrap_or("application/x-www-form-urlencoded");
        http.post(&req.url)
            .header(reqwest::header::CONTENT_TYPE, ct)
            .body(post.clone())
    } else {
        http.get(&req.url)
    };

    match builder.send().await {
        Ok(resp) => {
            let status = i32::from(resp.status().as_u16());
            match resp.bytes().await {
                Ok(bytes) => (status, bytes.to_vec()),
                Err(_) => (
                    achievements::ffi::RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR,
                    Vec::new(),
                ),
            }
        }
        Err(_) => (
            achievements::ffi::RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR,
            Vec::new(),
        ),
    }
}

async fn drive_client_requests(http: &reqwest::Client, client: &mut AchievementClient) {
    loop {
        let batch = client.take_http_requests();
        if batch.is_empty() {
            break;
        }
        for req in batch {
            let (status, body) = execute_rcheevos_http(http, &req).await;
            client.complete_http_request(req.id, status, &body);
        }
    }
}

/// Liga os pedidos HTTP e eventos do `EmuSession` ao `reqwest` e aos eventos Tauri.
pub fn install_session_bridge(app: &AppHandle) {
    let app_handle = app.clone();
    let session = Arc::clone(&app.state::<AppState>().session);
    session.set_ra_sink(Arc::new(move |msg| match msg {
        RaHostMessage::HttpRequest { session_gen, req } => {
            let app_h = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let http = app_h.state::<RaState>().http_client();
                let (status, body) = execute_rcheevos_http(&http, &req).await;
                app_h.state::<AppState>().session.deliver_ra_http_response(
                    session_gen,
                    req.id,
                    status,
                    body,
                );
            });
        }
        RaHostMessage::GameLoaded { snapshot, .. } => {
            handle_game_loaded(&app_handle, snapshot);
        }
        RaHostMessage::Event { event, .. } => {
            handle_runtime_event(&app_handle, event);
        }
    }));
}

impl LiveGameAchievements {
    pub fn mark_unlocked(&mut self, achievement_id: u32, hardcore: bool) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .ok();
        self.active_challenges.retain(|c| c.id != achievement_id);
        if let Some(item) = self
            .achievements
            .iter_mut()
            .find(|a| a.id == achievement_id)
        {
            if !item.unlocked && !item.unofficial {
                self.summary.num_unlocked_achievements =
                    self.summary.num_unlocked_achievements.saturating_add(1);
                self.summary.points_unlocked =
                    self.summary.points_unlocked.saturating_add(item.points);
            }
            item.unlocked = true;
            item.unlocked_hardcore |= hardcore;
            item.measured_progress = None;
            if item.unlock_time.is_none() {
                item.unlock_time = now;
            }
        }
    }

    pub fn update_progress(&mut self, id: u32, measured_progress: String, measured_percent: f32) {
        if let Some(item) = self.achievements.iter_mut().find(|a| a.id == id) {
            if !item.unlocked {
                item.measured_progress =
                    (!measured_progress.is_empty()).then_some(measured_progress);
                item.measured_percent = measured_percent;
            }
        }
    }

    pub fn show_challenge(&mut self, challenge: RaChallengeDto) {
        if let Some(existing) = self
            .active_challenges
            .iter_mut()
            .find(|c| c.id == challenge.id)
        {
            *existing = challenge;
        } else {
            self.active_challenges.push(challenge);
        }
    }

    pub fn hide_challenge(&mut self, id: u32) -> bool {
        let before = self.active_challenges.len();
        self.active_challenges.retain(|c| c.id != id);
        self.active_challenges.len() != before
    }

    pub fn upsert_tracker(&mut self, id: u32, display: String) -> bool {
        let mut changed = false;
        if let Some(existing) = self.active_trackers.iter_mut().find(|t| t.id == id) {
            if existing.display != display {
                existing.display = display.clone();
                changed = true;
            }
        } else {
            self.active_trackers.push(RaTrackerDto {
                id,
                display: display.clone(),
            });
            changed = true;
        }
        if self.active_trackers.len() == 1 {
            for lb in &mut self.leaderboards {
                if lb.state == LeaderboardState::Tracking {
                    lb.tracker_value = Some(display.clone());
                }
            }
        }
        changed
    }

    pub fn hide_tracker(&mut self, id: u32) -> bool {
        let before = self.active_trackers.len();
        self.active_trackers.retain(|t| t.id != id);
        self.active_trackers.len() != before
    }

    pub fn set_leaderboard_started(&mut self, id: u32, tracker_text: Option<String>) {
        if let Some(lb) = self.leaderboards.iter_mut().find(|l| l.id == id) {
            lb.state = LeaderboardState::Tracking;
            if tracker_text.is_some() {
                lb.tracker_value = tracker_text;
            }
        }
    }

    pub fn set_leaderboard_finished(&mut self, id: u32, final_score: Option<String>) {
        if let Some(lb) = self.leaderboards.iter_mut().find(|l| l.id == id) {
            lb.state = LeaderboardState::Active;
            lb.tracker_value = final_score;
        }
    }

    pub fn reset_indicators(&mut self) {
        self.active_challenges.clear();
        self.active_trackers.clear();
        for lb in &mut self.leaderboards {
            if lb.state == LeaderboardState::Tracking {
                lb.state = LeaderboardState::Active;
                lb.tracker_value = None;
            }
        }
    }

    pub fn to_view_dto(
        &self,
        badges_dir: Option<&Path>,
    ) -> (RaGameViewDto, Vec<(PathBuf, String)>) {
        let mut view = RaGameViewDto {
            status: "loaded".into(),
            game_id: Some(self.game_id),
            title: Some(self.title.clone()),
            badge_url: self.badge_url.clone(),
            hardcore: self.hardcore,
            hardcore_downgraded: self.hardcore_downgraded,
            compatible_rom_id: None,
            total_achievements: self.summary.num_core_achievements,
            unlocked_achievements: self.summary.num_unlocked_achievements,
            total_points: self.summary.points_core,
            unlocked_points: self.summary.points_unlocked,
            achievements: self.achievements.iter().cloned().map(Into::into).collect(),
            playing_live: true,
            rich_presence: self.rich_presence.clone(),
            leaderboards: self.leaderboards.iter().cloned().map(Into::into).collect(),
            active_trackers: self.active_trackers.clone(),
            active_challenges: self.active_challenges.clone(),
        };
        let mut pending = Vec::new();
        rewrite_view_badge_urls(&mut view, badges_dir, &mut pending);
        (view, pending)
    }
}

fn handle_game_loaded(app: &AppHandle, snapshot: RaLoadedSnapshot) {
    let ra_state = app.state::<RaState>();
    let rom_id = ra_state.active_rom();
    let rom_hash = ra_state.active_rom_hash();
    let badges_dir = app_badges_dir(app);
    if snapshot.hardcore_downgraded || snapshot.incompatible_version {
        ra_state.set_hardcore_active(false);
    }
    if !snapshot.incompatible_version {
        crate::gamelog::on_ra_game_loaded(app, rom_id.clone(), &snapshot);
    }
    let live = LiveGameAchievements {
        rom_id: rom_id.clone(),
        rom_hash: rom_hash.clone(),
        game_id: snapshot.game_id,
        title: snapshot.title.clone(),
        badge_url: snapshot.badge_url.clone(),
        hardcore: snapshot.hardcore,
        hardcore_downgraded: snapshot.hardcore_downgraded,
        incompatible_version: snapshot.incompatible_version,
        summary: snapshot.summary.clone(),
        achievements: snapshot.achievements,
        leaderboards: snapshot.leaderboards,
        active_trackers: Vec::new(),
        active_challenges: Vec::new(),
        rich_presence: snapshot.rich_presence,
    };
    if !live.incompatible_version && !is_incompatible_game_id(live.game_id) {
        let (raw_view, _) = live.to_view_dto(None);
        store_cached_game_view(badges_dir.as_deref(), rom_hash.as_deref(), &raw_view);
        let (_, pending_badges) = live.to_view_dto(badges_dir.as_deref());
        ra_state.queue_badge_downloads(pending_badges);
    }
    let mut pending_icon = Vec::new();
    let resolved_icon =
        resolve_badge_url_for_ui(badges_dir.as_deref(), snapshot.badge_url, &mut pending_icon);
    ra_state.queue_badge_downloads(pending_icon);

    let event_dto = RaGameLoadedEventDto {
        rom_id,
        game_id: snapshot.game_id,
        title: snapshot.title,
        badge_url: resolved_icon,
        hardcore: snapshot.hardcore,
        unlocked_achievements: snapshot.summary.num_unlocked_achievements,
        total_achievements: if snapshot.incompatible_version {
            0
        } else {
            snapshot.summary.num_core_achievements
        },
        unlocked_points: snapshot.summary.points_unlocked,
        total_points: snapshot.summary.points_core,
    };
    ra_state.set_live(Some(live));
    let _ = app.emit("ra-game-loaded", event_dto);
}

fn handle_runtime_event(app: &AppHandle, event: AchievementEvent) {
    let ra_state = app.state::<RaState>();
    let badges_dir = app_badges_dir(app);
    match event {
        AchievementEvent::AchievementTriggered {
            id,
            title,
            description,
            points,
            hardcore,
            badge_url,
            ..
        } => {
            if is_warning_achievement_id(id) {
                return;
            }
            ra_state.mark_live_unlocked(id, hardcore);
            if let Some(live_snap) = ra_state
                .live
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone()
            {
                let (raw_view, _) = live_snap.to_view_dto(None);
                store_cached_game_view(
                    badges_dir.as_deref(),
                    live_snap.rom_hash.as_deref(),
                    &raw_view,
                );
            }
            crate::gamelog::on_ra_achievement_unlocked(
                app,
                id,
                &title,
                &description,
                points,
                hardcore,
                badge_url.as_deref(),
            );
            let mut pending = Vec::new();
            let resolved_badge =
                resolve_badge_url_for_ui(badges_dir.as_deref(), badge_url, &mut pending);
            ra_state.queue_badge_downloads(pending);
            let _ = app.emit(
                "ra-achievement-unlocked",
                RaUnlockEventDto {
                    rom_id: ra_state.active_rom(),
                    id,
                    title,
                    description,
                    points,
                    hardcore,
                    badge_url: resolved_badge,
                    game_completed: false,
                },
            );
        }
        AchievementEvent::ProgressIndicatorShow {
            id,
            measured_progress,
            measured_percent,
            ..
        }
        | AchievementEvent::ProgressIndicatorUpdate {
            id,
            measured_progress,
            measured_percent,
            ..
        } => {
            let rom_id = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                let Some(live) = guard.as_mut() else {
                    return;
                };
                live.update_progress(id, measured_progress, measured_percent);
                live.rom_id.clone()
            };
            let _ = app.emit("ra-live-updated", RaLiveUpdatedEventDto { rom_id });
        }
        AchievementEvent::ChallengeIndicatorShow {
            id,
            title,
            description,
            points,
            badge_url,
            ..
        } => {
            let mut pending = Vec::new();
            let resolved_badge =
                resolve_badge_url_for_ui(badges_dir.as_deref(), badge_url, &mut pending);
            ra_state.queue_badge_downloads(pending);
            let rom_id = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                let Some(live) = guard.as_mut() else {
                    return;
                };
                live.show_challenge(RaChallengeDto {
                    id,
                    title,
                    description,
                    points,
                    badge_url: resolved_badge,
                });
                live.rom_id.clone()
            };
            let _ = app.emit("ra-live-updated", RaLiveUpdatedEventDto { rom_id });
        }
        AchievementEvent::ChallengeIndicatorHide { id } => {
            let notify = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                guard
                    .as_mut()
                    .and_then(|live| live.hide_challenge(id).then(|| live.rom_id.clone()))
            };
            if let Some(rom_id) = notify {
                let _ = app.emit("ra-live-updated", RaLiveUpdatedEventDto { rom_id });
            }
        }
        AchievementEvent::LeaderboardTrackerUpdate { id, display } => {
            let notify = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                guard.as_mut().and_then(|live| {
                    live.upsert_tracker(id, display)
                        .then(|| live.rom_id.clone())
                })
            };
            if let Some(rom_id) = notify {
                let _ = app.emit("ra-live-updated", RaLiveUpdatedEventDto { rom_id });
            }
        }
        AchievementEvent::LeaderboardTrackerHide { id } => {
            let notify = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                guard
                    .as_mut()
                    .and_then(|live| live.hide_tracker(id).then(|| live.rom_id.clone()))
            };
            if let Some(rom_id) = notify {
                let _ = app.emit("ra-live-updated", RaLiveUpdatedEventDto { rom_id });
            }
        }
        AchievementEvent::LeaderboardStarted {
            id,
            title,
            description,
            tracker_text,
        } => {
            let (rom_id, hardcore) = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(live) = guard.as_mut() {
                    live.set_leaderboard_started(id, tracker_text.clone());
                    (live.rom_id.clone(), live.hardcore)
                } else {
                    (ra_state.active_rom(), ra_state.is_hardcore_active())
                }
            };
            let _ = app.emit(
                "ra-leaderboard-event",
                RaLeaderboardEventDto {
                    rom_id,
                    kind: "started".into(),
                    id,
                    title,
                    description: (!description.is_empty()).then_some(description),
                    formatted_score: tracker_text,
                    best_score: None,
                    rank: None,
                    total_entries: None,
                    hardcore,
                },
            );
        }
        AchievementEvent::LeaderboardSubmitted {
            id,
            title,
            formatted_score,
            rank_info,
        } => {
            let score_opt = (!formatted_score.is_empty()).then_some(formatted_score);
            let (rom_id, hardcore) = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(live) = guard.as_mut() {
                    live.set_leaderboard_finished(id, score_opt.clone());
                    (live.rom_id.clone(), live.hardcore)
                } else {
                    (ra_state.active_rom(), ra_state.is_hardcore_active())
                }
            };
            let (best_score, rank, total_entries) = match rank_info {
                Some(r) => (
                    (!r.best_score.is_empty()).then_some(r.best_score),
                    Some(r.new_rank),
                    Some(r.num_entries),
                ),
                None => (None, None, None),
            };
            let _ = app.emit(
                "ra-leaderboard-event",
                RaLeaderboardEventDto {
                    rom_id,
                    kind: "submitted".into(),
                    id,
                    title,
                    description: None,
                    formatted_score: score_opt,
                    best_score,
                    rank,
                    total_entries,
                    hardcore,
                },
            );
        }
        AchievementEvent::LeaderboardFailed { id, title } => {
            let (rom_id, hardcore) = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(live) = guard.as_mut() {
                    live.set_leaderboard_finished(id, None);
                    (live.rom_id.clone(), live.hardcore)
                } else {
                    (ra_state.active_rom(), ra_state.is_hardcore_active())
                }
            };
            let _ = app.emit(
                "ra-leaderboard-event",
                RaLeaderboardEventDto {
                    rom_id,
                    kind: "failed".into(),
                    id,
                    title,
                    description: None,
                    formatted_score: None,
                    best_score: None,
                    rank: None,
                    total_entries: None,
                    hardcore,
                },
            );
        }
        AchievementEvent::RichPresenceChanged(rp) => {
            let rom_id = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                let Some(live) = guard.as_mut() else {
                    return;
                };
                if live.rich_presence == rp {
                    return;
                }
                live.rich_presence = rp;
                live.rom_id.clone()
            };
            let _ = app.emit("ra-live-updated", RaLiveUpdatedEventDto { rom_id });
        }
        AchievementEvent::ResetRequested => {
            let rom_id = {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                let Some(live) = guard.as_mut() else {
                    return;
                };
                live.reset_indicators();
                live.rom_id.clone()
            };
            let _ = app.emit("ra-live-updated", RaLiveUpdatedEventDto { rom_id });
        }
        AchievementEvent::RuntimeWarning { title, .. } => {
            let lower = title.to_ascii_lowercase();
            let mut notify_dto = None;
            {
                let mut guard = ra_state.live.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(live) = guard.as_mut() {
                    if lower.contains("unknown emulator") {
                        live.hardcore = false;
                        live.hardcore_downgraded = true;
                        ra_state.set_hardcore_active(false);
                    }
                    if lower.contains("unsupported game version") {
                        live.hardcore = false;
                        live.incompatible_version = true;
                        ra_state.set_hardcore_active(false);
                    }
                    let mut pending = Vec::new();
                    let icon = resolve_badge_url_for_ui(
                        badges_dir.as_deref(),
                        live.badge_url.clone(),
                        &mut pending,
                    );
                    notify_dto = Some(RaGameLoadedEventDto {
                        rom_id: live.rom_id.clone(),
                        game_id: live.game_id,
                        title: live.title.clone(),
                        badge_url: icon,
                        hardcore: live.hardcore,
                        unlocked_achievements: live.summary.num_unlocked_achievements,
                        total_achievements: 0,
                        unlocked_points: live.summary.points_unlocked,
                        total_points: live.summary.points_core,
                    });
                }
            }
            if let Some(dto) = notify_dto {
                let _ = app.emit("ra-game-loaded", dto);
            }
        }
        AchievementEvent::GameCompleted { hardcore } => {
            let live_title = ra_state
                .live
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .map(|l| (l.title.clone(), l.badge_url.clone()));
            if let Some((title, badge_url)) = live_title {
                let mut pending = Vec::new();
                let resolved_badge =
                    resolve_badge_url_for_ui(badges_dir.as_deref(), badge_url, &mut pending);
                ra_state.queue_badge_downloads(pending);
                let _ = app.emit(
                    "ra-achievement-unlocked",
                    RaUnlockEventDto {
                        rom_id: ra_state.active_rom(),
                        id: 0,
                        title,
                        description: String::new(),
                        points: 0,
                        hardcore,
                        badge_url: resolved_badge,
                        game_completed: true,
                    },
                );
            }
        }
        AchievementEvent::ServerError { api, message, .. } => {
            log::warn!("ra ({api}): {message}");
        }
        _ => {}
    }
}

/// Chamado em `load_game` logo depois de abrir o core. Devolve `true` quando o
/// modo Hardcore esta ativo para esta partida (desabilitando cheats e load state).
pub async fn on_game_launched(
    app: &AppHandle,
    core_id: Option<&str>,
    rom_id: Option<&str>,
    rom_path: &str,
) -> bool {
    let ra_state = app.state::<RaState>();
    ra_state.set_live(None);
    ra_state.set_hardcore_active(false);
    ra_state.set_active_rom(rom_id.map(str::to_owned));
    ra_state.set_active_rom_hash(None);

    let state = app.state::<AppState>();
    let Some(pool) = state.db.clone() else {
        return false;
    };
    let Ok((cfg, Some(token))) = credentials::load_ra_session(&pool, &OsKeyring).await else {
        return false;
    };
    if !cfg.connected || !cfg.enabled {
        return false;
    }
    let Some(username) = cfg.username else {
        return false;
    };

    let (cache_key, system_id) = match rom_id {
        Some(rid) => {
            let Ok(Some(rom)) = db::RomsRepo::new(pool).get(rid).await else {
                return false;
            };
            (rom.id, rom.system_id)
        }
        None => (rom_path.to_owned(), String::new()),
    };

    let preferred_console =
        console_id_for_system(&system_id).or_else(|| core_id.and_then(console_id_for_core));
    if !can_attempt_ra(&system_id, preferred_console, Path::new(rom_path)) {
        return false;
    }

    let extracted_hint = state.session.extracted_content_path();
    let cache_file = app_hash_cache_file(app);
    let resolved = resolve_rom_console_and_hash(
        &ra_state,
        cache_file.as_deref(),
        preferred_console,
        &cache_key,
        &system_id,
        rom_path,
        extracted_hint.as_deref(),
    )
    .await;

    let (console_id, rom_hash) = match (resolved, preferred_console) {
        (Some((cid, hash)), _) => (cid, Some(hash)),
        (None, Some(cid)) => (cid, None),
        (None, None) => return false,
    };

    ra_state.set_active_rom_hash(rom_hash.clone());
    let hardcore_active = cfg.hardcore && rom_hash.is_some();
    ra_state.set_hardcore_active(hardcore_active);
    state.session.init_achievements(RaSessionConfig {
        console_id,
        rom_path: rom_path.to_owned(),
        rom_hash,
        username,
        token,
        hardcore: cfg.hardcore,
        unofficial: cfg.unofficial,
        encore: cfg.encore,
    });
    hardcore_active
}

pub fn on_game_unloaded(app: &AppHandle) {
    let ra_state = app.state::<RaState>();
    ra_state.set_hardcore_active(false);
    ra_state.set_active_rom(None);
    ra_state.set_active_rom_hash(None);
    ra_state.set_live(None);
}

// ---------------------------------------------------------------------------
// DTOs exportados ao frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaStatus"))]
pub struct RaStatusDto {
    pub connected: bool,
    pub enabled: bool,
    pub hardcore: bool,
    pub unofficial: bool,
    pub encore: bool,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub score: u32,
    pub score_softcore: u32,
    pub linked_alvanista_username: Option<String>,
}

impl From<RaConfig> for RaStatusDto {
    fn from(c: RaConfig) -> Self {
        Self {
            connected: c.connected,
            enabled: c.enabled,
            hardcore: c.hardcore,
            unofficial: c.unofficial,
            encore: c.encore,
            username: c.username,
            display_name: c.display_name,
            avatar_url: c.avatar_url,
            score: c.score,
            score_softcore: c.score_softcore,
            linked_alvanista_username: c.linked_alvanista_username,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaAchievement"))]
pub struct RaAchievementDto {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub unlocked: bool,
    pub unlocked_hardcore: bool,
    pub unlock_time: Option<i64>,
    pub badge_url: Option<String>,
    pub badge_locked_url: Option<String>,
    pub measured_progress: Option<String>,
    pub measured_percent: f32,
    pub rarity: f32,
    pub rarity_hardcore: f32,
    #[cfg_attr(
        test,
        ts(type = "'standard' | 'missable' | 'progression' | 'win_condition'")
    )]
    pub kind: String,
    pub unofficial: bool,
    pub unsupported: bool,
}

impl From<AchievementEntry> for RaAchievementDto {
    fn from(a: AchievementEntry) -> Self {
        let kind = match a.kind {
            AchievementKind::Standard => "standard",
            AchievementKind::Missable => "missable",
            AchievementKind::Progression => "progression",
            AchievementKind::WinCondition => "win_condition",
        };
        Self {
            id: a.id,
            title: a.title,
            description: a.description,
            points: a.points,
            unlocked: a.unlocked,
            unlocked_hardcore: a.unlocked_hardcore,
            unlock_time: a.unlock_time,
            badge_url: a.badge_url,
            badge_locked_url: a.badge_locked_url,
            measured_progress: a.measured_progress,
            measured_percent: a.measured_percent,
            rarity: a.rarity,
            rarity_hardcore: a.rarity_hardcore,
            kind: kind.into(),
            unofficial: a.unofficial,
            unsupported: a.unsupported,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaLeaderboard"))]
pub struct RaLeaderboardDto {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub tracker_value: Option<String>,
    #[cfg_attr(test, ts(type = "'inactive' | 'active' | 'tracking' | 'disabled'"))]
    pub state: String,
    #[cfg_attr(test, ts(type = "'time' | 'score' | 'value'"))]
    pub format: String,
    pub lower_is_better: bool,
}

impl From<LeaderboardInfo> for RaLeaderboardDto {
    fn from(lb: LeaderboardInfo) -> Self {
        let state = match lb.state {
            LeaderboardState::Inactive => "inactive",
            LeaderboardState::Active => "active",
            LeaderboardState::Tracking => "tracking",
            LeaderboardState::Disabled => "disabled",
        };
        let format = match lb.format {
            LeaderboardFormat::Time => "time",
            LeaderboardFormat::Score => "score",
            LeaderboardFormat::Value => "value",
        };
        Self {
            id: lb.id,
            title: lb.title,
            description: lb.description,
            tracker_value: lb.tracker_value,
            state: state.into(),
            format: format.into(),
            lower_is_better: lb.lower_is_better,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaTracker"))]
pub struct RaTrackerDto {
    pub id: u32,
    pub display: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaChallenge"))]
pub struct RaChallengeDto {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub badge_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaGameView"))]
pub struct RaGameViewDto {
    #[cfg_attr(
        test,
        ts(
            type = "'not_connected' | 'unsupported_system' | 'unidentified' | 'incompatible_version' | 'loaded'"
        )
    )]
    pub status: String,
    pub game_id: Option<u32>,
    pub title: Option<String>,
    pub badge_url: Option<String>,
    pub hardcore: bool,
    pub hardcore_downgraded: bool,
    pub compatible_rom_id: Option<String>,
    pub total_achievements: u32,
    pub unlocked_achievements: u32,
    pub total_points: u32,
    pub unlocked_points: u32,
    pub achievements: Vec<RaAchievementDto>,
    #[serde(default)]
    pub playing_live: bool,
    #[serde(default)]
    pub rich_presence: Option<String>,
    #[serde(default)]
    pub leaderboards: Vec<RaLeaderboardDto>,
    #[serde(default)]
    pub active_trackers: Vec<RaTrackerDto>,
    #[serde(default)]
    pub active_challenges: Vec<RaChallengeDto>,
}

impl RaGameViewDto {
    fn empty(status: &str, hardcore: bool) -> Self {
        Self {
            status: status.into(),
            game_id: None,
            title: None,
            badge_url: None,
            hardcore,
            hardcore_downgraded: false,
            compatible_rom_id: None,
            total_achievements: 0,
            unlocked_achievements: 0,
            total_points: 0,
            unlocked_points: 0,
            achievements: Vec::new(),
            playing_live: false,
            rich_presence: None,
            leaderboards: Vec::new(),
            active_trackers: Vec::new(),
            active_challenges: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaGameLoadedEvent"))]
pub struct RaGameLoadedEventDto {
    pub rom_id: Option<String>,
    pub game_id: u32,
    pub title: String,
    pub badge_url: Option<String>,
    pub hardcore: bool,
    pub unlocked_achievements: u32,
    pub total_achievements: u32,
    pub unlocked_points: u32,
    pub total_points: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaUnlockEvent"))]
pub struct RaUnlockEventDto {
    pub rom_id: Option<String>,
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub hardcore: bool,
    pub badge_url: Option<String>,
    pub game_completed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaLeaderboardEvent"))]
pub struct RaLeaderboardEventDto {
    pub rom_id: Option<String>,
    #[cfg_attr(test, ts(type = "'started' | 'submitted' | 'failed'"))]
    pub kind: String,
    pub id: u32,
    pub title: String,
    pub description: Option<String>,
    pub formatted_score: Option<String>,
    pub best_score: Option<String>,
    pub rank: Option<u32>,
    pub total_entries: Option<u32>,
    pub hardcore: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaLiveUpdatedEvent"))]
pub struct RaLiveUpdatedEventDto {
    pub rom_id: Option<String>,
}

fn db_pool(state: &AppState) -> Result<db::Db, AppError> {
    state
        .db
        .clone()
        .ok_or_else(|| AppError::from("banco de dados indisponível"))
}

pub fn emit_ra_status_changed(app: &AppHandle, status: &RaStatusDto) {
    let _ = app.emit("ra-status-changed", status);
}

#[tauri::command]
pub async fn ra_status(state: State<'_, AppState>) -> Result<RaStatusDto, AppError> {
    let pool = db_pool(&state)?;
    let (cfg, _) = credentials::load_ra_session(&pool, &OsKeyring)
        .await
        .map_err(AppError::from)?;
    Ok(cfg.into())
}

#[tauri::command]
pub async fn ra_login(
    app: AppHandle,
    state: State<'_, AppState>,
    ra_state: State<'_, RaState>,
    username: String,
    password: String,
) -> Result<RaStatusDto, AppError> {
    let username = username.trim().to_owned();
    if username.is_empty() || password.is_empty() {
        return Err(AppError::from(
            "Informe usuário e senha do RetroAchievements.",
        ));
    }
    let pool = db_pool(&state)?;
    let http = ra_state.http_client();

    let user = {
        let mut client = AchievementClient::new();
        client
            .begin_login_with_password(&username, &password)
            .map_err(|_| AppError::from("Usuário ou senha inválidos no RetroAchievements."))?;
        // A senha já foi copiada para o payload temporário do `r=login2`; daqui
        // em diante só o `connect_token` devolvido pelo servidor é mantido.
        drop(password);
        drive_client_requests(&http, &mut client).await;

        let mut outcome = None;
        for o in client.take_outcomes() {
            if let AsyncOutcome::Login(res) = o {
                outcome = Some(res);
            }
        }
        match outcome {
            Some(Ok(u)) => u,
            Some(Err(RaError::InvalidCredentials)) => {
                return Err(AppError::from(
                    "Usuário ou senha inválidos no RetroAchievements.",
                ))
            }
            Some(Err(e)) => return Err(AppError::new(ErrorCode::Offline, e.to_string())),
            None => {
                return Err(AppError::new(
                    ErrorCode::Offline,
                    "Sem resposta do RetroAchievements.",
                ))
            }
        }
    };

    let cfg = credentials::save_ra_login(
        &pool,
        &OsKeyring,
        &user.username,
        Some(user.display_name),
        user.avatar_url,
        user.score,
        user.score_softcore,
        &user.token,
    )
    .await
    .map_err(AppError::from)?;

    let dto: RaStatusDto = cfg.into();
    emit_ra_status_changed(&app, &dto);
    crate::sync::engine::after_credentials_changed(&app);
    Ok(dto)
}

#[tauri::command]
pub async fn ra_logout(
    app: AppHandle,
    state: State<'_, AppState>,
    ra_state: State<'_, RaState>,
) -> Result<RaStatusDto, AppError> {
    let pool = db_pool(&state)?;
    let cfg = credentials::clear_ra_login(&pool, &OsKeyring)
        .await
        .map_err(AppError::from)?;
    ra_state.set_hardcore_active(false);
    ra_state.set_live(None);
    let dto: RaStatusDto = cfg.into();
    emit_ra_status_changed(&app, &dto);
    crate::sync::engine::after_credentials_changed(&app);
    Ok(dto)
}

#[tauri::command]
pub async fn ra_set_preferences(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: Option<bool>,
    hardcore: Option<bool>,
    unofficial: Option<bool>,
    encore: Option<bool>,
) -> Result<RaStatusDto, AppError> {
    let pool = db_pool(&state)?;
    let cfg = credentials::update_ra_preferences(&pool, enabled, hardcore, unofficial, encore)
        .await
        .map_err(AppError::from)?;
    let dto: RaStatusDto = cfg.into();
    emit_ra_status_changed(&app, &dto);
    crate::sync::engine::after_credentials_changed(&app);
    Ok(dto)
}

#[derive(Deserialize)]
struct UnlocksReply {
    #[serde(rename = "Success", default)]
    success: bool,
    #[serde(rename = "UserUnlocks", default)]
    user_unlocks: Vec<u32>,
}

async fn fetch_unlocks_set(
    http: &reqwest::Client,
    username: &str,
    token: &str,
    game_id: u32,
    hardcore: bool,
) -> HashSet<u32> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("r", "unlocks")
        .append_pair("u", username)
        .append_pair("t", token)
        .append_pair("g", &game_id.to_string())
        .append_pair("h", if hardcore { "1" } else { "0" })
        .finish();
    let req = OutboundHttpRequest {
        id: 0,
        url: "https://retroachievements.org/dorequest.php".into(),
        post_data: Some(body),
        content_type: Some("application/x-www-form-urlencoded".into()),
    };
    let (status, bytes) = execute_rcheevos_http(http, &req).await;
    if status != 200 {
        return HashSet::new();
    }
    serde_json::from_slice::<UnlocksReply>(&bytes)
        .ok()
        .filter(|r| r.success)
        .map(|r| r.user_unlocks.into_iter().collect())
        .unwrap_or_default()
}

#[derive(Deserialize)]
struct GameIdReply {
    #[serde(rename = "Success", default)]
    success: bool,
    #[serde(rename = "GameID", default)]
    game_id: u32,
}

async fn lookup_ra_game_id_by_hash(http: &reqwest::Client, hash: &str) -> Option<u32> {
    let url = format!("https://retroachievements.org/dorequest.php?r=gameid&m={hash}");
    let req = OutboundHttpRequest {
        id: 0,
        url,
        post_data: None,
        content_type: None,
    };
    let (status, bytes) = execute_rcheevos_http(http, &req).await;
    if status != 200 {
        return None;
    }
    let parsed: GameIdReply = serde_json::from_slice(&bytes).ok()?;
    (parsed.success && parsed.game_id > 0).then_some(parsed.game_id)
}

async fn find_compatible_sibling_rom_id(
    pool: &db::Db,
    ra_state: &RaState,
    http: &reqwest::Client,
    cache_file: Option<&Path>,
    current: &domain::library::Rom,
    preferred_console: Option<u32>,
) -> Option<String> {
    let base_stem = Path::new(&current.file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&current.file_path);
    let target_canon =
        scraping::rom_name::canonical_key(&scraping::rom_name::clean_rom_title(base_stem));
    if target_canon.is_empty() {
        return None;
    }
    let all_roms = db::RomsRepo::new(pool.clone()).list().await.ok()?;
    let siblings: Vec<domain::library::Rom> = all_roms
        .into_iter()
        .filter(|r| r.id != current.id && r.system_id == current.system_id)
        .filter(|r| {
            let stem = Path::new(&r.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&r.file_path);
            let canon =
                scraping::rom_name::canonical_key(&scraping::rom_name::clean_rom_title(stem));
            canon == target_canon
        })
        .take(8)
        .collect();

    for sib in siblings {
        let Some((_, sib_hash)) = resolve_rom_console_and_hash(
            ra_state,
            cache_file,
            preferred_console,
            &sib.id,
            &sib.system_id,
            &sib.file_path,
            None,
        )
        .await
        else {
            continue;
        };
        if let Some(gid) = lookup_ra_game_id_by_hash(http, &sib_hash).await {
            if !is_incompatible_game_id(gid) {
                return Some(sib.id);
            }
        }
    }
    None
}

#[tauri::command]
pub async fn ra_game_achievements(
    app: AppHandle,
    state: State<'_, AppState>,
    ra_state: State<'_, RaState>,
    rom_id: String,
) -> Result<RaGameViewDto, AppError> {
    let pool = db_pool(&state)?;
    let (cfg, Some(token)) = credentials::load_ra_session(&pool, &OsKeyring)
        .await
        .map_err(AppError::from)?
    else {
        return Ok(RaGameViewDto::empty("not_connected", false));
    };
    let Some(username) = cfg.username.clone() else {
        return Ok(RaGameViewDto::empty("not_connected", cfg.hardcore));
    };

    let cache_file = app_hash_cache_file(&app);
    let badges_dir = app_badges_dir(&app);
    let http = ra_state.http_client();

    // Se este mesmo jogo está rodando agora, o snapshot vivo já traz os
    // desbloqueios da sessão atual sem nova ida à rede.
    if let Some(live) = ra_state.live_for(&rom_id) {
        if live.incompatible_version || is_incompatible_game_id(live.game_id) {
            let compatible_rom_id =
                if let Ok(Some(cur_rom)) = db::RomsRepo::new(pool.clone()).get(&rom_id).await {
                    let pref = console_id_for_system(&cur_rom.system_id);
                    find_compatible_sibling_rom_id(
                        &pool,
                        &ra_state,
                        &http,
                        cache_file.as_deref(),
                        &cur_rom,
                        pref,
                    )
                    .await
                } else {
                    None
                };
            let mut dto = RaGameViewDto::empty("incompatible_version", false);
            dto.hardcore_downgraded = live.hardcore_downgraded;
            dto.compatible_rom_id = compatible_rom_id;
            return Ok(dto);
        }
        let (view, pending_badges) = live.to_view_dto(badges_dir.as_deref());
        ra_state.queue_badge_downloads(pending_badges);
        return Ok(view);
    }

    let Some(rom) = db::RomsRepo::new(pool.clone())
        .get(&rom_id)
        .await
        .map_err(AppError::from)?
    else {
        return Ok(RaGameViewDto::empty("unidentified", cfg.hardcore));
    };
    let preferred_console = console_id_for_system(&rom.system_id);
    if !can_attempt_ra(&rom.system_id, preferred_console, Path::new(&rom.file_path)) {
        return Ok(RaGameViewDto::empty("unsupported_system", cfg.hardcore));
    }

    let Some((_resolved_console_id, hash)) = resolve_rom_console_and_hash(
        &ra_state,
        cache_file.as_deref(),
        preferred_console,
        &rom.id,
        &rom.system_id,
        &rom.file_path,
        None,
    )
    .await
    else {
        return Ok(RaGameViewDto::empty("unidentified", cfg.hardcore));
    };

    let (loaded_game, mut entries, leaderboards, warnings) = {
        let mut client = AchievementClient::new();
        // `spectator = true` carrega definições (`r=achievementsets`) sem enviar
        // `r=startsession`, evitando marcar "Jogando" só por abrir a página do jogo.
        client.set_spectator(true);
        client.set_hardcore(cfg.hardcore);
        client.set_unofficial(cfg.unofficial);
        client
            .begin_login_with_token(&username, &token)
            .map_err(|e| AppError::from(e.to_string()))?;
        drive_client_requests(&http, &mut client).await;

        let mut logged_in = false;
        let mut auth_rejected = false;
        for o in client.take_outcomes() {
            match o {
                AsyncOutcome::Login(Ok(_)) => logged_in = true,
                AsyncOutcome::Login(Err(RaError::InvalidCredentials | RaError::ExpiredToken)) => {
                    auth_rejected = true;
                }
                _ => {}
            }
        }
        if !logged_in {
            if !auth_rejected {
                if let Some((cached_view, pending_badges)) =
                    load_cached_game_view(badges_dir.as_deref(), &hash)
                {
                    ra_state.queue_badge_downloads(pending_badges);
                    return Ok(cached_view);
                }
                return Err(AppError::new(
                    ErrorCode::Offline,
                    "Sem conexão com o RetroAchievements no momento.",
                ));
            }
            return Err(AppError::from(
                "Sessão expirada no RetroAchievements. Entre novamente em Configurações.",
            ));
        }

        client
            .begin_load_game_by_hash(&hash)
            .map_err(|e| AppError::from(e.to_string()))?;
        drive_client_requests(&http, &mut client).await;

        let game_load_outcomes = client.take_outcomes();
        let load_network_error = game_load_outcomes.iter().any(|o| {
            matches!(
                o,
                AsyncOutcome::GameLoad(Err(RaError::Api { code, .. }))
                    if *code != achievements::ffi::RC_NO_GAME_LOADED
            )
        });
        let Some(game) = client.game_info() else {
            if load_network_error {
                if let Some((cached_view, pending_badges)) =
                    load_cached_game_view(badges_dir.as_deref(), &hash)
                {
                    ra_state.queue_badge_downloads(pending_badges);
                    return Ok(cached_view);
                }
            }
            let compatible_rom_id = find_compatible_sibling_rom_id(
                &pool,
                &ra_state,
                &http,
                cache_file.as_deref(),
                &rom,
                preferred_console,
            )
            .await;
            let mut dto = RaGameViewDto::empty("unidentified", cfg.hardcore);
            dto.compatible_rom_id = compatible_rom_id;
            return Ok(dto);
        };
        let warnings = client.runtime_warnings();
        let list = client.achievements(cfg.unofficial);
        let lbs = client.leaderboards();
        (game, list, lbs, warnings)
    };

    if is_incompatible_game_id(loaded_game.id) || warnings.unsupported_version {
        let compatible_rom_id = find_compatible_sibling_rom_id(
            &pool,
            &ra_state,
            &http,
            cache_file.as_deref(),
            &rom,
            preferred_console,
        )
        .await;
        let mut dto = RaGameViewDto::empty("incompatible_version", false);
        dto.hardcore_downgraded = cfg.hardcore && warnings.unknown_emulator;
        dto.compatible_rom_id = compatible_rom_id;
        return Ok(dto);
    }

    let soft_task = {
        let (http, u, t, gid) = (
            http.clone(),
            username.clone(),
            token.clone(),
            loaded_game.id,
        );
        tauri::async_runtime::spawn(
            async move { fetch_unlocks_set(&http, &u, &t, gid, false).await },
        )
    };
    let hard_unlocks = fetch_unlocks_set(&http, &username, &token, loaded_game.id, true).await;
    let soft_unlocks = soft_task.await.unwrap_or_default();

    let mut unlocked_count = 0u32;
    let mut unlocked_points = 0u32;
    let mut total_count = 0u32;
    let mut total_points = 0u32;
    for item in &mut entries {
        let is_hard = hard_unlocks.contains(&item.id);
        let is_soft = soft_unlocks.contains(&item.id) || is_hard;
        item.unlocked = is_soft;
        item.unlocked_hardcore = is_hard;
        if !item.unofficial {
            total_count += 1;
            total_points += item.points;
            if is_soft {
                unlocked_count += 1;
                unlocked_points += item.points;
            }
        }
    }

    let hardcore_downgraded = cfg.hardcore && warnings.unknown_emulator;
    let effective_hardcore = cfg.hardcore && !hardcore_downgraded;

    let mut view = RaGameViewDto {
        status: "loaded".into(),
        game_id: Some(loaded_game.id),
        title: Some(loaded_game.title),
        badge_url: loaded_game.badge_url,
        hardcore: effective_hardcore,
        hardcore_downgraded,
        compatible_rom_id: None,
        total_achievements: total_count,
        unlocked_achievements: unlocked_count,
        total_points,
        unlocked_points,
        achievements: entries.into_iter().map(Into::into).collect(),
        playing_live: false,
        rich_presence: None,
        leaderboards: leaderboards.into_iter().map(Into::into).collect(),
        active_trackers: Vec::new(),
        active_challenges: Vec::new(),
    };
    store_cached_game_view(badges_dir.as_deref(), Some(&hash), &view);
    let mut pending_badges = Vec::new();
    rewrite_view_badge_urls(&mut view, badges_dir.as_deref(), &mut pending_badges);
    ra_state.queue_badge_downloads(pending_badges);
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use achievements::{
        console::{RC_CONSOLE_PLAYSTATION, RC_CONSOLE_PLAYSTATION_2, RC_CONSOLE_SATURN},
        hash_file,
    };
    use std::io::Write as _;

    #[test]
    fn compute_rom_hash_extracts_inner_rom_from_zip_and_preserves_arcade_zip() {
        let dir = tempfile::tempdir().unwrap();
        let payload = vec![0xA5u8; 4096];
        let raw_path = dir.path().join("River Raid (USA).a26");
        std::fs::write(&raw_path, &payload).unwrap();

        let zip_path = dir.path().join("River Raid (USA).zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zw.start_file("readme.html", opts).unwrap();
            zw.write_all(b"<html>bundle note</html>").unwrap();
            zw.start_file("River Raid (USA).A26", opts).unwrap();
            zw.write_all(&payload).unwrap();
            zw.finish().unwrap();
        }

        let a26_console = console_id_for_system("atari2600").unwrap();
        let raw_res =
            compute_rom_console_and_hash(Some(a26_console), "atari2600", &raw_path, None).unwrap();
        let zip_res =
            compute_rom_console_and_hash(Some(a26_console), "atari2600", &zip_path, None).unwrap();
        assert_eq!(zip_res, raw_res);
        assert_eq!(zip_res.0, a26_console);

        let arcade_zip = dir.path().join("mslug.zip");
        {
            let file = std::fs::File::create(&arcade_zip).unwrap();
            let mut zw = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("201-p1.p1", opts).unwrap();
            zw.write_all(b"chip").unwrap();
            zw.finish().unwrap();
        }
        let arcade_res =
            compute_rom_console_and_hash(Some(RC_CONSOLE_ARCADE), "arcade", &arcade_zip, None)
                .unwrap();
        let expected_arcade = hash_file(RC_CONSOLE_ARCADE, &arcade_zip).unwrap();
        assert_eq!(arcade_res, (RC_CONSOLE_ARCADE, expected_arcade));
    }

    #[test]
    fn resolves_generic_disc_system_persists_disk_cache_and_reuses_extracted_hint() {
        let dir = tempfile::tempdir().unwrap();
        let mut saturn_iso = vec![0u8; 16 * 2048];
        saturn_iso[..16].copy_from_slice(b"SEGA SEGASATURN ");
        let iso_path = dir.path().join("Panzer Dragoon.iso");
        std::fs::write(&iso_path, &saturn_iso).unwrap();

        // `system_id = "disc"` (sem `preferred_console`) resolve Saturn e grava cache em disco.
        assert!(can_attempt_ra("disc", None, &iso_path));
        assert!(!can_attempt_ra("chip8", None, Path::new("pong.ch8")));

        let resolved = compute_rom_console_and_hash(None, "disc", &iso_path, None).unwrap();
        assert_eq!(resolved.0, RC_CONSOLE_SATURN);
        assert_eq!(resolved.1.len(), 32);

        let cache_file = dir.path().join("ra_hashes.json");
        let state_a = RaState::default();
        let iso_str = iso_path.to_string_lossy().into_owned();
        state_a.store_console_and_hash(
            Some(&cache_file),
            "rom-1",
            &iso_str,
            resolved.0,
            resolved.1.clone(),
        );
        assert!(cache_file.is_file());

        // Nova instancia (simulando reinicio do app) recupera `(console_id, hash)` de `ra_hashes.json`.
        let state_b = RaState::default();
        assert_eq!(
            state_b.cached_console_and_hash(Some(&cache_file), "rom-1", &iso_str),
            Some(resolved.clone())
        );

        // `extracted_hint` evita descompactar de novo um `.7z` ja extraido pelo core-loader.
        let fake_7z = dir.path().join("Panzer Dragoon.7z");
        std::fs::write(&fake_7z, b"not-a-valid-7z-archive").unwrap();
        let from_hint = compute_rom_console_and_hash(
            Some(RC_CONSOLE_PLAYSTATION_2),
            "ps2",
            &fake_7z,
            Some(&iso_path),
        )
        .unwrap();
        assert_eq!(from_hint, resolved);
        assert_ne!(from_hint.0, RC_CONSOLE_PLAYSTATION);
    }

    #[test]
    fn badge_cache_sanitizes_urls_and_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            badge_cache_filename_from_url("https://media.retroachievements.org/Badge/12345.png"),
            Some((
                "12345.png".into(),
                "https://media.retroachievements.org/Badge/12345.png".into()
            ))
        );
        assert_eq!(
            badge_cache_filename_from_url(
                "https://media.retroachievements.org/Badge/12345_lock.png"
            ),
            Some((
                "12345_lock.png".into(),
                "https://media.retroachievements.org/Badge/12345_lock.png".into()
            ))
        );
        assert_eq!(
            badge_cache_filename_from_url("https://media.retroachievements.org/Images/009876.png"),
            Some((
                "game_009876.png".into(),
                "https://media.retroachievements.org/Images/009876.png".into()
            ))
        );

        // Rejeita HTTP inseguro, dominios externos e path traversal.
        assert_eq!(
            badge_cache_filename_from_url("http://media.retroachievements.org/Badge/12345.png"),
            None
        );
        assert_eq!(
            badge_cache_filename_from_url("https://evil.example.com/Badge/12345.png"),
            None
        );
        assert_eq!(
            badge_cache_filename_from_url(
                "https://media.retroachievements.org/Badge/../secret.png"
            ),
            None
        );
        assert_eq!(badge_cache_path(dir.path(), "../12345.png"), None);
        assert_eq!(badge_cache_path(dir.path(), "12345/evil.png"), None);
        assert_eq!(badge_cache_path(dir.path(), "12345.svg"), None);
        assert_eq!(
            badge_cache_path(dir.path(), "12345_lock.png"),
            Some(dir.path().join("12345_lock.png"))
        );
    }

    #[test]
    fn badge_cache_serves_local_rabadge_url_when_on_disk_and_queues_download_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let badges_dir = dir.path();
        let valid_png = b"\x89PNG\r\n\x1a\nfake-png-payload";

        // `12345.png` ja existe em disco; `12345_lock.png` ainda nao.
        std::fs::write(badges_dir.join("12345.png"), valid_png).unwrap();

        let mut pending = Vec::new();
        let hit = resolve_badge_url_for_ui(
            Some(badges_dir),
            Some("https://media.retroachievements.org/Badge/12345.png".into()),
            &mut pending,
        );
        assert_eq!(hit, Some(rabadge_url("12345.png")));
        assert!(pending.is_empty());

        let miss = resolve_badge_url_for_ui(
            Some(badges_dir),
            Some("https://media.retroachievements.org/Badge/12345_lock.png".into()),
            &mut pending,
        );
        assert_eq!(
            miss.as_deref(),
            Some("https://media.retroachievements.org/Badge/12345_lock.png")
        );
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].0, badges_dir.join("12345_lock.png"));

        // O resolvedor HTTP `rabadge://` serve 200 para PNG valido e 404 para traversal/ausente.
        let ok_resp = resolve_rabadge_response(Some(badges_dir), "/12345.png");
        assert_eq!(ok_resp.status(), 200);
        assert_eq!(ok_resp.body().as_slice(), valid_png);
        assert_eq!(
            resolve_rabadge_response(Some(badges_dir), "/../12345.png").status(),
            404
        );
        assert_eq!(
            resolve_rabadge_response(Some(badges_dir), "/99999.png").status(),
            404
        );
    }

    #[test]
    fn live_game_view_maps_leaderboards_trackers_challenges_progress_and_rich_presence() {
        let dir = tempfile::tempdir().unwrap();
        let badges_dir = dir.path();
        let valid_png = b"\x89PNG\r\n\x1a\nbadge";
        std::fs::write(badges_dir.join("game_001234.png"), valid_png).unwrap();
        std::fs::write(badges_dir.join("55555_lock.png"), valid_png).unwrap();

        let mut live = LiveGameAchievements {
            rom_id: Some("rom-snes-1".into()),
            rom_hash: Some("0123456789abcdef0123456789abcdef".into()),
            game_id: 42,
            title: "Super Metroid".into(),
            badge_url: Some("https://media.retroachievements.org/Images/001234.png".into()),
            hardcore: true,
            hardcore_downgraded: false,
            incompatible_version: false,
            summary: GameSummary {
                num_core_achievements: 2,
                num_unofficial_achievements: 0,
                num_unlocked_achievements: 0,
                num_unsupported_achievements: 0,
                points_core: 35,
                points_unlocked: 0,
            },
            achievements: vec![
                AchievementEntry {
                    id: 10,
                    title: "Energy Tank".into(),
                    description: "Collect 10 tanks".into(),
                    points: 10,
                    unlocked: false,
                    unlocked_hardcore: false,
                    unlock_time: None,
                    badge_name: "55555".into(),
                    badge_url: Some("https://media.retroachievements.org/Badge/55555.png".into()),
                    badge_locked_url: Some(
                        "https://media.retroachievements.org/Badge/55555_lock.png".into(),
                    ),
                    measured_progress: None,
                    measured_percent: 0.0,
                    rarity: 25.0,
                    rarity_hardcore: 12.0,
                    kind: AchievementKind::Progression,
                    unofficial: false,
                    unsupported: false,
                },
                AchievementEntry {
                    id: 11,
                    title: "Damageless Ridley".into(),
                    description: "Beat Ridley without taking damage".into(),
                    points: 25,
                    unlocked: false,
                    unlocked_hardcore: false,
                    unlock_time: None,
                    badge_name: "66666".into(),
                    badge_url: Some("https://media.retroachievements.org/Badge/66666.png".into()),
                    badge_locked_url: Some(
                        "https://media.retroachievements.org/Badge/66666_lock.png".into(),
                    ),
                    measured_progress: None,
                    measured_percent: 0.0,
                    rarity: 5.0,
                    rarity_hardcore: 2.0,
                    kind: AchievementKind::Missable,
                    unofficial: false,
                    unsupported: false,
                },
            ],
            leaderboards: vec![LeaderboardInfo {
                id: 900,
                title: "Ceres Escape".into(),
                description: "Escape Ceres as fast as possible".into(),
                tracker_value: None,
                state: LeaderboardState::Active,
                format: LeaderboardFormat::Time,
                lower_is_better: true,
            }],
            active_trackers: Vec::new(),
            active_challenges: Vec::new(),
            rich_presence: Some("Crateria | 99 Energy".into()),
        };

        live.update_progress(10, "4/10".into(), 40.0);
        live.show_challenge(RaChallengeDto {
            id: 11,
            title: "Damageless Ridley".into(),
            description: "Beat Ridley without taking damage".into(),
            points: 25,
            badge_url: Some("https://media.retroachievements.org/Badge/66666.png".into()),
        });
        live.set_leaderboard_started(900, Some("00:41.20".into()));
        assert!(live.upsert_tracker(1, "00:42.50".into()));

        let (view, pending) = live.to_view_dto(Some(badges_dir));
        assert!(view.playing_live);
        assert_eq!(view.rich_presence.as_deref(), Some("Crateria | 99 Energy"));
        assert_eq!(view.badge_url, Some(rabadge_url("game_001234.png")));
        assert_eq!(
            view.achievements[0].measured_progress.as_deref(),
            Some("4/10")
        );
        assert_eq!(
            view.achievements[0].badge_locked_url,
            Some(rabadge_url("55555_lock.png"))
        );
        assert_eq!(view.active_challenges.len(), 1);
        assert_eq!(view.active_trackers.len(), 1);
        assert_eq!(view.active_trackers[0].display, "00:42.50");
        assert_eq!(view.leaderboards.len(), 1);
        assert_eq!(view.leaderboards[0].state, "tracking");
        assert_eq!(
            view.leaderboards[0].tracker_value.as_deref(),
            Some("00:42.50")
        );
        assert!(view.leaderboards[0].lower_is_better);
        assert!(!pending.is_empty());

        // Desbloquear a conquista 11 remove o indicador de desafio ativo e soma pontos.
        live.mark_unlocked(11, true);
        assert!(live.active_challenges.is_empty());
        assert_eq!(live.summary.num_unlocked_achievements, 1);
        assert_eq!(live.summary.points_unlocked, 25);

        // Cache offline persiste a visao carregada sem estado efemero de sessao.
        store_cached_game_view(
            Some(badges_dir),
            live.rom_hash.as_deref(),
            &live.to_view_dto(None).0,
        );
        let (offline_view, _) =
            load_cached_game_view(Some(badges_dir), "0123456789abcdef0123456789abcdef")
                .expect("cached offline view");
        assert!(!offline_view.playing_live);
        assert!(offline_view.rich_presence.is_none());
        assert!(offline_view.active_trackers.is_empty());
        assert_eq!(offline_view.badge_url, Some(rabadge_url("game_001234.png")));
        assert_eq!(offline_view.leaderboards.len(), 1);
    }
}
