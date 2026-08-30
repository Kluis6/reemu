use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::input::DevicePortRepository;
use sqlx::Row;

pub struct DevicePortsRepo {
    db: Db,
}

impl DevicePortsRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DevicePortRepository for DevicePortsRepo {
    async fn list(&self) -> Result<Vec<(String, u8)>, RepoError> {
        let rows = sqlx::query("SELECT guid, port_index FROM device_port_assignment")
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        rows.iter()
            .map(|row| {
                let guid: String = row.try_get("guid").map_err(be)?;
                let port: i64 = row.try_get("port_index").map_err(be)?;
                Ok((guid, port.clamp(0, 3) as u8))
            })
            .collect()
    }

    async fn set(&self, guid: &str, port: u8) -> Result<(), RepoError> {
        // FK: `device_port_assignment.guid` referencia `controller_mappings`.
        // Garante uma linha (vazia) pro guid antes de atribuir a porta.
        sqlx::query(
            "INSERT OR IGNORE INTO controller_mappings (guid, display_name, layout_json, source) \
             VALUES (?1, ?1, '[]', 'user_override')",
        )
        .bind(guid)
        .execute(&self.db)
        .await
        .map_err(be)?;
        sqlx::query(
            "INSERT INTO device_port_assignment (guid, port_index) VALUES (?1, ?2) \
             ON CONFLICT(guid) DO UPDATE SET port_index = excluded.port_index",
        )
        .bind(guid)
        .bind(i64::from(port.min(3)))
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn clear(&self, guid: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM device_port_assignment WHERE guid = ?1")
            .bind(guid)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
