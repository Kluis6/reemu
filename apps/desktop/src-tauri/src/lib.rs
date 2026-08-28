mod commands;
mod video;

pub mod save_state;

use commands::AppState;
use domain::audio::AudioConfigRepository as _;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let db = open_db(app.handle());
            let audio_config = db
                .as_ref()
                .and_then(|pool| {
                    tauri::async_runtime::block_on(db::AudioConfigRepo::new(pool.clone()).get())
                        .ok()
                })
                .unwrap_or_default();
            app.manage(AppState::new(db, audio_config));

            match video::VideoSurface::spawn(app.handle()) {
                Some(vs) => {
                    app.state::<AppState>().video.lock().unwrap().replace(vs);
                }
                None => log::warn!("não foi possível criar a surface de vídeo — modo só-webview"),
            }

            #[cfg(feature = "dev-autoload")]
            dev_autoload(&app.state::<AppState>().session);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::current_focus,
            commands::toggle_focus,
            commands::load_game,
            commands::session_state,
            commands::get_audio_config,
            commands::update_audio_config,
            commands::list_installed_cores,
            commands::list_roms,
            commands::scan_library,
            commands::save_state,
            commands::list_save_states,
            commands::load_save_state,
            commands::delete_save_state,
            commands::input_key,
        ])
        .build(tauri::generate_context!())
        .expect("erro ao construir o app Tauri");

    app.run(|app_handle, event| match event {
        // Renderiza o frame do core na surface nativa a cada iteração do loop.
        tauri::RunEvent::MainEventsCleared => {
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
        _ => {}
    });
}

/// Abre o SQLite em `<app_data_dir>/reemu.db` (roda as migrations). `None`
/// se falhar — o app segue, os comandos de config retornam erro.
fn open_db<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<db::Db> {
    let dir = app
        .path()
        .app_data_dir()
        .ok()
        .or_else(|| std::env::var_os("REEMU_DATA_DIR").map(std::path::PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("app_data_dir {dir:?}: {e}");
    }
    let url = format!("sqlite://{}", dir.join("reemu.db").display());
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
