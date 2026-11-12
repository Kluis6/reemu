//! Criação do pool SQLite e execução das migrations.

use crate::DbError;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

/// Pool compartilhado. Clonar é barato (Arc interno) — passe um `Db` para
/// cada repositório.
pub type Db = SqlitePool;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Abre (criando se preciso) um banco em arquivo e roda as migrations.
/// `foreign_keys` é ligado em toda conexão — sem isso o SQLite ignora os
/// `REFERENCES`/`ON DELETE CASCADE` do schema. WAL + `synchronous=NORMAL`:
/// leituras não esperam as escritas (o scan grava enquanto a biblioteca lê)
/// e a escrita não perde dados numa queda do app, só numa queda de energia
/// (SQLite, "Write-Ahead Logging"). `busy_timeout`: com 5 conexões no pool,
/// uma escrita concorrente espera em vez de falhar com `database is locked`.
///
/// Antes de uma migration nova, uma cópia íntegra do banco vai para
/// `<arquivo>.bak` (`VACUUM INTO`): se a migration falhar no meio, a
/// biblioteca inteira não se perde.
pub async fn connect(database_url: &str) -> Result<Db, DbError> {
    let opts = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| DbError::Config(e.to_string()))?
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    if let Some(path) = file_path(database_url) {
        if has_pending_migrations(&pool).await? {
            match backup(&pool, &path).await {
                Ok(bak) => log::info!("SQLite: cópia antes de migrar em {}", bak.display()),
                // sem a cópia a migration segue: é o mesmo risco de antes
                Err(e) => log::warn!("SQLite: cópia antes de migrar falhou: {e}"),
            }
        }
    }

    run_migrations(&pool).await?;
    Ok(pool)
}

/// Caminho do arquivo numa URL `sqlite://<caminho>` ou `sqlite:<caminho>`;
/// `None` para banco em memória.
pub fn file_path(database_url: &str) -> Option<PathBuf> {
    let rest = database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))?;
    let rest = rest.split('?').next().unwrap_or(rest);
    (!rest.is_empty() && rest != ":memory:").then(|| PathBuf::from(rest))
}

/// Há migration embutida no binário que o banco (já com migrations) ainda não
/// aplicou? Banco novo (sem a tabela do sqlx) não conta: não há o que copiar.
pub async fn has_pending_migrations(pool: &Db) -> Result<bool, DbError> {
    let has_table: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(pool)
    .await?;
    if has_table.is_none() {
        return Ok(false);
    }
    let applied: Vec<(i64,)> = sqlx::query_as("SELECT version FROM _sqlx_migrations")
        .fetch_all(pool)
        .await?;
    Ok(MIGRATOR
        .iter()
        .any(|m| !applied.iter().any(|(v,)| *v == m.version)))
}

/// Cópia íntegra do banco em `<path>.bak` (`VACUUM INTO`: consistente mesmo
/// em WAL, com outras conexões abertas). Substitui a cópia anterior.
pub async fn backup(pool: &Db, path: &Path) -> Result<PathBuf, DbError> {
    let bak = path.with_extension("db.bak");
    if bak.exists() {
        std::fs::remove_file(&bak).map_err(|e| DbError::Config(e.to_string()))?;
    }
    let target = bak.to_string_lossy().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{target}'"))
        .execute(pool)
        .await?;
    Ok(bak)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_path_from_url() {
        assert_eq!(
            file_path("sqlite://C:\\dados\\reemu.db"),
            Some(PathBuf::from("C:\\dados\\reemu.db"))
        );
        assert_eq!(
            file_path("sqlite:/tmp/x.db?mode=rwc"),
            Some(PathBuf::from("/tmp/x.db"))
        );
        assert_eq!(file_path("sqlite::memory:"), None);
        assert_eq!(file_path("postgres://x"), None);
    }

    #[tokio::test]
    async fn file_db_opens_twice_and_backs_up() {
        let dir = std::env::temp_dir().join(format!("reemu-db-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        let url = format!("sqlite://{}", path.display());
        let pool = connect(&url).await.unwrap();
        // banco novo: tudo aplicado, nada pendente
        assert!(!has_pending_migrations(&pool).await.unwrap());
        let bak = backup(&pool, &path).await.unwrap();
        assert!(bak.ends_with("t.db.bak") && bak.is_file());
        // a cópia é um banco válido com as mesmas migrations
        let copy = connect(&format!("sqlite://{}", bak.display()))
            .await
            .unwrap();
        assert!(!has_pending_migrations(&copy).await.unwrap());
        copy.close().await;
        pool.close().await;
        // reabrir não refaz nada
        let pool = connect(&url).await.unwrap();
        pool.close().await;
        let _ = std::fs::remove_dir_all(&dir);
    }
}
