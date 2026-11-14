//! O motor ligado ao app: instalação no estado do Tauri e os momentos em que
//! uma passada roda (ao abrir, de tempos em tempos, antes de abrir e depois de
//! fechar um jogo, depois de gravar um state). Saiu de `engine.rs` sem
//! mudança de comportamento.

use super::*;

/// O motor no estado do Tauri: o que está rodando vem do `AppState`, e os
/// eventos vão para a tela no canal `sync`.
pub fn install(app: &AppHandle, sync: Arc<SyncService>, db: db::Db, dirs: Dirs) {
    let probe = app.clone();
    let playing: PlayingFn = Box::new(move || {
        let Some(state) = probe.try_state::<crate::commands::AppState>() else {
            return Playing::Idle;
        };
        let current_rom = lock(&state.current_rom).clone();
        playing_from(
            state.session.loaded_core().is_some(),
            state
                .loading_game
                .load(std::sync::atomic::Ordering::Relaxed),
            current_rom,
        )
    });
    let sink = app.clone();
    let emit: EmitFn = Box::new(move |event| {
        if let Err(e) = sink.emit(SYNC_EVENT, &event) {
            log::warn!("sync: não avisou a interface: {e}");
        }
    });
    // Ligar o sync (ou os game saves) roda uma passada já, sem esperar a de
    // 10 minutos nem um jogo (M3).
    let kicker = app.clone();
    sync.on_enabled(Box::new(move || {
        let app = kicker.clone();
        tauri::async_runtime::spawn(async move {
            let Some((engine, alva)) = parts(&app) else {
                return;
            };
            log_pass("passada ao ligar", engine.pass(&alva, PassScope::All).await);
        });
    }));
    // Depois de copiar ou desfazer configurações: o que o app guarda em
    // memória volta a ser o do banco.
    let reloader = app.clone();
    let reload: ReloadFn = Box::new(move || {
        let app = reloader.clone();
        Box::pin(async move {
            let Some(state) = app.try_state::<crate::commands::AppState>() else {
                return;
            };
            let Some(pool) = state.db.clone() else {
                return;
            };
            crate::commands::reload_copied_settings(&state, &pool).await;
        })
    });
    app.manage(Arc::new(Engine::new(sync, db, dirs, playing, emit, reload)));
}

/// O que está rodando, pelo estado da sessão. Com um load em andamento e um
/// core já aberto, o `current_rom` ainda é o do jogo anterior: vale
/// `Running(None)`. Sem core aberto, o load é `Loading` (a passada de antes de
/// abrir ainda escreve).
pub(super) fn playing_from(core_open: bool, loading: bool, current_rom: Option<String>) -> Playing {
    match (core_open, loading) {
        (true, true) => Playing::Running(None),
        (true, false) => Playing::Running(current_rom),
        (false, true) => Playing::Loading,
        (false, false) => Playing::Idle,
    }
}

pub(super) fn parts(app: &AppHandle) -> Option<(Arc<Engine>, tauri::State<'_, Alvanista>)> {
    let engine = app.try_state::<Arc<Engine>>()?.inner().clone();
    Some((engine, app.try_state::<Alvanista>()?))
}

/// A primeira passada com o app aberto e, dali em diante, uma a cada 10
/// minutos sem jogo aberto.
pub async fn run_on_start(app: AppHandle) {
    scheduled_pass(&app).await;
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(IDLE_EVERY).await;
            scheduled_pass(&app).await;
        }
    });
}

pub(super) async fn scheduled_pass(app: &AppHandle) {
    let Some((engine, alva)) = parts(app) else {
        return;
    };
    if engine.playing() != Playing::Idle {
        return;
    }
    log_pass("passada", engine.pass(&alva, PassScope::All).await);
}

/// Antes de abrir um jogo: espera a nuvem só daquele jogo (no máximo 5 s).
pub async fn before_launch(app: &AppHandle, rom_id: &str) {
    let Some((engine, alva)) = parts(app) else {
        return;
    };
    let outcome = engine.prelaunch(&alva, rom_id).await;
    log::info!("sync: antes de abrir: {outcome:?}");
}

/// Depois de gravar um save state: a passada dos states daquela ROM, 3 s
/// depois do último save da rajada (uma rajada de QuickSaves sobe uma vez, com
/// os últimos bytes). Subir com o jogo aberto vale: só lê o arquivo.
pub fn after_state_saved<R: tauri::Runtime>(app: &AppHandle<R>, rom_id: String) {
    let Some(engine) = app.try_state::<Arc<Engine>>().map(|e| e.inner().clone()) else {
        return;
    };
    let ticket = engine.state_ticket(&rom_id);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if !engine.state_settled(&rom_id, ticket).await {
            return;
        }
        let Some(alva) = app.try_state::<Alvanista>() else {
            return;
        };
        log_pass(
            "passada dos states",
            engine.pass(&alva, PassScope::State(rom_id)).await,
        );
    });
}

/// A metade dos states de "Sincronizar agora", por conta própria: a página
/// não espera por ela (M4), e desligar os states no meio a para (I1).
pub fn spawn_state_pass(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some((engine, alva)) = parts(&app) else {
            return;
        };
        log_pass(
            "passada dos states",
            engine.pass(&alva, PassScope::States).await,
        );
    });
}

/// Depois de fechar um jogo: a passada daquela ROM, sem esperar por ela.
pub fn after_unload(app: &AppHandle, rom_id: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some((engine, alva)) = parts(&app) else {
            return;
        };
        let (saves, states) = engine.after_close(&alva, &rom_id).await;
        log_pass("passada depois do jogo", saves);
        log_pass("passada dos states depois do jogo", states);
    });
}

pub(super) fn log_pass(what: &str, result: Result<PassReport, SyncError>) {
    match result {
        Ok(r) => log::info!(
            "sync: {what}: {} enviado(s), {} state(s) enviado(s), {} baixado(s), {} em conflito, {} no backup, {} adiado(s), {} erro(s)",
            r.uploaded,
            r.states_uploaded,
            r.downloaded,
            r.conflicts,
            r.backed_up,
            r.deferred,
            r.errors.len()
        ),
        Err(SyncError::NoAccount | SyncError::NotEnabled) => {}
        Err(e) => log::info!("sync: {what} terminou em {e}"),
    }
}
