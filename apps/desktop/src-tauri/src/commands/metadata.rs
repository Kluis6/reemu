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
