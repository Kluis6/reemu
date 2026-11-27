use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::metadata::{
    GameMetadata, GameMetadataEdits, MatchStatus, MetadataConfig, MetadataRepository, PendingMatch,
    ScrapeCandidate,
};
use sqlx::{QueryBuilder, Row, Sqlite};

pub struct MetadataRepo {
    db: Db,
}

#[derive(Debug, Clone, Default)]
pub struct RomMetadataSummary {
    pub title: String,
    pub cover_url: Option<String>,
    pub has_cover: bool,
    pub game_uuid: Option<String>,
    pub rom_region: Option<String>,
    pub rom_revision: Option<String>,
}

impl MetadataRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn cover_overrides(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, RepoError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT rom_id, json_extract(fields_json, '$.coverUrl') \
             FROM game_metadata_edits \
             WHERE COALESCE(json_extract(fields_json, '$.coverUrl'), '') <> ''",
        )
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        Ok(rows.into_iter().collect())
    }

    pub async fn cover_override_ids(&self) -> Result<std::collections::HashSet<String>, RepoError> {
        Ok(self.cover_overrides().await?.into_keys().collect())
    }

    pub async fn clear_artwork_edits(&self, rom_id: &str) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE game_metadata_edits \
             SET fields_json = json_patch( \
                 fields_json, \
                 '{\"coverUrl\":null,\"fanartUrl\":null,\"screenshotUrl\":null,\"logoUrl\":null}' \
             ) \
             WHERE rom_id = ?1",
        )
        .bind(rom_id)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    /// `rom_id -> (title, tem_cover_url)` de `game_metadata` numa leitura só,
    /// para `list_roms` montar os cards da biblioteca sem N queries.
    pub async fn titles_and_covers(
        &self,
    ) -> Result<std::collections::HashMap<String, (String, bool)>, RepoError> {
        Ok(self
            .library_summaries()
            .await?
            .into_iter()
            .map(|(rom_id, s)| (rom_id, (s.title, s.has_cover)))
            .collect())
    }

    /// Resumo leve de `game_metadata` para `list_roms` agrupar versões regionais
    /// e resolver capas sem desserializar todas as fichas completas.
    pub async fn library_summaries(
        &self,
    ) -> Result<std::collections::HashMap<String, RomMetadataSummary>, RepoError> {
        #[derive(sqlx::FromRow)]
        struct SummaryRow {
            rom_id: String,
            title: String,
            cover_url: Option<String>,
            has_cover: i64,
            game_uuid: Option<String>,
            rom_region: Option<String>,
            rom_revision: Option<String>,
        }

        let rows: Vec<SummaryRow> = sqlx::query_as(
            "SELECT rom_id, title, cover_url, \
                    ( (cover_url IS NOT NULL AND TRIM(cover_url) <> '') \
                      OR COALESCE(json_array_length(gamekave_json, '$.covers'), 0) > 0 ) AS has_cover, \
                    game_uuid, \
                    json_extract(gamekave_json, '$.romRegion') AS rom_region, \
                    json_extract(gamekave_json, '$.romRevision') AS rom_revision \
             FROM game_metadata",
        )
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.rom_id,
                    RomMetadataSummary {
                        title: row.title,
                        cover_url: row.cover_url.filter(|s| !s.trim().is_empty()),
                        has_cover: row.has_cover != 0,
                        game_uuid: row.game_uuid.filter(|s| !s.trim().is_empty()),
                        rom_region: row.rom_region.filter(|s| !s.trim().is_empty()),
                        rom_revision: row.rom_revision.filter(|s| !s.trim().is_empty()),
                    },
                )
            })
            .collect())
    }

    pub async fn is_user_confirmed(&self, rom_id: &str) -> Result<bool, RepoError> {
        let exists: i64 = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM scrape_matches WHERE rom_id = ?1 AND status = 'user_confirmed')",
        )
        .bind(rom_id)
        .fetch_one(&self.db)
        .await
        .map_err(be)?;
        Ok(exists != 0)
    }

    pub async fn get_by_game_uuid(
        &self,
        game_uuid: &str,
    ) -> Result<Option<GameMetadata>, RepoError> {
        let row = sqlx::query(
            "SELECT rom_id, title, description, cover_url, release_date, genre, provider_source, \
             game_uuid, developer, publisher, players, rating, age_rating, modes, \
             screenshot_url, title_screen_url, fanart_url, logo_url, video_url, gamekave_json \
             FROM game_metadata WHERE game_uuid = ?1 LIMIT 1",
        )
        .bind(game_uuid)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        row.as_ref().map(row_to_meta).transpose()
    }

    pub async fn cover_override(&self, rom_id: &str) -> Result<Option<String>, RepoError> {
        let cover: Option<Option<String>> = sqlx::query_scalar(
            "SELECT json_extract(fields_json, '$.coverUrl') FROM game_metadata_edits WHERE rom_id = ?1",
        )
        .bind(rom_id)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        Ok(cover.flatten())
    }

    pub async fn get_edits(&self, rom_id: &str) -> Result<Option<GameMetadataEdits>, RepoError> {
        let raw: Option<String> =
            sqlx::query_scalar("SELECT fields_json FROM game_metadata_edits WHERE rom_id = ?1")
                .bind(rom_id)
                .fetch_optional(&self.db)
                .await
                .map_err(be)?;
        raw.map(|json| serde_json::from_str(&json).map_err(|e| RepoError::Corrupt(e.to_string())))
            .transpose()
    }

    pub async fn pending_candidate(
        &self,
        rom_id: &str,
    ) -> Result<Option<ScrapeCandidate>, RepoError> {
        let raw: Option<String> = sqlx::query_scalar(
            "SELECT candidate_json FROM scrape_matches WHERE rom_id = ?1 AND status = 'pending_review'",
        )
        .bind(rom_id)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?
        .flatten();
        raw.map(|json| serde_json::from_str(&json).map_err(|e| RepoError::Corrupt(e.to_string())))
            .transpose()
    }

    pub async fn replace_pending_candidate(
        &self,
        rom_id: &str,
        candidate: &ScrapeCandidate,
    ) -> Result<bool, RepoError> {
        let json =
            serde_json::to_string(candidate).map_err(|e| RepoError::Corrupt(e.to_string()))?;
        let update = sqlx::query(
            "UPDATE scrape_matches SET provider = ?2, external_id = ?3, candidate_json = ?4
             WHERE rom_id = ?1 AND status = 'pending_review'",
        )
        .bind(rom_id)
        .bind(&candidate.provider)
        .bind(&candidate.external_id)
        .bind(json)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(update.rows_affected() != 0)
    }

    pub async fn resolve_pending_with_edits(
        &self,
        rom_id: &str,
        accept: bool,
        title: Option<&str>,
        edits: Option<&GameMetadataEdits>,
    ) -> Result<(), RepoError> {
        if !accept {
            sqlx::query("UPDATE scrape_matches SET status = 'no_match' WHERE rom_id = ?1 AND status = 'pending_review'")
                .bind(rom_id)
                .execute(&self.db)
                .await
                .map_err(be)?;
            return Ok(());
        }
        let mut tx = self.db.begin().await.map_err(be)?;
        let json: Option<String> = sqlx::query_scalar::<_, Option<String>>(
            "UPDATE scrape_matches SET status = 'user_confirmed'
             WHERE rom_id = ?1 AND status = 'pending_review' RETURNING candidate_json",
        )
        .bind(rom_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(be)?
        .flatten();
        let json = json.ok_or_else(|| RepoError::Backend("pendência sem candidato".into()))?;
        let candidate: ScrapeCandidate = serde_json::from_str(&json)
            .map_err(|e| RepoError::Corrupt(format!("candidate JSON inválido: {e}")))?;
        write_metadata(&mut *tx, &meta_from_candidate(rom_id, &candidate)).await?;
        super::roms_repo::RomsRepo::update_metadata(&mut tx, rom_id, title, None, edits).await?;
        tx.commit().await.map_err(be)?;
        Ok(())
    }

    /// Resolve várias correspondências pendentes em uma única transação SQLite.
    /// Devolve os `rom_id`s que estavam em `pending_review` e foram resolvidos.
    pub async fn resolve_pending_bulk(
        &self,
        rom_ids: &[String],
        accept: bool,
    ) -> Result<Vec<String>, RepoError> {
        const CHUNK: usize = 200;

        let mut seen = std::collections::HashSet::with_capacity(rom_ids.len());
        let unique: Vec<&str> = rom_ids
            .iter()
            .map(String::as_str)
            .filter(|id| !id.is_empty() && seen.insert(*id))
            .collect();
        if unique.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.db.begin().await.map_err(be)?;
        let mut resolved: Vec<String> = Vec::with_capacity(unique.len());

        if !accept {
            for chunk in unique.chunks(CHUNK) {
                let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
                    "UPDATE scrape_matches SET status = 'no_match' \
                     WHERE status = 'pending_review' AND rom_id IN (",
                );
                let mut sep = qb.separated(", ");
                for id in chunk {
                    sep.push_bind((*id).to_owned());
                }
                sep.push_unseparated(") RETURNING rom_id");
                let ids: Vec<String> = qb
                    .build_query_scalar::<String>()
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(be)?;
                resolved.extend(ids);
            }
        } else {
            for chunk in unique.chunks(CHUNK) {
                let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
                    "UPDATE scrape_matches SET status = 'user_confirmed' \
                     WHERE status = 'pending_review' AND rom_id IN (",
                );
                let mut sep = qb.separated(", ");
                for id in chunk {
                    sep.push_bind((*id).to_owned());
                }
                sep.push_unseparated(") RETURNING rom_id, candidate_json");
                let rows: Vec<(String, Option<String>)> = qb
                    .build_query_as::<(String, Option<String>)>()
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(be)?;
                for (rom_id, raw) in rows {
                    let json =
                        raw.ok_or_else(|| RepoError::Backend("pendência sem candidato".into()))?;
                    let candidate: ScrapeCandidate = serde_json::from_str(&json)
                        .map_err(|e| RepoError::Corrupt(format!("candidate JSON inválido: {e}")))?;
                    write_metadata(&mut *tx, &meta_from_candidate(&rom_id, &candidate)).await?;
                    resolved.push(rom_id);
                }
            }
        }

        tx.commit().await.map_err(be)?;
        Ok(resolved)
    }
}

async fn write_metadata<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
    m: &GameMetadata,
) -> Result<(), RepoError> {
    sqlx::query(
        "INSERT INTO game_metadata \
         (id, rom_id, title, description, cover_url, release_date, genre, provider_source, \
          developer, publisher, players, rating, age_rating, modes, \
          screenshot_url, title_screen_url, fanart_url, logo_url, video_url, gamekave_json, \
          game_uuid) \
         VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, \
                 ?14, ?15, ?16, ?17, ?18, ?19, ?20) \
         ON CONFLICT(rom_id) DO UPDATE SET \
           title = excluded.title, description = excluded.description, \
           cover_url = excluded.cover_url, release_date = excluded.release_date, \
           genre = excluded.genre, provider_source = excluded.provider_source, \
           developer = excluded.developer, publisher = excluded.publisher, \
           players = excluded.players, rating = excluded.rating, \
           age_rating = excluded.age_rating, modes = excluded.modes, \
           screenshot_url = excluded.screenshot_url, \
           title_screen_url = excluded.title_screen_url, \
           fanart_url = excluded.fanart_url, logo_url = excluded.logo_url, \
           video_url = excluded.video_url, gamekave_json = excluded.gamekave_json, \
           game_uuid = excluded.game_uuid",
    )
    .bind(&m.rom_id)
    .bind(&m.title)
    .bind(&m.description)
    .bind(&m.cover_url)
    .bind(&m.release_date)
    .bind(&m.genre)
    .bind(&m.provider_source)
    .bind(&m.details.developer)
    .bind(&m.details.publisher)
    .bind(&m.details.players)
    .bind(m.details.rating.map(i64::from))
    .bind(&m.details.age_rating)
    .bind(&m.details.modes)
    .bind(&m.details.screenshot_url)
    .bind(&m.details.title_screen_url)
    .bind(&m.details.fanart_url)
    .bind(&m.details.logo_url)
    .bind(&m.details.video_url)
    .bind(
        m.details
            .gamekave
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| RepoError::Corrupt(e.to_string()))?,
    )
    .bind(&m.game_uuid)
    .execute(executor)
    .await
    .map_err(be)?;
    Ok(())
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
        game_uuid: (c.provider == "gamekave" && !c.external_id.is_empty())
            .then(|| c.external_id.clone()),
        details: c.details.clone(),
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
        game_uuid: row.try_get("game_uuid").map_err(be)?,
        details: domain::metadata::GameDetails {
            developer: row.try_get("developer").map_err(be)?,
            publisher: row.try_get("publisher").map_err(be)?,
            players: row.try_get("players").map_err(be)?,
            rating: row
                .try_get::<Option<i64>, _>("rating")
                .map_err(be)?
                .and_then(|r| u8::try_from(r).ok()),
            age_rating: row.try_get("age_rating").map_err(be)?,
            modes: row.try_get("modes").map_err(be)?,
            screenshot_url: row.try_get("screenshot_url").map_err(be)?,
            title_screen_url: row.try_get("title_screen_url").map_err(be)?,
            fanart_url: row.try_get("fanart_url").map_err(be)?,
            logo_url: row.try_get("logo_url").map_err(be)?,
            video_url: row.try_get("video_url").map_err(be)?,
            gamekave: row
                .try_get::<Option<String>, _>("gamekave_json")
                .map_err(be)?
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|e| RepoError::Corrupt(e.to_string()))?,
        },
    })
}

#[async_trait]
impl MetadataRepository for MetadataRepo {
    async fn get_config(&self) -> Result<MetadataConfig, RepoError> {
        let row = sqlx::query(
            "SELECT provider, screenscraper_user, screenscraper_password, thegamesdb_api_key \
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
            thegamesdb_api_key: row.try_get("thegamesdb_api_key").map_err(be)?,
        })
    }

    async fn set_config(&self, cfg: &MetadataConfig) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE metadata_config SET provider = ?1, screenscraper_user = ?2, \
             screenscraper_password = ?3, thegamesdb_api_key = ?4 WHERE id = 1",
        )
        .bind(&cfg.provider)
        .bind(&cfg.screenscraper_user)
        .bind(&cfg.screenscraper_password)
        .bind(&cfg.thegamesdb_api_key)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn get_metadata(&self, rom_id: &str) -> Result<Option<GameMetadata>, RepoError> {
        let row = sqlx::query(
            "SELECT rom_id, title, description, cover_url, release_date, genre, provider_source, \
             game_uuid, developer, publisher, players, rating, age_rating, modes, \
             screenshot_url, title_screen_url, fanart_url, logo_url, video_url, gamekave_json \
             FROM game_metadata WHERE rom_id = ?1",
        )
        .bind(rom_id)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        row.as_ref().map(row_to_meta).transpose()
    }

    async fn upsert_metadata(&self, m: &GameMetadata) -> Result<(), RepoError> {
        write_metadata(&self.db, m).await
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
        self.resolve_pending_with_edits(rom_id, accept, None, None)
            .await
    }
}
