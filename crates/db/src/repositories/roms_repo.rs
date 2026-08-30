use crate::cascade::be;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::library::{Rom, RomRepository};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

pub struct RomsRepo {
    db: Db,
}

impl RomsRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Remove TODAS as ROMs da biblioteca (só os registros — arquivos em disco
    /// ficam). Save states / metadata caem em cascata. Devolve quantas saíram.
    pub async fn remove_all(&self) -> Result<u64, RepoError> {
        let r = sqlx::query("DELETE FROM roms")
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(r.rows_affected())
    }

    /// Remove todas as ROMs de um sistema (snes, nes, mastersystem…).
    /// Devolve quantas saíram.
    pub async fn remove_by_system(&self, system_id: &str) -> Result<u64, RepoError> {
        let r = sqlx::query("DELETE FROM roms WHERE system_id = ?1")
            .bind(system_id)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(r.rows_affected())
    }

    /// Remove todas as ROMs cujo `file_path` está sob `dir` (uma "biblioteca"
    /// inteira = uma pasta escaneada). Devolve quantas saíram.
    pub async fn remove_under_dir(&self, dir: &str) -> Result<u64, RepoError> {
        // normaliza pra terminar com '/' e escapa curingas de LIKE.
        let mut prefix = dir.trim_end_matches('/').to_string();
        prefix.push('/');
        let escaped = prefix
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let r = sqlx::query("DELETE FROM roms WHERE file_path LIKE ?1 ESCAPE '\\'")
            .bind(format!("{escaped}%"))
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(r.rows_affected())
    }
}

const SELECT_COLS: &str =
    "SELECT id, file_path, crc32, md5, system_id, added_at, last_played_at FROM roms";

fn row_to_rom(row: &SqliteRow) -> Result<Rom, RepoError> {
    Ok(Rom {
        id: row.try_get("id").map_err(be)?,
        file_path: row.try_get("file_path").map_err(be)?,
        crc32: row.try_get("crc32").map_err(be)?,
        md5: row.try_get("md5").map_err(be)?,
        system_id: row.try_get("system_id").map_err(be)?,
        added_at: row.try_get("added_at").map_err(be)?,
        last_played_at: row.try_get("last_played_at").map_err(be)?,
    })
}

#[async_trait]
impl RomRepository for RomsRepo {
    async fn add(&self, rom: &Rom) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO roms (id, file_path, crc32, md5, system_id, added_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&rom.id)
        .bind(&rom.file_path)
        .bind(&rom.crc32)
        .bind(&rom.md5)
        .bind(&rom.system_id)
        .bind(rom.added_at)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Rom>, RepoError> {
        let row = sqlx::query(&format!("{SELECT_COLS} WHERE id = ?1"))
            .bind(id)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.map(|r| row_to_rom(&r)).transpose()
    }

    async fn find_by_path(&self, file_path: &str) -> Result<Option<Rom>, RepoError> {
        let row = sqlx::query(&format!("{SELECT_COLS} WHERE file_path = ?1"))
            .bind(file_path)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.map(|r| row_to_rom(&r)).transpose()
    }

    async fn find_by_crc32(&self, crc32: &str) -> Result<Vec<Rom>, RepoError> {
        let rows = sqlx::query(&format!(
            "{SELECT_COLS} WHERE crc32 = ?1 ORDER BY file_path"
        ))
        .bind(crc32)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(row_to_rom).collect()
    }

    async fn list_by_system(&self, system_id: &str) -> Result<Vec<Rom>, RepoError> {
        let rows = sqlx::query(&format!(
            "{SELECT_COLS} WHERE system_id = ?1 ORDER BY file_path"
        ))
        .bind(system_id)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(row_to_rom).collect()
    }

    async fn list(&self) -> Result<Vec<Rom>, RepoError> {
        let rows = sqlx::query(&format!("{SELECT_COLS} ORDER BY system_id, file_path"))
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        rows.iter().map(row_to_rom).collect()
    }

    async fn remove(&self, id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM roms WHERE id = ?1")
            .bind(id)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    async fn mark_played(&self, id: &str, at_unix: i64) -> Result<(), RepoError> {
        sqlx::query("UPDATE roms SET last_played_at = ?2 WHERE id = ?1")
            .bind(id)
            .bind(at_unix)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
