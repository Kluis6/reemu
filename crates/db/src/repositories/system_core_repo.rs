use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::core_loader::SystemCoreRepository;
use domain::error::RepoError;
use sqlx::Row;

/// `system_core_prefs` — core preferido por plataforma.
pub struct SystemCoreRepo {
    db: Db,
}

impl SystemCoreRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SystemCoreRepository for SystemCoreRepo {
    async fn all(&self) -> Result<Vec<(String, String)>, RepoError> {
        let rows = sqlx::query("SELECT system_id, core_id FROM system_core_prefs")
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        rows.iter()
            .map(|r| {
                Ok((
                    r.try_get("system_id").map_err(be)?,
                    r.try_get("core_id").map_err(be)?,
                ))
            })
            .collect()
    }

    async fn set(&self, system_id: &str, core_id: &str) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO system_core_prefs (system_id, core_id) VALUES (?1, ?2) \
             ON CONFLICT(system_id) DO UPDATE SET core_id = excluded.core_id",
        )
        .bind(system_id)
        .bind(core_id)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn clear(&self, system_id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM system_core_prefs WHERE system_id = ?1")
            .bind(system_id)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
