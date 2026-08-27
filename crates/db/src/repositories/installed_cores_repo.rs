use crate::cascade::be;
use crate::convert::{render_backend_from_db, render_backend_to_db};
use crate::pool::Db;
use async_trait::async_trait;
use domain::core_loader::{CoreRenderRequirements, InstalledCore, InstalledCoreRepository};
use domain::error::RepoError;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

pub struct InstalledCoresRepo {
    db: Db,
}

impl InstalledCoresRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

fn row_to_core(row: &SqliteRow) -> Result<InstalledCore, RepoError> {
    let backend: Option<String> = row.try_get("render_backend").map_err(be)?;
    let render_requirements = match backend {
        None => None,
        Some(b) => Some(CoreRenderRequirements {
            render_backend: render_backend_from_db(&b)?,
            gl_version_min: row.try_get("gl_version_min").map_err(be)?,
            gl_profile: row.try_get("gl_profile").map_err(be)?,
            needs_depth_stencil: row
                .try_get::<Option<bool>, _>("needs_depth_stencil")
                .map_err(be)?
                .unwrap_or(false),
        }),
    };

    Ok(InstalledCore {
        core_id: row.try_get("core_id").map_err(be)?,
        version: row.try_get("version").map_err(be)?,
        installed_at: row.try_get("installed_at").map_err(be)?,
        render_requirements,
    })
}

const SELECT_COLS: &str = "SELECT core_id, version, installed_at, render_backend, \
    gl_version_min, gl_profile, needs_depth_stencil FROM installed_cores";

#[async_trait]
impl InstalledCoreRepository for InstalledCoresRepo {
    async fn register(&self, core: &InstalledCore) -> Result<(), RepoError> {
        // Só a identidade — render_* não são tocados aqui (decisão: só o
        // primeiro load os escreve, via set_render_requirements).
        sqlx::query(
            "INSERT INTO installed_cores (core_id, version, installed_at) \
             VALUES (?1, ?2, ?3) \
             ON CONFLICT(core_id) DO UPDATE SET \
               version = excluded.version, installed_at = excluded.installed_at",
        )
        .bind(&core.core_id)
        .bind(&core.version)
        .bind(core.installed_at)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn get(&self, core_id: &str) -> Result<Option<InstalledCore>, RepoError> {
        let row = sqlx::query(&format!("{SELECT_COLS} WHERE core_id = ?1"))
            .bind(core_id)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        match row {
            Some(r) => Ok(Some(row_to_core(&r)?)),
            None => Ok(None),
        }
    }

    async fn list(&self) -> Result<Vec<InstalledCore>, RepoError> {
        let rows = sqlx::query(&format!("{SELECT_COLS} ORDER BY core_id"))
            .fetch_all(&self.db)
            .await
            .map_err(be)?;
        rows.iter().map(row_to_core).collect()
    }

    async fn set_render_requirements(
        &self,
        core_id: &str,
        reqs: &CoreRenderRequirements,
    ) -> Result<(), RepoError> {
        let affected = sqlx::query(
            "UPDATE installed_cores SET \
               render_backend = ?1, gl_version_min = ?2, gl_profile = ?3, needs_depth_stencil = ?4 \
             WHERE core_id = ?5",
        )
        .bind(render_backend_to_db(&reqs.render_backend))
        .bind(&reqs.gl_version_min)
        .bind(&reqs.gl_profile)
        .bind(reqs.needs_depth_stencil)
        .bind(core_id)
        .execute(&self.db)
        .await
        .map_err(be)?
        .rows_affected();

        if affected == 0 {
            return Err(RepoError::Backend(format!(
                "core '{core_id}' não registrado — chame register() antes"
            )));
        }
        Ok(())
    }

    async fn remove(&self, core_id: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM installed_cores WHERE core_id = ?1")
            .bind(core_id)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }
}
