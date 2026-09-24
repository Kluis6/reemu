//! Comandos Tauri expostos ao frontend + estado da aplicação.
//!
//! Escopo desta etapa: plumbing. O `EmuSession` roda o core numa thread
//! dedicada; o foco é decidido aqui (Rust) e propagado pro React via evento
//! `focus-changed`. A surface nativa de vídeo (que consome
//! `session.take_latest_frame()`) e o `AudioSink` entram nas etapas 03/06.

use crate::save_state as save_svc;
use crate::video::VideoSurface;
use domain::audio::{AudioConfig, AudioConfigRepository};
use domain::core_loader::InstalledCoreRepository;
use domain::hotkeys::{HotkeyBinding, SystemAction};
use domain::library::RomRepository;
use domain::shader_chain::{AssignmentScope, ShaderChainStore};
use domain::video::{VideoConfig, VideoConfigRepository};
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
    /// `<dados>/system` — onde os cores procuram BIOS (`GET_SYSTEM_DIRECTORY`).
    pub system_dir: std::path::PathBuf,
    /// `<dados>/shaders` — pacote de shaders baixado (`libretro/slang-shaders`).
    pub shaders_dir: std::path::PathBuf,
    /// `<dados>/decorations` — bezels baixados (The Bezel Project) / importados.
    pub decorations_dir: std::path::PathBuf,
    /// `<dados>/profile` — imagem de avatar escolhida pelo usuário.
    pub profile_dir: std::path::PathBuf,
    /// `<dados>/appearance` — papel de parede da tela inicial escolhido pelo
    /// usuário (mesmo padrão do avatar: presença do arquivo = estado).
    pub appearance_dir: std::path::PathBuf,
    /// `<dados>/covers` — cache local das capas baixadas da libretro (uma
    /// vez por ROM, servido pelo protocolo `cover://` em `covers.rs`).
    pub covers_dir: std::path::PathBuf,
    /// Hotkeys de sistema carregadas do DB (`system_hotkeys`). `save_binding` /
    /// `clear_system_hotkey` recompõem via `refresh_hotkey_resolver`.
    pub hotkeys: Mutex<ComboHotkeyResolver>,
    /// Última ação de hotkey resolvida — pra disparar uma vez por "aperto"
    /// (o loop de eventos consulta o conjunto segurado a cada frame).
    pub last_hotkey: Mutex<Option<SystemAction>>,
    /// `rom_id` do jogo carregado agora (o path vai pro core, o id vai pro DB).
    /// `None` quando ocioso. Usado pelo QuickSave/QuickLoad das hotkeys.
    pub current_rom: Mutex<Option<String>>,
    /// Tempo de jogo ainda não gravado da ROM atual (`play_clock.rs`).
    pub play_clock: Mutex<crate::play_clock::PlayClock>,
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
    /// Coreografia jogo↔menu no modo de vídeo nativo (subsurface). Só usado
    /// quando `video` é `Some`.
    pub video_menu: Mutex<VideoMenu>,
    /// Último frame capturado da surface nativa quando o menu abriu — vira o
    /// fundo do menu de pausa (`pause_background`).
    pub pause_bg: Mutex<Option<(u32, u32, Vec<u8>)>>,
    /// Geometria `(x, y, w, h)` pedida pra subsurface no último `Resized`. O
    /// `reemu-video-pump` aplica — ele é o dono único da conexão Wayland (mexer
    /// nela de outra thread corrompe o `wl_display`).
    pub pending_surface_geom: Mutex<Option<(i32, i32, u32, u32)>>,
    /// Etapa 12 §Beetle: um `FrameProcessor` reconstruído no `VkDevice` de um
    /// core que criou o device ele mesmo. O negociador (numa thread do
    /// `emu-session`) deixa aqui; o `reemu-video-pump` faz a troca (drop do
    /// antigo + `attach_surface` do novo) — só ele pode tocar Wayland/wgpu.
    pub pending_gpu: Mutex<Option<crate::gpu::FrameProcessor>>,
    /// Handles da surface nativa pra o pump reanexar no `pending_gpu`.
    pub vk_reattach: Mutex<Option<(crate::gpu::SendHandles, u32, u32)>>,
    /// `true` do início de um `load_game` até ele terminar: o `reemu-video-pump`
    /// mantém a subsurface ESCONDIDA nesse meio-tempo (senão o último frame do
    /// jogo anterior fica "grudado" no `wl_surface` por cima da tela de
    /// "Carregando…"). Fecha a janela de corrida entre navegar e o pump ver
    /// `Idle`.
    pub loading_game: std::sync::atomic::AtomicBool,
}

/// Estado da transição jogo↔menu no vídeo nativo. O `reemu-video-pump` dirige.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VideoMenu {
    /// Jogo rodando — a subsurface apresenta os frames normalmente.
    Playing,
    /// Menu pedido — `n` ticks (~15ms cada) até capturar o frame e esconder a
    /// subsurface (deixa o último frame chegar fresco).
    Opening(u8),
    /// Menu aberto — subsurface escondida, a webview opaca mostra o menu.
    MenuUp,
    /// Menu fechando — `n` ticks segurando a subsurface escondida pra a
    /// animação de saída da webview terminar antes do jogo voltar por cima.
    Closing(u8),
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
        let system_dir = base.join("system");
        let shaders_dir = base.join("shaders");
        crate::gpu::set_shader_root(crate::shader_pack::install_dir(&shaders_dir));
        let decorations_dir = base.join("decorations");
        let profile_dir = base.join("profile");
        let appearance_dir = base.join("appearance");
        let covers_dir = base.join("covers");
        let mut cfg = SessionConfig::new(cores_dir.clone(), system_dir.clone(), save_dir.clone());
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
            system_dir,
            shaders_dir,
            decorations_dir,
            profile_dir,
            appearance_dir,
            covers_dir,
            hotkeys: Mutex::new(ComboHotkeyResolver::new(hotkeys)),
            last_hotkey: Mutex::new(None),
            current_rom: Mutex::new(None),
            play_clock: Mutex::new(Default::default()),
            gpu: Mutex::new(None),
            scrape: Arc::new(crate::scraping::ScrapeProgress::default()),
            scrape_stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            last_frame: Mutex::new(None),
            video_menu: Mutex::new(VideoMenu::Playing),
            pause_bg: Mutex::new(None),
            pending_surface_geom: Mutex::new(None),
            pending_gpu: Mutex::new(None),
            vk_reattach: Mutex::new(None),
            loading_game: std::sync::atomic::AtomicBool::new(false),
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
            crate::play_clock::total(&state, &rom_id).await,
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
    use domain::focus::{FocusManager, InputFocus};
    let state = app.state::<AppState>();
    let now = {
        let mut fc = state.focus.lock().unwrap_or_else(|p| p.into_inner());
        fc.toggle();
        fc.current()
    };
    // Vídeo nativo: dispara a coreografia da subsurface (capturar + esconder ao
    // abrir o menu; segurar escondida na saída). No modo canvas é inócuo.
    {
        let mut vm = state.video_menu.lock().unwrap_or_else(|p| p.into_inner());
        *vm = match now {
            InputFocus::MenuFocused => VideoMenu::Opening(2),
            InputFocus::GameFocused => VideoMenu::Closing(10),
        };
    }
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

fn pool(state: &AppState) -> Result<db::Db, String> {
    state
        .db
        .clone()
        .ok_or_else(|| "banco de dados indisponível".to_string())
}

mod bios;
mod cores;
mod game;
mod input;
mod library;
mod metadata;
mod save_states;
mod settings;
mod shaders;

pub use bios::*;
pub use cores::*;
pub use game::*;
pub use input::*;
pub use library::*;
pub use metadata::*;
pub use save_states::*;
pub use settings::*;
pub use shaders::*;
