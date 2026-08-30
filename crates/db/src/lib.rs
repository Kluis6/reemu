//! Crate `db`: implementações concretas dos repositórios (sqlx/SQLite) que
//! satisfazem as portas definidas em `domain`, sobre o schema em
//! `migrations/0001_init.sql`.
//!
//! Regra: nenhum tipo do `sqlx` vaza pela interface pública — os
//! repositórios recebem/retornam só tipos de `domain`. Erros concretos de
//! armazenamento são convertidos para `domain::error::RepoError`.

mod cascade;
mod convert;
mod pool;
mod repositories;

pub use pool::{connect, connect_in_memory, run_migrations, Db};
pub use repositories::{
    AudioConfigRepo, ControllerMappingsRepo, CoreOptionsRepo, DecorationRepo, DevicePortsRepo,
    InstalledCoresRepo, MetadataRepo, RomsRepo, SaveStateRepo, ShaderChainRepo, SystemHotkeysRepo,
};

use thiserror::Error;

/// Erro de infraestrutura do crate `db` (setup de pool / migrations).
/// Erros por-operação dos repositórios usam `domain::error::RepoError`.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("configuração de banco inválida: {0}")]
    Config(String),
    #[error("falha ao rodar migrations: {0}")]
    Migration(String),
    #[error("erro do SQLite: {0}")]
    Sqlite(String),
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        DbError::Sqlite(e.to_string())
    }
}

impl From<DbError> for domain::error::RepoError {
    fn from(e: DbError) -> Self {
        domain::error::RepoError::Backend(e.to_string())
    }
}

/// Migration inicial, embutida para inspeção/tooling. A execução real é via
/// `run_migrations` (sqlx migrator), não este `&str`.
pub const INIT_MIGRATION: &str = include_str!("../migrations/0001_init.sql");
