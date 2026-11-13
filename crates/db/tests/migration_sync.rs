//! Um banco de antes do cloud save (até a migration 20) passa pela 21 sem
//! perder nada do que já tinha.

mod common;

use common::{mem_db, seed_rom};
use db::{run_migrations, RomsRepo, SyncRepo};
use domain::library::RomRepository;

#[tokio::test]
async fn an_existing_database_migrates_and_keeps_its_data() {
    let db = mem_db().await;
    seed_rom(&db, "rom1", "nes").await;
    // Volta o banco para antes da 21.
    sqlx::query("DROP TABLE sync_base")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DROP TABLE sync_conflicts")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 21")
        .execute(&db)
        .await
        .unwrap();

    run_migrations(&db).await.unwrap();

    assert!(RomsRepo::new(db.clone())
        .get("rom1")
        .await
        .unwrap()
        .is_some());
    let repo = SyncRepo::new(db.clone());
    assert!(repo.base_all("file").await.unwrap().is_empty());
    assert!(repo.conflicts_all().await.unwrap().is_empty());

    // As colunas da remoção no site: o lugar do arquivo na base e a marca
    // no conflito (0 quando quem grava não diz).
    let cols = |table: &'static str| {
        let db = db.clone();
        async move {
            sqlx::query_as::<_, (String,)>(&format!(
                "SELECT name FROM pragma_table_info('{table}')"
            ))
            .fetch_all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|(n,)| n)
            .collect::<Vec<_>>()
        }
    };
    assert!(cols("sync_base").await.contains(&"backup_path".to_owned()));
    assert!(cols("sync_conflicts")
        .await
        .contains(&"remote_deleted".to_owned()));
    sqlx::query(
        "INSERT INTO sync_conflicts (key, local_hash, local_mtime, local_size, remote_save_id, \
         remote_version_id, remote_hash, remote_size, seen_at) \
         VALUES ('k', 'l', 1, 1, 's', 'v', 'r', 1, 1)",
    )
    .execute(&db)
    .await
    .unwrap();
    assert!(!repo.conflicts_all().await.unwrap()[0].remote_deleted);
}
