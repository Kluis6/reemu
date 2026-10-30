use crate::cascade::be;
use crate::pool::Db;
use domain::error::RepoError;
use sqlx::Row;

/// Proporção da imagem por jogo, plataforma ou todos (`aspect_assignments`).
pub struct AspectRepo {
    db: Db,
}

/// O que está atribuído EXATAMENTE em cada escopo (`None` = herda).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AspectAssignments {
    pub at_rom: Option<String>,
    pub at_system: Option<String>,
    pub at_default: Option<String>,
}

impl AspectAssignments {
    /// Vale o escopo mais específico; sem nenhum, a proporção do core.
    pub fn resolved(&self) -> &str {
        self.at_rom
            .as_deref()
            .or(self.at_system.as_deref())
            .or(self.at_default.as_deref())
            .unwrap_or("core")
    }
}

impl AspectRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    async fn one(&self, scope: &str, target: &str) -> Result<Option<String>, RepoError> {
        let row =
            sqlx::query("SELECT aspect FROM aspect_assignments WHERE scope = ?1 AND target = ?2")
                .bind(scope)
                .bind(target)
                .fetch_optional(&self.db)
                .await
                .map_err(be)?;
        row.map(|r| r.try_get("aspect").map_err(be)).transpose()
    }

    /// As três linhas da cascata do jogo. `rom_id` `None` = jogo fora da
    /// biblioteca (só plataforma e padrão).
    pub async fn get(
        &self,
        rom_id: Option<&str>,
        system_id: &str,
    ) -> Result<AspectAssignments, RepoError> {
        Ok(AspectAssignments {
            at_rom: match rom_id {
                Some(id) => self.one("rom", id).await?,
                None => None,
            },
            at_system: self.one("system", system_id).await?,
            at_default: self.one("default", "").await?,
        })
    }

    /// Grava no escopo; `aspect` `None` limpa (volta a herdar).
    pub async fn set(
        &self,
        scope: &str,
        target: &str,
        aspect: Option<&str>,
    ) -> Result<(), RepoError> {
        match aspect {
            Some(a) => {
                sqlx::query(
                    "INSERT INTO aspect_assignments (scope, target, aspect) VALUES (?1, ?2, ?3)
                     ON CONFLICT (scope, target) DO UPDATE SET aspect = excluded.aspect",
                )
                .bind(scope)
                .bind(target)
                .bind(a)
                .execute(&self.db)
                .await
                .map_err(be)?;
            }
            None => {
                sqlx::query("DELETE FROM aspect_assignments WHERE scope = ?1 AND target = ?2")
                    .bind(scope)
                    .bind(target)
                    .execute(&self.db)
                    .await
                    .map_err(be)?;
            }
        }
        Ok(())
    }
}
