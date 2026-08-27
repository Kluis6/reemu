use crate::cascade::{be, resolve_cascade};
use crate::convert::scope_from_db;
use crate::pool::Db;
use async_trait::async_trait;
use domain::decoration::{DecorationAssignment, DecorationResolver};
use domain::error::RepoError;
use sqlx::Row;

pub struct DecorationRepo {
    db: Db,
}

impl DecorationRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DecorationResolver for DecorationRepo {
    async fn resolve(
        &self,
        system_id: &str,
        rom_id: Option<&str>,
    ) -> Result<Option<DecorationAssignment>, RepoError> {
        resolve_cascade(
            &self.db,
            "SELECT scope, system_id, rom_id, pack_id, asset_path FROM decoration_assignments",
            system_id,
            rom_id,
            |row| {
                let scope_str: String = row.try_get("scope").map_err(be)?;
                Ok(DecorationAssignment {
                    scope: scope_from_db(&scope_str)?,
                    system_id: row.try_get("system_id").map_err(be)?,
                    rom_id: row.try_get("rom_id").map_err(be)?,
                    pack_id: row.try_get("pack_id").map_err(be)?,
                    asset_path: row.try_get("asset_path").map_err(be)?,
                })
            },
        )
        .await
    }
}
