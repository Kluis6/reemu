//! Banco migrado por um build com a outra quebra de linha nos `.sql` (CRLF no
//! checkout do Windows, LF no do Linux) tem que continuar abrindo; uma
//! migration com o conteúdo realmente mudado, não.

use db::{connect_in_memory, run_migrations};
use sha2::{Digest, Sha384};

async fn checksum(db: &db::Db, version: i64) -> Vec<u8> {
    let (c,): (Vec<u8>,) =
        sqlx::query_as("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
            .bind(version)
            .fetch_one(db)
            .await
            .unwrap();
    c
}

async fn set_checksum(db: &db::Db, version: i64, c: &[u8]) {
    sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
        .bind(c)
        .bind(version)
        .execute(db)
        .await
        .unwrap();
}

#[tokio::test]
async fn other_line_ending_is_healed() {
    let db = connect_in_memory().await.unwrap();
    let original = checksum(&db, 1).await;
    let sql = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0001_init.sql"
    ))
    .unwrap();
    // o texto com a quebra de linha que o binário NÃO embutiu
    let lf = sql.replace("\r\n", "\n");
    let crlf = lf.replace('\n', "\r\n");
    let other = if Sha384::digest(lf.as_bytes()).as_slice() == original.as_slice() {
        crlf
    } else {
        lf
    };
    set_checksum(&db, 1, Sha384::digest(other.as_bytes()).as_slice()).await;

    run_migrations(&db).await.expect("só a quebra de linha mudou: abre");
    assert_eq!(checksum(&db, 1).await, original);
}

#[tokio::test]
async fn changed_content_is_still_refused() {
    let db = connect_in_memory().await.unwrap();
    set_checksum(&db, 1, Sha384::digest(b"outro conteudo").as_slice()).await;
    assert!(run_migrations(&db).await.is_err());
}
