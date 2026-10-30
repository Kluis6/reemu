//! A migration do "Entrar com Alvanista" (0019) no banco de quem já usa o
//! ReEmu: nada do que existia muda, e só um banco que já teve a integração
//! (builds de pré-lançamento, com a migration como 0017 ou 0018) ganha o
//! marcador de sessão.

use db::run_migrations;
use sha2::{Digest, Sha384};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::BTreeMap;
use std::str::FromStr;

const MIGRATIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");

/// A migration `alvanista` dos builds de pré-lançamento.
const PRERELEASE_SQL: &str = "\
CREATE TABLE secret_fallback (
    account TEXT PRIMARY KEY,
    value   TEXT NOT NULL
);
CREATE TABLE alvanista_state (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

async fn memory_db() -> db::Db {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap()
}

/// Um banco migrado até `last` (inclusive), como uma versão mais velha do
/// ReEmu o deixou: as migrations até ali vão para uma pasta temporária e um
/// `Migrator` só com elas roda.
async fn db_up_to(last: i64, tag: &str) -> db::Db {
    let dir = std::env::temp_dir().join(format!("reemu-migrations-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(MIGRATIONS).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let version: i64 = name.split('_').next().unwrap().parse().unwrap();
        if version <= last {
            std::fs::copy(&path, dir.join(&name)).unwrap();
        }
    }
    let db = memory_db().await;
    Migrator::new(dir.as_path())
        .await
        .unwrap()
        .run(&db)
        .await
        .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    db
}

/// Todas as linhas de todas as tabelas (menos as do sqlx e do SQLite), cada
/// linha como texto (`quote` de cada coluna), em ordem.
async fn snapshot(db: &db::Db) -> BTreeMap<String, Vec<String>> {
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' \
         AND name NOT LIKE '\\_sqlx%' ESCAPE '\\' AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'",
    )
    .fetch_all(db)
    .await
    .unwrap();
    let mut out = BTreeMap::new();
    for (table,) in tables {
        let cols: Vec<(String,)> =
            sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .fetch_all(db)
                .await
                .unwrap();
        let row = cols
            .iter()
            .map(|(c,)| format!("quote(\"{c}\")"))
            .collect::<Vec<_>>()
            .join(" || '|' || ");
        let rows: Vec<(String,)> =
            sqlx::query_as(&format!("SELECT {row} FROM \"{table}\" ORDER BY 1"))
                .fetch_all(db)
                .await
                .unwrap();
        out.insert(table, rows.into_iter().map(|(r,)| r).collect());
    }
    out
}

#[tokio::test]
async fn a_v0_1_10_database_migrates_without_losing_data() {
    // A v0.1.10 parou na 0017 (cheats).
    let db = db_up_to(17, "v0110").await;
    for sql in [
        "UPDATE profile SET name = 'Luis', bio = 'oi', avatar = 'preset:4', onboarded = 1 WHERE id = 1",
        "INSERT INTO roms (id, file_path, crc32, md5, system_id, added_at, user_title, is_favorite, play_time_secs) \
         VALUES ('r1', '/roms/snes/a.sfc', 'abcd1234', 'ffff', 'snes', 1, 'Meu jogo', 1, 3600)",
        "INSERT INTO cheats (rom_id, description, code, enabled, position) VALUES ('r1', 'vidas', '7E0DBE:09', 1, 0)",
        "INSERT INTO library_folders (path, added_at) VALUES ('/roms', 1)",
    ] {
        sqlx::query(sql).execute(&db).await.unwrap();
    }
    let before = snapshot(&db).await;

    run_migrations(&db).await.unwrap();

    let after = snapshot(&db).await;
    for (table, rows) in &before {
        assert_eq!(after.get(table), Some(rows), "{table}");
    }
    // As tabelas da integração nascem vazias: sem marcador, o app não olha o
    // chaveiro nem a rede.
    assert!(after["alvanista_state"].is_empty());
    assert!(after["secret_fallback"].is_empty());
}

/// Banco de um build de pré-lançamento: `base` migrations do main e a
/// `alvanista` aplicada como `version`, com a integração já usada.
async fn prerelease_db(base: i64, version: i64) -> db::Db {
    let db = db_up_to(base, &format!("pre{version}")).await;
    sqlx::raw_sql(PRERELEASE_SQL).execute(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (?1, 'alvanista', 1, ?2, 0)",
    )
    .bind(version)
    .bind(Sha384::digest(PRERELEASE_SQL.as_bytes()).to_vec())
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO alvanista_state (key, value) VALUES ('device_id', 'dev'), ('mirror', 'm')",
    )
    .execute(&db)
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn a_prerelease_database_opens_and_its_session_is_checked_once() {
    // O fork (sobre a v0.1.9) usou a 0017; a primeira versão do PR, a 0018.
    for (base, version) in [(16, 17), (17, 18)] {
        let db = prerelease_db(base, version).await;
        run_migrations(&db).await.expect("o banco abre");

        let applied: Vec<(i64, String)> = sqlx::query_as(
            "SELECT version, description FROM _sqlx_migrations WHERE version >= 17 ORDER BY version",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        assert_eq!(
            applied,
            vec![
                (17, "cheats".to_owned()),
                (18, "aspect assignments".to_owned()),
                (19, "alvanista".to_owned()),
            ],
            "pré-lançamento na {version}"
        );
        let state: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM alvanista_state ORDER BY key")
                .fetch_all(&db)
                .await
                .unwrap();
        let state: Vec<(&str, &str)> = state
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            state,
            vec![
                ("device_id", "dev"),
                ("mirror", "m"),
                ("session", "migrated")
            ],
            "pré-lançamento na {version}"
        );
        // E abre de novo sem mexer em nada.
        run_migrations(&db).await.unwrap();
    }
}

#[tokio::test]
async fn a_prerelease_database_that_never_connected_gets_no_marker() {
    let db = db_up_to(17, "pre-unused").await;
    sqlx::raw_sql(PRERELEASE_SQL).execute(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (18, 'alvanista', 1, ?1, 0)",
    )
    .bind(Sha384::digest(PRERELEASE_SQL.as_bytes()).to_vec())
    .execute(&db)
    .await
    .unwrap();
    run_migrations(&db).await.unwrap();
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM alvanista_state")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(n, 0);
}
