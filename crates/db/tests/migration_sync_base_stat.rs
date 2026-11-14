//! Um banco com a base do cloud save (até a migration 21) passa pela 22 sem
//! perder linha: as de antes ficam sem tamanho, mtime e linha do state, e as
//! novas os guardam.

mod common;

use common::mem_db;
use db::{run_migrations, SyncRepo};
use domain::sync::BaseRow;

fn base(key: &str) -> BaseRow {
    BaseRow {
        key: key.into(),
        kind: "state".into(),
        remote_save_id: "s1".into(),
        remote_version_id: "v1".into(),
        remote_hash: "aa".into(),
        local_hash: "aa".into(),
        local_path: "/saves/a.state".into(),
        rom_crc32: None,
        synced_at: 10,
        wiped_at: None,
        backup_path: None,
        local_size: None,
        local_mtime_ns: None,
        local_row: None,
    }
}

#[tokio::test]
async fn a_base_from_before_migrates_and_keeps_its_rows() {
    let db = mem_db().await;
    // Volta o banco para antes da 22, com uma linha da base gravada então.
    for col in ["local_size", "local_mtime_ns", "local_row"] {
        sqlx::query(&format!("ALTER TABLE sync_base DROP COLUMN {col}"))
            .execute(&db)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 22")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sync_base (key, kind, remote_save_id, remote_version_id, remote_hash, \
         local_hash, local_path, synced_at) \
         VALUES ('state:snes/c/a__slot1.state', 'state', 's1', 'v1', 'aa', 'aa', '/a', 10)",
    )
    .execute(&db)
    .await
    .unwrap();

    run_migrations(&db).await.unwrap();

    let repo = SyncRepo::new(db.clone());
    let old = repo
        .base_get("state:snes/c/a__slot1.state")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (old.local_size, old.local_mtime_ns, old.local_row),
        (None, None, None)
    );
    assert_eq!(old.local_hash, "aa");

    let stat = BaseRow {
        local_size: Some(4096),
        local_mtime_ns: Some(1_794_564_000_123_456_700),
        local_row: Some("row-1".into()),
        ..base("state:snes/c/b__slot2.state")
    };
    repo.base_put(&stat).await.unwrap();
    assert_eq!(
        repo.base_get(&stat.key).await.unwrap().as_ref(),
        Some(&stat)
    );
    // A marca de apagado no site (D127) deixa o tamanho e o mtime.
    repo.base_mark_wiped(&stat.key, 99, None).await.unwrap();
    let wiped = repo.base_get(&stat.key).await.unwrap().unwrap();
    assert_eq!(wiped.wiped_at, Some(99));
    assert_eq!(
        (
            wiped.local_size,
            wiped.local_mtime_ns,
            wiped.local_row.as_deref()
        ),
        (Some(4096), Some(1_794_564_000_123_456_700), Some("row-1"))
    );
}
