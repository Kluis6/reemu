use crate::cascade::be;
use crate::convert::{option_type_from_db, option_type_to_db};
use crate::pool::Db;
use async_trait::async_trait;
use domain::core_options::{CoreOptionDefinition, CoreOptionsStore};
use domain::error::RepoError;
use sqlx::Row;

pub struct CoreOptionsRepo {
    db: Db,
}

impl CoreOptionsRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

/// PK determinística (a tabela pede `id TEXT`, e não temos gerador de UUID
/// no crate). `\u{1f}` (unit separator) não aparece em option keys reais.
fn row_id(core_id: &str, option_key: &str) -> String {
    format!("{core_id}\u{1f}{option_key}")
}

#[async_trait]
impl CoreOptionsStore for CoreOptionsRepo {
    async fn schema_for(&self, core_id: &str) -> Result<Vec<CoreOptionDefinition>, RepoError> {
        let rows = sqlx::query(
            "SELECT option_key, display_name, option_type, choices, default_value \
             FROM core_options_schema WHERE core_id = ?1 ORDER BY option_key",
        )
        .bind(core_id)
        .fetch_all(&self.db)
        .await
        .map_err(be)?;

        let mut defs = Vec::with_capacity(rows.len());
        for r in &rows {
            let kind: String = r.try_get("option_type").map_err(be)?;
            let choices: Option<String> = r.try_get("choices").map_err(be)?;
            let default_value: Option<String> = r.try_get("default_value").map_err(be)?;
            defs.push(CoreOptionDefinition {
                option_key: r.try_get("option_key").map_err(be)?,
                display_name: r.try_get("display_name").map_err(be)?,
                option_type: option_type_from_db(&kind, choices.as_deref())?,
                default_value: default_value.unwrap_or_default(),
            });
        }
        Ok(defs)
    }

    async fn get_value(
        &self,
        core_id: &str,
        option_key: &str,
    ) -> Result<Option<String>, RepoError> {
        let row = sqlx::query(
            "SELECT value FROM core_options_values WHERE core_id = ?1 AND option_key = ?2",
        )
        .bind(core_id)
        .bind(option_key)
        .fetch_optional(&self.db)
        .await
        .map_err(be)?;

        match row {
            Some(r) => Ok(Some(r.try_get("value").map_err(be)?)),
            None => Ok(None),
        }
    }

    async fn values_for(
        &self,
        core_id: &str,
    ) -> Result<std::collections::HashMap<String, String>, RepoError> {
        let rows =
            sqlx::query("SELECT option_key, value FROM core_options_values WHERE core_id = ?1")
                .bind(core_id)
                .fetch_all(&self.db)
                .await
                .map_err(be)?;
        rows.iter()
            .map(|r| {
                Ok((
                    r.try_get::<String, _>("option_key").map_err(be)?,
                    r.try_get::<String, _>("value").map_err(be)?,
                ))
            })
            .collect()
    }

    async fn set_value(
        &self,
        core_id: &str,
        option_key: &str,
        value: &str,
    ) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO core_options_values (id, core_id, option_key, value) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(core_id, option_key) DO UPDATE SET value = excluded.value",
        )
        .bind(row_id(core_id, option_key))
        .bind(core_id)
        .bind(option_key)
        .bind(value)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn replace_schema(
        &self,
        core_id: &str,
        defs: &[CoreOptionDefinition],
    ) -> Result<(), RepoError> {
        let mut tx = self.db.begin().await.map_err(be)?;

        sqlx::query("DELETE FROM core_options_schema WHERE core_id = ?1")
            .bind(core_id)
            .execute(&mut *tx)
            .await
            .map_err(be)?;

        for def in defs {
            let (kind, choices) = option_type_to_db(&def.option_type);
            sqlx::query(
                "INSERT INTO core_options_schema \
                 (id, core_id, option_key, display_name, option_type, choices, default_value) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(row_id(core_id, &def.option_key))
            .bind(core_id)
            .bind(&def.option_key)
            .bind(&def.display_name)
            .bind(kind)
            .bind(choices)
            .bind(&def.default_value)
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        }

        tx.commit().await.map_err(be)?;
        Ok(())
    }
}
