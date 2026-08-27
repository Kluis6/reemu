use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::save_state::{SaveRamMetadata, SaveStateMetadata, SaveStateRepository};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

pub struct SaveStateRepo {
    db: Db,
}

impl SaveStateRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

const STATE_COLS: &str = "SELECT id, rom_id, core_id, slot, file_path, thumbnail_path, \
    created_at, play_time_at_save FROM save_states";

fn row_to_state(row: &SqliteRow) -> Result<SaveStateMetadata, RepoError> {
    let slot: Option<i64> = row.try_get("slot").map_err(be)?;
    let play_time: Option<i64> = row.try_get("play_time_at_save").map_err(be)?;
    Ok(SaveStateMetadata {
        id: row.try_get("id").map_err(be)?,
        rom_id: row.try_get("rom_id").map_err(be)?,
        core_id: row.try_get("core_id").map_err(be)?,
        slot: slot.map(|v| v as u32),
        file_path: row.try_get("file_path").map_err(be)?,
        thumbnail_path: row.try_get("thumbnail_path").map_err(be)?,
        created_at: row.try_get("created_at").map_err(be)?,
        play_time_at_save: play_time.map(|v| v as u64),
    })
}

fn row_to_ram(row: &SqliteRow) -> Result<SaveRamMetadata, RepoError> {
    Ok(SaveRamMetadata {
        id: row.try_get("id").map_err(be)?,
        rom_id: row.try_get("rom_id").map_err(be)?,
        core_id: row.try_get("core_id").map_err(be)?,
        file_path: row.try_get("file_path").map_err(be)?,
        updated_at: row.try_get("updated_at").map_err(be)?,
    })
}

#[async_trait]
impl SaveStateRepository for SaveStateRepo {
    async fn record_state(&self, meta: &SaveStateMetadata) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO save_states \
             (id, rom_id, core_id, slot, file_path, thumbnail_path, created_at, play_time_at_save) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&meta.id)
        .bind(&meta.rom_id)
        .bind(&meta.core_id)
        .bind(meta.slot.map(|v| v as i64))
        .bind(&meta.file_path)
        .bind(&meta.thumbnail_path)
        .bind(meta.created_at)
        .bind(meta.play_time_at_save.map(|v| v as i64))
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn get_state(&self, id: &str) -> Result<Option<SaveStateMetadata>, RepoError> {
        let row = sqlx::query(&format!("{STATE_COLS} WHERE id = ?1"))
            .bind(id)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.as_ref().map(row_to_state).transpose()
    }

    async fn list_states_for_rom(&self, rom_id: &str) -> Result<Vec<SaveStateMetadata>, RepoError> {
        let rows = sqlx::query(&format!(
            "{STATE_COLS} WHERE rom_id = ?1 ORDER BY created_at DESC, id"
        ))
        .bind(rom_id)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(row_to_state).collect()
    }

    async fn find_state_in_slot(
        &self,
        rom_id: &str,
        core_id: &str,
        slot: u32,
    ) -> Result<Option<SaveStateMetadata>, RepoError> {
        let row = sqlx::query(&format!(
            "{STATE_COLS} WHERE rom_id = ?1 AND core_id = ?2 AND slot = ?3 \
             ORDER BY created_at DESC LIMIT 1"
        ))
        .bind(rom_id)
        .bind(core_id)
        .bind(slot as i64)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        row.as_ref().map(row_to_state).transpose()
    }

    async fn delete_state(&self, id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM save_states WHERE id = ?1")
            .bind(id)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    async fn get_save_ram(
        &self,
        rom_id: &str,
        core_id: &str,
    ) -> Result<Option<SaveRamMetadata>, RepoError> {
        let row = sqlx::query(
            "SELECT id, rom_id, core_id, file_path, updated_at FROM save_ram \
             WHERE rom_id = ?1 AND core_id = ?2",
        )
        .bind(rom_id)
        .bind(core_id)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        row.as_ref().map(row_to_ram).transpose()
    }

    async fn upsert_save_ram(&self, meta: &SaveRamMetadata) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO save_ram (id, rom_id, core_id, file_path, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(rom_id, core_id) DO UPDATE SET \
               file_path = excluded.file_path, updated_at = excluded.updated_at",
        )
        .bind(&meta.id)
        .bind(&meta.rom_id)
        .bind(&meta.core_id)
        .bind(&meta.file_path)
        .bind(meta.updated_at)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }
}
