use crate::cascade::be;
use crate::pool::Db;
use domain::error::RepoError;
use sqlx::Row;

/// Estado local da integração com o Alvanista: pares chave/valor
/// (`alvanista_state`). Os tokens não passam por aqui.
#[derive(Clone)]
pub struct AlvanistaRepo {
    db: Db,
}

impl AlvanistaRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub async fn state_get(&self, key: &str) -> Result<Option<String>, RepoError> {
        let row = sqlx::query("SELECT value FROM alvanista_state WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.map(|r| r.try_get("value").map_err(be)).transpose()
    }

    pub async fn state_set(&self, key: &str, value: &str) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO alvanista_state (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    /// Grava só se a chave ainda não existe (duas chamadas simultâneas não
    /// se sobrescrevem); quem chama relê o valor que ficou.
    pub async fn state_insert_if_absent(&self, key: &str, value: &str) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO alvanista_state (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO NOTHING",
        )
        .bind(key)
        .bind(value)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    pub async fn state_delete(&self, key: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM alvanista_state WHERE key = ?1")
            .bind(key)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    /// Chave "Usar o catálogo do Alvanista para identificar meus jogos" (ligada por padrão).
    pub async fn catalog_enabled(&self) -> Result<bool, RepoError> {
        Ok(self.state_get("catalog.enabled").await?.as_deref() != Some("0"))
    }

    pub async fn set_catalog_enabled(&self, enabled: bool) -> Result<(), RepoError> {
        self.state_set("catalog.enabled", if enabled { "1" } else { "0" })
            .await?;
        if enabled {
            self.state_delete("catalog.disabled").await?;
        }
        Ok(())
    }

    pub async fn catalog_disabled_by_server(&self) -> Result<bool, RepoError> {
        Ok(self.state_get("catalog.disabled").await?.as_deref() == Some("1"))
    }

    pub async fn set_catalog_disabled_by_server(&self, disabled: bool) -> Result<(), RepoError> {
        if disabled {
            self.state_set("catalog.disabled", "1").await
        } else {
            self.state_delete("catalog.disabled").await
        }
    }

    /// O catálogo está ativo (chave ligada e rota habilitada no servidor)?
    pub async fn catalog_active(&self) -> Result<bool, RepoError> {
        Ok(self.catalog_enabled().await? && !self.catalog_disabled_by_server().await?)
    }
}
