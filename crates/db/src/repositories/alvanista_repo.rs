use crate::cascade::be;
use crate::pool::Db;
use domain::error::RepoError;
use sqlx::Row;

/// Estado local da integração com o Alvanista: reserva do chaveiro
/// (`secret_fallback`) e pares chave/valor (`alvanista_state`).
pub struct AlvanistaRepo {
    db: Db,
}

impl AlvanistaRepo {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn fallback_get(&self, account: &str) -> Result<Option<String>, RepoError> {
        self.get("secret_fallback", "account", account).await
    }

    pub async fn fallback_set(&self, account: &str, value: &str) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO secret_fallback (account, value) VALUES (?1, ?2) \
             ON CONFLICT(account) DO UPDATE SET value = excluded.value",
        )
        .bind(account)
        .bind(value)
        .execute(&self.db)
        .await
        .map_err(be)?;
        Ok(())
    }

    pub async fn fallback_delete(&self, account: &str) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM secret_fallback WHERE account = ?1")
            .bind(account)
            .execute(&self.db)
            .await
            .map_err(be)?;
        Ok(())
    }

    pub async fn state_get(&self, key: &str) -> Result<Option<String>, RepoError> {
        self.get("alvanista_state", "key", key).await
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

    /// `table`/`column` são literais deste arquivo, nunca entrada externa.
    async fn get(&self, table: &str, column: &str, key: &str) -> Result<Option<String>, RepoError> {
        let sql = format!("SELECT value FROM {table} WHERE {column} = ?1");
        let row = sqlx::query(&sql)
            .bind(key)
            .fetch_optional(&self.db)
            .await
            .map_err(be)?;
        row.map(|r| r.try_get("value").map_err(be)).transpose()
    }
}
