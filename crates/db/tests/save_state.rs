//! Repositório de metadata de save state / save RAM.

mod common;

use common::{mem_db, seed_rom};
use db::SaveStateRepo;
use domain::save_state::{SaveRamMetadata, SaveStateMetadata, SaveStateRepository};

fn state(id: &str, rom: &str, core: &str, slot: Option<u32>, created_at: i64) -> SaveStateMetadata {
    SaveStateMetadata {
        id: id.into(),
        rom_id: rom.into(),
        core_id: core.into(),
        slot,
        file_path: format!("/saves/{id}.state"),
        thumbnail_path: Some(format!("/saves/{id}.png")),
        created_at,
        play_time_at_save: Some(3600),
    }
}

#[tokio::test]
async fn record_list_get_delete_states() {
    let db = mem_db().await;
    seed_rom(&db, "rom1", "nes").await;
    let repo = SaveStateRepo::new(db);

    repo.record_state(&state("s1", "rom1", "mesen", Some(0), 100))
        .await
        .unwrap();
    repo.record_state(&state("s2", "rom1", "mesen", Some(1), 200))
        .await
        .unwrap();
    repo.record_state(&state("s3", "rom1", "mesen", None, 300))
        .await
        .unwrap();

    // mais recente primeiro
    let list = repo.list_states_for_rom("rom1").await.unwrap();
    assert_eq!(
        list.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["s3", "s2", "s1"]
    );

    let got = repo.get_state("s2").await.unwrap().unwrap();
    assert_eq!(got.slot, Some(1));
    assert_eq!(got.play_time_at_save, Some(3600));

    repo.delete_state("s2").await.unwrap();
    assert!(repo.get_state("s2").await.unwrap().is_none());
    assert_eq!(repo.list_states_for_rom("rom1").await.unwrap().len(), 2);
}

#[tokio::test]
async fn find_state_in_slot_returns_latest_in_that_slot() {
    let db = mem_db().await;
    seed_rom(&db, "rom1", "nes").await;
    let repo = SaveStateRepo::new(db);

    repo.record_state(&state("old", "rom1", "mesen", Some(2), 100))
        .await
        .unwrap();
    repo.record_state(&state("new", "rom1", "mesen", Some(2), 999))
        .await
        .unwrap();
    repo.record_state(&state("other", "rom1", "mesen", Some(3), 500))
        .await
        .unwrap();

    let hit = repo
        .find_state_in_slot("rom1", "mesen", 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(hit.id, "new");

    assert!(repo
        .find_state_in_slot("rom1", "mesen", 7)
        .await
        .unwrap()
        .is_none());
    // slot é por (rom, core)
    assert!(repo
        .find_state_in_slot("rom1", "outro-core", 2)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn state_requires_existing_rom() {
    let repo = SaveStateRepo::new(mem_db().await);
    let res = repo
        .record_state(&state("s1", "rom-fantasma", "mesen", Some(0), 1))
        .await;
    assert!(res.is_err(), "FK save_states -> roms deve barrar");
}

#[tokio::test]
async fn cascade_delete_from_rom_removes_states() {
    let db = mem_db().await;
    seed_rom(&db, "rom1", "nes").await;
    let repo = SaveStateRepo::new(db.clone());
    repo.record_state(&state("s1", "rom1", "mesen", Some(0), 1))
        .await
        .unwrap();

    sqlx::query("DELETE FROM roms WHERE id = 'rom1'")
        .execute(&db)
        .await
        .unwrap();

    assert!(repo.get_state("s1").await.unwrap().is_none());
}

#[tokio::test]
async fn save_ram_is_one_per_rom_core_and_upserts() {
    let db = mem_db().await;
    seed_rom(&db, "rom1", "gba").await;
    let repo = SaveStateRepo::new(db);

    assert!(repo.get_save_ram("rom1", "mgba").await.unwrap().is_none());

    repo.upsert_save_ram(&SaveRamMetadata {
        id: "sr1".into(),
        rom_id: "rom1".into(),
        core_id: "mgba".into(),
        file_path: "/saves/rom1.srm".into(),
        updated_at: 100,
    })
    .await
    .unwrap();

    // segundo upsert (mesmo rom+core) atualiza, não duplica nem falha
    repo.upsert_save_ram(&SaveRamMetadata {
        id: "sr2".into(),
        rom_id: "rom1".into(),
        core_id: "mgba".into(),
        file_path: "/saves/rom1-v2.srm".into(),
        updated_at: 200,
    })
    .await
    .unwrap();

    let ram = repo.get_save_ram("rom1", "mgba").await.unwrap().unwrap();
    assert_eq!(ram.file_path, "/saves/rom1-v2.srm");
    assert_eq!(ram.updated_at, 200);

    // core diferente = entrada separada
    repo.upsert_save_ram(&SaveRamMetadata {
        id: "sr3".into(),
        rom_id: "rom1".into(),
        core_id: "vba-next".into(),
        file_path: "/saves/rom1-vba.srm".into(),
        updated_at: 300,
    })
    .await
    .unwrap();
    assert_eq!(
        repo.get_save_ram("rom1", "vba-next")
            .await
            .unwrap()
            .unwrap()
            .file_path,
        "/saves/rom1-vba.srm"
    );
}
