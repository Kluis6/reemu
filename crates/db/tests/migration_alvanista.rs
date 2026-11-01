//! Um banco de antes do Alvanista (até a migration 18) passa pela 19 sem perder
//! nada do que já tinha.

mod common;

use common::{mem_db, seed_rom};
use db::{run_migrations, AlvanistaRepo, RomsRepo};
use domain::library::RomRepository;

#[tokio::test]
async fn an_existing_database_migrates_and_keeps_its_data() {
    let db = mem_db().await;
    seed_rom(&db, "rom1", "nes").await;
    // Volta o banco para antes da 19.
    sqlx::query("DROP TABLE alvanista_state")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 19")
        .execute(&db)
        .await
        .unwrap();

    run_migrations(&db).await.unwrap();

    assert!(RomsRepo::new(db.clone())
        .get("rom1")
        .await
        .unwrap()
        .is_some());
    let alva = AlvanistaRepo::new(db);
    assert_eq!(alva.state_get("session").await.unwrap(), None);
    alva.state_set("session", "1").await.unwrap();
    assert_eq!(
        alva.state_get("session").await.unwrap().as_deref(),
        Some("1")
    );
}
