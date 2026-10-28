use crate::cascade::be;
use crate::pool::Db;
use domain::cheats::Cheat;
use domain::error::RepoError;
use sqlx::Row;

/// Cheats por código de cada jogo (doc 17).
pub struct CheatsRepo {
    db: Db,
}

fn row_to_cheat(r: &sqlx::sqlite::SqliteRow) -> Result<Cheat, RepoError> {
    Ok(Cheat {
        id: r.try_get("id").map_err(be)?,
        rom_id: r.try_get("rom_id").map_err(be)?,
        description: r.try_get("description").map_err(be)?,
        code: r.try_get("code").map_err(be)?,
        enabled: r.try_get::<i64, _>("enabled").map_err(be)? != 0,
        position: r.try_get("position").map_err(be)?,
    })
}

impl CheatsRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Cheats do jogo, na ordem da lista.
    pub async fn list(&self, rom_id: &str) -> Result<Vec<Cheat>, RepoError> {
        let rows = sqlx::query(
            "SELECT id, rom_id, description, code, enabled, position FROM cheats
             WHERE rom_id = ?1 ORDER BY position, id",
        )
        .bind(rom_id)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(row_to_cheat).collect()
    }

    /// Acrescenta no fim da lista, desligado ou ligado conforme `enabled`.
    pub async fn add(
        &self,
        rom_id: &str,
        description: &str,
        code: &str,
        enabled: bool,
    ) -> Result<Cheat, RepoError> {
        let row = sqlx::query(
            "INSERT INTO cheats (rom_id, description, code, enabled, position)
             VALUES (?1, ?2, ?3, ?4,
                     (SELECT COALESCE(MAX(position), -1) + 1 FROM cheats WHERE rom_id = ?1))
             RETURNING id, rom_id, description, code, enabled, position",
        )
        .bind(rom_id)
        .bind(description)
        .bind(code)
        .bind(i64::from(enabled))
        .fetch_one(&self.db)
        .await
        .map_err(be)?;
        row_to_cheat(&row)
    }

    /// Troca descrição, código e o liga/desliga. `None` se o cheat não existe.
    pub async fn update(
        &self,
        id: i64,
        description: &str,
        code: &str,
        enabled: bool,
    ) -> Result<Option<Cheat>, RepoError> {
        let row = sqlx::query(
            "UPDATE cheats SET description = ?2, code = ?3, enabled = ?4 WHERE id = ?1
             RETURNING id, rom_id, description, code, enabled, position",
        )
        .bind(id)
        .bind(description)
        .bind(code)
        .bind(i64::from(enabled))
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        row.as_ref().map(row_to_cheat).transpose()
    }

    /// Apaga e devolve o `rom_id` dele (pra reaplicar a lista do jogo).
    pub async fn delete(&self, id: i64) -> Result<Option<String>, RepoError> {
        let row = sqlx::query("DELETE FROM cheats WHERE id = ?1 RETURNING rom_id")
            .bind(id)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.map(|r| r.try_get("rom_id").map_err(be)).transpose()
    }
}
