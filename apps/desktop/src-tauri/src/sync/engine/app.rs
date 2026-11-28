//! O motor ligado ao app: instalação no estado do Tauri e os momentos em que
//! uma passada roda (ao abrir, de tempos em tempos, antes de abrir e depois de
//! fechar um jogo, depois de gravar um state), e a checagem de antes de abrir
//! que segura o jogo num conflito (`launch_check`). Saiu de `engine.rs` na
//! divisão do motor.

use super::*;
use crate::app_error::{AppError, ErrorCode};

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
    let engine = Arc::new(Engine::new(sync, db.clone(), dirs, playing, emit, reload));
    let ra_app = app.clone();
    let ra_db = db;
    engine.set_on_ra_changed(Box::new(move || {
        let app = ra_app.clone();
        let pool = ra_db.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(cfg) = crate::credentials::load_ra_config(&pool).await {
                let dto: crate::ra::RaStatusDto = cfg.into();
                crate::ra::emit_ra_status_changed(&app, &dto);
            }
        });
    }));
    app.manage(engine);
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

/// A primeira passada com o app aberto e, dali em diante, cadencia adaptativa
/// sem jogo aberto: `5 min` em primeiro plano e recuo gradual em segundo plano
/// (`10m -> 20m -> 40m -> 60m -> 120m`), acordando na volta ao foco.
pub async fn run_on_start(app: AppHandle) {
    scheduled_pass(&app).await;
    tauri::async_runtime::spawn(async move {
        let mut bg_step = 0u32;
        let mut last_pass_at = tokio::time::Instant::now();
        loop {
            if let Some(coord) = app.try_state::<crate::presence::ActivityCoordinator>() {
                coord
                    .wait_next_cycle(
                        crate::presence::SYNC_FG_EVERY,
                        &crate::presence::SYNC_BG_STEPS,
                        &mut bg_step,
                        last_pass_at,
                    )
                    .await;
            } else {
                tokio::time::sleep(crate::presence::SYNC_FG_EVERY).await;
            }
            last_pass_at = tokio::time::Instant::now();
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

/// Antes de abrir um jogo: espera a nuvem só daquele jogo (no máximo 5 s). Um
/// conflito achado ali segura o jogo (`Engine::launch_check`).
pub async fn before_launch(
    app: &AppHandle,
    rom_id: &str,
    decide_later: bool,
) -> Result<(), AppError> {
    let Some((engine, alva)) = parts(app) else {
        return Ok(());
    };
    engine.launch_check(&alva, rom_id, decide_later).await
}

impl Engine {
    /// A checagem de antes de abrir, e o que ela diz ao `load_game`. Um
    /// conflito deste jogo achado agora (outra máquina mudou o save depois
    /// da última passada daqui) para o jogo antes de o core abrir: o erro
    /// `syncConflict` faz a tela de jogo perguntar (D118) e tentar de novo.
    /// Enquanto a pessoa decide, nada é escrito: a checagem já terminou e o
    /// conflito só ficou guardado. `decide_later`: ela já disse "Decidir
    /// depois", e o jogo abre com o save daqui. O prazo de 5 s e o toast de
    /// "não conferida" ficam como estão.
    pub async fn launch_check(
        &self,
        alva: &Alvanista,
        rom_id: &str,
        decide_later: bool,
    ) -> Result<(), AppError> {
        let outcome = self.prelaunch(alva, rom_id).await;
        log::info!("sync: antes de abrir: {outcome:?}");
        match outcome {
            PrelaunchOutcome::Conflict if !decide_later => Err(AppError::new(
                ErrorCode::SyncConflict,
                "o save deste jogo mudou aqui e em outra máquina",
            )),
            _ => Ok(()),
        }
    }
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

/// Depois de mudar credenciais locais (login/logout/modo do RetroAchievements
/// ou chaves de metadados): envia ao cofre criptografado em segundo plano.
pub fn after_credentials_changed(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some((engine, alva)) = parts(&app) else {
            return;
        };
        if let Err(e) = engine.push_credentials_now(&alva).await {
            if !matches!(
                e,
                SyncError::NoAccount | SyncError::NotEnabled | SyncError::Paused
            ) {
                log::info!("sync: envio imediato de credenciais terminou em {e}");
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn cloud_save_idle_schedule_runs_every_5m_in_fg_and_backs_off_to_2h_in_bg() {
        let coord = crate::presence::ActivityCoordinator::new();
        let passes = Arc::new(AtomicUsize::new(0));
        let passes_bg = passes.clone();
        let coord_bg = coord.clone();

        let worker = tokio::spawn(async move {
            let mut bg_step = 0u32;
            let mut last_pass_at = Instant::now();
            loop {
                coord_bg
                    .wait_next_cycle(
                        crate::presence::SYNC_FG_EVERY,
                        &crate::presence::SYNC_BG_STEPS,
                        &mut bg_step,
                        last_pass_at,
                    )
                    .await;
                if coord_bg.is_shutting_down() {
                    break;
                }
                last_pass_at = Instant::now();
                passes_bg.fetch_add(1, Ordering::SeqCst);
            }
        });

        for _ in 0..5 {
            tokio::task::yield_now().await;
        }

        // Em primeiro plano: 5m por passada.
        tokio::time::advance(Duration::from_secs(5 * 60)).await;
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert_eq!(passes.load(Ordering::SeqCst), 1);

        // Em segundo plano: 10m -> 20m -> 40m -> 60m -> 120m.
        coord.set_ui_active(false, false);
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        for (idx, mins) in [10u64, 20, 40, 60, 120].into_iter().enumerate() {
            tokio::time::advance(Duration::from_secs(mins * 60 - 1)).await;
            for _ in 0..5 {
                tokio::task::yield_now().await;
            }
            assert_eq!(passes.load(Ordering::SeqCst), 1 + idx);

            tokio::time::advance(Duration::from_secs(1)).await;
            for _ in 0..5 {
                tokio::task::yield_now().await;
            }
            assert_eq!(passes.load(Ordering::SeqCst), 2 + idx);
        }

        // Passaram 6m em segundo plano (>= 5m) e a janela volta ao foco -> passada imediata!
        tokio::time::advance(Duration::from_secs(6 * 60)).await;
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert_eq!(passes.load(Ordering::SeqCst), 6);

        coord.set_ui_active(true, true);
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert_eq!(passes.load(Ordering::SeqCst), 7);

        coord.mark_shutting_down();
        let _ = worker.await;
    }
}
