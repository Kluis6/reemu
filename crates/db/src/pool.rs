//! Criação do pool SQLite e execução das migrations.

use crate::DbError;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Pool compartilhado. Clonar é barato (Arc interno) — passe um `Db` para
/// cada repositório.
pub type Db = SqlitePool;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Abre (criando se preciso) um banco em arquivo e roda as migrations.
/// `foreign_keys` é ligado em toda conexão — sem isso o SQLite ignora os
/// `REFERENCES`/`ON DELETE CASCADE` do schema.
pub async fn connect(database_url: &str) -> Result<Db, DbError> {
    let opts = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| DbError::Config(e.to_string()))?
        .foreign_keys(true)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    run_migrations(&pool).await?;
    Ok(pool)
}

/// Banco em memória para testes. `max_connections(1)` garante que a mesma
/// conexão (e portanto o mesmo banco em memória) seja reusada durante o teste.
pub async fn connect_in_memory() -> Result<Db, DbError> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(|e| DbError::Config(e.to_string()))?
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;

    run_migrations(&pool).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &Db) -> Result<(), DbError> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(|e| DbError::Migration(e.to_string()))
}
