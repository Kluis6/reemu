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

            // Child window X11 pro vídeo só com `REEMU_X11_VIDEO=1` (ver
            // main.rs): no padrão, o vídeo do jogo é renderizado na webview.
            if std::env::var_os("REEMU_X11_VIDEO").is_some() {
                match video::VideoSurface::spawn(app.handle()) {
                    Some(vs) => {
                        app.state::<AppState>().video.lock().unwrap().replace(vs);
                    }
                    None => {
                        log::warn!("não foi possível criar a surface de vídeo — modo só-webview")
                    }
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
            commands::session_state,
            commands::get_audio_config,
            commands::update_audio_config,
            commands::list_installed_cores,
            commands::get_core_options,
            commands::set_core_option,
            commands::get_shader_info,
            commands::set_shader,
            commands::get_rom_shader,
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
        // Renderiza o frame do core na surface nativa a cada iteração do loop.
        tauri::RunEvent::MainEventsCleared => {
            // A ponte de input roda em `spawn_input_bridge` (thread própria) —
            // aqui só o render da surface nativa de vídeo, que precisa da
            // thread principal.
            let state = app_handle.state::<AppState>();
            let mut guard = state.video.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(vs) = guard.as_mut() {
                vs.render(&state.session);
            }
        }
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Resized(size),
            ..
        } if label == "main" => {
            let state = app_handle.state::<AppState>();
            let mut guard = state.video.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(vs) = guard.as_mut() {
                vs.resize(size.width, size.height);
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
