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
        let row = sqlx::query("SELECT integer_scaling FROM video_config WHERE id = 1")
            .fetch_optional(&self.db)
            .await
            .map_err(be)?
            .ok_or_else(|| RepoError::Corrupt("video_config sem a linha id=1".into()))?;

        Ok(VideoConfig {
            integer_scaling: row.try_get("integer_scaling").map_err(be)?,
        })
    }

    async fn update(&self, config: &VideoConfig) -> Result<(), RepoError> {
        sqlx::query("UPDATE video_config SET integer_scaling = ?1 WHERE id = 1")
            .bind(config.integer_scaling)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
