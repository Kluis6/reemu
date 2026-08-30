use crate::cascade::{be, resolve_cascade};
use crate::convert::{scope_from_db, scope_to_db};
use crate::pool::Db;
use async_trait::async_trait;
use domain::error::RepoError;
use domain::shader_chain::{
    AssignmentScope, ShaderChainAssignment, ShaderChainResolver, ShaderChainStore, ShaderFormat,
    ShaderPreset,
};
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

/// `id` determinístico da atribuição, por escopo/alvo.
fn assignment_id(scope: AssignmentScope, system_id: Option<&str>, rom_id: Option<&str>) -> String {
    match scope {
        AssignmentScope::Default => "default".to_string(),
        AssignmentScope::System => format!("system:{}", system_id.unwrap_or("")),
        AssignmentScope::Rom => format!("rom:{}", rom_id.unwrap_or("")),
    }
}

#[async_trait]
impl ShaderChainStore for ShaderChainRepo {
    async fn upsert_preset(&self, preset: &ShaderPreset) -> Result<(), RepoError> {
        // `format` é sempre 'slang' no MVP (CHECK do schema).
        sqlx::query(
            "INSERT INTO shader_presets (id, name, source_path, format, is_builtin, includes_bezel) \
             VALUES (?1, ?2, ?3, 'slang', ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET \
               name = excluded.name, source_path = excluded.source_path, \
               is_builtin = excluded.is_builtin, includes_bezel = excluded.includes_bezel",
        )
        .bind(&preset.id)
        .bind(&preset.name)
        .bind(&preset.source_path)
        .bind(preset.is_builtin as i64)
        .bind(preset.includes_bezel as i64)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn list_presets(&self) -> Result<Vec<ShaderPreset>, RepoError> {
        let rows = sqlx::query(
            "SELECT id, name, source_path, is_builtin, includes_bezel FROM shader_presets \
             ORDER BY is_builtin DESC, name",
        )
        .fetch_all(&self.db)
        .await
        .map_err(be)?;
        rows.iter()
            .map(|r| {
                Ok(ShaderPreset {
                    id: r.try_get("id").map_err(be)?,
                    name: r.try_get("name").map_err(be)?,
                    source_path: r.try_get("source_path").map_err(be)?,
                    format: ShaderFormat::Slang,
                    is_builtin: r.try_get::<i64, _>("is_builtin").map_err(be)? != 0,
                    includes_bezel: r.try_get::<i64, _>("includes_bezel").map_err(be)? != 0,
                })
            })
            .collect()
    }

    async fn set_assignment(
        &self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
        preset_id: &str,
    ) -> Result<(), RepoError> {
        let mut tx = self.db.begin().await.map_err(be)?;
        clear_scope(&mut tx, scope, system_id, rom_id).await?;
        sqlx::query(
            "INSERT INTO shader_chain_assignments (id, scope, system_id, rom_id, preset_id) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(assignment_id(scope, system_id, rom_id))
        .bind(scope_to_db(scope))
        .bind(system_id)
        .bind(rom_id)
        .bind(preset_id)
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        tx.commit().await.map_err(be)?;
        Ok(())
    }

    async fn clear_assignment(
        &self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
    ) -> Result<(), RepoError> {
        let mut tx = self.db.begin().await.map_err(be)?;
        clear_scope(&mut tx, scope, system_id, rom_id).await?;
        tx.commit().await.map_err(be)?;
        Ok(())
    }

    async fn set_parameter_override(
        &self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
        key: &str,
        value: &str,
    ) -> Result<(), RepoError> {
        let aid = assignment_id(scope, system_id, rom_id);
        // a atribuição precisa existir (FK) — a UI cria uma antes de mexer.
        let exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM shader_chain_assignments WHERE id = ?1")
                .bind(&aid)
                .fetch_optional(&self.db)
                .await
                .map_err(be)?;
        if exists.is_none() {
            return Err(RepoError::Backend(format!(
                "sem shader atribuído ao escopo '{aid}' — escolha um preset antes de ajustar parâmetros"
            )));
        }
        sqlx::query(
            "INSERT INTO shader_parameter_overrides (id, assignment_id, parameter_key, value) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(assignment_id, parameter_key) DO UPDATE SET value = excluded.value",
        )
        .bind(format!("{aid}::{key}"))
        .bind(&aid)
        .bind(key)
        .bind(value)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn clear_parameter_overrides(
        &self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
    ) -> Result<(), RepoError> {
        let aid = assignment_id(scope, system_id, rom_id);
        sqlx::query("DELETE FROM shader_parameter_overrides WHERE assignment_id = ?1")
            .bind(&aid)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}

async fn clear_scope(
    tx: &mut sqlx::SqliteConnection,
    scope: AssignmentScope,
    system_id: Option<&str>,
    rom_id: Option<&str>,
) -> Result<(), RepoError> {
    let q = match scope {
        AssignmentScope::Default => {
            sqlx::query("DELETE FROM shader_chain_assignments WHERE scope = 'default'")
        }
        AssignmentScope::System => sqlx::query(
            "DELETE FROM shader_chain_assignments WHERE scope = 'system' AND system_id = ?1",
        )
        .bind(system_id),
        AssignmentScope::Rom => {
            sqlx::query("DELETE FROM shader_chain_assignments WHERE scope = 'rom' AND rom_id = ?1")
                .bind(rom_id)
        }
    };
    q.execute(&mut *tx).await.map_err(be)?;
    Ok(())
}
