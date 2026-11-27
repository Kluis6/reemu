//! Catálogo do Alvanista (`/api/partner/v1/games/*`): identifica as ROMs da
//! biblioteca pelos hashes, tamanho, nome e serial, preenche `game_metadata` e
//! guarda a resposta em `rom_resolution`.

pub mod answer;
pub mod identify;
pub mod pass;

pub use pass::{
    CatalogEngine, CatalogError, CatalogEvent, CatalogStatus, PassReport, PassScope, SearchGameDto,
};

use crate::alvanista::Alvanista;
use crate::app_error::AppError;
use crate::commands::GameMetadataDto;
use domain::metadata::ResolutionState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const CATALOG_EVENT: &str = "catalog";

/// Acompanhamento periódico enquanto houver arquivos `pending` ou com mídia
/// pendente, e nenhum jogo estiver rodando (D140 / Review Focus 3).
const IDLE_EVERY: Duration = Duration::from_secs(30 * 60);

pub fn install(app: &AppHandle, db: db::Db, covers_dir: PathBuf) {
    let probe = app.clone();
    let is_playing = Arc::new(move || {
        let Some(state) = probe.try_state::<crate::commands::AppState>() else {
            return false;
        };
        state.session.loaded_core().is_some()
            || state
                .loading_game
                .load(std::sync::atomic::Ordering::Relaxed)
    });
    let sink = app.clone();
    let emit = Arc::new(move |ev: CatalogEvent| {
        if let Err(e) = sink.emit(CATALOG_EVENT, &ev) {
            log::warn!("catálogo: não avisou a interface: {e}");
        }
    });
    app.manage(Arc::new(CatalogEngine::new(
        db, covers_dir, is_playing, emit,
    )));
}

fn parts(app: &AppHandle) -> Option<(Arc<CatalogEngine>, tauri::State<'_, Alvanista>)> {
    let engine = app.try_state::<Arc<CatalogEngine>>()?.inner().clone();
    Some((engine, app.try_state::<Alvanista>()?))
}

fn log_pass(where_: &str, res: Result<PassReport, CatalogError>) {
    match res {
        Ok(r) if r.sent > 0 || r.identified_late > 0 => log::info!(
            "catálogo ({where_}): enviados={} identificados={} fila={} sem_resposta={} tardios={}",
            r.sent,
            r.found,
            r.pending,
            r.unknown,
            r.identified_late
        ),
        Ok(_) | Err(CatalogError::AuthEnded) | Err(CatalogError::Disabled) => {}
        Err(e) => log::info!("catálogo ({where_}): {e}"),
    }
}

/// Primeira passada ao abrir o app (depois de `refresh_on_start`) e laço de 30
/// minutos enquanto houver pendências e nenhum jogo estiver rodando.
pub fn spawn_on_start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Some((engine, alva)) = parts(&app) {
            if !engine.is_playing() {
                log_pass(
                    "abertura",
                    engine.run_pass(&alva, PassScope::All, None).await,
                );
            }
        }
        loop {
            tokio::time::sleep(IDLE_EVERY).await;
            let Some((engine, alva)) = parts(&app) else {
                continue;
            };
            if engine.is_playing() || !alva.has_session().await || !engine.is_enabled().await {
                continue;
            }
            if !engine.has_follow_up().await {
                continue;
            }
            log_pass(
                "periódica",
                engine.run_pass(&alva, PassScope::All, None).await,
            );
        }
    });
}

/// Dispara uma passada em segundo plano (após conectar a conta ou adicionar ROMs).
pub fn trigger_in_background(app: &AppHandle, scope: PassScope) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some((engine, alva)) = parts(&app) else {
            return;
        };
        log_pass("segundo plano", engine.run_pass(&alva, scope, None).await);
    });
}

/// Antes da leva dos provedores locais (`start_metadata_scan`, D151): consulta
/// o catálogo primeiro para que só os `unknown` sigam para a cascata local.
pub async fn run_before_own_scraper(app: &AppHandle, scope: PassScope) {
    let Some((engine, alva)) = parts(app) else {
        return;
    };
    log_pass(
        "antes do scraping",
        engine.run_pass(&alva, scope, None).await,
    );
}

/// Para `rescrape_rom` (D151): pergunta ao catálogo primeiro quando conectado e
/// ligado. Devolve `Some("auto")` se o catálogo identificou, `Some("none")` se
/// ficou `pending` na fila do catálogo, ou `None` quando é `unknown` (ou quando
/// o catálogo está desligado/desconectado) para a cascata local rodar.
pub async fn resolve_one_for_rescrape(app: &AppHandle, rom_id: &str) -> Option<&'static str> {
    let (engine, alva) = parts(app)?;
    if !alva.has_session().await || !engine.is_enabled().await {
        return None;
    }
    match engine
        .run_pass(&alva, PassScope::Roms(vec![rom_id.to_owned()]), None)
        .await
    {
        Ok(_) => {}
        Err(CatalogError::Disabled | CatalogError::AuthEnded | CatalogError::Transient(_)) => {
            return None;
        }
        Err(_) => return None,
    }
    let row = db::RomResolutionRepo::new(engine.db().clone())
        .get(rom_id)
        .await
        .ok()
        .flatten()?;
    match row.state {
        ResolutionState::Found | ResolutionState::Manual => Some("auto"),
        ResolutionState::Pending => Some("none"),
        ResolutionState::Unknown => None,
    }
}

fn required_parts(
    app: &AppHandle,
) -> Result<(Arc<CatalogEngine>, tauri::State<'_, Alvanista>), AppError> {
    parts(app).ok_or_else(|| AppError::from("catálogo indisponível"))
}

#[tauri::command]
pub async fn catalog_status(app: AppHandle) -> Result<CatalogStatus, AppError> {
    let (engine, alva) = required_parts(&app)?;
    Ok(engine.status(&alva).await)
}

#[tauri::command]
pub async fn catalog_set_enabled(app: AppHandle, enabled: bool) -> Result<CatalogStatus, AppError> {
    let (engine, alva) = required_parts(&app)?;
    engine
        .set_enabled(enabled)
        .await
        .map_err(|e| AppError::from(e.to_string()))?;
    if enabled {
        trigger_in_background(&app, PassScope::All);
    }
    Ok(engine.status(&alva).await)
}

#[tauri::command]
pub async fn catalog_run_now(
    app: AppHandle,
    lang: Option<String>,
) -> Result<CatalogStatus, AppError> {
    let (engine, alva) = required_parts(&app)?;
    match engine
        .run_pass(&alva, PassScope::All, lang.as_deref())
        .await
    {
        Ok(_) | Err(CatalogError::Disabled | CatalogError::AuthEnded) => {
            Ok(engine.status(&alva).await)
        }
        Err(e) => Err(AppError::from(e.to_string())),
    }
}

#[tauri::command]
pub async fn catalog_search(
    app: AppHandle,
    q: String,
    system: String,
) -> Result<Vec<SearchGameDto>, AppError> {
    let (engine, alva) = required_parts(&app)?;
    engine
        .search(&alva, &q, &system)
        .await
        .map_err(|e| AppError::from(e.to_string()))
}

#[tauri::command]
pub async fn catalog_pick(
    app: AppHandle,
    rom_id: String,
    game_id: String,
    lang: Option<String>,
) -> Result<GameMetadataDto, AppError> {
    let (engine, alva) = required_parts(&app)?;
    let meta = engine
        .pick(&alva, &rom_id, &game_id, lang.as_deref())
        .await
        .map_err(|e| AppError::from(e.to_string()))?;
    Ok(
        crate::commands::metadata::load_rom_metadata(engine.db(), &rom_id, lang.as_deref())
            .await?
            .unwrap_or_else(|| GameMetadataDto::from(meta)),
    )
}
