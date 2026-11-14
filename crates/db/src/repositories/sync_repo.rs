use crate::cascade::be;
use crate::pool::Db;
use domain::error::RepoError;
use domain::sync::{BaseRow, ConflictRow};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

const BASE_COLS: &str = "key, kind, remote_save_id, remote_version_id, remote_hash, \
     local_hash, local_path, rom_crc32, synced_at, wiped_at, backup_path, local_size, \
     local_mtime_ns";
const CONFLICT_COLS: &str = "key, local_hash, local_mtime, local_size, remote_save_id, \
     remote_version_id, remote_hash, remote_device_label, remote_client_mtime, \
     remote_size, remote_deleted, rom_id, seen_at";

/// Base local do cloud save (`sync_base`) e conflitos abertos
/// (`sync_conflicts`).
pub struct SyncRepo {
    db: Db,
}

fn base_from(r: &SqliteRow) -> Result<BaseRow, RepoError> {
    Ok(BaseRow {
        key: r.try_get("key").map_err(be)?,
        kind: r.try_get("kind").map_err(be)?,
        remote_save_id: r.try_get("remote_save_id").map_err(be)?,
        remote_version_id: r.try_get("remote_version_id").map_err(be)?,
        remote_hash: r.try_get("remote_hash").map_err(be)?,
        local_hash: r.try_get("local_hash").map_err(be)?,
        local_path: r.try_get("local_path").map_err(be)?,
        rom_crc32: r.try_get("rom_crc32").map_err(be)?,
        synced_at: r.try_get("synced_at").map_err(be)?,
        wiped_at: r.try_get("wiped_at").map_err(be)?,
        backup_path: r.try_get("backup_path").map_err(be)?,
        local_size: r.try_get("local_size").map_err(be)?,
        local_mtime_ns: r.try_get("local_mtime_ns").map_err(be)?,
    })
}

fn conflict_from(r: &SqliteRow) -> Result<ConflictRow, RepoError> {
    Ok(ConflictRow {
        key: r.try_get("key").map_err(be)?,
        local_hash: r.try_get("local_hash").map_err(be)?,
        local_mtime: r.try_get("local_mtime").map_err(be)?,
        local_size: r.try_get("local_size").map_err(be)?,
        remote_save_id: r.try_get("remote_save_id").map_err(be)?,
        remote_version_id: r.try_get("remote_version_id").map_err(be)?,
        remote_hash: r.try_get("remote_hash").map_err(be)?,
        remote_device_label: r.try_get("remote_device_label").map_err(be)?,
        remote_client_mtime: r.try_get("remote_client_mtime").map_err(be)?,
        remote_size: r.try_get("remote_size").map_err(be)?,
        remote_deleted: r.try_get("remote_deleted").map_err(be)?,
        rom_id: r.try_get("rom_id").map_err(be)?,
        seen_at: r.try_get("seen_at").map_err(be)?,
    })
}

impl SyncRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn base_get(&self, key: &str) -> Result<Option<BaseRow>, RepoError> {
        let row = sqlx::query(&format!("SELECT {BASE_COLS} FROM sync_base WHERE key = ?1"))
            .bind(key)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.as_ref().map(base_from).transpose()
    }

    /// Insere ou substitui a linha da chave, `wiped_at`, `backup_path`, o
    /// tamanho e o mtime incluídos: um sync novo grava a linha sem a marca e
    /// a limpa.
    pub async fn base_put(&self, b: &BaseRow) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO sync_base (key, kind, remote_save_id, remote_version_id, remote_hash, \
             local_hash, local_path, rom_crc32, synced_at, wiped_at, backup_path, \
             local_size, local_mtime_ns) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
             ON CONFLICT(key) DO UPDATE SET kind = excluded.kind, \
             remote_save_id = excluded.remote_save_id, \
             remote_version_id = excluded.remote_version_id, \
             remote_hash = excluded.remote_hash, local_hash = excluded.local_hash, \
             local_path = excluded.local_path, rom_crc32 = excluded.rom_crc32, \
             synced_at = excluded.synced_at, wiped_at = excluded.wiped_at, \
             backup_path = excluded.backup_path, local_size = excluded.local_size, \
             local_mtime_ns = excluded.local_mtime_ns",
        )
        .bind(&b.key)
        .bind(&b.kind)
        .bind(&b.remote_save_id)
        .bind(&b.remote_version_id)
        .bind(&b.remote_hash)
        .bind(&b.local_hash)
        .bind(&b.local_path)
        .bind(&b.rom_crc32)
        .bind(b.synced_at)
        .bind(b.wiped_at)
        .bind(&b.backup_path)
        .bind(b.local_size)
        .bind(b.local_mtime_ns)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    /// Esquece a base e os conflitos inteiros (outra conta nesta máquina),
    /// numa transação: nunca sobra conflito sem a base de que ele fala.
    pub async fn clear_all(&self) -> Result<(), RepoError> {
        let mut tx = self.db.begin().await.map_err(be)?;
        sqlx::query("DELETE FROM sync_conflicts")
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        sqlx::query("DELETE FROM sync_base")
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        tx.commit().await.map_err(be)
    }

    pub async fn base_delete(&self, key: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM sync_base WHERE key = ?1")
            .bind(key)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    pub async fn base_all(&self, kind: &str) -> Result<Vec<BaseRow>, RepoError> {
        let rows = sqlx::query(&format!(
            "SELECT {BASE_COLS} FROM sync_base WHERE kind = ?1 ORDER BY key"
        ))
        .bind(kind)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(base_from).collect()
    }

    /// Marca a base como apagada no site, com o lugar para onde o arquivo foi
    /// (`None` quando nada se mudou: a linha própria de um state, D127).
    pub async fn base_mark_wiped(
        &self,
        key: &str,
        at: i64,
        backup_path: Option<&str>,
    ) -> Result<(), RepoError> {
        sqlx::query("UPDATE sync_base SET wiped_at = ?2, backup_path = ?3 WHERE key = ?1")
            .bind(key)
            .bind(at)
            .bind(backup_path)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    pub async fn conflict_put(&self, c: &ConflictRow) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO sync_conflicts (key, local_hash, local_mtime, local_size, \
             remote_save_id, remote_version_id, remote_hash, remote_device_label, \
             remote_client_mtime, remote_size, remote_deleted, rom_id, seen_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13) \
             ON CONFLICT(key) DO UPDATE SET local_hash = excluded.local_hash, \
             local_mtime = excluded.local_mtime, local_size = excluded.local_size, \
             remote_save_id = excluded.remote_save_id, \
             remote_version_id = excluded.remote_version_id, \
             remote_hash = excluded.remote_hash, \
             remote_device_label = excluded.remote_device_label, \
             remote_client_mtime = excluded.remote_client_mtime, \
             remote_size = excluded.remote_size, \
             remote_deleted = excluded.remote_deleted, rom_id = excluded.rom_id, \
             seen_at = excluded.seen_at",
        )
        .bind(&c.key)
        .bind(&c.local_hash)
        .bind(c.local_mtime)
        .bind(c.local_size)
        .bind(&c.remote_save_id)
        .bind(&c.remote_version_id)
        .bind(&c.remote_hash)
        .bind(&c.remote_device_label)
        .bind(c.remote_client_mtime)
        .bind(c.remote_size)
        .bind(c.remote_deleted)
        .bind(&c.rom_id)
        .bind(c.seen_at)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    pub async fn conflict_delete(&self, key: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM sync_conflicts WHERE key = ?1")
            .bind(key)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    pub async fn conflicts_all(&self) -> Result<Vec<ConflictRow>, RepoError> {
        let rows = sqlx::query(&format!(
            "SELECT {CONFLICT_COLS} FROM sync_conflicts ORDER BY seen_at, key"
        ))
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(conflict_from).collect()
    }

    pub async fn conflicts_for_rom(&self, rom_id: &str) -> Result<Vec<ConflictRow>, RepoError> {
        let rows = sqlx::query(&format!(
            "SELECT {CONFLICT_COLS} FROM sync_conflicts WHERE rom_id = ?1 ORDER BY seen_at, key"
        ))
        .bind(rom_id)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(conflict_from).collect()
    }
}
