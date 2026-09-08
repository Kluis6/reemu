mod bios;
mod commands;
mod core_catalog;
mod decoration;
mod gpu;
mod scraping;
mod shader_pack;
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
                    let state = app.state::<AppState>();
                    // Etapa 12 B3b: publica os handles do `VkDevice` do
                    // compositor na sessão. Com `REEMU_HW=vulkan`, um core que
                    // negocia Vulkan passa a rodar in-process no mesmo device.
                    if let Some(dev) = fp.vulkan_shared_device() {
                        state.session.attach_vulkan_device(dev);
                    }
                    state.gpu.lock().unwrap().replace(fp);
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
                            // Aplica a geometria UMA vez agora — o `Resized` não
                            // dispara na carga inicial, então sem isto a
                            // subsurface fica no tamanho/posição do spawn (a
                            // janela ainda podia estar assentando) e o jogo sai
                            // torto. O pump reconfigura no 1º tick.
                            *state
                                .pending_surface_geom
                                .lock()
                                .unwrap_or_else(|p| p.into_inner()) =
                                Some(current_surface_geom(app.handle()));
                            spawn_video_pump(app.handle().clone());
                            log::info!("vídeo nativo ativo");
                            // SAFETY: os ponteiros wl vivem enquanto o `vs` no
                            // AppState viver (resto do app). Só o pump usa isto
                            // (única thread dona da conexão Wayland).
                            *state.vk_reattach.lock().unwrap_or_else(|p| p.into_inner()) = Some((
                                unsafe { gpu::SendHandles::new(h.display, h.window) },
                                win_size.width,
                                win_size.height,
                            ));
                        } else {
                            log::warn!("attach_surface falhou — segue no canvas");
                        }
                    }
                    None => log::warn!("surface de vídeo indisponível — modo canvas"),
                }
            }

            // Negociador Vulkan §Beetle (D2/D3): com `REEMU_HW=vulkan`, um core
            // que EXIGE criar o `VkDevice` (Beetle PSX HW) chama isto — recria o
            // `FrameProcessor` no device do core, reanexa a surface, e devolve
            // os handles pra `emu-session` montar a ponte de frame.
            {
                let app_h = app.handle().clone();
                let negotiator: domain::core_loader::VulkanDeviceNegotiator =
                    std::sync::Arc::new(move |neg| {
                        // Roda numa thread do `emu-session` — NÃO pode tocar
                        // Wayland/wgpu-surface. Só constrói o FP e deixa em
                        // `pending_gpu`; o video pump faz a troca.
                        let (fp, shared) =
                            unsafe { gpu::FrameProcessor::from_core_negotiation(neg) }?;
                        *app_h
                            .state::<AppState>()
                            .pending_gpu
                            .lock()
                            .unwrap_or_else(|p| p.into_inner()) = Some(fp);
                        log::info!("FrameProcessor reconstruído no device do core (§Beetle)");
                        Ok(shared)
                    });
                app.state::<AppState>()
                    .session
                    .attach_vulkan_negotiator(negotiator);
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
            commands::reset_core_options,
            commands::get_shader_info,
            commands::set_shader,
            commands::get_rom_shader,
            commands::list_slangp_dir,
            commands::shader_pack_status,
            commands::download_shader_pack,
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
            commands::list_bios_status,
            commands::import_bios_file,
            commands::remove_bios_file,
            commands::list_roms,
            commands::remove_rom,
            commands::set_rom_favorite,
            commands::list_rom_sources,
            commands::remove_rom_source,
            commands::remove_rom_system,
            commands::list_system_cores,
            commands::set_system_core,
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
                let (ox, oy) = csd_offset(app_handle);
                *state
                    .pending_surface_geom
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) =
                    Some((ox, oy, size.width.max(1), size.height.max(1)));
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
/// Deslocamento do CSD (borda/título): `(0,0)` em fullscreen e no caso comum do
/// Wayland (não expõe posição global), a espessura da decoração em janela X11.
fn csd_offset(app: &tauri::AppHandle) -> (i32, i32) {
    app.get_webview_window("main")
        .and_then(|w| {
            let i = w.inner_position().ok()?;
            let o = w.outer_position().ok()?;
            Some(((i.x - o.x).max(0), (i.y - o.y).max(0)))
        })
        .unwrap_or((0, 0))
}

/// Geometria `(x, y, w, h)` que a subsurface de vídeo deve ter agora: tamanho =
/// área de conteúdo da janela, posição = deslocamento do CSD. Usado na carga
/// inicial e na troca de `FrameProcessor` (§Beetle) — o `Resized` usa o tamanho
/// que vem no evento.
fn current_surface_geom(app: &tauri::AppHandle) -> (i32, i32, u32, u32) {
    let size = app
        .get_webview_window("main")
        .and_then(|w| w.inner_size().ok())
        .unwrap_or_default();
    let (ox, oy) = csd_offset(app);
    (ox, oy, size.width.max(1), size.height.max(1))
}

fn spawn_video_pump(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("reemu-video-pump".into())
        .spawn(move || {
            // A subsurface está escondida agora? (só o pump apresenta/esconde,
            // então este bool acompanha o estado real.)
            let mut hidden = true;
            loop {
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

                // Etapa 12 §Beetle: um core criou o `VkDevice` ele mesmo e o
                // negociador deixou um `FrameProcessor` novo em `pending_gpu`.
                // A troca (drop do antigo + `attach_surface` do novo) só pode
                // rodar AQUI — o pump é a única thread dona da conexão Wayland.
                // Antes do `step_vk_local` pra o 1º frame Vulkan já pegar o FP
                // do device certo.
                if let Some(mut new_fp) = state
                    .pending_gpu
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take()
                {
                    let mut slot = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
                    drop(slot.take()); // dropa o FP antigo (+ surface) nesta thread
                    if let Some((h, w, ht)) = state
                        .vk_reattach
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_ref()
                    {
                        // SAFETY: os handles wl vivem enquanto o `VideoSurface`
                        // no AppState viver.
                        let ok = unsafe { new_fp.attach_surface(h.display(), h.window(), *w, *ht) };
                        if !ok {
                            log::warn!("§Beetle: reanexar surface no FP novo falhou — canvas");
                        }
                    }
                    *slot = Some(new_fp);
                    drop(slot);
                    // O FP novo nasceu com a config de surface do spawn —
                    // reconfigura pro tamanho/posição atuais no próximo tick.
                    *state
                        .pending_surface_geom
                        .lock()
                        .unwrap_or_else(|p| p.into_inner()) = Some(current_surface_geom(&app));
                    log::info!("§Beetle: FrameProcessor trocado pro device do core");
                }

                // Etapa 12 B3b/D4: um core Vulkan in-process roda AQUI, nesta
                // thread — a mesma que submete o wgpu (o `retro_run` de cores
                // como o Beetle submete direto na `VkQueue` compartilhada). O
                // `step_vk_local` já faz o pacing; quando ele produz um frame,
                // NÃO dormimos no fim do loop. Sem core Vulkan local, cai pro
                // `take_latest_frame` de sempre (software/GL via `emu-session`).
                let (frame, stepped_vk) = match state.session.step_vk_local() {
                    Some(f) => (Some(f), true),
                    None => (state.session.take_latest_frame(), false),
                };
                let idle = matches!(state.session.state(), emu_session::SessionState::Idle);
                let vm = *state.video_menu.lock().unwrap_or_else(|p| p.into_inner());

                let loading =
                    state.loading_game.load(std::sync::atomic::Ordering::Relaxed);

                use commands::VideoMenu::*;
                match vm {
                    Playing => {
                        if loading || idle {
                            // Jogo descarregado, trocando de ROM ou em pleno
                            // load: esconde a subsurface pra a webview
                            // (biblioteca / "Carregando…") aparecer — sem deixar
                            // o último frame do jogo anterior grudado no
                            // `wl_surface`.
                            if !hidden {
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
                        } else if let Some(f) = frame.as_ref() {
                            let mut gpu = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
                            if let Some(fp) = gpu.as_mut() {
                                fp.render_to_surface(Some(f));
                            }
                            hidden = false; // o present remapeia a subsurface
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

                // O `step_vk_local` já dá o ritmo (pacing por acumulador do
                // core). Sem core Vulkan local, mantém os ~15ms de sempre.
                if !stepped_vk {
                    std::thread::sleep(std::time::Duration::from_millis(15));
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
    std::thread::spawn(move || match session.load(&core, &rom, std::collections::HashMap::new()) {
        Ok(av) => log::info!(
            "dev-autoload: core {}x{} @ {} fps",
            av.geometry.base_width,
            av.geometry.base_height,
            av.timing.fps
        ),
        Err(e) => log::error!("dev-autoload: {e}"),
    });
}
