//! Metadata / scraping (etapa 09).

use super::*;
use crate::app_error::AppError;

use domain::library::RomRepository;
use domain::metadata::{
    GameKaveData, GameMetadata, GameMetadataEdits, MetadataConfig, MetadataRepository,
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "MetadataConfig"))]
pub struct MetadataConfigDto {
    pub provider: String,
    pub screenscraper_user: Option<String>,
    pub screenscraper_password: Option<String>,
    /// Chave de API do TheGamesDB (provedor de reserva). `#[serde(default)]`:
    /// frontend antigo não manda o campo.
    #[serde(default)]
    pub thegamesdb_api_key: Option<String>,
}

#[tauri::command]
pub async fn get_metadata_config(
    state: State<'_, AppState>,
) -> Result<MetadataConfigDto, AppError> {
    let repo = db::MetadataRepo::new(pool(&state)?);
    let c = crate::credentials::load_config(&repo, &crate::credentials::OsKeyring).await?;
    Ok(MetadataConfigDto {
        provider: c.provider,
        screenscraper_user: c.screenscraper_user,
        screenscraper_password: c.screenscraper_password,
        thegamesdb_api_key: c.thegamesdb_api_key,
    })
}

#[tauri::command]
pub async fn set_metadata_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    config: MetadataConfigDto,
) -> Result<(), AppError> {
    let norm = |s: Option<String>| s.filter(|v| !v.trim().is_empty());
    let db_pool = pool(&state)?;
    let cfg = MetadataConfig {
        provider: config.provider,
        screenscraper_user: norm(config.screenscraper_user),
        screenscraper_password: norm(config.screenscraper_password),
        thegamesdb_api_key: norm(config.thegamesdb_api_key),
    };
    crate::credentials::save_config_tracked(&db_pool, &crate::credentials::OsKeyring, cfg)
        .await
        .map_err(AppError::from)?;
    crate::sync::engine::after_credentials_changed(&app);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "GameMetadata"))]
pub struct GameMetadataDto {
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    pub provider_source: Option<String>,
    pub game_uuid: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub players: Option<String>,
    /// 0 a 100
    pub rating: Option<u8>,
    pub age_rating: Option<String>,
    pub modes: Option<String>,
    pub screenshot_url: Option<String>,
    pub title_screen_url: Option<String>,
    pub fanart_url: Option<String>,
    pub logo_url: Option<String>,
    pub video_url: Option<String>,
    #[cfg_attr(
        test,
        ts(
            type = "{ descriptions: Record<string, string>; releaseDates: Record<string, string>; regionalCovers: Record<string, string>; romRegion: string | null; romRevision: string | null; timeToBeatMinutes: [number | null, number | null, number | null]; covers: string[]; heroes: string[]; screenshots: string[]; trailers: string[]; achievementCount: number | null; soundtrackCount: number | null } | null"
        )
    )]
    pub gamekave: Option<GameKaveData>,
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
            game_uuid: m.game_uuid,
            developer: m.details.developer,
            publisher: m.details.publisher,
            players: m.details.players,
            rating: m.details.rating,
            age_rating: m.details.age_rating,
            modes: m.details.modes,
            screenshot_url: m.details.screenshot_url,
            title_screen_url: m.details.title_screen_url,
            fanart_url: m.details.fanart_url,
            logo_url: m.details.logo_url,
            video_url: m.details.video_url,
            gamekave: m.details.gamekave,
        }
    }
}

fn apply_edits(mut dto: GameMetadataDto, edits: GameMetadataEdits) -> GameMetadataDto {
    for (target, value) in [
        (&mut dto.description, edits.description),
        (&mut dto.release_date, edits.release_date),
        (&mut dto.genre, edits.genre),
        (&mut dto.developer, edits.developer),
        (&mut dto.publisher, edits.publisher),
        (&mut dto.players, edits.players),
        (&mut dto.age_rating, edits.age_rating),
        (&mut dto.modes, edits.modes),
        (&mut dto.cover_url, edits.cover_url),
        (&mut dto.fanart_url, edits.fanart_url),
        (&mut dto.screenshot_url, edits.screenshot_url),
        (&mut dto.logo_url, edits.logo_url),
    ] {
        if let Some(value) = value {
            *target = (!value.trim().is_empty()).then(|| value.trim().to_owned());
        }
    }
    if let Some(rating) = edits.rating {
        dto.rating = rating
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|&value| value <= 100);
    }
    dto
}

pub(crate) async fn load_rom_metadata(
    pool: &db::Db,
    rom_id: &str,
    language: Option<&str>,
) -> Result<Option<GameMetadataDto>, AppError> {
    let repo = db::MetadataRepo::new(pool.clone());
    let imported = repo.get_metadata(rom_id).await.map_err(AppError::from)?;
    let edits = repo.get_edits(rom_id).await.map_err(AppError::from)?;
    if imported.is_none() && edits.is_none() {
        return Ok(None);
    }
    let Some(rom) = db::RomsRepo::new(pool.clone())
        .get(rom_id)
        .await
        .map_err(AppError::from)?
    else {
        return Ok(None);
    };
    let detected_region = scraping::rom_name::detect_rom_region(
        &rom.file_path,
        rom.ident_name.as_deref(),
        rom.serial.as_deref(),
    );
    let has_cover_edit = edits
        .as_ref()
        .and_then(|e| e.cover_url.as_deref())
        .is_some_and(|u| !u.trim().is_empty());
    let imported = match imported {
        Some(meta) if !has_cover_edit => Some(
            crate::covers::heal_rom_front_cover_if_needed(pool, None, rom_id, &rom, meta).await,
        ),
        other => other,
    };
    let mut dto = GameMetadataDto::from(imported.unwrap_or_else(|| GameMetadata {
        rom_id: rom_id.to_owned(),
        title: super::library::rom_title(&rom),
        description: None,
        cover_url: None,
        release_date: None,
        genre: None,
        provider_source: None,
        game_uuid: None,
        details: Default::default(),
    }));
    if let Some(title) = rom.user_title {
        dto.title = title;
    }
    if let Some(catalog) = &dto.gamekave {
        let reg = catalog.rom_region.as_deref().or(detected_region);
        if let Some(regional) = scraping::rom_name::pick_regional_cover(
            &catalog.regional_covers,
            reg,
            dto.cover_url.as_deref(),
        ) {
            dto.cover_url = Some(regional.to_owned());
        }
        if let Some(language) = language {
            let tag = language.to_ascii_lowercase();
            if let Some(description) = catalog.descriptions.get(&tag).or_else(|| {
                let (base, _) = tag.split_once('-')?;
                catalog.descriptions.get(base)
            }) {
                dto.description = Some(description.clone());
            }
        }
    }
    if let Some(edits) = edits {
        dto = apply_edits(dto, edits);
    }
    Ok(Some(dto))
}

#[tauri::command]
pub async fn get_rom_metadata(
    state: State<'_, AppState>,
    rom_id: String,
    language: Option<String>,
) -> Result<Option<GameMetadataDto>, AppError> {
    let pool = pool(&state)?;
    load_rom_metadata(&pool, &rom_id, language.as_deref()).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "PendingMatch"))]
pub struct PendingMatchDto {
    pub rom_id: String,
    pub file_stem: String,
    pub provider: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
}

#[tauri::command]
pub async fn list_pending_matches(
    state: State<'_, AppState>,
) -> Result<Vec<PendingMatchDto>, AppError> {
    let list = db::MetadataRepo::new(pool(&state)?)
        .list_pending()
        .await
        .map_err(AppError::from)?;
    Ok(list
        .into_iter()
        .map(|p| PendingMatchDto {
            rom_id: p.rom_id,
            file_stem: p.file_stem,
            provider: p.provider,
            title: p.candidate.title,
            description: p.candidate.description,
            cover_url: p.candidate.cover_url,
            release_date: p.candidate.release_date,
            genre: p.candidate.genre,
        })
        .collect())
}

#[tauri::command]
pub async fn review_pending_match(
    state: State<'_, AppState>,
    rom_id: String,
    refresh: bool,
) -> Result<Option<GameMetadataDto>, AppError> {
    let pool = pool(&state)?;
    let repo = db::MetadataRepo::new(pool.clone());
    let stored = repo
        .pending_candidate(&rom_id)
        .await
        .map_err(AppError::from)?
        .ok_or("correspondência pendente indisponível")?;
    let candidate = if refresh {
        let cfg = metadata_config(&pool).await?;
        let Some(candidate) = scraping::preview_rom(&pool, cfg, &rom_id).await? else {
            return Ok(None);
        };
        if !repo
            .replace_pending_candidate(&rom_id, &candidate)
            .await
            .map_err(AppError::from)?
        {
            return Err("correspondência pendente indisponível".into());
        }
        candidate
    } else {
        stored
    };
    Ok(Some(
        GameMetadata {
            rom_id,
            title: candidate.title,
            description: candidate.description,
            cover_url: candidate.cover_url,
            release_date: candidate.release_date,
            genre: candidate.genre,
            provider_source: Some(candidate.provider.clone()),
            game_uuid: (candidate.provider == "gamekave" && !candidate.external_id.is_empty())
                .then_some(candidate.external_id),
            details: candidate.details,
        }
        .into(),
    ))
}

#[tauri::command]
pub async fn read_game_artwork(path: String) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::covers::read_artwork_file(std::path::Path::new(&path))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn resolve_pending_match(
    state: State<'_, AppState>,
    rom_id: String,
    accept: bool,
    title: Option<String>,
    details: Option<GameMetadataEdits>,
) -> Result<(), AppError> {
    super::library::validate_metadata_edits(details.as_ref())?;
    db::MetadataRepo::new(pool(&state)?)
        .resolve_pending_with_edits(&rom_id, accept, title.as_deref(), details.as_ref())
        .await
        .map_err(AppError::from)?;
    // Aceito = pode ter trazido uma `cover_url` nova — descarta o cache pra
    // próxima leitura buscar essa em vez de continuar servindo a antiga.
    if accept {
        crate::covers::invalidate(&state.covers_dir, &rom_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn resolve_pending_matches_bulk(
    state: State<'_, AppState>,
    rom_ids: Vec<String>,
    accept: bool,
) -> Result<usize, AppError> {
    let resolved = db::MetadataRepo::new(pool(&state)?)
        .resolve_pending_bulk(&rom_ids, accept)
        .await
        .map_err(AppError::from)?;
    if accept {
        for rom_id in &resolved {
            crate::covers::invalidate(&state.covers_dir, rom_id);
        }
    }
    Ok(resolved.len())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "ScrapeProgress"))]
pub struct ScrapeProgressDto {
    pub running: bool,
    #[cfg_attr(
        test,
        ts(
            type = "'idle' | 'preparing' | 'running' | 'cancelling' | 'completed' | 'cancelled' | 'failed'"
        )
    )]
    pub status: &'static str,
    pub done: usize,
    pub total: usize,
    pub auto: usize,
    pub pending: usize,
    pub failed: usize,
    // Aviso do provedor (ss_dev_login), independente do estado da leva.
    pub error: Option<String>,
}

#[tauri::command]
pub fn metadata_scan_progress(state: State<'_, AppState>) -> ScrapeProgressDto {
    use scraping::ScrapeState;
    let (status, done, total, auto, pending, failed, error) = state.scrape.snapshot();
    ScrapeProgressDto {
        running: status.is_active(),
        status: match status {
            ScrapeState::Idle => "idle",
            ScrapeState::Preparing => "preparing",
            ScrapeState::Running => "running",
            ScrapeState::Cancelling => "cancelling",
            ScrapeState::Completed => "completed",
            ScrapeState::Cancelled => "cancelled",
            ScrapeState::Failed => "failed",
        },
        done,
        total,
        auto,
        pending,
        failed,
        error,
    }
}

#[tauri::command]
pub fn cancel_metadata_scan(state: State<'_, AppState>) {
    state.scrape.cancel();
}

/// Configuração de metadados com as credenciais do chaveiro do sistema, pra
/// crate `scraping`.
async fn metadata_config(pool: &db::Db) -> Result<domain::metadata::MetadataConfig, String> {
    let repo = db::MetadataRepo::new(pool.clone());
    crate::credentials::load_config(&repo, &crate::credentials::OsKeyring).await
}

/// Dispara uma leva de scraping em background e volta na hora. O progresso é
/// consultado por `metadata_scan_progress`.
#[tauri::command]
pub async fn start_metadata_scan(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let pool = pool(&state)?;
    let progress = state.scrape.clone();
    if !progress.begin() {
        return Err("já tem um scraping em andamento".into());
    }
    let covers = crate::covers::CoverDir(state.covers_dir.clone());
    tauri::async_runtime::spawn(async move {
        let run = async {
            crate::catalog::run_before_own_scraper(&app, crate::catalog::PassScope::All).await;
            let cfg = metadata_config(&pool).await?;
            scraping::scrape_pending(pool, progress.clone(), cfg, &covers).await
        };
        let result = run.await;
        progress.finish(result.is_err());
        if let Err(e) = result {
            log::warn!("metadata: leva falhou: {e}");
        }
    });
    Ok(())
}

/// "Buscar de novo" de um jogo só (gaveta de informações). Espera a consulta
/// terminar e devolve o resultado: `"auto"` (metadado aplicado), `"pending"`
/// (foi pra revisão) ou `"none"` (nenhum provedor achou).
#[tauri::command]
pub async fn rescrape_rom(
    app: AppHandle,
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<&'static str, AppError> {
    if let Some(outcome) = crate::catalog::resolve_one_for_rescrape(&app, &rom_id).await {
        return Ok(outcome);
    }
    let pool = pool(&state)?;
    let cfg = metadata_config(&pool).await?;
    let covers = crate::covers::CoverDir(state.covers_dir.clone());
    let outcome = scraping::scrape_one(pool, cfg, &covers, &rom_id).await?;
    Ok(match outcome {
        scraping::Outcome::Auto => "auto",
        scraping::Outcome::Pending => "pending",
        scraping::Outcome::NoMatch => "none",
    })
}
