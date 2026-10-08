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
    heal_line_ending_checksums(pool).await?;
    MIGRATOR
        .run(pool)
        .await
        .map_err(|e| DbError::Migration(e.to_string()))
}

/// O sqlx guarda o SHA-384 do texto de cada migration aplicada e recusa abrir
/// o banco se o arquivo embutido no binário mudou ("migration N was
/// previously applied but has been modified"). Só que o texto depende do
/// checkout: o git no Windows (`core.autocrlf`) entrega os `.sql` com CRLF,
/// o do Linux com LF — e um arquivo novo escrito com LF num checkout CRLF
/// mistura os dois. Foi o que travou o banco em 2026-10-08: a 0013 aplicada
/// com CRLF (app instalado) e embutida com LF no `tauri dev` → sem banco,
/// nenhuma configuração salvava.
///
/// Aqui, antes de migrar: se o checksum guardado é o do MESMO texto só com a
/// outra quebra de linha, ele é atualizado pro do binário. Qualquer mudança
/// de conteúdo continua sendo recusada pelo sqlx como antes.
async fn heal_line_ending_checksums(pool: &Db) -> Result<(), DbError> {
    use sha2::{Digest, Sha384};

    let has_table: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if has_table.is_none() {
        return Ok(()); // banco novo: nada aplicado ainda
    }
    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations")
            .fetch_all(pool)
            .await?;
    for (version, stored) in applied {
        let Some(m) = MIGRATOR.iter().find(|m| m.version == version) else {
            continue;
        };
        if stored == m.checksum.as_ref() {
            continue;
        }
        let lf = m.sql.replace("\r\n", "\n");
        let crlf = lf.replace('\n', "\r\n");
        let same_text = [lf, crlf]
            .iter()
            .any(|t| Sha384::digest(t.as_bytes()).as_slice() == stored.as_slice());
        if same_text {
            sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
                .bind(m.checksum.as_ref())
                .bind(version)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}
