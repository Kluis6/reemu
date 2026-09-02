//! Comandos Tauri expostos ao frontend + estado da aplicação.
//!
//! Escopo desta etapa: plumbing. O `EmuSession` roda o core numa thread
//! dedicada; o foco é decidido aqui (Rust) e propagado pro React via evento
//! `focus-changed`. A surface nativa de vídeo (que consome
//! `session.take_latest_frame()`) e o `AudioSink` entram nas etapas 03/06.

use crate::video::VideoSurface;
use domain::audio::{AudioConfig, AudioConfigRepository};
use domain::core_loader::InstalledCoreRepository;
use domain::hotkeys::{HotkeyBinding, SystemAction};
use domain::shader_chain::{AssignmentScope, ShaderChainStore};
use emu_session::{EmuSession, FocusController, SessionConfig, SessionState};
use input_desktop::ComboHotkeyResolver;
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
    /// `<dados>/cores` — onde a descoberta de cores procura `*_libretro.so`.
    pub cores_dir: std::path::PathBuf,
    /// Hotkeys de sistema carregadas do DB (`system_hotkeys`). `save_binding` /
    /// `clear_system_hotkey` recompõem via `refresh_hotkey_resolver`.
    pub hotkeys: Mutex<ComboHotkeyResolver>,
    /// Última ação de hotkey resolvida — pra disparar uma vez por "aperto"
    /// (o loop de eventos consulta o conjunto segurado a cada frame).
    pub last_hotkey: Mutex<Option<SystemAction>>,
    /// `rom_id` do jogo carregado agora (o path vai pro core, o id vai pro DB).
    /// `None` quando ocioso. Usado pelo QuickSave/QuickLoad das hotkeys.
    pub current_rom: Mutex<Option<String>>,
    /// Contexto GPU pro processamento de frame (etapa 04). `None` = sem
    /// adapter wgpu; `poll_frame` cai no caminho CPU (`to_rgba8`).
    pub gpu: Mutex<Option<crate::gpu::FrameProcessor>>,
    /// Progresso da leva de scraping de metadata (etapa 09).
    pub scrape: Arc<crate::scraping::ScrapeProgress>,
    /// Flag de cancelamento da leva de scraping.
    pub scrape_stop: Arc<std::sync::atomic::AtomicBool>,
    /// Último frame enviado pro canvas — pra gerar o thumbnail no save state.
    /// Atualizado com throttle no `poll_frame`.
    pub last_frame: Mutex<Option<CachedFrame>>,
}

/// Cópia de um frame RGBA8 + quando foi tirada (throttle do cache de thumbnail).
pub struct CachedFrame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub at: std::time::Instant,
}

/// Slot reservado pro QuickSave/QuickLoad das hotkeys (os slots manuais da UI
/// usam 1+).
const QUICK_SLOT: u32 = 0;

impl AppState {
    pub fn new(
        base: std::path::PathBuf,
        db: Option<db::Db>,
        audio_config: AudioConfig,
        hotkeys: Vec<HotkeyBinding>,
    ) -> Self {
        let save_dir = base.join("saves");
        let cores_dir = base.join("cores");
        let mut cfg = SessionConfig::new(cores_dir.clone(), base.join("system"), save_dir.clone());
        cfg.enable_gamepad = true;
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
            cores_dir,
            hotkeys: Mutex::new(ComboHotkeyResolver::new(hotkeys)),
            last_hotkey: Mutex::new(None),
            current_rom: Mutex::new(None),
            gpu: Mutex::new(None),
            scrape: Arc::new(crate::scraping::ScrapeProgress::default()),
            scrape_stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            last_frame: Mutex::new(None),
        }
    }
}

#[derive(Serialize, Clone)]
struct FocusChanged {
    focus: &'static str,
}

#[derive(Serialize, Clone)]
struct HotkeyAction {
    action: &'static str,
    ok: bool,
    message: String,
}

/// Resolve as hotkeys de sistema a partir do conjunto segurado (teclado +
/// gamepad) e dispara a ação **uma vez por aperto**. Chamado a cada frame
/// pelo loop de eventos, antes de qualquer roteamento pro jogo (prioridade).
pub(crate) fn poll_hotkeys<R: tauri::Runtime>(app: &AppHandle<R>) {
    use domain::hotkeys::HotkeyResolver;
    let state = app.state::<AppState>();
    let held = input_desktop::held::snapshot();
    let action = state
        .hotkeys
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .resolve(&held);
    {
        let mut last = state.last_hotkey.lock().unwrap_or_else(|p| p.into_inner());
        if action == *last {
            return;
        }
        *last = action;
    }
    let Some(action) = action else { return };
    match action {
        SystemAction::ToggleMenuOverlay => {
            toggle_and_emit(app);
        }
        SystemAction::QuickSave => spawn_quick_state(app.clone(), true),
        SystemAction::QuickLoad => spawn_quick_state(app.clone(), false),
    }
}

/// Nome curto de um pulso de navegação de menu, pro evento `menu-nav`.
pub(crate) fn nav_pulse_name(p: input_desktop::NavPulse) -> &'static str {
    use input_desktop::NavPulse::*;
    match p {
        Up => "up",
        Down => "down",
        Left => "left",
        Right => "right",
        Confirm => "confirm",
        Back => "back",
        Search => "search",
        Context => "context",
    }
}

/// QuickSave (`save = true`) / QuickLoad no slot [`QUICK_SLOT`] do jogo
/// carregado agora. Numa task async — o resultado volta pro frontend por
/// `hotkey-action` (toast).
fn spawn_quick_state<R: tauri::Runtime>(app: AppHandle<R>, save: bool) {
    tauri::async_runtime::spawn(async move {
        let action = if save { "quick_save" } else { "quick_load" };
        let result = quick_state(&app, save).await;
        let (ok, message) = match result {
            Ok(msg) => (true, msg),
            Err(msg) => (false, msg),
        };
        let _ = app.emit(
            "hotkey-action",
            HotkeyAction {
                action,
                ok,
                message,
            },
        );
    });
}

async fn quick_state<R: tauri::Runtime>(app: &AppHandle<R>, save: bool) -> Result<String, String> {
    let state = app.state::<AppState>();
    let rom_id = state
        .current_rom
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .ok_or("nenhum jogo carregado")?;
    let core_id = state.session.loaded_core().ok_or("nenhum core carregado")?;
    let repo = db::SaveStateRepo::new(pool(&state)?);

    if save {
        let bytes = state
            .session
            .save_state()
            .map_err(|e| e.to_string())?
            .ok_or("o core não suporta save state")?;
        let thumb = {
            let f = state.last_frame.lock().unwrap_or_else(|p| p.into_inner());
            f.as_ref()
                .and_then(|c| thumbnail_png(c.w, c.h, &c.rgba, 320))
        };
        save_svc::save(
            &repo,
            &state.save_dir,
            &rom_id,
            &core_id,
            Some(QUICK_SLOT),
            &bytes,
            thumb.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?;
        Ok("QuickSave gravado".into())
    } else {
        let meta = save_svc::list(&repo, &rom_id)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|m| m.slot == Some(QUICK_SLOT))
            .ok_or("nenhum QuickSave pra este jogo")?;
        let bytes = std::fs::read(&meta.file_path).map_err(|e| e.to_string())?;
        if state
            .session
            .restore_state(bytes)
            .map_err(|e| e.to_string())?
        {
            Ok("QuickLoad aplicado".into())
        } else {
            Err("o core recusou o save state".into())
        }
    }
}

pub(crate) fn focus_str(f: domain::focus::InputFocus) -> &'static str {
    match f {
        domain::focus::InputFocus::GameFocused => "GameFocused",
        domain::focus::InputFocus::MenuFocused => "MenuFocused",
    }
}

/// Alterna o foco e emite `focus-changed`. Usado pelo comando `toggle_focus`,
/// pela hotkey de teclado e pelo botão de menu do gamepad.
pub(crate) fn toggle_and_emit<R: tauri::Runtime>(app: &AppHandle<R>) -> &'static str {
    use domain::focus::FocusManager;
    let state = app.state::<AppState>();
    let now = {
        let mut fc = state.focus.lock().unwrap_or_else(|p| p.into_inner());
        fc.toggle();
        fc.current()
    };
    let s = focus_str(now);
    let _ = app.emit("focus-changed", FocusChanged { focus: s });
    s
}

/// Ponte de log do frontend pro stdout do Rust (a webview transparente
/// esconde crashes de render).
#[tauri::command]
pub fn js_log(level: String, message: String) {
    match level.as_str() {
        "error" => log::error!("[js] {message}"),
        "warn" => log::warn!("[js] {message}"),
        _ => log::info!("[js] {message}"),
    }
}

#[tauri::command]
pub fn current_focus(state: State<'_, AppState>) -> &'static str {
    use domain::focus::FocusManager;
    focus_str(state.focus.lock().unwrap().current())
}

#[tauri::command]
pub fn toggle_focus(app: AppHandle) -> Result<&'static str, String> {
    Ok(toggle_and_emit(&app))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedGame {
    base_width: u32,
    base_height: u32,
    fps: f64,
    sample_rate: f64,
    /// Proporção de exibição pedida pelo core (ex: ~1.306 pra SNES 4:3). `0`
    /// = usar `base_width/base_height`.
    aspect_ratio: f32,
}

#[tauri::command]
pub async fn load_game(
    app: AppHandle,
    core_id: String,
    rom_path: String,
    rom_id: Option<String>,
) -> Result<LoadedGame, String> {
    use domain::core_options::CoreOptionsStore;
    let state = app.state::<AppState>();
    let session = Arc::clone(&state.session);

    // Alimenta o core com os valores de opção salvos ANTES de ele carregar
    // (ele pede via `GET_VARIABLE` já durante o load).
    if let Some(pool) = state.db.clone() {
        if let Ok(vals) = db::CoreOptionsRepo::new(pool).values_for(&core_id).await {
            emu_session::set_pending_core_option_values(vals);
        }
    }

    // O load é bloqueante (dlopen + retro_init); tira da thread async.
    let av = {
        let core_id = core_id.clone();
        tauri::async_runtime::spawn_blocking(move || session.load(&core_id, &rom_path))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
    };

    // Persiste o schema que o core declarou (repopula todo load). Antes,
    // garante a linha em `installed_cores` — `core_options_schema.core_id` tem
    // FK pra ela, e a descoberta de cores (varredura de disco) não registra
    // nada sozinha.
    if let Some(pool) = state.db.clone() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let cores = db::InstalledCoresRepo::new(pool.clone());
        if matches!(cores.get(&core_id).await, Ok(None)) {
            let version = emu_session::discover_cores(&state.cores_dir)
                .into_iter()
                .find(|c| c.core_id == core_id)
                .map(|c| c.library_version)
                .unwrap_or_default();
            if let Err(e) = cores
                .register(&domain::core_loader::InstalledCore {
                    core_id: core_id.clone(),
                    version,
                    installed_at: now,
                    render_requirements: None,
                })
                .await
            {
                log::warn!("registrando core em installed_cores: {e}");
            }
        }

        let schema = emu_session::core_options();
        if let Err(e) = db::CoreOptionsRepo::new(pool)
            .replace_schema(&core_id, &schema)
            .await
        {
            log::warn!("salvando schema de core options: {e}");
        }
    }

    // "Continuar jogando" — marca a hora do load.
    if let (Some(pool), Some(rid)) = (state.db.clone(), rom_id.as_deref()) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let _ = db::RomsRepo::new(pool).mark_played(rid, now).await;
    }

    // Shader atribuído (rom → sistema → default, ou `plain`); depois a
    // decoração — pulada se o shader já desenha a própria moldura.
    let shader_has_bezel = apply_resolved_shader(&state, rom_id.as_deref()).await;
    apply_resolved_decoration(&state, rom_id.as_deref(), shader_has_bezel).await;

    let display_aspect = {
        let guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        guard
            .as_ref()
            .and_then(|fp| fp.decoration_aspect())
            .unwrap_or(av.geometry.aspect_ratio)
    };

    // Guarda o `rom_id` (do DB) pro QuickSave/QuickLoad; `None` se o jogo veio
    // de fora da biblioteca.
    *state.current_rom.lock().unwrap_or_else(|p| p.into_inner()) = rom_id;
    Ok(LoadedGame {
        base_width: av.geometry.base_width,
        base_height: av.geometry.base_height,
        fps: av.timing.fps,
        sample_rate: av.timing.sample_rate,
        aspect_ratio: display_aspect,
    })
}

/// `system_id` de uma rom (ou `""` sem rom).
async fn system_of(pool: &db::Db, rom_id: Option<&str>) -> String {
    match rom_id {
        Some(rid) => db::RomsRepo::new(pool.clone())
            .get(rid)
            .await
            .ok()
            .flatten()
            .map(|r| r.system_id)
            .unwrap_or_default(),
        None => String::new(),
    }
}

/// Resolve o shader do jogo e aplica no `FrameProcessor`. Devolve se o preset
/// ativo desenha a própria moldura (`includes_bezel`) — pra exclusão mútua.
async fn apply_resolved_shader(state: &AppState, rom_id: Option<&str>) -> bool {
    apply_resolved_shader_ex(state, rom_id, false).await
}

/// Resolve o shader do jogo (cascata rom→sistema→default), aplica o preset e os
/// overrides de parâmetro no processador GPU. `force` recarrega o preset mesmo
/// se já for o ativo (usado ao "restaurar padrões" — o `set_preset` zera os
/// params antes dos overrides). Devolve `includes_bezel` (exclusão mútua).
async fn apply_resolved_shader_ex(state: &AppState, rom_id: Option<&str>, force: bool) -> bool {
    use domain::shader_chain::ShaderChainResolver;
    let Some(pool) = state.db.clone() else {
        return false;
    };
    let system = system_of(&pool, rom_id).await;
    let sc = db::ShaderChainRepo::new(pool);
    let (target, has_bezel, overrides) = match sc.resolve(&system, rom_id).await {
        Ok(Some(a)) => {
            let (src, bezel) = sc
                .list_presets()
                .await
                .ok()
                .and_then(|ps| ps.into_iter().find(|p| p.id == a.preset_id))
                .map(|p| (p.source_path, p.includes_bezel))
                .unwrap_or_else(|| ("plain".into(), false));
            (src, bezel, a.parameter_overrides)
        }
        _ => ("plain".into(), false, Default::default()),
    };
    let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(fp) = guard.as_mut() {
        if force || fp.preset_source() != target {
            if let Err(e) = fp.set_preset(&target) {
                log::warn!("shader do jogo: {e}");
            }
        }
        for (k, v) in &overrides {
            if let Ok(val) = v.parse::<f32>() {
                fp.set_shader_param(k, val);
            }
        }
    }
    has_bezel
}

/// Resolve a decoração do jogo (`DecorationRepo`) e aplica no `FrameProcessor`.
/// Pulada quando `shader_has_bezel` (exclusão mútua Mega Bezel × decoração).
async fn apply_resolved_decoration(state: &AppState, rom_id: Option<&str>, shader_has_bezel: bool) {
    // Override de teste: REEMU_BEZEL=/caminho/bezel.png
    let want = if let Some(p) = std::env::var_os("REEMU_BEZEL") {
        let path = std::path::PathBuf::from(p);
        crate::decoration::decode_png(&path)
            .inspect_err(|e| log::warn!("REEMU_BEZEL: {e}"))
            .ok()
            .map(|(rgba, w, h)| {
                let vp =
                    library_scan::viewport_for_image(&path).map(|v| crate::gpu::DecoViewport {
                        x: v.x as f32,
                        y: v.y as f32,
                        w: v.w as f32,
                        h: v.h as f32,
                    });
                log::info!("decoração: REEMU_BEZEL {} ({w}x{h})", path.display());
                (rgba, w, h, vp)
            })
    } else if shader_has_bezel {
        log::info!("decoração: pulada (shader ativo já desenha moldura)");
        None
    } else {
        resolve_decoration(state, rom_id).await
    };
    if want.is_none() {
        log::info!("decoração: nenhuma pra este jogo");
    }
    let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(fp) = guard.as_mut() {
        fp.set_decoration(want);
    } else {
        log::warn!("decoração: sem GPU");
    }
}

async fn resolve_decoration(
    state: &AppState,
    rom_id: Option<&str>,
) -> Option<(Vec<u8>, u32, u32, Option<crate::gpu::DecoViewport>)> {
    use domain::decoration::DecorationResolver;
    let pool = state.db.clone()?;
    let system = system_of(&pool, rom_id).await;
    let a = match db::DecorationRepo::new(pool).resolve(&system, rom_id).await {
        Ok(Some(a)) => a,
        Ok(None) => {
            log::info!("decoração: sem atribuição (sistema '{system}', rom {rom_id:?})");
            return None;
        }
        Err(e) => {
            log::warn!("decoração: erro ao resolver: {e}");
            return None;
        }
    };
    log::info!("decoração: atribuição {:?} → {}", a.scope, a.asset_path);
    let path = std::path::Path::new(&a.asset_path);
    let (rgba, w, h) = crate::decoration::decode_png(path)
        .inspect_err(|e| log::warn!("decoração {}: {e}", a.asset_path))
        .ok()?;
    let vp = library_scan::viewport_for_image(path).map(|v| crate::gpu::DecoViewport {
        x: v.x as f32,
        y: v.y as f32,
        w: v.w as f32,
        h: v.h as f32,
    });
    log::info!(
        "decoração: {} ({w}x{h}){}",
        a.asset_path,
        if vp.is_some() { " +viewport" } else { "" }
    );
    Some((rgba, w, h, vp))
}

#[tauri::command]
pub async fn import_decoration_pack(
    state: State<'_, AppState>,
    path: String,
) -> Result<usize, String> {
    let pool = pool(&state)?;
    crate::decoration::import_pack(&pool, std::path::Path::new(&path)).await
}

/// Remove todas as decorações (packs + atribuições) e tira a moldura ativa.
#[tauri::command]
pub async fn clear_decorations(state: State<'_, AppState>) -> Result<(), String> {
    use domain::decoration::DecorationStore;
    if let Some(pool) = state.db.clone() {
        db::DecorationRepo::new(pool)
            .clear_all()
            .await
            .map_err(|e| e.to_string())?;
    }
    let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(fp) = guard.as_mut() {
        fp.set_decoration(None);
    }
    Ok(())
}

#[tauri::command]
pub async fn unload_game(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let session = Arc::clone(&state.session);
    *state.current_rom.lock().unwrap_or_else(|p| p.into_inner()) = None;
    tauri::async_runtime::spawn_blocking(move || session.unload())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Frame mais recente do core como RGBA8, prefixado por
/// `[width: u32 LE][height: u32 LE]` (8 bytes). Corpo vazio = sem frame novo.
/// A `PlayScreen` consome num loop de `requestAnimationFrame` e pinta no canvas.
#[tauri::command]
pub fn poll_frame(state: State<'_, AppState>) -> tauri::ipc::Response {
    use domain::frame_source::{rotate_rgba, to_rgba8, FrameOrigin};
    let Some(frame) = state.session.take_latest_frame() else {
        return tauri::ipc::Response::new(Vec::new());
    };
    let rot = frame.metadata.rotation_degrees;

    // Caminho GPU (etapa 04 — shader chain). Cai no CPU em qualquer falha.
    {
        let mut gpu = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(fp) = gpu.as_mut() {
            if let Some((w, h, rgba)) = fp.process(&frame) {
                let (rgba, w, h) = rotate_rgba(&rgba, w, h, rot);
                cache_thumb_frame(&state, w, h, &rgba);
                return pack_frame(w, h, &rgba);
            }
        }
    }

    let FrameOrigin::SoftwareRawBuffer {
        data,
        pitch,
        format,
    } = frame.origin
    else {
        return tauri::ipc::Response::new(Vec::new());
    };
    let (w, h) = (frame.metadata.native_width, frame.metadata.native_height);
    let (rgba, w, h) = rotate_rgba(&to_rgba8(&data, w, h, pitch, format), w, h, rot);
    cache_thumb_frame(&state, w, h, &rgba);
    pack_frame(w, h, &rgba)
}

/// Guarda uma cópia do frame pra thumbnail do save state — no máximo 1×/500ms
/// (o clone de um frame grande custa; a frescura do thumb não precisa de 60fps).
fn cache_thumb_frame(state: &AppState, w: u32, h: u32, rgba: &[u8]) {
    let mut slot = state.last_frame.lock().unwrap_or_else(|p| p.into_inner());
    let stale = slot
        .as_ref()
        .map_or(true, |c| c.at.elapsed().as_millis() >= 500);
    if stale {
        *slot = Some(CachedFrame {
            w,
            h,
            rgba: rgba.to_vec(),
            at: std::time::Instant::now(),
        });
    }
}

/// `[w u32 LE][h u32 LE][rgba8…]` — o formato que a `PlayScreen` espera.
fn pack_frame(w: u32, h: u32, rgba: &[u8]) -> tauri::ipc::Response {
    let mut out = Vec::with_capacity(8 + rgba.len());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(rgba);
    tauri::ipc::Response::new(out)
}

/// Fecha o app. O flush final da save RAM acontece no handler de
/// `ExitRequested` (`lib.rs`), que cobre também o X da janela / Alt+F4.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn is_fullscreen(window: tauri::WebviewWindow) -> bool {
    window.is_fullscreen().unwrap_or(false)
}

#[tauri::command]
pub fn set_fullscreen(window: tauri::WebviewWindow, value: bool) -> Result<(), String> {
    window.set_fullscreen(value).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderInfo {
    /// Preset ativo (`plain` quando não há GPU).
    pub active: String,
    pub available: Vec<String>,
    /// `false` = sem adapter wgpu; a troca de preset não tem efeito.
    pub gpu: bool,
}

#[tauri::command]
pub fn get_shader_info(state: State<'_, AppState>) -> ShaderInfo {
    let guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    match guard.as_ref() {
        Some(fp) => ShaderInfo {
            active: fp.preset_name().to_string(),
            available: crate::gpu::builtin_preset_names(),
            gpu: true,
        },
        None => ShaderInfo {
            active: "plain".into(),
            available: crate::gpu::builtin_preset_names(),
            gpu: false,
        },
    }
}

/// Registra os presets embutidos (`plain`/`crt`/`lcd`) em `shader_presets` no
/// startup — `shader_chain_assignments.preset_id` tem FK pra essa tabela.
pub async fn seed_builtin_shader_presets(pool: &db::Db) {
    let sc = db::ShaderChainRepo::new(pool.clone());
    for name in crate::gpu::builtin_preset_names() {
        let p = domain::shader_chain::ShaderPreset {
            id: name.clone(),
            name: name.clone(),
            source_path: name.clone(),
            format: domain::shader_chain::ShaderFormat::Slang,
            is_builtin: true,
            includes_bezel: false,
        };
        if let Err(e) = sc.upsert_preset(&p).await {
            log::warn!("seed do preset '{name}': {e}");
        }
    }
}

/// `builtin:<nome>` ou o próprio caminho `.slangp` (o que `set_preset` aceita).
fn preset_id_of(name: &str) -> (String, String, bool) {
    if name.ends_with(".slangp") {
        let stem = std::path::Path::new(name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("preset")
            .to_string();
        (name.to_string(), stem, false)
    } else {
        (name.to_string(), name.to_string(), true)
    }
}

/// Aplica `name` no processador GPU agora e, se `scope` for dado, persiste:
/// `scope = "default"` (todos os jogos) ou `"rom"` (só `rom_id`; `name` vazio
/// = limpar a atribuição desse jogo).
#[tauri::command]
pub async fn set_shader(
    state: State<'_, AppState>,
    name: String,
    scope: Option<String>,
    rom_id: Option<String>,
) -> Result<(), String> {
    let clearing = name.is_empty();
    if !clearing {
        let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        let Some(fp) = guard.as_mut() else {
            return Err("sem GPU — shader indisponível".into());
        };
        fp.set_preset(&name).inspect_err(|e| log::warn!("{e}"))?;
    }

    let Some(scope) = scope else { return Ok(()) };
    let sc = db::ShaderChainRepo::new(pool(&state)?);
    match scope.as_str() {
        "rom" if clearing => {
            let rid = rom_id.ok_or("scope 'rom' precisa de rom_id")?;
            sc.clear_assignment(AssignmentScope::Rom, None, Some(&rid))
                .await
                .map_err(|e| e.to_string())
        }
        "default" | "rom" => {
            let (id, pname, is_builtin) = preset_id_of(&name);
            // heurística: presets com "bezel" no nome já desenham a moldura
            // → o DecorationResolver é pulado (exclusão mútua).
            let includes_bezel = name.to_lowercase().contains("bezel");
            sc.upsert_preset(&domain::shader_chain::ShaderPreset {
                id: id.clone(),
                name: pname,
                source_path: name.clone(),
                format: domain::shader_chain::ShaderFormat::Slang,
                is_builtin,
                includes_bezel,
            })
            .await
            .map_err(|e| e.to_string())?;
            if scope == "default" {
                sc.set_assignment(AssignmentScope::Default, None, None, &id)
                    .await
            } else {
                let rid = rom_id.ok_or("scope 'rom' precisa de rom_id")?;
                sc.set_assignment(AssignmentScope::Rom, None, Some(&rid), &id)
                    .await
            }
            .map_err(|e| e.to_string())
        }
        other => Err(format!("scope desconhecido '{other}'")),
    }
}

/// Preset de shader resolvido pra um jogo (rom → sistema → default). `None` =
/// nenhum (usa `plain`). `from_rom` diz se veio de uma atribuição do próprio
/// jogo (pra UI de "Shader deste jogo").
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomShader {
    pub source_path: Option<String>,
    pub from_rom: bool,
}

#[tauri::command]
pub async fn get_rom_shader(
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<RomShader, String> {
    use domain::shader_chain::ShaderChainResolver;
    let pool = pool(&state)?;
    let system = db::RomsRepo::new(pool.clone())
        .get(&rom_id)
        .await
        .map_err(|e| e.to_string())?
        .map(|r| r.system_id)
        .unwrap_or_default();
    let sc = db::ShaderChainRepo::new(pool);
    let Some(assign) = sc
        .resolve(&system, Some(&rom_id))
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(RomShader {
            source_path: None,
            from_rom: false,
        });
    };
    let src = sc
        .list_presets()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|p| p.id == assign.preset_id)
        .map(|p| p.source_path);
    Ok(RomShader {
        source_path: src,
        from_rom: assign.scope == AssignmentScope::Rom,
    })
}

/// Um parâmetro ajustável do shader ativo (`#pragma parameter NAME "label"
/// default min max step`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderParamDto {
    pub name: String,
    pub label: String,
    pub value: f32,
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// Parâmetros do shader carregado agora no processador GPU. Vazio pros builtins
/// (`plain`/`crt`/`lcd`) e quando não há GPU.
#[tauri::command]
pub fn get_shader_params(state: State<'_, AppState>) -> Vec<ShaderParamDto> {
    let guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
    let Some(fp) = guard.as_ref() else {
        return Vec::new();
    };
    fp.shader_param_meta()
        .iter()
        .map(|m| ShaderParamDto {
            value: fp.shader_param_value(&m.name).unwrap_or(m.default),
            name: m.name.clone(),
            label: m.label.clone(),
            default: m.default,
            min: m.min,
            max: m.max,
            step: m.step,
        })
        .collect()
}

/// Ajusta um parâmetro do shader agora e, se `scope` for dado, persiste
/// (`"default"` ou `"rom"` + `rom_id`). O escopo precisa já ter um shader
/// atribuído (a UI garante isso antes de mostrar os controles).
#[tauri::command]
pub async fn set_shader_param(
    state: State<'_, AppState>,
    name: String,
    value: f32,
    scope: Option<String>,
    rom_id: Option<String>,
) -> Result<(), String> {
    {
        let mut guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(fp) = guard.as_mut() {
            fp.set_shader_param(&name, value);
        }
    }
    let Some(scope) = scope else { return Ok(()) };
    let sc = db::ShaderChainRepo::new(pool(&state)?);
    let v = value.to_string();
    match scope.as_str() {
        "default" => sc
            .set_parameter_override(AssignmentScope::Default, None, None, &name, &v)
            .await
            .map_err(|e| e.to_string()),
        "rom" => {
            let rid = rom_id.ok_or("scope 'rom' precisa de rom_id")?;
            sc.set_parameter_override(AssignmentScope::Rom, None, Some(&rid), &name, &v)
                .await
                .map_err(|e| e.to_string())
        }
        other => Err(format!("scope desconhecido '{other}'")),
    }
}

/// Volta os parâmetros do shader pros defaults do preset: limpa os overrides
/// persistidos do escopo e recarrega o preset no processador GPU.
#[tauri::command]
pub async fn reset_shader_params(
    state: State<'_, AppState>,
    scope: Option<String>,
    rom_id: Option<String>,
) -> Result<(), String> {
    if let Some(scope) = &scope {
        let sc = db::ShaderChainRepo::new(pool(&state)?);
        let r = match scope.as_str() {
            "default" => {
                sc.clear_parameter_overrides(AssignmentScope::Default, None, None)
                    .await
            }
            "rom" => {
                let rid = rom_id.as_deref().ok_or("scope 'rom' precisa de rom_id")?;
                sc.clear_parameter_overrides(AssignmentScope::Rom, None, Some(rid))
                    .await
            }
            other => return Err(format!("scope desconhecido '{other}'")),
        };
        r.map_err(|e| e.to_string())?;
    }
    apply_resolved_shader_ex(&state, rom_id.as_deref(), true).await;
    Ok(())
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
    let cfg: AudioConfig = config.into();
    let repo = db::AudioConfigRepo::new(pool(&state)?);
    repo.update(&cfg).await.map_err(|e| e.to_string())?;

    // Aplica ao vivo: recria o sink na thread do core com a config nova.
    let cfg2 = cfg.clone();
    let session = state.session.clone();
    let r = tauri::async_runtime::spawn_blocking(move || {
        session.reload_audio(Box::new(move || {
            match audio_desktop::CpalAudioSink::new(&cfg2) {
                Ok(s) => Some(Box::new(s) as _),
                Err(e) => {
                    log::error!("áudio indisponível ({e}) — rodando sem som");
                    None
                }
            }
        }))
    })
    .await
    .map_err(|e| e.to_string())?;
    r.map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCoreDto {
    pub core_id: String,
    /// Nome amigável do core (`retro_system_info.library_name`).
    pub name: String,
    pub version: String,
    /// Extensões de ROM aceitas, sem ponto.
    pub extensions: Vec<String>,
    /// Backend de render detectado num load anterior (`installed_cores`, se houver).
    pub render_backend: Option<String>,
}

/// Cores disponíveis: varre `<dados>/cores/*_libretro.<suf>` e cruza com a
/// tabela `installed_cores` (que guarda o backend de render detectado no
/// primeiro load).
#[tauri::command]
pub async fn list_installed_cores(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledCoreDto>, String> {
    let discovered = emu_session::discover_cores(&state.cores_dir);

    let backends: std::collections::HashMap<String, Option<String>> = match state.db.as_ref() {
        Some(pool) => db::InstalledCoresRepo::new(pool.clone())
            .list()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|c| {
                (
                    c.core_id,
                    c.render_requirements
                        .map(|r| format!("{:?}", r.render_backend)),
                )
            })
            .collect(),
        None => Default::default(),
    };

    Ok(discovered
        .into_iter()
        .map(|c| InstalledCoreDto {
            name: if c.library_name.is_empty() {
                c.core_id.clone()
            } else {
                c.library_name
            },
            version: c.library_version,
            extensions: c.valid_extensions,
            render_backend: backends.get(&c.core_id).cloned().flatten(),
            core_id: c.core_id,
        })
        .collect())
}

// --- catálogo de cores (etapa 10, MVP) --------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCoreDto {
    pub core_id: String,
    pub name: String,
    pub systems: String,
    pub license: String,
    pub installed: bool,
    /// `"software"` = roda hoje; `"opengl"` = precisa de contexto GL (baixa,
    /// mas `load_game` recusa até a etapa 02 passo 4).
    pub hw: &'static str,
}

#[tauri::command]
pub fn list_core_catalog(state: State<'_, AppState>) -> Vec<CatalogCoreDto> {
    let installed: std::collections::HashSet<String> =
        emu_session::discover_cores(&state.cores_dir)
            .into_iter()
            .map(|c| c.core_id)
            .collect();
    crate::core_catalog::CATALOG
        .iter()
        .map(|e| CatalogCoreDto {
            core_id: e.id.to_string(),
            name: e.name.to_string(),
            systems: e.systems.to_string(),
            license: e.license.to_string(),
            installed: installed.contains(e.id),
            hw: e.hw.as_str(),
        })
        .collect()
}

#[tauri::command]
pub async fn download_core(state: State<'_, AppState>, core_id: String) -> Result<(), String> {
    let dir = state.cores_dir.clone();
    crate::core_catalog::download(&dir, &core_id).await?;
    log::info!("core instalado: {core_id}");
    Ok(())
}

#[tauri::command]
pub fn remove_core(state: State<'_, AppState>, core_id: String) -> Result<(), String> {
    crate::core_catalog::remove(&state.cores_dir, &core_id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreOptionDto {
    pub key: String,
    pub display_name: String,
    /// Escolhas possíveis (as opções libretro são sempre enumeradas).
    pub choices: Vec<String>,
    pub default_value: String,
    pub value: String,
}

/// Schema + valor atual das core options. Se um core está carregado agora e é
/// esse `core_id`, lê do core (fonte da verdade em runtime); senão, do DB.
#[tauri::command]
pub async fn get_core_options(
    state: State<'_, AppState>,
    core_id: String,
) -> Result<Vec<CoreOptionDto>, String> {
    use domain::core_options::{CoreOptionType, CoreOptionsStore};

    let live_matches = state.session.loaded_core().as_deref() == Some(core_id.as_str());
    let (defs, values) = if live_matches {
        (
            emu_session::core_options(),
            emu_session::core_option_values(),
        )
    } else if let Some(pool) = state.db.clone() {
        let repo = db::CoreOptionsRepo::new(pool);
        let defs = repo.schema_for(&core_id).await.map_err(|e| e.to_string())?;
        let values = repo.values_for(&core_id).await.map_err(|e| e.to_string())?;
        (defs, values)
    } else {
        (Vec::new(), Default::default())
    };

    Ok(defs
        .into_iter()
        .map(|d| {
            let choices = match d.option_type {
                CoreOptionType::Combo { choices } => choices,
                CoreOptionType::Bool => vec!["disabled".into(), "enabled".into()],
                CoreOptionType::Range { .. } => Vec::new(),
            };
            let value = values
                .get(&d.option_key)
                .cloned()
                .unwrap_or_else(|| d.default_value.clone());
            CoreOptionDto {
                key: d.option_key,
                display_name: d.display_name,
                choices,
                default_value: d.default_value,
                value,
            }
        })
        .collect())
}

/// Troca uma core option. Aplica no core carregado (efeito no próximo frame)
/// e persiste no DB.
#[tauri::command]
pub async fn set_core_option(
    state: State<'_, AppState>,
    core_id: String,
    key: String,
    value: String,
) -> Result<(), String> {
    use domain::core_options::CoreOptionsStore;

    if state.session.loaded_core().as_deref() == Some(core_id.as_str())
        && !emu_session::set_core_option(&key, &value)
    {
        return Err("opção ou valor inválido pro core carregado".into());
    }
    if let Some(pool) = state.db.clone() {
        db::CoreOptionsRepo::new(pool)
            .set_value(&core_id, &key, &value)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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
    /// Boxart da libretro (o `<img>` tenta carregar; cai num placeholder se 404).
    pub boxart: Option<String>,
    /// Unix (s) do último load — pra "Continuar jogando". `None` = nunca.
    pub last_played_at: Option<i64>,
    /// Unix (s) de quando entrou na biblioteca — pra "Adicionados recentemente".
    pub added_at: i64,
}

#[tauri::command]
pub async fn list_roms(state: State<'_, AppState>) -> Result<Vec<RomDto>, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let roms = repo.list().await.map_err(|e| e.to_string())?;
    Ok(roms
        .into_iter()
        .map(|r| {
            let title = std::path::Path::new(&r.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&r.file_path)
                .to_string();
            RomDto {
                boxart: library_scan::libretro_boxart_url(&r.system_id, &title),
                title,
                id: r.id,
                system_id: r.system_id,
                file_path: r.file_path,
                last_played_at: r.last_played_at,
                added_at: r.added_at,
            }
        })
        .collect())
}

/// Remove a ROM da biblioteca (só o registro no banco — o arquivo em disco
/// fica; um novo scan a readiciona). Save states / metadata caem em cascata.
#[tauri::command]
pub async fn remove_rom(state: State<'_, AppState>, rom_id: String) -> Result<(), String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    repo.remove(&rom_id).await.map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomSourceDto {
    /// Pasta raiz (dois níveis acima do arquivo, ex.: `.../RetroBat/roms`).
    pub path: String,
    pub count: usize,
}

/// Agrupa as ROMs por pasta de origem (a "biblioteca" — dois níveis acima do
/// arquivo, o que casa com `roms/<sistema>/<jogo>`), pra oferecer remoção em
/// bloco. Ordenado por contagem desc.
#[tauri::command]
pub async fn list_rom_sources(state: State<'_, AppState>) -> Result<Vec<RomSourceDto>, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let roms = repo.list().await.map_err(|e| e.to_string())?;
    let mut by_dir: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for r in &roms {
        let p = std::path::Path::new(&r.file_path);
        let root = p
            .parent()
            .and_then(|d| d.parent())
            .or_else(|| p.parent())
            .unwrap_or(p);
        *by_dir
            .entry(root.to_string_lossy().into_owned())
            .or_default() += 1;
    }
    let mut out: Vec<RomSourceDto> = by_dir
        .into_iter()
        .map(|(path, count)| RomSourceDto { path, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.path.cmp(&b.path)));
    Ok(out)
}

/// Remove todas as ROMs de um sistema (snes, nes, …). Devolve a contagem.
#[tauri::command]
pub async fn remove_rom_system(
    state: State<'_, AppState>,
    system_id: String,
) -> Result<u64, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo
        .remove_by_system(&system_id)
        .await
        .map_err(|e| e.to_string())?;
    log::info!("biblioteca: {n} ROM(s) removida(s) do sistema {system_id}");
    Ok(n)
}

/// Remove todas as ROMs sob `path` (uma biblioteca inteira). Devolve a contagem.
#[tauri::command]
pub async fn remove_rom_source(state: State<'_, AppState>, path: String) -> Result<u64, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo
        .remove_under_dir(&path)
        .await
        .map_err(|e| e.to_string())?;
    log::info!("biblioteca: {n} ROM(s) removida(s) de {path}");
    Ok(n)
}

/// Esvazia a biblioteca inteira (todos os sistemas). Devolve a contagem.
#[tauri::command]
pub async fn clear_library(state: State<'_, AppState>) -> Result<u64, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let n = repo.remove_all().await.map_err(|e| e.to_string())?;
    log::info!("biblioteca: limpa ({n} ROM(s) removida(s))");
    Ok(n)
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

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressDto {
    pub current: usize,
    pub total: usize,
    pub file: String,
}

#[tauri::command]
pub async fn scan_library(
    state: State<'_, AppState>,
    path: String,
    on_progress: tauri::ipc::Channel<ScanProgressDto>,
) -> Result<ScanReportDto, String> {
    let repo = db::RomsRepo::new(pool(&state)?);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let r = library_scan::scan_into(&repo, std::path::Path::new(&path), now, |p| {
        let _ = on_progress.send(ScanProgressDto {
            current: p.current,
            total: p.total,
            file: p.file,
        });
    })
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

// --- metadata / scraping (etapa 09) -----------------------------------

use domain::metadata::{GameMetadata, MetadataConfig, MetadataRepository};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataConfigDto {
    pub provider: String,
    pub screenscraper_user: Option<String>,
    pub screenscraper_password: Option<String>,
}

#[tauri::command]
pub async fn get_metadata_config(state: State<'_, AppState>) -> Result<MetadataConfigDto, String> {
    let c = db::MetadataRepo::new(pool(&state)?)
        .get_config()
        .await
        .map_err(|e| e.to_string())?;
    Ok(MetadataConfigDto {
        provider: c.provider,
        screenscraper_user: c.screenscraper_user,
        screenscraper_password: c.screenscraper_password,
    })
}

#[tauri::command]
pub async fn set_metadata_config(
    state: State<'_, AppState>,
    config: MetadataConfigDto,
) -> Result<(), String> {
    let norm = |s: Option<String>| s.filter(|v| !v.trim().is_empty());
    db::MetadataRepo::new(pool(&state)?)
        .set_config(&MetadataConfig {
            provider: config.provider,
            screenscraper_user: norm(config.screenscraper_user),
            screenscraper_password: norm(config.screenscraper_password),
        })
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameMetadataDto {
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    pub provider_source: Option<String>,
}

impl From<GameMetadata> for GameMetadataDto {
    fn from(m: GameMetadata) -> Self {
        Self {
            title: m.title,
            description: m.description,
            cover_url: m.cover_url,
            release_date: m.release_date,
            genre: m.genre,
            provider_source: m.provider_source,
        }
    }
}

#[tauri::command]
pub async fn get_rom_metadata(
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<Option<GameMetadataDto>, String> {
    Ok(db::MetadataRepo::new(pool(&state)?)
        .get_metadata(&rom_id)
        .await
        .map_err(|e| e.to_string())?
        .map(Into::into))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingMatchDto {
    pub rom_id: String,
    pub file_stem: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
}

#[tauri::command]
pub async fn list_pending_matches(
    state: State<'_, AppState>,
) -> Result<Vec<PendingMatchDto>, String> {
    let list = db::MetadataRepo::new(pool(&state)?)
        .list_pending()
        .await
        .map_err(|e| e.to_string())?;
    Ok(list
        .into_iter()
        .map(|p| PendingMatchDto {
            rom_id: p.rom_id,
            file_stem: p.file_stem,
            title: p.candidate.title,
            description: p.candidate.description,
            cover_url: p.candidate.cover_url,
            release_date: p.candidate.release_date,
            genre: p.candidate.genre,
        })
        .collect())
}

#[tauri::command]
pub async fn resolve_pending_match(
    state: State<'_, AppState>,
    rom_id: String,
    accept: bool,
) -> Result<(), String> {
    db::MetadataRepo::new(pool(&state)?)
        .resolve_pending(&rom_id, accept)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeProgressDto {
    pub running: bool,
    pub done: usize,
    pub total: usize,
    pub auto: usize,
    pub pending: usize,
    pub failed: usize,
}

#[tauri::command]
pub fn metadata_scan_progress(state: State<'_, AppState>) -> ScrapeProgressDto {
    let (running, done, total, auto, pending, failed) = state.scrape.snapshot();
    ScrapeProgressDto {
        running,
        done,
        total,
        auto,
        pending,
        failed,
    }
}

#[tauri::command]
pub fn cancel_metadata_scan(state: State<'_, AppState>) {
    state
        .scrape_stop
        .store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Dispara uma leva de scraping em background e volta na hora. O progresso é
/// consultado por `metadata_scan_progress`.
#[tauri::command]
pub async fn start_metadata_scan(state: State<'_, AppState>) -> Result<(), String> {
    if state
        .scrape
        .running
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err("já tem um scraping em andamento".into());
    }
    let pool = pool(&state)?;
    let progress = state.scrape.clone();
    let stop = state.scrape_stop.clone();
    stop.store(false, std::sync::atomic::Ordering::Relaxed);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::scraping::scrape_pending(pool, progress.clone(), stop).await {
            log::warn!("metadata: leva falhou: {e}");
            progress
                .running
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
    });
    Ok(())
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
    pub has_thumbnail: bool,
}

fn save_dto(m: domain::save_state::SaveStateMetadata) -> SaveStateDto {
    SaveStateDto {
        id: m.id,
        slot: m.slot,
        created_at: m.created_at,
        file_path: m.file_path,
        has_thumbnail: m.thumbnail_path.is_some(),
    }
}

/// Reduz o frame RGBA8 pra no máximo `max_w` de largura (nearest) e codifica
/// como PNG — o thumbnail do save state.
fn thumbnail_png(w: u32, h: u32, rgba: &[u8], max_w: u32) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
        return None;
    }
    let scale = (w as f32 / max_w as f32).max(1.0);
    let (tw, th) = ((w as f32 / scale) as u32, (h as f32 / scale) as u32);
    let (tw, th) = (tw.max(1), th.max(1));
    let mut small = vec![0u8; (tw * th * 4) as usize];
    for y in 0..th {
        let sy = (y as f32 * scale) as u32;
        for x in 0..tw {
            let sx = (x as f32 * scale) as u32;
            let si = ((sy.min(h - 1) * w + sx.min(w - 1)) * 4) as usize;
            let di = ((y * tw + x) * 4) as usize;
            small[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, tw, th);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().ok()?.write_image_data(&small).ok()?;
    Some(out)
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
    let thumb = {
        let f = state.last_frame.lock().unwrap_or_else(|p| p.into_inner());
        f.as_ref()
            .and_then(|c| thumbnail_png(c.w, c.h, &c.rgba, 320))
    };
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let meta = save_svc::save(
        &repo,
        &state.save_dir,
        &rom_id,
        &core_id,
        slot,
        &bytes,
        thumb.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(save_dto(meta))
}

/// PNG do thumbnail de um save state (corpo vazio se não tem).
#[tauri::command]
pub async fn read_save_thumbnail(
    state: State<'_, AppState>,
    state_id: String,
) -> Result<tauri::ipc::Response, String> {
    use domain::save_state::SaveStateRepository;
    let repo = db::SaveStateRepo::new(pool(&state)?);
    let path = repo
        .get_state(&state_id)
        .await
        .map_err(|e| e.to_string())?
        .and_then(|m| m.thumbnail_path);
    let bytes = match path {
        Some(p) => std::fs::read(&p).unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(tauri::ipc::Response::new(bytes))
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

/// Recebe `KeyboardEvent.code` da webview (keydown/keyup).
///
/// - Em modo de captura de binding: o evento vai pro frontend, não pro jogo.
/// - `Escape` sempre alterna o menu (rede de segurança independente do DB).
/// - Qualquer tecla entra/sai do conjunto segurado (`input_desktop::held`) —
///   o loop de eventos resolve as hotkeys configuradas a partir dele, com
///   prioridade sobre o input de jogo.
/// - O mapa teclado→RetroPad só vale em `GameFocused`.
#[tauri::command]
pub fn input_key(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
    pressed: bool,
) -> Result<(), String> {
    use domain::focus::{FocusManager, InputFocus};

    // Modo de captura de binding: o evento vai pro frontend, não pro jogo.
    if input_desktop::capture::is_capturing() {
        if pressed && !code.is_empty() {
            let ev = domain::input::RawInputEvent::Keyboard {
                scancode: input_desktop::keymap::key_scancode(&code),
            };
            let _ = app.emit("raw-input-captured", &ev);
        }
        return Ok(());
    }

    if pressed && code == "Escape" {
        toggle_and_emit(&app);
        return Ok(());
    }

    // Conjunto segurado (resolução de hotkey de combinação no loop de eventos).
    if !code.is_empty() {
        let kev = domain::input::RawInputEvent::Keyboard {
            scancode: input_desktop::keymap::key_scancode(&code),
        };
        if pressed {
            input_desktop::held::press(kev);
        } else {
            input_desktop::held::release(&kev);
        }
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

// --- captura de binding (etapa 05) -----------------------------------

fn retropad_from_str(s: &str) -> Option<domain::input::RetroPadButton> {
    use domain::input::RetroPadButton::*;
    Some(match s {
        "A" => A,
        "B" => B,
        "X" => X,
        "Y" => Y,
        "L1" => L1,
        "L2" => L2,
        "L3" => L3,
        "R1" => R1,
        "R2" => R2,
        "R3" => R3,
        "Up" => Up,
        "Down" => Down,
        "Left" => Left,
        "Right" => Right,
        "Start" => Start,
        "Select" => Select,
        _ => return None,
    })
}

/// Entra em modo de captura: os próximos eventos brutos (teclado via
/// `input_key`, gamepad via a thread de `emu-session`) vão pro frontend por
/// `raw-input-captured` em vez de irem pro jogo.
#[tauri::command]
pub fn start_binding_capture() -> Result<(), String> {
    emu_session::retropad().clear();
    input_desktop::capture::begin();
    Ok(())
}

#[tauri::command]
pub fn cancel_binding_capture() -> Result<(), String> {
    input_desktop::capture::end();
    Ok(())
}

/// Grava a combinação capturada. `target`:
/// - `"system_hotkey"` → `target_key` = `SystemAction::as_wire()`
/// - `"controller_mapping"` → `target_key` = `"<guid>::<display_name>::<Botão>"`
#[tauri::command]
pub async fn save_binding(
    state: State<'_, AppState>,
    target: String,
    target_key: String,
    trigger: Vec<domain::input::RawInputEvent>,
) -> Result<(), String> {
    input_desktop::capture::end();
    if trigger.is_empty() {
        return Err("nenhum input capturado".into());
    }
    match target.as_str() {
        "system_hotkey" => {
            use domain::hotkeys::SystemHotkeyRepository;
            let action = SystemAction::from_wire(&target_key)
                .ok_or_else(|| format!("ação desconhecida: {target_key}"))?;
            let pool = pool(&state)?;
            db::SystemHotkeysRepo::new(pool.clone())
                .set(&HotkeyBinding {
                    action,
                    trigger,
                    device_guid: None,
                })
                .await
                .map_err(|e| e.to_string())?;
            refresh_hotkey_resolver(&state, &pool).await
        }
        "controller_mapping" => {
            use domain::input::{
                ControllerLayoutEntry, ControllerMapping, ControllerMappingRepository,
                MappingSource,
            };
            let mut parts = target_key.splitn(3, "::");
            let guid = parts.next().unwrap_or_default().to_string();
            let display_name = parts.next().unwrap_or("Controle").to_string();
            let button = parts
                .next()
                .and_then(retropad_from_str)
                .ok_or_else(|| format!("target_key inválido: {target_key}"))?;
            if guid.is_empty() {
                return Err("guid vazio".into());
            }
            let repo = db::ControllerMappingsRepo::new(pool(&state)?);
            let mut mapping =
                repo.get(&guid)
                    .await
                    .map_err(|e| e.to_string())?
                    .unwrap_or(ControllerMapping {
                        guid: guid.clone(),
                        display_name: display_name.clone(),
                        layout: Vec::new(),
                        source: MappingSource::UserOverride,
                    });
            mapping.display_name = display_name;
            mapping.source = MappingSource::UserOverride;
            mapping.layout.retain(|e| e.button != button);
            mapping
                .layout
                .push(ControllerLayoutEntry { trigger, button });
            repo.upsert(&mapping).await.map_err(|e| e.to_string())?;
            refresh_controller_mappings(&state).await
        }
        other => Err(format!("target desconhecido: {other}")),
    }
}

/// Relê `controller_mappings` do DB e publica no override global lido pela
/// thread de gamepad (`input_desktop::mappings`).
async fn refresh_controller_mappings(state: &AppState) -> Result<(), String> {
    use domain::input::ControllerMappingRepository;
    let list = db::ControllerMappingsRepo::new(pool(state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?;
    input_desktop::mappings::set(list);
    Ok(())
}

/// Carrega `controller_mappings` + `device_port_assignment` nos overrides
/// globais. Best-effort no startup.
pub async fn load_controller_mappings(pool: &db::Db) {
    use domain::input::{ControllerMappingRepository, DevicePortRepository};
    if let Ok(list) = db::ControllerMappingsRepo::new(pool.clone()).list().await {
        input_desktop::mappings::set(list);
    }
    if let Ok(ports) = db::DevicePortsRepo::new(pool.clone()).list().await {
        input_desktop::mappings::set_ports(
            ports.into_iter().map(|(g, p)| (g, p as usize)).collect(),
        );
    }
}

#[tauri::command]
pub async fn clear_controller_mapping(
    state: State<'_, AppState>,
    guid: String,
) -> Result<(), String> {
    use domain::input::ControllerMappingRepository;
    db::ControllerMappingsRepo::new(pool(&state)?)
        .delete(&guid)
        .await
        .map_err(|e| e.to_string())?;
    refresh_controller_mappings(&state).await
}

async fn refresh_device_ports(state: &AppState) -> Result<(), String> {
    use domain::input::DevicePortRepository;
    let ports = db::DevicePortsRepo::new(pool(state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?;
    input_desktop::mappings::set_ports(ports.into_iter().map(|(g, p)| (g, p as usize)).collect());
    Ok(())
}

#[tauri::command]
pub async fn set_device_port(
    state: State<'_, AppState>,
    guid: String,
    port: u8,
) -> Result<(), String> {
    use domain::input::DevicePortRepository;
    db::DevicePortsRepo::new(pool(&state)?)
        .set(&guid, port)
        .await
        .map_err(|e| e.to_string())?;
    refresh_device_ports(&state).await
}

#[tauri::command]
pub async fn clear_device_port(state: State<'_, AppState>, guid: String) -> Result<(), String> {
    use domain::input::DevicePortRepository;
    db::DevicePortsRepo::new(pool(&state)?)
        .clear(&guid)
        .await
        .map_err(|e| e.to_string())?;
    refresh_device_ports(&state).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePortDto {
    pub guid: String,
    pub port: u8,
}

#[tauri::command]
pub async fn list_device_ports(state: State<'_, AppState>) -> Result<Vec<DevicePortDto>, String> {
    use domain::input::DevicePortRepository;
    Ok(db::DevicePortsRepo::new(pool(&state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(guid, port)| DevicePortDto { guid, port })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamepadDto {
    pub guid: String,
    pub name: String,
}

#[tauri::command]
pub fn list_gamepads(state: State<'_, AppState>) -> Vec<GamepadDto> {
    state
        .session
        .connected_gamepads()
        .into_iter()
        .map(|(guid, name)| GamepadDto { guid, name })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyBindingDto {
    pub action: String,
    pub trigger: Vec<domain::input::RawInputEvent>,
}

#[tauri::command]
pub async fn list_system_hotkeys(
    state: State<'_, AppState>,
) -> Result<Vec<HotkeyBindingDto>, String> {
    use domain::hotkeys::SystemHotkeyRepository;
    let bindings = db::SystemHotkeysRepo::new(pool(&state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?;
    Ok(bindings
        .into_iter()
        .map(|b| HotkeyBindingDto {
            action: b.action.as_wire().to_string(),
            trigger: b.trigger,
        })
        .collect())
}

#[tauri::command]
pub async fn clear_system_hotkey(state: State<'_, AppState>, action: String) -> Result<(), String> {
    use domain::hotkeys::SystemHotkeyRepository;
    let action =
        SystemAction::from_wire(&action).ok_or_else(|| format!("ação desconhecida: {action}"))?;
    let pool = pool(&state)?;
    db::SystemHotkeysRepo::new(pool.clone())
        .delete(action)
        .await
        .map_err(|e| e.to_string())?;
    refresh_hotkey_resolver(&state, &pool).await
}

/// Relê `system_hotkeys` do DB e recompõe o `ComboHotkeyResolver` do `AppState`.
async fn refresh_hotkey_resolver(state: &AppState, pool: &db::Db) -> Result<(), String> {
    let bindings = load_system_hotkeys(pool).await.map_err(|e| e.to_string())?;
    *state.hotkeys.lock().unwrap_or_else(|p| p.into_inner()) = ComboHotkeyResolver::new(bindings);
    Ok(())
}

/// Carrega as hotkeys do DB, semeando o default (`ToggleMenuOverlay` = `F1`)
/// se a tabela estiver vazia. Usado no startup e por `refresh_hotkey_resolver`.
pub async fn load_system_hotkeys(pool: &db::Db) -> Result<Vec<HotkeyBinding>, db::DbError> {
    use domain::hotkeys::SystemHotkeyRepository;
    let repo = db::SystemHotkeysRepo::new(pool.clone());
    let existing = repo
        .list()
        .await
        .map_err(|e| db::DbError::Sqlite(e.to_string()))?;
    if existing.is_empty() {
        let default = HotkeyBinding {
            action: SystemAction::ToggleMenuOverlay,
            trigger: vec![domain::input::RawInputEvent::Keyboard {
                scancode: input_desktop::keymap::key_scancode("F1"),
            }],
            device_guid: None,
        };
        if repo.set(&default).await.is_ok() {
            return Ok(vec![default]);
        }
    }
    Ok(existing)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerEntryDto {
    pub button: String,
    pub trigger: Vec<domain::input::RawInputEvent>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerMappingDto {
    pub guid: String,
    pub display_name: String,
    pub source: String,
    pub entries: Vec<ControllerEntryDto>,
}

#[tauri::command]
pub async fn list_controller_mappings(
    state: State<'_, AppState>,
) -> Result<Vec<ControllerMappingDto>, String> {
    use domain::input::ControllerMappingRepository;
    let mappings = db::ControllerMappingsRepo::new(pool(&state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?;
    Ok(mappings
        .into_iter()
        .map(|m| ControllerMappingDto {
            guid: m.guid,
            display_name: m.display_name,
            source: m.source.as_wire().to_string(),
            entries: m
                .layout
                .into_iter()
                .map(|e| ControllerEntryDto {
                    button: format!("{:?}", e.button),
                    trigger: e.trigger,
                })
                .collect(),
        })
        .collect())
}
