use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::hotkeys::{HotkeyBinding, SystemAction, SystemHotkeyRepository};
use domain::input::RawInputEvent;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

pub struct SystemHotkeysRepo {
    db: Db,
}

impl SystemHotkeysRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

fn row_to_binding(row: &SqliteRow) -> Result<HotkeyBinding, RepoError> {
    let action: String = row.try_get("action").map_err(be)?;
    let action = SystemAction::from_wire(&action).ok_or_else(|| {
        RepoError::Corrupt(format!("system_hotkeys.action desconhecida: {action}"))
    })?;
    let trigger_json: String = row.try_get("trigger_json").map_err(be)?;
    let trigger: Vec<RawInputEvent> = serde_json::from_str(&trigger_json)
        .map_err(|e| RepoError::Corrupt(format!("system_hotkeys.trigger_json inválido: {e}")))?;
    Ok(HotkeyBinding {
        action,
        trigger,
        device_guid: row.try_get("device_guid").map_err(be)?,
    })
}

#[async_trait]
impl SystemHotkeyRepository for SystemHotkeysRepo {
    async fn list(&self) -> Result<Vec<HotkeyBinding>, RepoError> {
        let rows = sqlx::query("SELECT action, trigger_json, device_guid FROM system_hotkeys")
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        rows.iter().map(row_to_binding).collect()
    }

    async fn set(&self, binding: &HotkeyBinding) -> Result<(), RepoError> {
        if binding.trigger.is_empty() {
            return Err(RepoError::Backend("trigger vazio".into()));
        }
        let trigger_json = serde_json::to_string(&binding.trigger)
            .map_err(|e| RepoError::Backend(e.to_string()))?;
        // `id` = nome da ação: garante uma linha por `SystemAction`.
        sqlx::query(
            "INSERT OR REPLACE INTO system_hotkeys (id, action, trigger_json, device_guid) \
             VALUES (?1, ?1, ?2, ?3)",
        )
        .bind(binding.action.as_wire())
        .bind(&trigger_json)
        .bind(&binding.device_guid)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn delete(&self, action: SystemAction) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM system_hotkeys WHERE action = ?1")
            .bind(action.as_wire())
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
