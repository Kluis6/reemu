use crate::cascade::be;
use crate::pool::Db;
use domain::error::RepoError;
use domain::metadata::{ResolutionState, RomResolution};
use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Row};
use std::collections::HashSet;

#[derive(Clone)]
pub struct RomResolutionRepo {
    db: Db,
}

fn row_to_resolution(row: &SqliteRow) -> Result<RomResolution, RepoError> {
    let raw_state: String = row.try_get("state").map_err(be)?;
    let state = ResolutionState::parse(&raw_state).ok_or_else(|| {
        RepoError::Corrupt(format!("estado de rom_resolution inválido: {raw_state}"))
    })?;
    Ok(RomResolution {
        rom_id: row.try_get("rom_id").map_err(be)?,
        state,
        game_uuid: row.try_get("game_uuid").map_err(be)?,
        matched_by: row.try_get("matched_by").map_err(be)?,
        reason: row.try_get("reason").map_err(be)?,
        retry_after: row.try_get("retry_after").map_err(be)?,
        media_pending: row.try_get::<i64, _>("media_pending").map_err(be)? != 0,
        answer_json: row.try_get("answer_json").map_err(be)?,
        resolved_at: row.try_get("resolved_at").map_err(be)?,
    })
}

const SELECT_COLS: &str = "SELECT rom_id, state, game_uuid, matched_by, reason, \
    retry_after, media_pending, answer_json, resolved_at FROM rom_resolution";

impl RomResolutionRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn get(&self, rom_id: &str) -> Result<Option<RomResolution>, RepoError> {
        let row = sqlx::query(AssertSqlSafe(format!("{SELECT_COLS} WHERE rom_id = ?1")))
            .bind(rom_id)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.as_ref().map(row_to_resolution).transpose()
    }

    pub async fn put(&self, row: &RomResolution) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO rom_resolution \
             (rom_id, state, game_uuid, matched_by, reason, retry_after, media_pending, answer_json, resolved_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
             ON CONFLICT(rom_id) DO UPDATE SET \
               state = excluded.state, \
               game_uuid = excluded.game_uuid, \
               matched_by = excluded.matched_by, \
               reason = excluded.reason, \
               retry_after = excluded.retry_after, \
               media_pending = excluded.media_pending, \
               answer_json = excluded.answer_json, \
               resolved_at = excluded.resolved_at",
        )
        .bind(&row.rom_id)
        .bind(row.state.as_str())
        .bind(&row.game_uuid)
        .bind(&row.matched_by)
        .bind(&row.reason)
        .bind(row.retry_after)
        .bind(i64::from(row.media_pending))
        .bind(&row.answer_json)
        .bind(row.resolved_at)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    pub async fn all_found(&self) -> Result<Vec<RomResolution>, RepoError> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "{SELECT_COLS} WHERE state IN ('found', 'manual') ORDER BY rom_id"
        )))
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter().map(row_to_resolution).collect()
    }

    pub async fn by_game_uuid(&self, game_uuid: &str) -> Result<Option<RomResolution>, RepoError> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "{SELECT_COLS} WHERE game_uuid = ?1 AND answer_json IS NOT NULL \
             ORDER BY resolved_at DESC LIMIT 1"
        )))
        .bind(game_uuid)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;
        row.as_ref().map(row_to_resolution).transpose()
    }

    /// ROMs cujo prazo de nova tentativa já chegou: `pending`, `found` com
    /// `media_pending = 1` resolvida há menos de 7 dias, e `unknown` por falha
    /// temporária (`lookup_failed`).
    pub async fn pending_due(&self, now: i64) -> Result<Vec<String>, RepoError> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT rom_id FROM rom_resolution \
             WHERE retry_after IS NOT NULL AND retry_after <= ?1 \
               AND ( \
                 state = 'pending' \
                 OR (state = 'found' AND media_pending != 0 AND resolved_at > ?1 - 7 * 86400) \
                 OR (state = 'unknown' AND reason = 'lookup_failed') \
               ) \
             ORDER BY rom_id",
        )
        .bind(now)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        Ok(ids)
    }

    /// ROMs da biblioteca que ainda não têm linha em `rom_resolution`.
    pub async fn unresolved_rom_ids(&self) -> Result<Vec<String>, RepoError> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT r.id FROM roms r \
             LEFT JOIN rom_resolution rr ON rr.rom_id = r.id \
             WHERE rr.rom_id IS NULL \
             ORDER BY r.system_id, r.file_path",
        )
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        Ok(ids)
    }

    /// Há algo que o temporizador de 30 minutos ainda precise acompanhar?
    pub async fn has_follow_up(&self, now: i64) -> Result<bool, RepoError> {
        let has_unresolved: i64 = sqlx::query_scalar(
            "SELECT EXISTS( \
               SELECT 1 FROM roms r \
               LEFT JOIN rom_resolution rr ON rr.rom_id = r.id \
               WHERE rr.rom_id IS NULL \
             )",
        )
        .fetch_one(&self.db)
        .await
        .map_err(be)?;
        if has_unresolved != 0 {
            return Ok(true);
        }
        let has_waiting: i64 = sqlx::query_scalar(
            "SELECT EXISTS( \
               SELECT 1 FROM rom_resolution \
               WHERE retry_after IS NOT NULL \
                 AND ( \
                   state = 'pending' \
                   OR (state = 'found' AND media_pending != 0 AND resolved_at > ?1 - 7 * 86400) \
                   OR (state = 'unknown' AND reason = 'lookup_failed') \
                 ) \
             )",
        )
        .bind(now)
        .fetch_one(&self.db)
        .await
        .map_err(be)?;
        Ok(has_waiting != 0)
    }

    /// Contagens `(found, pending, unknown)` para a linha de estado em Metadados.
    /// `manual` entra junto de `found` (identificado no catálogo).
    pub async fn counts(&self) -> Result<(usize, usize, usize), RepoError> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT state, COUNT(*) FROM rom_resolution GROUP BY state")
                .fetch_all(&self.db)
                .await
                .map_err(be)?;
        let (mut found, mut pending, mut unknown) = (0usize, 0usize, 0usize);
        for (st, n) in rows {
            let n = usize::try_from(n.max(0)).unwrap_or(0);
            match st.as_str() {
                "found" | "manual" => found += n,
                "pending" => pending += n,
                "unknown" => unknown += n,
                _ => {}
            }
        }
        Ok((found, pending, unknown))
    }

    /// Ao desconectar a conta, linhas `pending` (ainda sem dados) saem da fila
    /// para os provedores locais poderem assumir; `found` e `manual` ficam.
    pub async fn clear_pending(&self) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM rom_resolution WHERE state = 'pending'")
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// ROMs que os provedores próprios do ReEmu não devem consultar (D145):
    /// `found` e `manual` sempre; `pending` enquanto o catálogo está ativo.
    pub async fn skip_own_scraper_ids(&self) -> Result<HashSet<String>, RepoError> {
        let include_pending = super::AlvanistaRepo::new(self.db.clone())
            .catalog_active()
            .await?;
        let sql = if include_pending {
            "SELECT rom_id FROM rom_resolution WHERE state IN ('found', 'manual', 'pending')"
        } else {
            "SELECT rom_id FROM rom_resolution WHERE state IN ('found', 'manual')"
        };
        let ids: Vec<String> = sqlx::query_scalar(sql)
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        Ok(ids.into_iter().collect())
    }
}
