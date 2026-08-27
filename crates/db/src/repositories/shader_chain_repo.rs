use crate::cascade::{be, resolve_cascade};
use crate::convert::scope_from_db;
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::shader_chain::{AssignmentScope, ShaderChainAssignment, ShaderChainResolver};
use sqlx::Row;
use std::collections::HashMap;

pub struct ShaderChainRepo {
    db: Db,
}

impl ShaderChainRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    async fn overrides_for(
        &self,
        assignment_id: &str,
    ) -> Result<HashMap<String, String>, RepoError> {
        let rows = sqlx::query(
            "SELECT parameter_key, value FROM shader_parameter_overrides WHERE assignment_id = ?1",
        )
        .bind(assignment_id)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;

        let mut map = HashMap::with_capacity(rows.len());
        for r in &rows {
            let key: String = r.try_get("parameter_key").map_err(be)?;
            let value: String = r.try_get("value").map_err(be)?;
            map.insert(key, value);
        }
        Ok(map)
    }
}

struct RawAssignment {
    id: String,
    scope: AssignmentScope,
    system_id: Option<String>,
    rom_id: Option<String>,
    preset_id: String,
}

#[async_trait]
impl ShaderChainResolver for ShaderChainRepo {
    async fn resolve(
        &self,
        system_id: &str,
        rom_id: Option<&str>,
    ) -> Result<Option<ShaderChainAssignment>, RepoError> {
        let raw = resolve_cascade(
            &self.db,
            "SELECT id, scope, system_id, rom_id, preset_id FROM shader_chain_assignments",
            system_id,
            rom_id,
            |row| {
                let scope_str: String = row.try_get("scope").map_err(be)?;
                Ok(RawAssignment {
                    id: row.try_get("id").map_err(be)?,
                    scope: scope_from_db(&scope_str)?,
                    system_id: row.try_get("system_id").map_err(be)?,
                    rom_id: row.try_get("rom_id").map_err(be)?,
                    preset_id: row.try_get("preset_id").map_err(be)?,
                })
            },
        )
        .await?;

        let Some(raw) = raw else {
            return Ok(None);
        };

        let parameter_overrides = self.overrides_for(&raw.id).await?;
        Ok(Some(ShaderChainAssignment {
            scope: raw.scope,
            system_id: raw.system_id,
            rom_id: raw.rom_id,
            preset_id: raw.preset_id,
            parameter_overrides,
        }))
    }
}
