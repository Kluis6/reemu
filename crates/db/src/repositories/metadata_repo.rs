use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::metadata::{
    GameMetadata, MatchStatus, MetadataConfig, MetadataRepository, PendingMatch, ScrapeCandidate,
};
use sqlx::Row;

pub struct MetadataRepo {
    db: Db,
}

impl MetadataRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

fn status_to_db(s: MatchStatus) -> &'static str {
    match s {
        MatchStatus::AutoMatched => "auto_matched",
        MatchStatus::PendingReview => "pending_review",
        MatchStatus::UserConfirmed => "user_confirmed",
        MatchStatus::NoMatch => "no_match",
    }
}

fn meta_from_candidate(rom_id: &str, c: &ScrapeCandidate) -> GameMetadata {
    GameMetadata {
        rom_id: rom_id.to_string(),
        title: c.title.clone(),
        description: c.description.clone(),
        cover_url: c.cover_url.clone(),
        release_date: c.release_date.clone(),
        genre: c.genre.clone(),
        provider_source: Some(c.provider.clone()),
    }
}

fn row_to_meta(row: &sqlx::sqlite::SqliteRow) -> Result<GameMetadata, RepoError> {
    Ok(GameMetadata {
        rom_id: row.try_get("rom_id").map_err(be)?,
        title: row.try_get("title").map_err(be)?,
        description: row.try_get("description").map_err(be)?,
        cover_url: row.try_get("cover_url").map_err(be)?,
        release_date: row.try_get("release_date").map_err(be)?,
        genre: row.try_get("genre").map_err(be)?,
        provider_source: row.try_get("provider_source").map_err(be)?,
    })
}

#[async_trait]
impl MetadataRepository for MetadataRepo {
    async fn get_config(&self) -> Result<MetadataConfig, RepoError> {
        let row = sqlx::query(
            "SELECT provider, screenscraper_user, screenscraper_password \
             FROM metadata_config WHERE id = 1",
        )
        .fetch_optional(&self.db)
        .await
        .map_err(be)?
        .ok_or_else(|| RepoError::Corrupt("metadata_config sem a linha id=1".into()))?;
        Ok(MetadataConfig {
            provider: row.try_get("provider").map_err(be)?,
            screenscraper_user: row.try_get("screenscraper_user").map_err(be)?,
            screenscraper_password: row.try_get("screenscraper_password").map_err(be)?,
        })
    }

    async fn set_config(&self, cfg: &MetadataConfig) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE metadata_config SET provider = ?1, screenscraper_user = ?2, \
             screenscraper_password = ?3 WHERE id = 1",
        )
        .bind(&cfg.provider)
        .bind(&cfg.screenscraper_user)
        .bind(&cfg.screenscraper_password)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn get_metadata(&self, rom_id: &str) -> Result<Option<GameMetadata>, RepoError> {
        let row = sqlx::query(
            "SELECT rom_id, title, description, cover_url, release_date, genre, provider_source \
             FROM game_metadata WHERE rom_id = ?1",
        )
        .bind(rom_id)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        row.as_ref().map(row_to_meta).transpose()
    }

    async fn upsert_metadata(&self, m: &GameMetadata) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO game_metadata \
             (id, rom_id, title, description, cover_url, release_date, genre, provider_source) \
             VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(rom_id) DO UPDATE SET \
               title = excluded.title, description = excluded.description, \
               cover_url = excluded.cover_url, release_date = excluded.release_date, \
               genre = excluded.genre, provider_source = excluded.provider_source",
        )
        .bind(&m.rom_id)
        .bind(&m.title)
        .bind(&m.description)
        .bind(&m.cover_url)
        .bind(&m.release_date)
        .bind(&m.genre)
        .bind(&m.provider_source)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn record_match(
        &self,
        rom_id: &str,
        candidate: &ScrapeCandidate,
        status: MatchStatus,
    ) -> Result<(), RepoError> {
        let json = serde_json::to_string(candidate)
            .map_err(|e| RepoError::Backend(format!("candidate JSON: {e}")))?;
        sqlx::query(
            "INSERT INTO scrape_matches \
             (id, rom_id, provider, external_id, confidence_score, status, candidate_json) \
             VALUES (?1, ?1, ?2, ?3, NULL, ?4, ?5) \
             ON CONFLICT(rom_id) DO UPDATE SET \
               provider = excluded.provider, external_id = excluded.external_id, \
               status = excluded.status, candidate_json = excluded.candidate_json",
        )
        .bind(rom_id)
        .bind(&candidate.provider)
        .bind(&candidate.external_id)
        .bind(status_to_db(status))
        .bind(&json)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn rom_ids_without_match(&self) -> Result<Vec<String>, RepoError> {
        let rows = sqlx::query(
            "SELECT r.id FROM roms r \
             LEFT JOIN scrape_matches m ON m.rom_id = r.id \
             WHERE m.rom_id IS NULL ORDER BY r.system_id, r.file_path",
        )
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(|r| r.try_get("id").map_err(be)).collect()
    }

    async fn list_pending(&self) -> Result<Vec<PendingMatch>, RepoError> {
        let rows = sqlx::query(
            "SELECT m.rom_id, m.provider, m.external_id, m.candidate_json, r.file_path \
             FROM scrape_matches m JOIN roms r ON r.id = m.rom_id \
             WHERE m.status = 'pending_review' ORDER BY r.file_path",
        )
        .fetch_all(&self.db)
        .await
        .map_err(be)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in &rows {
            let json: Option<String> = r.try_get("candidate_json").map_err(be)?;
            let Some(json) = json else { continue };
            let candidate: ScrapeCandidate = serde_json::from_str(&json)
                .map_err(|e| RepoError::Corrupt(format!("candidate JSON inválido: {e}")))?;
            let file_path: String = r.try_get("file_path").map_err(be)?;
            let file_stem = std::path::Path::new(&file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file_path)
                .to_string();
            out.push(PendingMatch {
                rom_id: r.try_get("rom_id").map_err(be)?,
                file_stem,
                provider: r.try_get("provider").map_err(be)?,
                external_id: r.try_get("external_id").map_err(be)?,
                candidate,
            });
        }
        Ok(out)
    }

    async fn resolve_pending(&self, rom_id: &str, accept: bool) -> Result<(), RepoError> {
        if !accept {
            sqlx::query("UPDATE scrape_matches SET status = 'no_match' WHERE rom_id = ?1")
                .bind(rom_id)
                .execute(&self.db)
                .await
                .map_err(be)?;
            return Ok(());
        }
        let json: Option<String> = sqlx::query_scalar::<_, Option<String>>(
            "SELECT candidate_json FROM scrape_matches WHERE rom_id = ?1",
        )
        .bind(rom_id)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?
        .flatten();
        let json = json.ok_or_else(|| RepoError::Backend("pendência sem candidato".into()))?;
        let candidate: ScrapeCandidate = serde_json::from_str(&json)
            .map_err(|e| RepoError::Corrupt(format!("candidate JSON inválido: {e}")))?;
        self.upsert_metadata(&meta_from_candidate(rom_id, &candidate))
            .await?;
        sqlx::query("UPDATE scrape_matches SET status = 'user_confirmed' WHERE rom_id = ?1")
            .bind(rom_id)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
