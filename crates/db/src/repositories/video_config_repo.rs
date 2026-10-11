use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::video::{VideoConfig, VideoConfigRepository};
use sqlx::Row;

pub struct VideoConfigRepo {
    db: Db,
}

impl VideoConfigRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl VideoConfigRepository for VideoConfigRepo {
    async fn get(&self) -> Result<VideoConfig, RepoError> {
        // A linha id=1 é inserida pela migration; se sumir, é corrupção.
        let row = sqlx::query(
            "SELECT integer_scaling, hdr, hdr_paper_white_nits, hdr_highlights, hdr_peak_nits \
             FROM video_config WHERE id = 1",
        )
        .fetch_optional(&self.db)
        .await
        .map_err(be)?
        .ok_or_else(|| RepoError::Corrupt("video_config sem a linha id=1".into()))?;

        let nits = |col: &str| -> Result<u32, RepoError> {
            let v: i64 = row.try_get(col).map_err(be)?;
            Ok(v.clamp(0, u32::MAX as i64) as u32)
        };
        Ok(VideoConfig {
            integer_scaling: row.try_get("integer_scaling").map_err(be)?,
            hdr: row.try_get("hdr").map_err(be)?,
            hdr_paper_white_nits: nits("hdr_paper_white_nits")?,
            hdr_highlights: row.try_get("hdr_highlights").map_err(be)?,
            hdr_peak_nits: nits("hdr_peak_nits")?,
        })
    }

    async fn update(&self, config: &VideoConfig) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE video_config SET integer_scaling = ?1, hdr = ?2, \
             hdr_paper_white_nits = ?3, hdr_highlights = ?4, hdr_peak_nits = ?5 WHERE id = 1",
        )
        .bind(config.integer_scaling)
        .bind(config.hdr)
        .bind(i64::from(config.hdr_paper_white_nits))
        .bind(config.hdr_highlights)
        .bind(i64::from(config.hdr_peak_nits))
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }
}
