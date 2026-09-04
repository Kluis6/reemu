mod commands;
mod core_catalog;
mod decoration;
mod gpu;
mod scraping;
mod video;

pub mod save_state;

use commands::AppState;
use domain::audio::AudioConfigRepository as _;
use tauri::{Emitter, Manager};

/// Flag por env var com um default: ligada, salvo `KEY=0|false|off|no`.
fn env_flag(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => default,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let base = data_dir(app.handle());
            log::info!("dados: {}", base.display());
            let db = open_db(&base);
            let audio_config = db
                .as_ref()
                .and_then(|pool| {
                    tauri::async_runtime::block_on(db::AudioConfigRepo::new(pool.clone()).get())
                        .ok()
                })
                .unwrap_or_default();
            let hotkeys = db
                .as_ref()
                .and_then(|pool| {
                    tauri::async_runtime::block_on(commands::load_system_hotkeys(pool)).ok()
                })
                .unwrap_or_default();
            if let Some(pool) = db.as_ref() {
                tauri::async_runtime::block_on(commands::load_controller_mappings(pool));
                tauri::async_runtime::block_on(commands::seed_builtin_shader_presets(pool));
            }
            app.manage(AppState::new(base, db, audio_config, hotkeys));

            // Contexto GPU pro processamento de frame (etapa 04 — shader chain).
            // Headless: sem surface, não conflita com o GTK. Se não houver
            // adapter, o `poll_frame` segue no caminho CPU.
            match gpu::FrameProcessor::new() {
                Some(fp) => {
                    app.state::<AppState>().gpu.lock().unwrap().replace(fp);
                }
                None => log::warn!("sem GPU wgpu — frame do core vai cru pro canvas"),
            }

            // Ponte de input gamepad → frontend numa thread própria. O event
            // loop do Tauri fica em `Wait` quando a webview está ociosa (sem
            // animação), então `MainEventsCleared` NÃO tiquetaqueia no launcher
            // — era por isso que o controle não navegava os menus (só durante
            // o jogo, quando o `poll_frame` do canvas acorda o loop). Esta
            // thread roda sempre, ~60Hz.
            spawn_input_bridge(app.handle().clone());

            // Surface nativa de vídeo (wl_subsurface `place_above`) — padrão no
            // Linux/Wayland. `REEMU_NATIVE_VIDEO=0` volta pro `<canvas>` na
            // webview. Sem Wayland, `VideoSurface::spawn` devolve `None` e o
            // canvas assume sozinho.
            if env_flag("REEMU_NATIVE_VIDEO", true) {
                let win_size = app
                    .handle()
                    .get_webview_window("main")
                    .and_then(|w| w.inner_size().ok())
                    .unwrap_or_default();
                match video::VideoSurface::spawn(app.handle()) {
                    Some((vs, h)) => {
                        let state = app.state::<AppState>();
                        let mut gpu = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
                        // SAFETY: `vs` (subsurface + conn) vive no AppState pelo
                        // resto do app, mantendo os handles válidos.
                        let attached = match gpu.as_mut() {
                            Some(fp) => unsafe {
                                fp.attach_surface(
                                    h.display,
                                    h.window,
                                    win_size.width,
                                    win_size.height,
                                )
                            },
                            None => false,
                        };
                        drop(gpu);
                        if attached {
                            state
                                .video
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .replace(vs);
                            spawn_video_pump(app.handle().clone());
                            log::info!("vídeo nativo ativo");
                        } else {
                            log::warn!("attach_surface falhou — segue no canvas");
                        }
                    }
                    None => log::warn!("surface de vídeo indisponível — modo canvas"),
                }
            }

            #[cfg(feature = "dev-autoload")]
            dev_autoload(&app.state::<AppState>().session);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::js_log,
            commands::current_focus,
            commands::toggle_focus,
            commands::load_game,
            commands::unload_game,
            commands::poll_frame,
            commands::native_video_active,
            commands::pause_background,
            commands::session_state,
            commands::get_audio_config,
            commands::update_audio_config,
            commands::list_installed_cores,
            commands::get_core_options,
            commands::set_core_option,
            commands::get_shader_info,
            commands::set_shader,
            commands::get_rom_shader,
            commands::list_slangp_dir,
            commands::get_shader_params,
            commands::set_shader_param,
            commands::reset_shader_params,
            commands::import_decoration_pack,
            commands::clear_decorations,
            commands::is_fullscreen,
            commands::set_fullscreen,
            commands::quit_app,
            commands::list_core_catalog,
            commands::download_core,
            commands::remove_core,
            commands::list_roms,
            commands::remove_rom,
            commands::list_rom_sources,
            commands::remove_rom_source,
            commands::remove_rom_system,
            commands::clear_library,
            commands::scan_library,
            commands::get_metadata_config,
            commands::set_metadata_config,
            commands::get_rom_metadata,
            commands::list_pending_matches,
            commands::resolve_pending_match,
            commands::metadata_scan_progress,
            commands::cancel_metadata_scan,
            commands::start_metadata_scan,
            commands::save_state,
            commands::list_save_states,
            commands::load_save_state,
            commands::delete_save_state,
            commands::read_save_thumbnail,
            commands::input_key,
            commands::start_binding_capture,
            commands::cancel_binding_capture,
            commands::save_binding,
            commands::list_system_hotkeys,
            commands::clear_system_hotkey,
            commands::list_controller_mappings,
            commands::clear_controller_mapping,
            commands::list_gamepads,
            commands::set_device_port,
            commands::clear_device_port,
            commands::list_device_ports,
        ])
        .build(tauri::generate_context!())
        .expect("erro ao construir o app Tauri");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Resized(size),
            ..
        } if label == "main" => {
            // Só REGISTRA a geometria — quem aplica (mexe na conexão Wayland +
            // no swapchain wgpu) é o `reemu-video-pump`, dono único disso.
            let state = app_handle.state::<AppState>();
            if state
                .video
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_some()
            {
                // Deslocamento da decoração (CSD): 0 em fullscreen, a espessura
                // da borda/título em janela. Wayland costuma não expor posição
                // global → cai em (0,0), o certo no caso comum (fullscreen).
                let (ox, oy) = app_handle
                    .get_webview_window("main")
                    .and_then(|w| {
                        let i = w.inner_position().ok()?;
                        let o = w.outer_position().ok()?;
                        Some(((i.x - o.x).max(0), (i.y - o.y).max(0)))
                    })
                    .unwrap_or((0, 0));
                *state
                    .pending_surface_geom
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = Some((ox, oy, size.width, size.height));
            }
        }
        // Fechamento (X da janela, Alt+F4, `quit_app`): descarrega o jogo antes
        // de sair pra fazer o flush final da save RAM `.srm`. Sem isso só o
        // flush periódico (10s) protegia. Idempotente se já não há jogo.
        tauri::RunEvent::ExitRequested { .. } => {
            let _ = app_handle.state::<AppState>().session.unload();
        }
        _ => {}
    });
}

/// Thread dedicada que apresenta o frame do core na surface nativa (~60Hz).
/// Necessária porque, sem o `<canvas>` fazendo `poll_frame`, o event loop do
/// Tauri fica ocioso e `MainEventsCleared` não tiquetaqueia. `render_to_surface`
/// só toca wgpu (`Send`/`Sync`), então roda fora da thread principal.
fn spawn_video_pump(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("reemu-video-pump".into())
        .spawn(move || {
            // A subsurface está escondida agora? (só o pump apresenta/esconde,
            // então este bool acompanha o estado real.)
            let mut hidden = true;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(15));
                let state = app.state::<AppState>();
                {
                    let vg = state.video.lock().unwrap_or_else(|p| p.into_inner());
                    if vg.is_none() {
                        break; // surface removida — encerra a thread
                    }
                }

                // Geometria pendente do último `Resized` (registrada pela thread
                // principal; aplicada AQUI porque só o pump toca Wayland + wgpu).
                if let Some((x, y, w, h)) = state
                    .pending_surface_geom
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                {
                    if let Some(vs) = state
                        .video
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_ref()
                    {
                        vs.reconfigure(x, y, w, h);
                    }
                    if let Some(fp) = state.gpu.lock().unwrap_or_else(|p| p.into_inner()).as_mut() {
                        fp.resize_surface(w, h);
                    }
                }

                let frame = state.session.take_latest_frame();
                let idle = matches!(state.session.state(), emu_session::SessionState::Idle);
                let vm = *state.video_menu.lock().unwrap_or_else(|p| p.into_inner());

                use commands::VideoMenu::*;
                match vm {
                    Playing => {
                        if let Some(f) = frame.as_ref() {
                            let mut gpu = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
                            if let Some(fp) = gpu.as_mut() {
                                fp.render_to_surface(Some(f));
                            }
                            hidden = false; // o present remapeia a subsurface
                        } else if idle && !hidden {
                            // Jogo descarregado ou troca de ROM: esconde a
                            // subsurface pra a webview (biblioteca / "carregando")
                            // aparecer. Antes pintava um retângulo preto por cima
                            // da UI (`clear_surface`).
                            if let Some(vs) = state
                                .video
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .as_ref()
                            {
                                vs.set_hidden(true);
                            }
                            hidden = true;
                        }
                    }
                    Opening(0) => {
                        let cap = state
                            .gpu
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .as_mut()
                            .and_then(|fp| fp.capture_surface_frame());
                        *state.pause_bg.lock().unwrap_or_else(|p| p.into_inner()) = cap;
                        if let Some(vs) = state
                            .video
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .as_ref()
                        {
                            vs.set_hidden(true);
                        }
                        hidden = true;
                        *state.video_menu.lock().unwrap_or_else(|p| p.into_inner()) = MenuUp;
                    }
                    Opening(n) => {
                        // mantém o último frame fresco até capturar
                        if let (Some(f), Some(fp)) = (
                            frame.as_ref(),
                            state.gpu.lock().unwrap_or_else(|p| p.into_inner()).as_mut(),
                        ) {
                            fp.render_to_surface(Some(f));
                        }
                        *state.video_menu.lock().unwrap_or_else(|p| p.into_inner()) =
                            Opening(n - 1);
                    }
                    MenuUp => {}
                    Closing(0) => {
                        *state.video_menu.lock().unwrap_or_else(|p| p.into_inner()) = Playing;
                    }
                    Closing(n) => {
                        *state.video_menu.lock().unwrap_or_else(|p| p.into_inner()) =
                            Closing(n - 1);
                    }
                }
            }
        })
        .expect("spawn reemu-video-pump");
}

/// Thread dedicada que faz a ponte gamepad→frontend (~60Hz). Independente do
/// event loop do Tauri, que fica ocioso quando a webview não anima.
fn spawn_input_bridge(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("reemu-input-bridge".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(16));
            let state = app.state::<AppState>();

            // Botão de menu do gamepad (`Mode`) → alterna o foco (= Escape).
            if state.session.take_menu_request() {
                commands::toggle_and_emit(&app);
            }

            // Eventos brutos capturados em modo de binding → frontend.
            for ev in state.session.take_captured_inputs() {
                let _ = app.emit("raw-input-captured", &ev);
            }

            // Navegação de menu pelo gamepad → frontend (a Gamepad API do
            // WebKitGTK não enxerga o controle). O frontend (`useMenuNav`)
            // decide o contexto: no `/play` só age com o jogo pausado, então
            // durante a partida o d-pad vai só pro RetroPad. Emitir sempre
            // evita depender do timing do `SessionState`.
            for pulse in state.session.take_nav_pulses() {
                let _ = app.emit("menu-nav", commands::nav_pulse_name(pulse));
            }

            // Hotkeys de sistema (teclado + gamepad).
            commands::poll_hotkeys(&app);
        })
        .expect("spawn reemu-input-bridge");
}

/// Diretório único de dados do app: `REEMU_DATA_DIR` (testes/dev) ou o
/// `app_data_dir` da plataforma, com `temp_dir` como último recurso. Tudo —
/// SQLite, cores, saves, system — pendura aqui.
fn data_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> std::path::PathBuf {
    let dir = std::env::var_os("REEMU_DATA_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| app.path().app_data_dir().ok())
        .unwrap_or_else(std::env::temp_dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("criando {dir:?}: {e}");
    }
    dir
}

/// Abre o SQLite em `<base>/reemu.db` (roda as migrations). `None` se falhar
/// — o app segue, os comandos de config retornam erro.
fn open_db(base: &std::path::Path) -> Option<db::Db> {
    let url = format!("sqlite://{}", base.join("reemu.db").display());
    match tauri::async_runtime::block_on(db::connect(&url)) {
        Ok(pool) => {
            log::info!("SQLite: {url}");
            Some(pool)
        }
        Err(e) => {
            log::error!("SQLite {url}: {e}");
            None
        }
    }
}

/// Carrega `REEMU_DEV_CORE`/`REEMU_DEV_ROM` no startup — só pra testar a
/// surface de vídeo enquanto a UI de biblioteca não existe.
#[cfg(feature = "dev-autoload")]
fn dev_autoload(session: &std::sync::Arc<emu_session::EmuSession>) {
    let (Ok(core), Ok(rom)) = (
        std::env::var("REEMU_DEV_CORE"),
        std::env::var("REEMU_DEV_ROM"),
    ) else {
        log::warn!("dev-autoload: defina REEMU_DEV_CORE e REEMU_DEV_ROM");
        return;
    };
    let session = std::sync::Arc::clone(session);
    std::thread::spawn(move || match session.load(&core, &rom) {
        Ok(av) => log::info!(
            "dev-autoload: core {}x{} @ {} fps",
            av.geometry.base_width,
            av.geometry.base_height,
            av.timing.fps
        ),
        Err(e) => log::error!("dev-autoload: {e}"),
    });
}
