//! Comandos Tauri expostos ao frontend + estado da aplicação.
//!
//! Escopo desta etapa: plumbing. O `EmuSession` roda o core numa thread
//! dedicada; o foco é decidido aqui (Rust) e propagado pro React via evento
//! `focus-changed`. A surface nativa de vídeo (que consome
//! `session.take_latest_frame()`) e o `AudioSink` entram nas etapas 03/06.

use crate::video::VideoSurface;
use domain::audio::{AudioConfig, AudioConfigRepository};
use domain::core_loader::InstalledCoreRepository;
use emu_session::{EmuSession, FocusController, SessionConfig, SessionState};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub session: Arc<EmuSession>,
    pub focus: Mutex<FocusController>,
    /// `None` se a `wgpu::Surface` não pôde ser criada (roda em modo só-webview).
    pub video: Mutex<Option<VideoSurface>>,
    /// `None` se o SQLite não abriu.
    pub db: Option<db::Db>,
    pub save_dir: std::path::PathBuf,
}

impl AppState {
    pub fn new(db: Option<db::Db>, audio_config: AudioConfig) -> Self {
        let base = dirs_or_temp();
        let save_dir = base.join("saves");
        let mut cfg = SessionConfig::new(base.join("cores"), base.join("system"), save_dir.clone());
        cfg.audio_sink = Some(Box::new(move || {
            match audio_desktop::CpalAudioSink::new(&audio_config) {
                Ok(s) => Some(Box::new(s) as _),
                Err(e) => {
                    log::error!("áudio indisponível ({e}) — rodando sem som");
                    None
                }
            }
        }));
        let session = Arc::new(EmuSession::spawn(cfg));
        let focus = Mutex::new(FocusController::new(Arc::clone(&session)));
        Self {
            session,
            focus,
            video: Mutex::new(None),
            db,
            save_dir,
        }
    }
}

fn dirs_or_temp() -> std::path::PathBuf {
    // Placeholder — a etapa 03/07 troca por AppHandle::path().app_data_dir().
    std::env::var_os("REEMU_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[derive(Serialize, Clone)]
struct FocusChanged {
    focus: &'static str,
}

fn focus_str(f: domain::focus::InputFocus) -> &'static str {
    match f {
        domain::focus::InputFocus::GameFocused => "GameFocused",
        domain::focus::InputFocus::MenuFocused => "MenuFocused",
    }
}

#[tauri::command]
pub fn current_focus(state: State<'_, AppState>) -> &'static str {
    use domain::focus::FocusManager;
    focus_str(state.focus.lock().unwrap().current())
}

#[tauri::command]
pub fn toggle_focus(app: AppHandle, state: State<'_, AppState>) -> Result<&'static str, String> {
    use domain::focus::FocusManager;
    let now = {
        let mut fc = state.focus.lock().map_err(|_| "focus lock poisoned")?;
        fc.toggle();
        fc.current()
    };
    let s = focus_str(now);
    app.emit("focus-changed", FocusChanged { focus: s })
        .map_err(|e| e.to_string())?;
    Ok(s)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedGame {
    base_width: u32,
    base_height: u32,
    fps: f64,
    sample_rate: f64,
}

#[tauri::command]
pub async fn load_game(
    app: AppHandle,
    core_id: String,
    rom_path: String,
) -> Result<LoadedGame, String> {
    // O load é bloqueante (dlopen + retro_init); tira da thread async.
    let state = app.state::<AppState>();
    let session = Arc::clone(&state.session);
    let av = tauri::async_runtime::spawn_blocking(move || session.load(&core_id, &rom_path))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    Ok(LoadedGame {
        base_width: av.geometry.base_width,
        base_height: av.geometry.base_height,
        fps: av.timing.fps,
        sample_rate: av.timing.sample_rate,
    })
}

#[tauri::command]
pub fn session_state(state: State<'_, AppState>) -> &'static str {
    match state.session.state() {
        SessionState::Idle => "Idle",
        SessionState::Running => "Running",
        SessionState::Paused => "Paused",
    }
}

// --- configs persistidas (crates/db) -------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioConfigDto {
    pub output_device_id: Option<String>,
    pub output_device_name: Option<String>,
    pub rate_control_enabled: bool,
    pub rate_control_delta: f32,
    pub sample_rate_preference: Option<u32>,
}

impl From<AudioConfig> for AudioConfigDto {
    fn from(c: AudioConfig) -> Self {
        Self {
            output_device_id: c.output_device_id,
            output_device_name: c.output_device_name,
            rate_control_enabled: c.rate_control_enabled,
            rate_control_delta: c.rate_control_delta,
            sample_rate_preference: c.sample_rate_preference,
        }
    }
}

impl From<AudioConfigDto> for AudioConfig {
    fn from(c: AudioConfigDto) -> Self {
        Self {
            output_device_id: c.output_device_id,
            output_device_name: c.output_device_name,
            rate_control_enabled: c.rate_control_enabled,
            rate_control_delta: c.rate_control_delta,
            sample_rate_preference: c.sample_rate_preference,
        }
    }
}

fn pool(state: &AppState) -> Result<db::Db, String> {
    state
        .db
        .clone()
        .ok_or_else(|| "banco de dados indisponível".to_string())
}

#[tauri::command]
pub async fn get_audio_config(state: State<'_, AppState>) -> Result<AudioConfigDto, String> {
    let repo = db::AudioConfigRepo::new(pool(&state)?);
    repo.get().await.map(Into::into).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_audio_config(
    state: State<'_, AppState>,
    config: AudioConfigDto,
) -> Result<(), String> {
    let repo = db::AudioConfigRepo::new(pool(&state)?);
    repo.update(&config.into()).await.map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCoreDto {
    pub core_id: String,
    pub version: String,
    pub render_backend: Option<String>,
}

#[tauri::command]
pub async fn list_installed_cores(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledCoreDto>, String> {
    let repo = db::InstalledCoresRepo::new(pool(&state)?);
    let cores = repo.list().await.map_err(|e| e.to_string())?;
    Ok(cores
        .into_iter()
        .map(|c| InstalledCoreDto {
            core_id: c.core_id,
            version: c.version,
            render_backend: c
                .render_requirements
                .map(|r| format!("{:?}", r.render_backend)),
        })
        .collect())
}

// --- biblioteca (crates/library-scan) -----------------------------------

use domain::library::RomRepository;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomDto {
    pub id: String,
    pub title: String,
    pub system_id: String,
    pub file_path: String,
}

#[tauri::command]
pub async fn list_roms(state: State<'_, AppState>) -> Result<Vec<RomDto>, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let roms = repo.list().await.map_err(|e| e.to_string())?;
    Ok(roms
        .into_iter()
        .map(|r| RomDto {
            title: std::path::Path::new(&r.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&r.file_path)
                .to_string(),
            id: r.id,
            system_id: r.system_id,
            file_path: r.file_path,
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReportDto {
    pub found: usize,
    pub added: usize,
    pub skipped_known: usize,
    pub skipped_unrecognized: usize,
    pub errors: usize,
}

#[tauri::command]
pub async fn scan_library(
    state: State<'_, AppState>,
    path: String,
) -> Result<ScanReportDto, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let r = library_scan::scan_into(&repo, std::path::Path::new(&path), now)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ScanReportDto {
        found: r.found,
        added: r.added,
        skipped_known: r.skipped_known,
        skipped_unrecognized: r.skipped_unrecognized,
        errors: r.errors,
    })
}

// --- save states (etapa 08) --------------------------------------------

use crate::save_state as save_svc;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveStateDto {
    pub id: String,
    pub slot: Option<u32>,
    pub created_at: i64,
    pub file_path: String,
}

fn save_dto(m: domain::save_state::SaveStateMetadata) -> SaveStateDto {
    SaveStateDto {
        id: m.id,
        slot: m.slot,
        created_at: m.created_at,
        file_path: m.file_path,
    }
}

#[tauri::command]
pub async fn save_state(
    state: State<'_, AppState>,
    rom_id: String,
    slot: Option<u32>,
) -> Result<SaveStateDto, String> {
    let core_id = state
        .session
        .loaded_core()
        .ok_or_else(|| "nenhum core carregado".to_string())?;
    let bytes = state
        .session
        .save_state()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "o core não suporta save state".to_string())?;
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let meta = save_svc::save(&repo, &state.save_dir, &rom_id, &core_id, slot, &bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(save_dto(meta))
}

#[tauri::command]
pub async fn list_save_states(
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<Vec<SaveStateDto>, String> {
    let repo = db::SaveStateRepo::new(pool(&state)?);
    Ok(save_svc::list(&repo, &rom_id)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(save_dto)
        .collect())
}

#[tauri::command]
pub async fn load_save_state(state: State<'_, AppState>, state_id: String) -> Result<(), String> {
    let running = state.session.loaded_core();
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let meta = save_svc::load_bytes(&repo, &state_id, running.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&meta.file_path).map_err(|e| e.to_string())?;
    if state
        .session
        .restore_state(bytes)
        .map_err(|e| e.to_string())?
    {
        Ok(())
    } else {
        Err("retro_unserialize recusou o state".to_string())
    }
}

#[tauri::command]
pub async fn delete_save_state(state: State<'_, AppState>, state_id: String) -> Result<(), String> {
    let repo = db::SaveStateRepo::new(pool(&state)?);
    save_svc::delete(&repo, &state_id)
        .await
        .map_err(|e| e.to_string())
}

// --- input (etapa 05) -------------------------------------------------

/// Recebe `KeyboardEvent.code` da webview (keydown/keyup). Hotkey padrão
/// (Escape/F1) alterna o menu; senão, o mapa teclado→RetroPad só vale em
/// `GameFocused`.
#[tauri::command]
pub fn input_key(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
    pressed: bool,
) -> Result<(), String> {
    use domain::focus::{FocusManager, InputFocus};

    if pressed && matches!(code.as_str(), "Escape" | "F1") {
        let now = {
            let mut fc = state.focus.lock().map_err(|_| "focus lock")?;
            fc.toggle();
            fc.current()
        };
        let s = focus_str(now);
        let _ = app.emit("focus-changed", FocusChanged { focus: s });
        return Ok(());
    }

    let game_focused = state
        .focus
        .lock()
        .map(|f| f.current())
        .unwrap_or(InputFocus::GameFocused)
        == InputFocus::GameFocused;

    if let Some((port, button)) = input_desktop::keymap::web_code_to_retropad(&code) {
        emu_session::retropad().set(port as usize, button, pressed && game_focused);
    }
    Ok(())
}
