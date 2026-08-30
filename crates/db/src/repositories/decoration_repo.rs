use crate::cascade::{be, resolve_cascade};
use crate::convert::{scope_from_db, scope_to_db};
use crate::pool::Db;
use async_trait::async_trait;
use domain::decoration::{
    DecorationAssignment, DecorationPack, DecorationPackSource, DecorationResolver, DecorationStore,
};
use domain::error::RepoError;
use domain::shader_chain::AssignmentScope;
use sqlx::Row;

pub struct DecorationRepo {
    db: Db,
}

impl DecorationRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Quantas linhas há em `decoration_assignments` (pra log/diagnóstico).
    pub async fn count_assignments(&self) -> Result<i64, RepoError> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM decoration_assignments")
            .fetch_one(&self.db)
            .await
            .map_err(be)
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

fn source_to_db(s: DecorationPackSource) -> &'static str {
    match s {
        DecorationPackSource::Bundled => "bundled",
        DecorationPackSource::UserImported => "user_imported",
    }
}

fn assign_id(a: &DecorationAssignment) -> String {
    match a.scope {
        AssignmentScope::Default => format!("{}:default", a.pack_id),
        AssignmentScope::System => {
            format!(
                "{}:system:{}",
                a.pack_id,
                a.system_id.as_deref().unwrap_or("")
            )
        }
        AssignmentScope::Rom => {
            format!("{}:rom:{}", a.pack_id, a.rom_id.as_deref().unwrap_or(""))
        }
    }
}

#[async_trait]
impl DecorationStore for DecorationRepo {
    async fn upsert_pack(&self, pack: &DecorationPack) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO decoration_packs (id, name, source, base_path) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, source = excluded.source, \
               base_path = excluded.base_path",
        )
        .bind(&pack.id)
        .bind(&pack.name)
        .bind(source_to_db(pack.source))
        .bind(&pack.base_path)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn replace_assignments(
        &self,
        pack_id: &str,
        assignments: &[DecorationAssignment],
    ) -> Result<(), RepoError> {
        let mut tx = self.db.begin().await.map_err(be)?;
        // MVP: um pack ativo — limpa tudo e insere o novo.
        sqlx::query("DELETE FROM decoration_assignments")
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        // Packs grandes têm cobertura duplicada (mesmo jogo em pastas
        // diferentes) → `INSERT OR IGNORE` pra um dup não abortar tudo (os
        // índices únicos parciais por escopo cuidam da unicidade).
        for a in assignments {
            sqlx::query(
                "INSERT OR IGNORE INTO decoration_assignments \
                 (id, scope, system_id, rom_id, pack_id, asset_path) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )
            .bind(assign_id(a))
            .bind(scope_to_db(a.scope))
            .bind(&a.system_id)
            .bind(&a.rom_id)
            .bind(pack_id)
            .bind(&a.asset_path)
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        }
        tx.commit().await.map_err(be)?;
        Ok(())
    }

    async fn remove_pack(&self, pack_id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM decoration_packs WHERE id = ?1")
            .bind(pack_id)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    async fn clear_all(&self) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM decoration_packs")
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
