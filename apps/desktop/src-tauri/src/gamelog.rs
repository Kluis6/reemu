//! Integracao nativa do "Meu Log" (Alvanista / GameKave) e da thread automatica
//! de check-in da sessao (conquistas RetroAchievements + capturas de tela,
//! notas de audio e gravacoes de video).
//!
//! Funciona para qualquer jogo mapeado no catalogo GameKave/Alvanista
//! (`game_uuid`), com ou sem RetroAchievements, e tambem salva todas as
//! capturas localmente em `<dados>/screenshots` e `<dados>/recordings`.

use crate::alvanista::client::{partner_error_code, ClientError};
use crate::alvanista::Alvanista;
use crate::app_error::AppError;
use crate::commands::{AppState, VideoMenu};
use achievements::RaLoadedSnapshot;
use audio_desktop::mic::{start_mic_recording, MicRecorderHandle};
use domain::hotkeys::{HotkeyBinding, SystemAction, SystemHotkeyRepository};
use domain::library::RomRepository;
use domain::metadata::MetadataRepository;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const PREFS_FILE: &str = "gamelog_prefs.json";
/// Limite de nota de audio no GameKave e 15 min (900_000 ms). Paramos com folga.
const MAX_AUDIO_NOTE_DURATION: Duration = Duration::from_secs(895);
/// Intervalo entre amostras de quadro durante gravacao de video (~30 fps).
const VIDEO_FRAME_INTERVAL_MS: u64 = 33;
/// Resolucao maxima de gravacao de video em tempo real.
const MAX_VIDEO_WIDTH: u32 = 1280;
const MAX_VIDEO_HEIGHT: u32 = 720;

type VideoFramePacket = (u32, u32, Vec<u8>);

static VIDEO_TAP_ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST_VIDEO_SAMPLE_MS: AtomicU64 = AtomicU64::new(0);
static VIDEO_FRAME_TX: Mutex<Option<SyncSender<VideoFramePacket>>> = Mutex::new(None);

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Fast-path chamado no `reemu-video-pump` e no `poll_frame`.
/// Quando nao ha gravacao ativa, faz apenas uma leitura atomica `Relaxed`.
pub fn should_sample_video_frame() -> bool {
    if !VIDEO_TAP_ACTIVE.load(Ordering::Relaxed) {
        return false;
    }
    let now = now_epoch_ms();
    let prev = LAST_VIDEO_SAMPLE_MS.load(Ordering::Relaxed);
    if now.saturating_sub(prev) < VIDEO_FRAME_INTERVAL_MS {
        return false;
    }
    LAST_VIDEO_SAMPLE_MS
        .compare_exchange(prev, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
}

pub fn push_video_frame(w: u32, h: u32, rgba: Vec<u8>) {
    if !VIDEO_TAP_ACTIVE.load(Ordering::Relaxed) || w == 0 || h == 0 {
        return;
    }
    if let Ok(guard) = VIDEO_FRAME_TX.try_lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.try_send((w, h, rgba));
        }
    }
}

fn apply_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

fn probe_ffmpeg(candidate: &Path) -> bool {
    let mut cmd = Command::new(candidate);
    apply_no_window(&mut cmd);
    cmd.arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

pub fn discover_ffmpeg_bin(data_dir: &Path) -> Option<PathBuf> {
    if let Some(env_bin) = std::env::var_os("FFMPEG_BIN") {
        let p = PathBuf::from(env_bin);
        if probe_ffmpeg(&p) {
            return Some(p);
        }
    }

    let local_tool = if cfg!(windows) {
        data_dir.join("tools").join("ffmpeg.exe")
    } else {
        data_dir.join("tools").join("ffmpeg")
    };
    if local_tool.is_file() && probe_ffmpeg(&local_tool) {
        return Some(local_tool);
    }

    let path_cmd = PathBuf::from(if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    });
    if probe_ffmpeg(&path_cmd) {
        return Some(path_cmd);
    }

    #[cfg(windows)]
    {
        if let Some(local_app) = std::env::var_os("LOCALAPPDATA") {
            let base = PathBuf::from(local_app);
            let winget_link = base
                .join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join("ffmpeg.exe");
            if winget_link.is_file() && probe_ffmpeg(&winget_link) {
                return Some(winget_link);
            }
            let packages = base.join("Microsoft").join("WinGet").join("Packages");
            if packages.is_dir() {
                for entry in walkdir::WalkDir::new(packages)
                    .max_depth(4)
                    .into_iter()
                    .flatten()
                {
                    if entry.file_type().is_file()
                        && entry
                            .file_name()
                            .to_str()
                            .is_some_and(|n| n.eq_ignore_ascii_case("ffmpeg.exe"))
                        && probe_ffmpeg(entry.path())
                    {
                        return Some(entry.path().to_path_buf());
                    }
                }
            }
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            let scoop = PathBuf::from(profile)
                .join("scoop")
                .join("shims")
                .join("ffmpeg.exe");
            if scoop.is_file() && probe_ffmpeg(&scoop) {
                return Some(scoop);
            }
        }
        let choco = PathBuf::from(r"C:\ProgramData\chocolatey\bin\ffmpeg.exe");
        if choco.is_file() && probe_ffmpeg(&choco) {
            return Some(choco);
        }
    }

    None
}

fn normalize_visibility(raw: Option<&str>, fallback: &str) -> String {
    match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("public") => "public".to_owned(),
        Some("friends") => "friends".to_owned(),
        Some("private") => "private".to_owned(),
        _ => fallback.to_owned(),
    }
}

fn is_valid_uuid(raw: &str) -> bool {
    let s = raw.trim();
    if s.len() != 36 {
        return false;
    }
    for (idx, b) in s.bytes().enumerate() {
        let hyphen = matches!(idx, 8 | 13 | 18 | 23);
        if hyphen {
            if b != b'-' {
                return false;
            }
        } else if !b.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamelogPrefsFile {
    #[serde(default = "default_true")]
    pub auto_session_checkin: bool,
    #[serde(default = "default_true")]
    pub auto_attach_captures: bool,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub hotkeys_seeded: bool,
}

fn default_true() -> bool {
    true
}

fn default_visibility() -> String {
    "friends".to_owned()
}

impl Default for GamelogPrefsFile {
    fn default() -> Self {
        Self {
            auto_session_checkin: true,
            auto_attach_captures: true,
            visibility: default_visibility(),
            hotkeys_seeded: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamelogPrefsDto {
    pub auto_session_checkin: bool,
    pub auto_attach_captures: bool,
    pub visibility: String,
}

impl From<&GamelogPrefsFile> for GamelogPrefsDto {
    fn from(p: &GamelogPrefsFile) -> Self {
        Self {
            auto_session_checkin: p.auto_session_checkin,
            auto_attach_captures: p.auto_attach_captures,
            visibility: p.visibility.clone(),
        }
    }
}

fn read_prefs_file(data_dir: &Path) -> GamelogPrefsFile {
    let path = data_dir.join(PREFS_FILE);
    std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<GamelogPrefsFile>(&b).ok())
        .unwrap_or_default()
}

fn write_prefs_file(data_dir: &Path, prefs: &GamelogPrefsFile) -> Result<(), String> {
    let path = data_dir.join(PREFS_FILE);
    let bytes = serde_json::to_vec_pretty(prefs).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// Semeia os 3 atalhos padrao do Meu Log (`F8`, `F9`, `F10`) uma unica vez na
/// instalacao, sem sobrescrever atalhos que a pessoa ja tenha gravado ou apagado.
pub async fn seed_default_hotkeys_once(data_dir: &Path, pool: &db::Db) {
    let mut prefs = read_prefs_file(data_dir);
    if prefs.hotkeys_seeded {
        return;
    }
    let repo = db::SystemHotkeysRepo::new(pool.clone());
    let Ok(existing) = repo.list().await else {
        return;
    };

    // Garante F1 pro menu caso o banco esteja completamente vazio, para que
    // `load_system_hotkeys` nao deixe de adicionar F1 por encontrarmos a tabela populada.
    if existing.is_empty() {
        let _ = repo
            .set(&HotkeyBinding {
                action: SystemAction::ToggleMenuOverlay,
                trigger: vec![domain::input::RawInputEvent::Keyboard {
                    scancode: input_desktop::keymap::key_scancode("F1"),
                }],
                device_guid: None,
            })
            .await;
    }

    let defaults = [
        (SystemAction::CaptureScreenshot, "F8"),
        (SystemAction::ToggleAudioNote, "F9"),
        (SystemAction::ToggleVideoRecording, "F10"),
    ];

    for (action, key_code) in defaults {
        if existing.iter().any(|b| b.action == action) {
            continue;
        }
        let sc = input_desktop::keymap::key_scancode(key_code);
        let trigger = vec![domain::input::RawInputEvent::Keyboard { scancode: sc }];
        if existing.iter().any(|b| b.trigger == trigger) {
            continue;
        }
        let _ = repo
            .set(&HotkeyBinding {
                action,
                trigger,
                device_guid: None,
            })
            .await;
    }

    prefs.hotkeys_seeded = true;
    let _ = write_prefs_file(data_dir, &prefs);
}

#[derive(Debug, Clone, Default)]
struct ActivePlaySession {
    rom_id: Option<String>,
    game_id: Option<String>,
    game_title: Option<String>,
    system_id: Option<String>,
    ra_game_id: u32,
    hardcore: bool,
    session_id: Option<i64>,
    checkin_id: Option<String>,
    post_id: Option<String>,
    started_at: Option<Instant>,
}

struct ActiveMicNote {
    handle: MicRecorderHandle,
    rom_id: Option<String>,
    game_id: Option<String>,
    system_id: Option<String>,
    title: Option<String>,
    caption: Option<String>,
}

struct ActiveVideoCapture {
    started_at: Instant,
    rom_id: Option<String>,
    game_id: Option<String>,
    system_id: Option<String>,
    caption: Option<String>,
    ffmpeg_bin: PathBuf,
    temp_video_mp4: PathBuf,
    final_mp4: PathBuf,
    encoder_thread: Option<JoinHandle<Result<u64, String>>>,
}

pub struct GamelogState {
    data_dir: PathBuf,
    ffmpeg_bin: Mutex<Option<PathBuf>>,
    prefs: Mutex<GamelogPrefsFile>,
    session: Mutex<ActivePlaySession>,
    mic: Mutex<Option<ActiveMicNote>>,
    video: Mutex<Option<ActiveVideoCapture>>,
    sync_lock: tokio::sync::Mutex<()>,
}

impl GamelogState {
    pub fn new(data_dir: PathBuf) -> Self {
        let prefs = read_prefs_file(&data_dir);
        let ffmpeg = discover_ffmpeg_bin(&data_dir);
        if let Some(p) = &ffmpeg {
            log::info!("meu-log: ffmpeg detectado em {}", p.display());
        } else {
            log::info!("meu-log: ffmpeg nao encontrado no PATH (gravacao de video indisponivel ate instalar)");
        }
        Self {
            data_dir,
            ffmpeg_bin: Mutex::new(ffmpeg),
            prefs: Mutex::new(prefs),
            session: Mutex::new(ActivePlaySession::default()),
            mic: Mutex::new(None),
            video: Mutex::new(None),
            sync_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn ensure_ffmpeg(&self) -> Option<PathBuf> {
        let mut g = self.ffmpeg_bin.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(p) = g.as_ref() {
            return Some(p.clone());
        }
        let found = discover_ffmpeg_bin(&self.data_dir);
        *g = found.clone();
        found
    }

    pub fn prefs_snapshot(&self) -> GamelogPrefsFile {
        self.prefs.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn runtime_dto(&self) -> GamelogRuntimeStateDto {
        let prefs = self.prefs_snapshot();
        let (audio_recording, audio_elapsed_ms) = {
            let g = self.mic.lock().unwrap_or_else(|p| p.into_inner());
            match g.as_ref() {
                Some(m) => (true, m.handle.elapsed_ms()),
                None => (false, 0),
            }
        };
        let (video_recording, video_elapsed_ms) = {
            let g = self.video.lock().unwrap_or_else(|p| p.into_inner());
            match g.as_ref() {
                Some(v) => (true, v.started_at.elapsed().as_millis() as u64),
                None => (false, 0),
            }
        };
        let sess = self
            .session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let ffmpeg_available = self
            .ffmpeg_bin
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some();

        GamelogRuntimeStateDto {
            audio_recording,
            audio_elapsed_ms,
            video_recording,
            video_elapsed_ms,
            ffmpeg_available,
            active_rom_id: sess.rom_id,
            active_game_id: sess.game_id,
            active_session_post_id: sess.post_id,
            active_ra_game_id: sess.ra_game_id,
            prefs: GamelogPrefsDto::from(&prefs),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamelogRuntimeStateDto {
    pub audio_recording: bool,
    pub audio_elapsed_ms: u64,
    pub video_recording: bool,
    pub video_elapsed_ms: u64,
    pub ffmpeg_available: bool,
    pub active_rom_id: Option<String>,
    pub active_game_id: Option<String>,
    pub active_session_post_id: Option<String>,
    pub active_ra_game_id: u32,
    pub prefs: GamelogPrefsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamelogUsageDto {
    pub used_bytes: i64,
    pub quota_bytes: i64,
    pub item_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamelogMediaDto {
    pub key: String,
    pub url: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_ms: Option<i32>,
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamelogItemDto {
    pub id: String,
    pub game_id: String,
    pub kind: String,
    pub title: Option<String>,
    pub body: String,
    pub platform_id: Option<String>,
    pub visibility: String,
    pub state: String,
    pub taken_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub media: Vec<GamelogMediaDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamelogListDto {
    pub items: Vec<GamelogItemDto>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamelogCaptureResultDto {
    pub local_path: String,
    pub uploaded_item: Option<GamelogItemDto>,
    pub attached_post_id: Option<String>,
    pub warning: Option<String>,
}

// --- Wire structs do backend Alvanista / GameKave --------------------------

#[derive(Deserialize)]
struct UsageWire {
    #[serde(default)]
    used_bytes: i64,
    #[serde(default)]
    quota_bytes: i64,
    #[serde(default)]
    item_count: i64,
}

#[derive(Deserialize)]
struct MediaWire {
    #[serde(default)]
    key: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    mime_type: String,
    #[serde(default)]
    size_bytes: i64,
    #[serde(default)]
    width: Option<i32>,
    #[serde(default)]
    height: Option<i32>,
    #[serde(default)]
    duration_ms: Option<i32>,
    #[serde(default)]
    poster_url: Option<String>,
}

#[derive(Deserialize)]
struct LogItemWire {
    id: String,
    game_id: String,
    kind: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    platform_id: Option<String>,
    #[serde(default)]
    visibility: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    taken_at: Option<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    media: Vec<MediaWire>,
}

impl From<LogItemWire> for GamelogItemDto {
    fn from(w: LogItemWire) -> Self {
        Self {
            id: w.id,
            game_id: w.game_id,
            kind: w.kind,
            title: w.title,
            body: w.body,
            platform_id: w.platform_id,
            visibility: if w.visibility.is_empty() {
                "private".to_owned()
            } else {
                w.visibility
            },
            state: if w.state.is_empty() {
                "ready".to_owned()
            } else {
                w.state
            },
            taken_at: w.taken_at,
            created_at: w.created_at,
            updated_at: w.updated_at,
            media: w
                .media
                .into_iter()
                .map(|m| GamelogMediaDto {
                    key: m.key,
                    url: m.url,
                    mime_type: m.mime_type,
                    size_bytes: m.size_bytes,
                    width: m.width,
                    height: m.height,
                    duration_ms: m.duration_ms,
                    poster_url: m.poster_url,
                })
                .collect(),
        }
    }
}

#[derive(Deserialize)]
struct ListWire {
    #[serde(default)]
    items: Vec<LogItemWire>,
    #[serde(default)]
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct ReserveUploadWire {
    item_id: String,
    upload_url: String,
    #[serde(default)]
    upload_headers: HashMap<String, String>,
}

#[derive(Deserialize)]
struct PlaySessionSyncRespWire {
    #[serde(default)]
    session_id: Option<i64>,
    #[serde(default)]
    checkin_id: Option<String>,
    #[serde(default)]
    post_id: Option<String>,
}

fn map_client_error(err: ClientError) -> String {
    match err {
        ClientError::AuthEnded => "no_account".to_owned(),
        ClientError::Transient(_) => "offline".to_owned(),
        ClientError::Rejected(code) => {
            if code == "insufficient_scope" {
                "needs_consent".to_owned()
            } else {
                code
            }
        }
    }
}

fn map_non_success(status: u16, body: &serde_json::Value) -> String {
    match partner_error_code(body) {
        Some("insufficient_scope") => "needs_consent".to_owned(),
        Some("quota_exceeded") => "quota_exceeded".to_owned(),
        Some("file_too_large") => "file_too_large".to_owned(),
        Some("unsupported_media" | "unsupported_codec" | "invalid_media") => {
            "unsupported_media".to_owned()
        }
        Some(code) if !code.is_empty() => code.to_owned(),
        _ if status == 403 => "needs_consent".to_owned(),
        _ if status == 507 => "quota_exceeded".to_owned(),
        _ if status == 413 => "file_too_large".to_owned(),
        _ => "failed".to_owned(),
    }
}

async fn has_gamelog_scope(alva: &Alvanista) -> bool {
    if !alva.has_session().await {
        return false;
    }
    let st = alva.cached_status().await;
    st.connected && !st.ended && st.scopes.iter().any(|s| s == "gamelog")
}

async fn resolve_rom_mapping(
    pool: &db::Db,
    rom_id: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let rom = db::RomsRepo::new(pool.clone())
        .get(rom_id)
        .await
        .ok()
        .flatten();
    let system_id = rom.as_ref().map(|r| r.system_id.clone());
    let fallback_title = rom.as_ref().and_then(|r| {
        r.user_title.clone().or_else(|| {
            Path::new(&r.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_owned)
        })
    });

    let meta = db::MetadataRepo::new(pool.clone())
        .get_metadata(rom_id)
        .await
        .ok()
        .flatten();
    let mut game_uuid = meta
        .as_ref()
        .and_then(|m| m.game_uuid.clone())
        .filter(|u| is_valid_uuid(u));
    let title = meta
        .as_ref()
        .map(|m| m.title.clone())
        .filter(|t| !t.trim().is_empty())
        .or(fallback_title);

    if game_uuid.is_none() {
        if let Some(res) = db::RomResolutionRepo::new(pool.clone())
            .get(rom_id)
            .await
            .ok()
            .flatten()
        {
            game_uuid = res.game_uuid.filter(|u| is_valid_uuid(u));
        }
    }

    (game_uuid.map(|u| u.to_ascii_lowercase()), title, system_id)
}

fn emit_state<R: tauri::Runtime>(app: &AppHandle<R>) {
    if let Some(gl) = app.try_state::<Arc<GamelogState>>() {
        let _ = app.emit("gamelog-state-changed", gl.runtime_dto());
    }
}

fn emit_items_updated<R: tauri::Runtime>(app: &AppHandle<R>, game_id: Option<&str>) {
    let _ = app.emit("gamelog-updated", serde_json::json!({ "gameId": game_id }));
}

// --- Sincronia da sessao unificada (Check-in + RetroAchievements + Midia) --

#[derive(Default)]
struct SyncUnlockPayload {
    id: u32,
    title: String,
    description: String,
    points: u32,
    badge_name: String,
    hardcore: bool,
    unlocked_at: String,
}

async fn sync_active_play_session<R: tauri::Runtime>(
    app: &AppHandle<R>,
    unlocks: Vec<SyncUnlockPayload>,
) -> Result<Option<String>, String> {
    let Some(gl) = app.try_state::<Arc<GamelogState>>() else {
        return Ok(None);
    };
    let Some(alva) = app.try_state::<Alvanista>() else {
        return Ok(None);
    };
    let _guard = gl.sync_lock.lock().await;

    let prefs = gl.prefs_snapshot();
    if !prefs.auto_session_checkin && unlocks.is_empty() {
        return Ok(None);
    }
    if !has_gamelog_scope(&alva).await {
        return Ok(None);
    }

    let snap = gl.session.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if snap.game_id.is_none() && snap.ra_game_id == 0 {
        return Ok(None);
    }

    let playtime_min = snap
        .started_at
        .map(|t| (t.elapsed().as_secs() / 60) as i64)
        .unwrap_or(0);

    let mut payload = serde_json::Map::new();
    if let Some(gid) = &snap.game_id {
        payload.insert("game_id".into(), serde_json::Value::String(gid.clone()));
    }
    if snap.ra_game_id > 0 {
        payload.insert("ra_game_id".into(), serde_json::json!(snap.ra_game_id));
    }
    if let Some(title) = &snap.game_title {
        payload.insert(
            "game_title".into(),
            serde_json::Value::String(title.clone()),
        );
    }
    if let Some(sys) = &snap.system_id {
        payload.insert(
            "platform_slug".into(),
            serde_json::Value::String(sys.clone()),
        );
    }
    payload.insert("hardcore".into(), serde_json::Value::Bool(snap.hardcore));
    payload.insert("playtime_min".into(), serde_json::json!(playtime_min));
    payload.insert(
        "visibility".into(),
        serde_json::Value::String(prefs.visibility),
    );
    if !unlocks.is_empty() {
        let arr: Vec<serde_json::Value> = unlocks
            .into_iter()
            .map(|u| {
                serde_json::json!({
                    "id": u.id,
                    "title": u.title,
                    "description": u.description,
                    "points": u.points,
                    "badge_name": u.badge_name,
                    "hardcore": u.hardcore,
                    "unlocked_at": u.unlocked_at,
                })
            })
            .collect();
        payload.insert(
            "unlocked_achievements".into(),
            serde_json::Value::Array(arr),
        );
    }

    let (status, val) = alva
        .partner_json(
            reqwest::Method::POST,
            "/api/partner/v1/play-sessions/sync",
            Some(&serde_json::Value::Object(payload)),
        )
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let parsed: PlaySessionSyncRespWire =
        serde_json::from_value(val).map_err(|_| "failed".to_owned())?;
    {
        let mut s = gl.session.lock().unwrap_or_else(|p| p.into_inner());
        if parsed.session_id.is_some() {
            s.session_id = parsed.session_id;
        }
        if parsed.checkin_id.as_deref().is_some_and(|c| !c.is_empty()) {
            s.checkin_id = parsed.checkin_id;
        }
        if parsed.post_id.as_deref().is_some_and(|p| !p.is_empty()) {
            s.post_id = parsed.post_id.clone();
        }
    }
    emit_state(app);
    Ok(parsed.post_id)
}

async fn attach_item_to_active_session<R: tauri::Runtime>(
    app: &AppHandle<R>,
    item_id: &str,
    explicit_game_id: Option<&str>,
    caption: Option<&str>,
    visibility: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(gl) = app.try_state::<Arc<GamelogState>>() else {
        return Err("failed".into());
    };
    let Some(alva) = app.try_state::<Alvanista>() else {
        return Err("no_account".into());
    };
    if !has_gamelog_scope(&alva).await {
        return Err("needs_consent".into());
    }

    let _ = sync_active_play_session(app, Vec::new()).await;

    let snap = gl.session.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let prefs = gl.prefs_snapshot();

    let game_id = explicit_game_id
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| is_valid_uuid(s))
        .or(snap.game_id.clone());

    let mut payload = serde_json::Map::new();
    payload.insert(
        "item_id".into(),
        serde_json::Value::String(item_id.trim().to_owned()),
    );
    if let Some(gid) = &game_id {
        payload.insert("game_id".into(), serde_json::Value::String(gid.clone()));
    }
    if snap.ra_game_id > 0 {
        payload.insert("ra_game_id".into(), serde_json::json!(snap.ra_game_id));
    }
    if let Some(title) = &snap.game_title {
        payload.insert(
            "game_title".into(),
            serde_json::Value::String(title.clone()),
        );
    }
    if let Some(sys) = &snap.system_id {
        payload.insert(
            "platform_slug".into(),
            serde_json::Value::String(sys.clone()),
        );
    }
    if let Some(cap) = caption.map(str::trim).filter(|s| !s.is_empty()) {
        payload.insert("caption".into(), serde_json::Value::String(cap.to_owned()));
    }
    payload.insert(
        "visibility".into(),
        serde_json::Value::String(normalize_visibility(visibility, &prefs.visibility)),
    );

    let (status, val) = alva
        .partner_json(
            reqwest::Method::POST,
            "/api/partner/v1/play-sessions/attach-media",
            Some(&serde_json::Value::Object(payload)),
        )
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let post_id = val
        .get("post_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    if let Some(pid) = &post_id {
        gl.session.lock().unwrap_or_else(|p| p.into_inner()).post_id = Some(pid.clone());
        emit_state(app);
    }
    Ok(post_id)
}

/// Chamado em `load_game` quando qualquer ROM abre.
pub async fn on_game_launched(app: &AppHandle, rom_id: Option<&str>) {
    let state = app.state::<AppState>();
    let (game_id, game_title, system_id) = match (state.db.as_ref(), rom_id) {
        (Some(pool), Some(rid)) => resolve_rom_mapping(pool, rid).await,
        _ => (None, None, None),
    };
    let ra_hardcore = app
        .try_state::<crate::ra::RaState>()
        .is_some_and(|r| r.is_hardcore_active());
    crate::gas::on_game_launched(app, rom_id, game_id.clone(), system_id.clone(), ra_hardcore);

    let Some(gl) = app.try_state::<Arc<GamelogState>>() else {
        return;
    };

    // Preserva eventual `ra_game_id` caso o `GameLoaded` do rcheevos ja tenha chegado.
    {
        let mut s = gl.session.lock().unwrap_or_else(|p| p.into_inner());
        let same_rom = s.rom_id.as_deref() == rom_id && rom_id.is_some();
        let kept_ra = if same_rom { s.ra_game_id } else { 0 };
        let kept_hc = if same_rom { s.hardcore } else { false };
        *s = ActivePlaySession {
            rom_id: rom_id.map(str::to_owned),
            game_id,
            game_title,
            system_id,
            ra_game_id: kept_ra,
            hardcore: kept_hc,
            session_id: None,
            checkin_id: None,
            post_id: None,
            started_at: Some(Instant::now()),
        };
    }
    emit_state(app);

    let app_bg = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sync_active_play_session(&app_bg, Vec::new()).await {
            log::debug!("meu-log: sync inicial da sessao ignorado: {e}");
        }
    });
}

/// Chamado em `ra.rs` quando o RetroAchievements identifica o jogo.
pub fn on_ra_game_loaded(app: &AppHandle, rom_id: Option<String>, snapshot: &RaLoadedSnapshot) {
    let Some(gl) = app.try_state::<Arc<GamelogState>>() else {
        return;
    };
    {
        let mut s = gl.session.lock().unwrap_or_else(|p| p.into_inner());
        if s.rom_id.is_none() {
            s.rom_id = rom_id;
        }
        s.ra_game_id = snapshot.game_id;
        s.hardcore = snapshot.hardcore;
        if s.game_title.is_none() && !snapshot.title.trim().is_empty() {
            s.game_title = Some(snapshot.title.clone());
        }
        if s.started_at.is_none() {
            s.started_at = Some(Instant::now());
        }
    }
    emit_state(app);

    let app_bg = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sync_active_play_session(&app_bg, Vec::new()).await {
            log::debug!("meu-log: upgrade RA da sessao ignorado: {e}");
        }
    });
}

/// Chamado em `ra.rs` a cada conquista desbloqueada na partida.
pub fn on_ra_achievement_unlocked(
    app: &AppHandle,
    id: u32,
    title: &str,
    description: &str,
    points: u32,
    hardcore: bool,
    badge_url: Option<&str>,
) {
    let badge_name = badge_url
        .and_then(|u| u.rsplit('/').next())
        .map(|seg| {
            seg.split('?')
                .next()
                .unwrap_or(seg)
                .trim_end_matches(".png")
                .trim_end_matches(".jpg")
                .trim_end_matches("_lock")
                .to_owned()
        })
        .unwrap_or_default();

    let unlock = SyncUnlockPayload {
        id,
        title: title.to_owned(),
        description: description.to_owned(),
        points,
        badge_name,
        hardcore,
        unlocked_at: now_rfc3339(),
    };
    let app_bg = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = sync_active_play_session(&app_bg, vec![unlock]).await {
            log::debug!("meu-log: sync de conquista na sessao ignorado: {e}");
        }
    });
}

/// Chamado em `unload_game` e no fechamento do app.
/// Finaliza gravacoes pendentes e encerra o check-in da sessao no Alvanista.
pub async fn on_game_unloaded(app: &AppHandle) {
    crate::gas::on_game_unloaded(app);
    let Some(gl) = app.try_state::<Arc<GamelogState>>() else {
        return;
    };

    let has_mic = gl.mic.lock().unwrap_or_else(|p| p.into_inner()).is_some();
    if has_mic {
        let _ = stop_audio_note_inner(app, None, None, None).await;
    }
    let has_video = gl.video.lock().unwrap_or_else(|p| p.into_inner()).is_some();
    if has_video {
        let _ = stop_video_recording_inner(app, None, None).await;
    }

    let prev = {
        let mut s = gl.session.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::take(&mut *s)
    };
    emit_state(app);

    if prev.game_id.is_none() && prev.ra_game_id == 0 {
        return;
    }
    let Some(alva) = app.try_state::<Alvanista>() else {
        return;
    };
    if !has_gamelog_scope(&alva).await {
        return;
    }

    let playtime_min = prev
        .started_at
        .map(|t| ((t.elapsed().as_secs() + 30) / 60) as i64)
        .unwrap_or(0);
    let mut payload = serde_json::Map::new();
    if let Some(gid) = prev.game_id {
        payload.insert("game_id".into(), serde_json::Value::String(gid));
    }
    if prev.ra_game_id > 0 {
        payload.insert("ra_game_id".into(), serde_json::json!(prev.ra_game_id));
    }
    payload.insert("playtime_min".into(), serde_json::json!(playtime_min));

    let alva_owned = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(a) = alva_owned.try_state::<Alvanista>() {
            let _ = a
                .partner_json(
                    reqwest::Method::POST,
                    "/api/partner/v1/play-sessions/end",
                    Some(&serde_json::Value::Object(payload)),
                )
                .await;
        }
    });
}

// --- Captura de tela em resolucao cheia ------------------------------------

fn capture_full_rgba(state: &AppState) -> Option<(u32, u32, Vec<u8>)> {
    if let Some(c) = state
        .last_frame
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
    {
        if c.w > 0 && c.h > 0 && c.rgba.len() == (c.w * c.h * 4) as usize {
            return Some((c.w, c.h, c.rgba.clone()));
        }
    }

    let menu_up = matches!(
        *state.video_menu.lock().unwrap_or_else(|p| p.into_inner()),
        VideoMenu::MenuUp
    );
    if menu_up {
        if let Some((w, h, rgba)) = state
            .pause_bg
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
        {
            if w > 0 && h > 0 && rgba.len() == (w * h * 4) as usize {
                return Some((w, h, rgba));
            }
        }
    }

    if let Some((w, h, rgba)) = state
        .gpu
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_mut()
        .and_then(|fp| fp.capture_surface_frame())
    {
        if w > 0 && h > 0 && rgba.len() == (w * h * 4) as usize {
            return Some((w, h, rgba));
        }
    }

    state
        .pause_bg
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .filter(|(w, h, rgba)| *w > 0 && *h > 0 && rgba.len() == (*w * *h * 4) as usize)
}

fn encode_png_rgba(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
        return Err("quadro invalido para gerar PNG".into());
    }
    let mut out = Vec::with_capacity((w * h) as usize);
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc
            .write_header()
            .map_err(|e| format!("cabecalho PNG: {e}"))?;
        writer
            .write_image_data(rgba)
            .map_err(|e| format!("dados PNG: {e}"))?;
    }
    Ok(out)
}

fn resize_rgba_nearest(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32, dst: &mut Vec<u8>) {
    let needed = (dw * dh * 4) as usize;
    dst.resize(needed, 0);
    if sw == dw && sh == dh && src.len() == needed {
        dst.copy_from_slice(src);
        return;
    }
    for y in 0..dh {
        let sy = ((u64::from(y) * u64::from(sh)) / u64::from(dh.max(1))) as u32;
        let sy = sy.min(sh.saturating_sub(1));
        for x in 0..dw {
            let sx = ((u64::from(x) * u64::from(sw)) / u64::from(dw.max(1))) as u32;
            let sx = sx.min(sw.saturating_sub(1));
            let si = ((sy * sw + sx) * 4) as usize;
            let di = ((y * dw + x) * 4) as usize;
            if si + 4 <= src.len() && di + 4 <= dst.len() {
                dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
            }
        }
    }
}

fn compute_recording_dimensions(w: u32, h: u32) -> (u32, u32) {
    let scale_w = MAX_VIDEO_WIDTH as f32 / w.max(1) as f32;
    let scale_h = MAX_VIDEO_HEIGHT as f32 / h.max(1) as f32;
    let scale = scale_w.min(scale_h).min(1.0);
    let rw = ((w as f32 * scale) as u32).max(2) & !1;
    let rh = ((h as f32 * scale) as u32).max(2) & !1;
    (rw.max(2), rh.max(2))
}

// --- Upload pro Meu Log (screenshot / audio multipart + video 3 passos) ----

struct UploadMeta<'a> {
    game_id: &'a str,
    kind: &'a str,
    title: Option<&'a str>,
    body: Option<&'a str>,
    platform_id: Option<&'a str>,
    visibility: &'a str,
    taken_at: Option<&'a str>,
}

async fn upload_multipart_item(
    alva: &Alvanista,
    meta: &UploadMeta<'_>,
    filename: &str,
    mime_type: &str,
    bytes: Vec<u8>,
) -> Result<GamelogItemDto, String> {
    let clean_gid = meta.game_id.trim().to_ascii_lowercase();
    if !is_valid_uuid(&clean_gid) {
        return Err("game_not_mapped".into());
    }
    let path = format!("/api/partner/v1/gamelog/games/{clean_gid}/items");
    let kind = meta.kind.to_owned();
    let title = meta
        .title
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let body = meta
        .body
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let platform_id = meta
        .platform_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let visibility = meta.visibility.to_owned();
    let taken_at = meta
        .taken_at
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let fname = filename.to_owned();
    let mime = mime_type.to_owned();

    let (status, val) = alva
        .partner_multipart(reqwest::Method::POST, &path, move || {
            let part = reqwest::multipart::Part::bytes(bytes.clone())
                .file_name(fname.clone())
                .mime_str(&mime)
                .map_err(|e| e.to_string())?;
            let mut form = reqwest::multipart::Form::new()
                .text("kind", kind.clone())
                .text("visibility", visibility.clone());
            if let Some(t) = &title {
                form = form.text("title", t.clone());
            }
            if let Some(b) = &body {
                form = form.text("body", b.clone());
            }
            if let Some(p) = &platform_id {
                form = form.text("platform_id", p.clone());
            }
            if let Some(ta) = &taken_at {
                form = form.text("taken_at", ta.clone());
            }
            Ok(form.part("file", part))
        })
        .await
        .map_err(map_client_error)?;

    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let parsed: LogItemWire = serde_json::from_value(val).map_err(|_| "failed".to_owned())?;
    Ok(parsed.into())
}

async fn upload_video_three_step(
    alva: &Alvanista,
    meta: &UploadMeta<'_>,
    mime_type: &str,
    bytes: Vec<u8>,
) -> Result<GamelogItemDto, String> {
    let clean_gid = meta.game_id.trim().to_ascii_lowercase();
    if !is_valid_uuid(&clean_gid) {
        return Err("game_not_mapped".into());
    }
    if bytes.is_empty() {
        return Err("invalid_media".into());
    }

    let mut reserve = serde_json::Map::new();
    reserve.insert("kind".into(), serde_json::Value::String("video".into()));
    reserve.insert("size_bytes".into(), serde_json::json!(bytes.len() as i64));
    reserve.insert(
        "content_type".into(),
        serde_json::Value::String(mime_type.to_owned()),
    );
    reserve.insert(
        "visibility".into(),
        serde_json::Value::String(meta.visibility.to_owned()),
    );
    if let Some(b) = meta.body.map(str::trim).filter(|s| !s.is_empty()) {
        reserve.insert("body".into(), serde_json::Value::String(b.to_owned()));
    }
    if let Some(p) = meta.platform_id.map(str::trim).filter(|s| !s.is_empty()) {
        reserve.insert(
            "platform_id".into(),
            serde_json::Value::String(p.to_owned()),
        );
    }
    if let Some(ta) = meta.taken_at.map(str::trim).filter(|s| !s.is_empty()) {
        reserve.insert("taken_at".into(), serde_json::Value::String(ta.to_owned()));
    }

    let reserve_path = format!("/api/partner/v1/gamelog/games/{clean_gid}/uploads");
    let (status, val) = alva
        .partner_json(
            reqwest::Method::POST,
            &reserve_path,
            Some(&serde_json::Value::Object(reserve)),
        )
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let reserved: ReserveUploadWire =
        serde_json::from_value(val).map_err(|_| "failed".to_owned())?;

    alva.put_presigned_bytes(&reserved.upload_url, &reserved.upload_headers, bytes)
        .await?;

    let complete_path = format!(
        "/api/partner/v1/gamelog/items/{}/complete",
        reserved.item_id
    );
    for _ in 0..24 {
        let (c_status, c_val) = alva
            .partner_json(reqwest::Method::POST, &complete_path, None)
            .await
            .map_err(map_client_error)?;
        if c_status == 202 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        if !(200..300).contains(&c_status) {
            return Err(map_non_success(c_status, &c_val));
        }
        let item: LogItemWire = serde_json::from_value(c_val).map_err(|_| "failed".to_owned())?;
        return Ok(item.into());
    }

    Err("processing_timeout".into())
}

#[allow(clippy::too_many_arguments)]
async fn upload_and_maybe_attach<R: tauri::Runtime>(
    app: &AppHandle<R>,
    game_id: Option<&str>,
    system_id: Option<&str>,
    kind: &str,
    title: Option<&str>,
    caption: Option<&str>,
    visibility: Option<&str>,
    attach_override: Option<bool>,
    filename: &str,
    mime_type: &str,
    bytes: Vec<u8>,
) -> (Option<GamelogItemDto>, Option<String>, Option<String>) {
    let Some(gl) = app.try_state::<Arc<GamelogState>>() else {
        return (None, None, None);
    };
    let Some(alva) = app.try_state::<Alvanista>() else {
        return (None, None, Some("no_account".into()));
    };
    if !alva.has_session().await {
        return (None, None, Some("no_account".into()));
    }
    if !has_gamelog_scope(&alva).await {
        return (None, None, Some("needs_consent".into()));
    }
    let Some(gid) = game_id.filter(|g| is_valid_uuid(g)) else {
        return (None, None, Some("game_not_mapped".into()));
    };

    let prefs = gl.prefs_snapshot();
    let vis = normalize_visibility(visibility, &prefs.visibility);
    // A API de anexos gera uma cópia da mídia para o post da sessão.
    let should_attach = vis != "private" && attach_override.unwrap_or(prefs.auto_attach_captures);
    let taken_at = now_rfc3339();
    let meta = UploadMeta {
        game_id: gid,
        kind,
        title,
        body: caption,
        platform_id: system_id,
        visibility: &vis,
        taken_at: Some(&taken_at),
    };

    let uploaded = if kind == "video" {
        upload_video_three_step(&alva, &meta, mime_type, bytes).await
    } else {
        upload_multipart_item(&alva, &meta, filename, mime_type, bytes).await
    };

    match uploaded {
        Ok(item) => {
            emit_items_updated(app, Some(gid));
            if should_attach {
                match attach_item_to_active_session(app, &item.id, Some(gid), caption, Some(&vis))
                    .await
                {
                    Ok(post_id) => (Some(item), post_id, None),
                    Err(err) => (Some(item), None, Some(err)),
                }
            } else {
                (Some(item), None, None)
            }
        }
        Err(err) => (None, None, Some(err)),
    }
}

// --- Implementacao interna das acoes de captura ----------------------------

async fn capture_screenshot_inner<R: tauri::Runtime>(
    app: &AppHandle<R>,
    caption: Option<String>,
    attach_to_session: Option<bool>,
) -> Result<GamelogCaptureResultDto, String> {
    let state = app.state::<AppState>();
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| "failed".to_owned())?;

    let (w, h, rgba) = capture_full_rgba(&state)
        .ok_or_else(|| "nenhum quadro de jogo disponivel para captura".to_owned())?;
    let png_bytes = encode_png_rgba(w, h, &rgba)?;

    let sess = gl.session.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let folder = sess
        .rom_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("avulsas");
    let shots_dir = gl.data_dir.join("screenshots").join(folder);
    std::fs::create_dir_all(&shots_dir)
        .map_err(|e| format!("nao foi possivel criar pasta de capturas: {e}"))?;
    let ts = now_epoch_ms();
    let filename = format!("shot-{ts}.png");
    let local_file = shots_dir.join(&filename);
    std::fs::write(&local_file, &png_bytes)
        .map_err(|e| format!("nao foi possivel salvar captura: {e}"))?;

    let (uploaded_item, attached_post_id, warning) = upload_and_maybe_attach(
        app,
        sess.game_id.as_deref(),
        sess.system_id.as_deref(),
        "screenshot",
        None,
        caption.as_deref(),
        None,
        attach_to_session,
        &filename,
        "image/png",
        png_bytes,
    )
    .await;

    Ok(GamelogCaptureResultDto {
        local_path: local_file.to_string_lossy().into_owned(),
        uploaded_item,
        attached_post_id,
        warning,
    })
}

async fn start_audio_note_inner<R: tauri::Runtime>(
    app: &AppHandle<R>,
    rom_id: Option<String>,
    game_id: Option<String>,
    title: Option<String>,
    caption: Option<String>,
) -> Result<GamelogRuntimeStateDto, String> {
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| "failed".to_owned())?;
    if gl.mic.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
        return Err("already_recording".into());
    }

    let sess = gl.session.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let eff_rom = rom_id.or(sess.rom_id);
    let mut eff_game = game_id
        .filter(|g| is_valid_uuid(g))
        .map(|g| g.to_ascii_lowercase())
        .or(sess.game_id);
    let mut eff_sys = sess.system_id;

    if eff_game.is_none() {
        if let (Some(pool), Some(rid)) = (app.state::<AppState>().db.as_ref(), eff_rom.as_deref()) {
            let (gid, _, sys) = resolve_rom_mapping(pool, rid).await;
            eff_game = gid;
            if eff_sys.is_none() {
                eff_sys = sys;
            }
        }
    }

    let folder = eff_rom.as_deref().unwrap_or("avulsas");
    let rec_dir = gl.data_dir.join("recordings").join(folder);
    let wav_path = rec_dir.join(format!("audiolog-{}.wav", now_epoch_ms()));

    let handle = start_mic_recording(wav_path, MAX_AUDIO_NOTE_DURATION)?;
    *gl.mic.lock().unwrap_or_else(|p| p.into_inner()) = Some(ActiveMicNote {
        handle,
        rom_id: eff_rom,
        game_id: eff_game,
        system_id: eff_sys,
        title,
        caption,
    });
    let dto = gl.runtime_dto();
    let _ = app.emit("gamelog-state-changed", &dto);
    Ok(dto)
}

async fn stop_audio_note_inner<R: tauri::Runtime>(
    app: &AppHandle<R>,
    title_override: Option<String>,
    caption_override: Option<String>,
    attach_to_session: Option<bool>,
) -> Result<GamelogCaptureResultDto, String> {
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| "failed".to_owned())?;
    let active = gl
        .mic
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
        .ok_or_else(|| "not_recording".to_owned())?;
    emit_state(app);

    let done = tauri::async_runtime::spawn_blocking(move || active.handle.stop())
        .await
        .map_err(|e| e.to_string())??;

    let wav_bytes = std::fs::read(&done.path)
        .map_err(|e| format!("nao foi possivel ler a nota de audio gravada: {e}"))?;
    let filename = done
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audiolog.wav")
        .to_owned();

    let final_title = title_override.or(active.title);
    let final_caption = caption_override.or(active.caption);
    let same_active_rom = gl
        .session
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .rom_id
        .as_deref()
        == active.rom_id.as_deref()
        && active.rom_id.is_some();
    let attach = attach_to_session.or_else(|| (!same_active_rom).then_some(false));

    let (uploaded_item, attached_post_id, warning) = upload_and_maybe_attach(
        app,
        active.game_id.as_deref(),
        active.system_id.as_deref(),
        "audio",
        final_title.as_deref(),
        final_caption.as_deref(),
        None,
        attach,
        &filename,
        "audio/wav",
        wav_bytes,
    )
    .await;

    Ok(GamelogCaptureResultDto {
        local_path: done.path.to_string_lossy().into_owned(),
        uploaded_item,
        attached_post_id,
        warning,
    })
}

fn start_video_recording_inner<R: tauri::Runtime>(
    app: &AppHandle<R>,
    caption: Option<String>,
) -> Result<GamelogRuntimeStateDto, String> {
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| "failed".to_owned())?;
    if gl.video.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
        return Err("already_recording".into());
    }

    let state = app.state::<AppState>();
    if state.session.loaded_core().is_none() {
        return Err("nenhum jogo aberto para gravar video".into());
    }

    let ffmpeg_bin = gl.ensure_ffmpeg().ok_or_else(|| {
        "ffmpeg nao encontrado no sistema. Instale o FFmpeg para gravar videos MP4.".to_owned()
    })?;

    let (first_w, first_h, first_rgba) = capture_full_rgba(&state).ok_or_else(|| {
        "aguarde o primeiro quadro do jogo antes de iniciar a gravacao".to_owned()
    })?;
    let (rec_w, rec_h) = compute_recording_dimensions(first_w, first_h);

    let sess = gl.session.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let folder = sess.rom_id.as_deref().unwrap_or("avulsas");
    let rec_dir = gl.data_dir.join("recordings").join(folder);
    std::fs::create_dir_all(&rec_dir)
        .map_err(|e| format!("nao foi possivel criar pasta de gravacoes: {e}"))?;

    let ts = now_epoch_ms();
    let temp_video_mp4 = rec_dir.join(format!(".raw-video-{ts}.mp4"));
    let temp_audio_wav = rec_dir.join(format!(".raw-audio-{ts}.wav"));
    let final_mp4 = rec_dir.join(format!("video-{ts}.mp4"));

    audio_desktop::tap::start_game_audio_capture(&temp_audio_wav)?;

    let size_arg = format!("{rec_w}x{rec_h}");
    let mut cmd = Command::new(&ffmpeg_bin);
    apply_no_window(&mut cmd);
    cmd.args([
        "-y",
        "-loglevel",
        "error",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-s",
        &size_arg,
        "-r",
        "30",
        "-i",
        "pipe:0",
        "-an",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "23",
        "-pix_fmt",
        "yuv420p",
    ])
    .arg(&temp_video_mp4)
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::piped());

    let mut child: Child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = audio_desktop::tap::stop_game_audio_capture();
            let _ = std::fs::remove_file(&temp_audio_wav);
            return Err(format!("falha ao iniciar ffmpeg: {e}"));
        }
    };

    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = audio_desktop::tap::stop_game_audio_capture();
        let _ = std::fs::remove_file(&temp_audio_wav);
        return Err("stdin do ffmpeg indisponivel".into());
    };

    let (tx, rx) = mpsc::sync_channel::<(u32, u32, Vec<u8>)>(8);
    let _ = tx.try_send((first_w, first_h, first_rgba));

    let encoder_thread = std::thread::Builder::new()
        .name("reemu-video-encoder".into())
        .spawn(move || -> Result<u64, String> {
            let mut scaled = Vec::with_capacity((rec_w * rec_h * 4) as usize);
            let mut last_scaled: Option<Vec<u8>> = None;
            let mut written_frames: u64 = 0;
            let tick = Duration::from_millis(33);

            loop {
                match rx.recv_timeout(tick) {
                    Ok((fw, fh, rgba)) => {
                        resize_rgba_nearest(&rgba, fw, fh, rec_w, rec_h, &mut scaled);
                        if stdin.write_all(&scaled).is_err() {
                            break;
                        }
                        last_scaled = Some(scaled.clone());
                        written_frames += 1;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(prev) = last_scaled.as_ref() {
                            if stdin.write_all(prev).is_err() {
                                break;
                            }
                            written_frames += 1;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            let _ = stdin.flush();
            drop(stdin);
            let output = child
                .wait_with_output()
                .map_err(|e| format!("aguardando ffmpeg: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
                return Err(format!("ffmpeg encerrou com erro: {stderr}"));
            }
            Ok(written_frames)
        })
        .map_err(|e| format!("falha na thread de video: {e}"))?;

    *VIDEO_FRAME_TX.lock().unwrap_or_else(|p| p.into_inner()) = Some(tx);
    LAST_VIDEO_SAMPLE_MS.store(0, Ordering::Relaxed);
    VIDEO_TAP_ACTIVE.store(true, Ordering::Release);

    *gl.video.lock().unwrap_or_else(|p| p.into_inner()) = Some(ActiveVideoCapture {
        started_at: Instant::now(),
        rom_id: sess.rom_id,
        game_id: sess.game_id,
        system_id: sess.system_id,
        caption,
        ffmpeg_bin,
        temp_video_mp4,
        final_mp4,
        encoder_thread: Some(encoder_thread),
    });

    let dto = gl.runtime_dto();
    let _ = app.emit("gamelog-state-changed", &dto);
    Ok(dto)
}

async fn stop_video_recording_inner<R: tauri::Runtime>(
    app: &AppHandle<R>,
    caption_override: Option<String>,
    attach_to_session: Option<bool>,
) -> Result<GamelogCaptureResultDto, String> {
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| "failed".to_owned())?;
    let mut active = gl
        .video
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
        .ok_or_else(|| "not_recording".to_owned())?;

    VIDEO_TAP_ACTIVE.store(false, Ordering::Release);
    let _ = VIDEO_FRAME_TX
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    let finished_audio = audio_desktop::tap::stop_game_audio_capture();
    emit_state(app);

    let ffmpeg_bin = active.ffmpeg_bin.clone();
    let temp_video_mp4 = active.temp_video_mp4.clone();
    let final_mp4 = active.final_mp4.clone();
    let encoder = active.encoder_thread.take();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if let Some(handle) = encoder {
            let frames = handle
                .join()
                .map_err(|_| "thread de video interrompida".to_owned())??;
            if frames == 0 {
                let _ = std::fs::remove_file(&temp_video_mp4);
                if let Some(a) = &finished_audio {
                    let _ = std::fs::remove_file(&a.path);
                }
                return Err("nenhum quadro capturado na gravacao".into());
            }
        }

        let has_audio = finished_audio.as_ref().is_some_and(|a| a.frames > 0);
        let mut mux = Command::new(&ffmpeg_bin);
        apply_no_window(&mut mux);
        mux.args(["-y", "-loglevel", "error", "-i"])
            .arg(&temp_video_mp4);
        if let (true, Some(audio)) = (has_audio, finished_audio.as_ref()) {
            mux.arg("-i").arg(&audio.path).args([
                "-c:v",
                "copy",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-shortest",
                "-movflags",
                "+faststart",
            ]);
        } else {
            mux.args(["-c:v", "copy", "-movflags", "+faststart"]);
        }
        mux.arg(&final_mp4)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let out = mux
            .output()
            .map_err(|e| format!("falha ao finalizar MP4: {e}"))?;
        let _ = std::fs::remove_file(&temp_video_mp4);
        if let Some(a) = &finished_audio {
            let _ = std::fs::remove_file(&a.path);
        }
        if !out.status.success() {
            let err_txt = String::from_utf8_lossy(&out.stderr).trim().to_owned();
            return Err(format!("falha ao empacotar MP4: {err_txt}"));
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    let mp4_bytes = std::fs::read(&active.final_mp4)
        .map_err(|e| format!("nao foi possivel ler o video gravado: {e}"))?;
    let filename = active
        .final_mp4
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("video.mp4")
        .to_owned();
    let final_caption = caption_override.or(active.caption);
    let same_active_rom = gl
        .session
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .rom_id
        .as_deref()
        == active.rom_id.as_deref()
        && active.rom_id.is_some();
    let attach = attach_to_session.or_else(|| (!same_active_rom).then_some(false));

    let (uploaded_item, attached_post_id, warning) = upload_and_maybe_attach(
        app,
        active.game_id.as_deref(),
        active.system_id.as_deref(),
        "video",
        None,
        final_caption.as_deref(),
        None,
        attach,
        &filename,
        "video/mp4",
        mp4_bytes,
    )
    .await;

    Ok(GamelogCaptureResultDto {
        local_path: active.final_mp4.to_string_lossy().into_owned(),
        uploaded_item,
        attached_post_id,
        warning,
    })
}

// --- Atalhos de sistema (teclado / gamepad) --------------------------------

#[derive(Serialize, Clone)]
struct HotkeyToastPayload {
    action: &'static str,
    ok: bool,
    message: String,
}

fn describe_capture_toast(kind_label: &str, res: &GamelogCaptureResultDto) -> String {
    if res.attached_post_id.is_some() {
        format!("{kind_label} salva no Meu Log e publicada na sessao!")
    } else if res.uploaded_item.is_some() {
        format!("{kind_label} salva e enviada ao Meu Log!")
    } else if res.warning.as_deref() == Some("needs_consent") {
        format!("{kind_label} salva localmente (autorize Meu Log para enviar ao Alvanista).")
    } else if res.warning.as_deref() == Some("game_not_mapped") {
        format!(
            "{kind_label} salva localmente (vincule o jogo ao catalogo para enviar ao Meu Log)."
        )
    } else {
        format!("{kind_label} salva localmente.")
    }
}

pub fn hotkey_capture_screenshot<R: tauri::Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let (ok, message) = match capture_screenshot_inner(&app, None, None).await {
            Ok(res) => (true, describe_capture_toast("Captura de tela", &res)),
            Err(e) => (false, e),
        };
        let _ = app.emit(
            "hotkey-action",
            HotkeyToastPayload {
                action: "capture_screenshot",
                ok,
                message,
            },
        );
    });
}

pub fn hotkey_toggle_audio_note<R: tauri::Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let recording_now = app
            .try_state::<Arc<GamelogState>>()
            .is_some_and(|gl| gl.mic.lock().unwrap_or_else(|p| p.into_inner()).is_some());

        let (ok, message) = if recording_now {
            match stop_audio_note_inner(&app, None, None, None).await {
                Ok(res) => (true, describe_capture_toast("Nota de audio", &res)),
                Err(e) => (false, e),
            }
        } else {
            match start_audio_note_inner(&app, None, None, None, None).await {
                Ok(_) => (
                    true,
                    "Gravando nota de audio... Pressione o atalho novamente para concluir.".into(),
                ),
                Err(e) => (false, e),
            }
        };

        let _ = app.emit(
            "hotkey-action",
            HotkeyToastPayload {
                action: "toggle_audio_note",
                ok,
                message,
            },
        );
    });
}

pub fn hotkey_toggle_video_recording<R: tauri::Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let recording_now = app
            .try_state::<Arc<GamelogState>>()
            .is_some_and(|gl| gl.video.lock().unwrap_or_else(|p| p.into_inner()).is_some());

        let (ok, message) = if recording_now {
            let _ = app.emit(
                "hotkey-action",
                HotkeyToastPayload {
                    action: "toggle_video_recording",
                    ok: true,
                    message: "Finalizando e enviando video ao Meu Log...".into(),
                },
            );
            match stop_video_recording_inner(&app, None, None).await {
                Ok(res) => (true, describe_capture_toast("Gravacao de video", &res)),
                Err(e) => (false, e),
            }
        } else {
            match start_video_recording_inner(&app, None) {
                Ok(_) => (
                    true,
                    "Gravando video da partida... Pressione o atalho novamente para parar.".into(),
                ),
                Err(e) => (false, e),
            }
        };

        let _ = app.emit(
            "hotkey-action",
            HotkeyToastPayload {
                action: "toggle_video_recording",
                ok,
                message,
            },
        );
    });
}

// --- Comandos Tauri expostos para React (`PauseMenu` + `RomDetail`) --------

#[tauri::command]
pub fn gamelog_state(app: AppHandle) -> Result<GamelogRuntimeStateDto, AppError> {
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| AppError::from("Meu Log indisponivel"))?;
    Ok(gl.runtime_dto())
}

#[tauri::command]
pub fn gamelog_set_prefs(
    app: AppHandle,
    auto_session_checkin: Option<bool>,
    auto_attach_captures: Option<bool>,
    visibility: Option<String>,
) -> Result<GamelogPrefsDto, AppError> {
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| AppError::from("Meu Log indisponivel"))?;
    let next = {
        let mut prefs = gl.prefs.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(v) = auto_session_checkin {
            prefs.auto_session_checkin = v;
        }
        if let Some(v) = auto_attach_captures {
            prefs.auto_attach_captures = v;
        }
        if let Some(vis) = visibility.as_deref() {
            prefs.visibility = normalize_visibility(Some(vis), &prefs.visibility);
        }
        write_prefs_file(&gl.data_dir, &prefs).map_err(AppError::from)?;
        GamelogPrefsDto::from(&*prefs)
    };
    emit_state(&app);
    Ok(next)
}

#[tauri::command]
pub async fn gamelog_usage(app: AppHandle) -> Result<GamelogUsageDto, String> {
    let alva = app
        .try_state::<Alvanista>()
        .ok_or_else(|| "no_account".to_owned())?;
    let (status, val) = alva
        .partner_json(reqwest::Method::GET, "/api/partner/v1/gamelog/usage", None)
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let u: UsageWire = serde_json::from_value(val).map_err(|_| "failed".to_owned())?;
    Ok(GamelogUsageDto {
        used_bytes: u.used_bytes,
        quota_bytes: u.quota_bytes,
        item_count: u.item_count,
    })
}

#[tauri::command]
pub async fn gamelog_list_items(
    app: AppHandle,
    game_id: String,
    cursor: Option<String>,
    limit: Option<u32>,
) -> Result<GamelogListDto, String> {
    let clean_gid = game_id.trim().to_ascii_lowercase();
    if !is_valid_uuid(&clean_gid) {
        return Err("invalid_request".into());
    }
    let alva = app
        .try_state::<Alvanista>()
        .ok_or_else(|| "no_account".to_owned())?;
    let lim = limit.unwrap_or(20).clamp(1, 50);
    let path = match cursor.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => {
            let mut u = url::Url::parse("http://localhost").unwrap();
            u.query_pairs_mut()
                .append_pair("limit", &lim.to_string())
                .append_pair("cursor", c);
            format!(
                "/api/partner/v1/gamelog/games/{clean_gid}/items?{}",
                u.query().unwrap_or_default()
            )
        }
        None => format!("/api/partner/v1/gamelog/games/{clean_gid}/items?limit={lim}"),
    };
    let (status, val) = alva
        .partner_json(reqwest::Method::GET, &path, None)
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let parsed: ListWire = serde_json::from_value(val).map_err(|_| "failed".to_owned())?;
    Ok(GamelogListDto {
        items: parsed.items.into_iter().map(Into::into).collect(),
        next_cursor: parsed.next_cursor,
    })
}

#[tauri::command]
pub async fn gamelog_create_note(
    app: AppHandle,
    game_id: String,
    rom_id: Option<String>,
    title: Option<String>,
    body: String,
    visibility: Option<String>,
    attach_to_session: Option<bool>,
) -> Result<GamelogItemDto, String> {
    let clean_gid = game_id.trim().to_ascii_lowercase();
    if !is_valid_uuid(&clean_gid) {
        return Err("invalid_request".into());
    }
    let clean_body = body.trim();
    if clean_body.is_empty() {
        return Err("invalid_request".into());
    }
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| "failed".to_owned())?;
    let alva = app
        .try_state::<Alvanista>()
        .ok_or_else(|| "no_account".to_owned())?;

    let prefs = gl.prefs_snapshot();
    let vis = normalize_visibility(visibility.as_deref(), &prefs.visibility);

    let mut platform_id: Option<String> = None;
    if let (Some(pool), Some(rid)) = (app.state::<AppState>().db.as_ref(), rom_id.as_deref()) {
        let (_, _, sys) = resolve_rom_mapping(pool, rid).await;
        platform_id = sys;
    }

    let mut payload = serde_json::Map::new();
    payload.insert("kind".into(), serde_json::Value::String("note".into()));
    payload.insert(
        "body".into(),
        serde_json::Value::String(clean_body.to_owned()),
    );
    payload.insert("visibility".into(), serde_json::Value::String(vis.clone()));
    if let Some(t) = title.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        payload.insert("title".into(), serde_json::Value::String(t.to_owned()));
    }
    if let Some(sys) = platform_id.as_deref() {
        payload.insert(
            "platform_id".into(),
            serde_json::Value::String(sys.to_owned()),
        );
    }

    let path = format!("/api/partner/v1/gamelog/games/{clean_gid}/items");
    let (status, val) = alva
        .partner_json(
            reqwest::Method::POST,
            &path,
            Some(&serde_json::Value::Object(payload)),
        )
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let item: GamelogItemDto = serde_json::from_value::<LogItemWire>(val)
        .map_err(|_| "failed".to_owned())?
        .into();

    emit_items_updated(&app, Some(&clean_gid));

    let same_active_game = gl
        .session
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .game_id
        .as_deref()
        == Some(clean_gid.as_str());
    if vis != "private"
        && attach_to_session.unwrap_or(same_active_game && prefs.auto_attach_captures)
    {
        let _ =
            attach_item_to_active_session(&app, &item.id, Some(&clean_gid), None, Some(&vis)).await;
    }

    Ok(item)
}

fn infer_media_kind_and_mime(
    path: &Path,
    explicit_kind: Option<&str>,
) -> Result<(&'static str, &'static str), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match (explicit_kind, ext.as_str()) {
        (Some("screenshot"), _) | (None, "png") => Ok(("screenshot", "image/png")),
        (None, "jpg" | "jpeg") => Ok(("screenshot", "image/jpeg")),
        (None, "webp") => Ok(("screenshot", "image/webp")),
        (None, "gif") => Ok(("screenshot", "image/gif")),
        (Some("video"), "webm") | (None, "webm") => Ok(("video", "video/webm")),
        (Some("video"), "mov") | (None, "mov") => Ok(("video", "video/quicktime")),
        (Some("video"), _) | (None, "mp4" | "m4v") => Ok(("video", "video/mp4")),
        (Some("audio"), "mp3") | (None, "mp3") => Ok(("audio", "audio/mpeg")),
        (Some("audio"), "ogg" | "oga") | (None, "ogg" | "oga") => Ok(("audio", "audio/ogg")),
        (Some("audio"), "m4a" | "aac") | (None, "m4a" | "aac") => Ok(("audio", "audio/mp4")),
        (Some("audio"), "flac") | (None, "flac") => Ok(("audio", "audio/flac")),
        (Some("audio"), _) | (None, "wav") => Ok(("audio", "audio/wav")),
        _ => Err("unsupported_media".into()),
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn gamelog_upload_file(
    app: AppHandle,
    game_id: String,
    rom_id: Option<String>,
    file_path: String,
    kind: Option<String>,
    title: Option<String>,
    caption: Option<String>,
    visibility: Option<String>,
    attach_to_session: Option<bool>,
) -> Result<GamelogItemDto, String> {
    let clean_gid = game_id.trim().to_ascii_lowercase();
    if !is_valid_uuid(&clean_gid) {
        return Err("invalid_request".into());
    }
    let path = PathBuf::from(&file_path);
    let (resolved_kind, mime_type) = infer_media_kind_and_mime(&path, kind.as_deref())?;
    let bytes = std::fs::read(&path).map_err(|_| "not_found".to_owned())?;
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload.bin")
        .to_owned();

    let mut platform_id: Option<String> = None;
    if let (Some(pool), Some(rid)) = (app.state::<AppState>().db.as_ref(), rom_id.as_deref()) {
        let (_, _, sys) = resolve_rom_mapping(pool, rid).await;
        platform_id = sys;
    }

    let same_active_game = app.try_state::<Arc<GamelogState>>().is_some_and(|gl| {
        gl.session
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .game_id
            .as_deref()
            == Some(clean_gid.as_str())
    });
    let attach = attach_to_session.or_else(|| (!same_active_game).then_some(false));

    let (item, _, warn) = upload_and_maybe_attach(
        &app,
        Some(&clean_gid),
        platform_id.as_deref(),
        resolved_kind,
        title.as_deref(),
        caption.as_deref(),
        visibility.as_deref(),
        attach,
        &filename,
        mime_type,
        bytes,
    )
    .await;

    match item {
        Some(it) => Ok(it),
        None => Err(warn.unwrap_or_else(|| "failed".into())),
    }
}

#[tauri::command]
pub async fn gamelog_patch_item(
    app: AppHandle,
    item_id: String,
    title: Option<String>,
    body: Option<String>,
    visibility: Option<String>,
) -> Result<GamelogItemDto, String> {
    let clean_id = item_id.trim();
    if clean_id.is_empty() {
        return Err("invalid_request".into());
    }
    let alva = app
        .try_state::<Alvanista>()
        .ok_or_else(|| "no_account".to_owned())?;

    let mut payload = serde_json::Map::new();
    if let Some(t) = title {
        payload.insert("title".into(), serde_json::Value::String(t));
    }
    if let Some(b) = body {
        payload.insert("body".into(), serde_json::Value::String(b));
    }
    if let Some(v) = visibility {
        payload.insert(
            "visibility".into(),
            serde_json::Value::String(normalize_visibility(Some(&v), "private")),
        );
    }

    let path = format!("/api/partner/v1/gamelog/items/{clean_id}");
    let (status, val) = alva
        .partner_json(
            reqwest::Method::PATCH,
            &path,
            Some(&serde_json::Value::Object(payload)),
        )
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    let item: GamelogItemDto = serde_json::from_value::<LogItemWire>(val)
        .map_err(|_| "failed".to_owned())?
        .into();
    emit_items_updated(&app, Some(&item.game_id));
    Ok(item)
}

#[tauri::command]
pub async fn gamelog_delete_item(app: AppHandle, item_id: String) -> Result<(), String> {
    let clean_id = item_id.trim();
    if clean_id.is_empty() {
        return Err("invalid_request".into());
    }
    let alva = app
        .try_state::<Alvanista>()
        .ok_or_else(|| "no_account".to_owned())?;
    let path = format!("/api/partner/v1/gamelog/items/{clean_id}");
    let (status, val) = alva
        .partner_json(reqwest::Method::DELETE, &path, None)
        .await
        .map_err(map_client_error)?;
    if !(200..300).contains(&status) {
        return Err(map_non_success(status, &val));
    }
    emit_items_updated(&app, None);
    Ok(())
}

#[tauri::command]
pub async fn gamelog_attach_to_session(
    app: AppHandle,
    item_id: String,
    game_id: Option<String>,
    caption: Option<String>,
    visibility: Option<String>,
) -> Result<Option<String>, String> {
    attach_item_to_active_session(
        &app,
        &item_id,
        game_id.as_deref(),
        caption.as_deref(),
        visibility.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn gamelog_capture_screenshot(
    app: AppHandle,
    caption: Option<String>,
    attach_to_session: Option<bool>,
) -> Result<GamelogCaptureResultDto, String> {
    capture_screenshot_inner(&app, caption, attach_to_session).await
}

#[tauri::command]
pub async fn gamelog_start_audio_note(
    app: AppHandle,
    rom_id: Option<String>,
    game_id: Option<String>,
    title: Option<String>,
    caption: Option<String>,
) -> Result<GamelogRuntimeStateDto, String> {
    start_audio_note_inner(&app, rom_id, game_id, title, caption).await
}

#[tauri::command]
pub async fn gamelog_stop_audio_note(
    app: AppHandle,
    title: Option<String>,
    caption: Option<String>,
    attach_to_session: Option<bool>,
) -> Result<GamelogCaptureResultDto, String> {
    stop_audio_note_inner(&app, title, caption, attach_to_session).await
}

#[tauri::command]
pub fn gamelog_cancel_audio_note(app: AppHandle) -> Result<GamelogRuntimeStateDto, String> {
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| "failed".to_owned())?;
    let active = gl.mic.lock().unwrap_or_else(|p| p.into_inner()).take();
    if let Some(m) = active {
        m.handle.cancel();
    }
    let dto = gl.runtime_dto();
    let _ = app.emit("gamelog-state-changed", &dto);
    Ok(dto)
}

#[tauri::command]
pub fn gamelog_start_video_recording(
    app: AppHandle,
    caption: Option<String>,
) -> Result<GamelogRuntimeStateDto, String> {
    start_video_recording_inner(&app, caption)
}

#[tauri::command]
pub async fn gamelog_stop_video_recording(
    app: AppHandle,
    caption: Option<String>,
    attach_to_session: Option<bool>,
) -> Result<GamelogCaptureResultDto, String> {
    stop_video_recording_inner(&app, caption, attach_to_session).await
}

#[tauri::command]
pub fn gamelog_open_captures_dir(app: AppHandle, rom_id: Option<String>) -> Result<(), String> {
    let gl = app
        .try_state::<Arc<GamelogState>>()
        .ok_or_else(|| "failed".to_owned())?;
    let base = match rom_id.as_deref().filter(|s| !s.is_empty()) {
        Some(rid) => {
            let shots = gl.data_dir.join("screenshots").join(rid);
            let recs = gl.data_dir.join("recordings").join(rid);
            if recs.exists() {
                recs
            } else {
                let _ = std::fs::create_dir_all(&shots);
                shots
            }
        }
        None => {
            let shots = gl.data_dir.join("screenshots");
            let _ = std::fs::create_dir_all(&shots);
            shots
        }
    };
    open::that_detached(base).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gamelog_open_session_post(post_id: String) -> Result<(), String> {
    let clean = post_id.trim();
    if clean.is_empty()
        || clean.len() > 120
        || !clean
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err("invalid_request".into());
    }
    let url = format!("{}/posts/{clean}", crate::alvanista::config::base_urls().0);
    crate::alvanista::open_in_browser(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_dimensions_are_even_and_bounded() {
        assert_eq!(compute_recording_dimensions(256, 224), (256, 224));
        assert_eq!(compute_recording_dimensions(321, 241), (320, 240));
        let (w, h) = compute_recording_dimensions(1920, 1080);
        assert_eq!((w, h), (1280, 720));
        assert_eq!(w % 2, 0);
        assert_eq!(h % 2, 0);
    }

    #[test]
    fn full_png_encoder_produces_valid_png() {
        let rgba = vec![255u8; 4 * 4 * 4];
        let png = encode_png_rgba(4, 4, &rgba).unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    }

    #[test]
    fn infer_media_kind_supports_images_audio_and_video() {
        assert_eq!(
            infer_media_kind_and_mime(Path::new("shot.png"), None).unwrap(),
            ("screenshot", "image/png")
        );
        assert_eq!(
            infer_media_kind_and_mime(Path::new("note.wav"), None).unwrap(),
            ("audio", "audio/wav")
        );
        assert_eq!(
            infer_media_kind_and_mime(Path::new("clip.mp4"), None).unwrap(),
            ("video", "video/mp4")
        );
        assert!(infer_media_kind_and_mime(Path::new("rom.iso"), None).is_err());
    }

    #[tokio::test]
    async fn multipart_and_three_step_video_uploads_roundtrip_against_partner_api() {
        use crate::alvanista::client::Client;
        use crate::alvanista::store::test_support::{store_with, tokens, MemSecrets, SERIAL};
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let _serial = SERIAL.lock().await;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let base_for_srv = base.clone();
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_srv = Arc::clone(&seen);

        std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(15);
            let mut complete_calls = 0usize;
            while Instant::now() < until {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&chunk[..n]);
                            if let Some(hdr_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                                let hdr_str = String::from_utf8_lossy(&buf[..hdr_end]);
                                let content_len = hdr_str
                                    .lines()
                                    .find_map(|l| {
                                        let (k, v) = l.split_once(':')?;
                                        k.trim()
                                            .eq_ignore_ascii_case("content-length")
                                            .then(|| v.trim().parse::<usize>().ok())
                                            .flatten()
                                    })
                                    .unwrap_or(0);
                                if buf.len() >= hdr_end + 4 + content_len {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
                let raw = String::from_utf8_lossy(&buf);
                let first_line = raw.lines().next().unwrap_or_default().to_owned();
                seen_srv.lock().unwrap().push(first_line.clone());

                let (code, body) = if first_line.starts_with(
                    "POST /api/partner/v1/gamelog/games/11111111-2222-3333-4444-555555555555/items ",
                ) {
                    (
                        201,
                        r#"{"id":"item_shot_1","game_id":"11111111-2222-3333-4444-555555555555","kind":"screenshot","body":"Chefe","visibility":"friends","state":"ready","created_at":"2026-11-27T01:00:00Z","updated_at":"2026-11-27T01:00:00Z","media":[{"key":"k1","url":"https://cdn/s.png","mime_type":"image/png","size_bytes":120}]}"#
                            .to_owned(),
                    )
                } else if first_line.starts_with(
                    "POST /api/partner/v1/gamelog/games/11111111-2222-3333-4444-555555555555/uploads ",
                ) {
                    (
                        201,
                        format!(
                            r#"{{"item_id":"item_vid_1","upload_url":"{base_for_srv}/r2-put/vid1","upload_headers":{{"Content-Type":"video/mp4"}}}}"#
                        ),
                    )
                } else if first_line.starts_with("PUT /r2-put/vid1 ") {
                    assert!(
                        !raw.to_ascii_lowercase().contains("\r\nauthorization:"),
                        "presigned R2 PUT must not send Authorization header"
                    );
                    (200, String::new())
                } else if first_line
                    .starts_with("POST /api/partner/v1/gamelog/items/item_vid_1/complete ")
                {
                    complete_calls += 1;
                    if complete_calls == 1 {
                        (202, r#"{"state":"processing"}"#.to_owned())
                    } else {
                        (
                            200,
                            r#"{"id":"item_vid_1","game_id":"11111111-2222-3333-4444-555555555555","kind":"video","body":"Clip","visibility":"friends","state":"ready","created_at":"2026-11-27T01:00:00Z","updated_at":"2026-11-27T01:00:05Z","media":[{"key":"v1","url":"https://cdn/v.mp4","mime_type":"video/mp4","size_bytes":2048,"poster_url":"https://cdn/p.jpg"}]}"#
                                .to_owned(),
                        )
                    }
                } else {
                    (404, r#"{"error":"not_found"}"#.to_owned())
                };

                let _ = write!(
                    stream,
                    "HTTP/1.1 {code} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });

        let (store, _) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(base, store));
        alva.store()
            .start_session(&tokens("gl1"), &["profile:read", "gamelog"])
            .await
            .unwrap();

        let game_id = "11111111-2222-3333-4444-555555555555";
        let shot = upload_multipart_item(
            &alva,
            &UploadMeta {
                game_id,
                kind: "screenshot",
                title: None,
                body: Some("Chefe"),
                platform_id: Some("ps2"),
                visibility: "friends",
                taken_at: Some("2026-11-27T01:00:00Z"),
            },
            "shot.png",
            "image/png",
            vec![1, 2, 3, 4],
        )
        .await
        .unwrap();
        assert_eq!(shot.id, "item_shot_1");
        assert_eq!(shot.kind, "screenshot");

        let vid = upload_video_three_step(
            &alva,
            &UploadMeta {
                game_id,
                kind: "video",
                title: None,
                body: Some("Clip"),
                platform_id: Some("gamecube"),
                visibility: "friends",
                taken_at: Some("2026-11-27T01:00:00Z"),
            },
            "video/mp4",
            vec![0u8; 32],
        )
        .await
        .unwrap();
        assert_eq!(vid.id, "item_vid_1");
        assert_eq!(vid.kind, "video");
        assert_eq!(
            vid.media[0].poster_url.as_deref(),
            Some("https://cdn/p.jpg")
        );
        assert!(seen.lock().unwrap().len() >= 4);
    }
}
