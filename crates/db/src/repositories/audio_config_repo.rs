use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::audio::{AudioConfig, AudioConfigRepository};
use domain::error::RepoError;
use sqlx::Row;

pub struct AudioConfigRepo {
    db: Db,
}

impl AudioConfigRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AudioConfigRepository for AudioConfigRepo {
    async fn get(&self) -> Result<AudioConfig, RepoError> {
        // A linha id=1 é inserida pela migration; se sumir, é corrupção.
        let row = sqlx::query(
            "SELECT output_device_id, output_device_name, rate_control_enabled, \
             rate_control_delta, sample_rate_preference FROM audio_config WHERE id = 1",
        )
        .fetch_optional(&self.db)
        .await
        .map_err(be)?
        .ok_or_else(|| RepoError::Corrupt("audio_config sem a linha id=1".into()))?;

        let delta: f64 = row.try_get("rate_control_delta").map_err(be)?;
        let sample_rate: Option<i64> = row.try_get("sample_rate_preference").map_err(be)?;

        Ok(AudioConfig {
            output_device_id: row.try_get("output_device_id").map_err(be)?,
            output_device_name: row.try_get("output_device_name").map_err(be)?,
            rate_control_enabled: row.try_get("rate_control_enabled").map_err(be)?,
            rate_control_delta: delta as f32,
            sample_rate_preference: sample_rate.map(|v| v as u32),
        })
    }

    async fn update(&self, config: &AudioConfig) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE audio_config SET \
             output_device_id = ?1, output_device_name = ?2, rate_control_enabled = ?3, \
             rate_control_delta = ?4, sample_rate_preference = ?5 WHERE id = 1",
        )
        .bind(&config.output_device_id)
        .bind(&config.output_device_name)
        .bind(config.rate_control_enabled)
        .bind(config.rate_control_delta as f64)
        .bind(config.sample_rate_preference.map(|v| v as i64))
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }
}
