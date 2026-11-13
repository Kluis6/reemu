//! Um banco de antes do cloud save (até a migration 19) passa pela 20 sem
//! perder nada do que já tinha.

mod common;

use common::{mem_db, seed_rom};
use db::{run_migrations, RomsRepo, SyncRepo};
use domain::library::RomRepository;

#[tokio::test]
async fn an_existing_database_migrates_and_keeps_its_data() {
    let db = mem_db().await;
    seed_rom(&db, "rom1", "nes").await;
    // Volta o banco para antes da 20.
    sqlx::query("DROP TABLE sync_base")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DROP TABLE sync_conflicts")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 20")
        .execute(&db)
        .await
        .unwrap();

    run_migrations(&db).await.unwrap();

    assert!(RomsRepo::new(db.clone())
        .get("rom1")
        .await
        .unwrap()
        .is_some());
    let repo = SyncRepo::new(db);
    assert!(repo.base_all("file").await.unwrap().is_empty());
    assert!(repo.conflicts_all().await.unwrap().is_empty());
}
