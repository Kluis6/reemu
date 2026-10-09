//! Metadata / scraping (etapa 09).

use super::*;

use domain::metadata::{GameMetadata, MetadataConfig, MetadataRepository};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
pub async fn get_metadata_config(state: State<'_, AppState>) -> Result<MetadataConfigDto, String> {
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
    state: State<'_, AppState>,
    config: MetadataConfigDto,
) -> Result<(), String> {
    let norm = |s: Option<String>| s.filter(|v| !v.trim().is_empty());
    let repo = db::MetadataRepo::new(pool(&state)?);
    let cfg = MetadataConfig {
        provider: config.provider,
        screenscraper_user: norm(config.screenscraper_user),
        screenscraper_password: norm(config.screenscraper_password),
        thegamesdb_api_key: norm(config.thegamesdb_api_key),
    };
    crate::credentials::save_config(&repo, &crate::credentials::OsKeyring, cfg).await
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
        .map_err(|e| e.to_string())?;
    // Aceito = pode ter trazido uma `cover_url` nova — descarta o cache pra
    // próxima leitura buscar essa em vez de continuar servindo a antiga.
    if accept {
        crate::covers::invalidate(&state.covers_dir, &rom_id);
    }
    Ok(())
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
    /// Por que a leva parou (código: `ss_dev_login`), `None` se não parou.
    pub error: Option<String>,
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
        error: state.scrape.error(),
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
    let covers_dir = state.covers_dir.clone();
    stop.store(false, std::sync::atomic::Ordering::Relaxed);
    tauri::async_runtime::spawn(async move {
        if let Err(e) =
            crate::scraping::scrape_pending(pool, progress.clone(), stop, covers_dir).await
        {
            log::warn!("metadata: leva falhou: {e}");
            progress
                .running
                .store(false, std::sync::atomic::Ordering::Relaxed);
        }
    });
    Ok(())
}

/// "Buscar de novo" de um jogo só (gaveta de informações). Espera a consulta
/// terminar e devolve o resultado: `"auto"` (metadado aplicado), `"pending"`
/// (foi pra revisão) ou `"none"` (nenhum provedor achou).
#[tauri::command]
pub async fn rescrape_rom(
    state: State<'_, AppState>,
    rom_id: String,
) -> Result<&'static str, String> {
    let outcome =
        crate::scraping::scrape_one(pool(&state)?, state.covers_dir.clone(), &rom_id).await?;
    Ok(match outcome {
        crate::scraping::Outcome::Auto => "auto",
        crate::scraping::Outcome::Pending => "pending",
        crate::scraping::Outcome::NoMatch => "none",
    })
}
