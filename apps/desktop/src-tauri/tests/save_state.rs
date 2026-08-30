//! Orquestração de save state: arquivo em disco + metadata + validação de
//! core no load. Contra `db::SaveStateRepo` + SQLite in-memory.

use app_lib::save_state::{self, SaveError};
use domain::library::{Rom, RomRepository};
use domain::save_state::SaveStateRepository;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

fn scratch() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "reemu-savetest-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

async fn setup() -> (db::Db, PathBuf) {
    let db = db::connect_in_memory().await.unwrap();
    let roms = db::RomsRepo::new(db.clone());
    roms.add(&Rom {
        id: "rom1".into(),
        file_path: "/roms/rom1.nes".into(),
        crc32: "AA".into(),
        md5: "BB".into(),
        system_id: "nes".into(),
        added_at: 0,
        last_played_at: None,
    })
    .await
    .unwrap();
    (db, scratch())
}

#[tokio::test]
async fn save_writes_file_and_records_metadata() {
    let (db, dir) = setup().await;
    let repo = db::SaveStateRepo::new(db);

    let meta = save_state::save(&repo, &dir, "rom1", "mesen", Some(0), b"STATE-BYTES-A")
        .await
        .unwrap();

    assert_eq!(std::fs::read(&meta.file_path).unwrap(), b"STATE-BYTES-A");
    let listed = save_state::list(&repo, "rom1").await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, meta.id);
    assert_eq!(listed[0].slot, Some(0));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn saving_same_slot_replaces_previous() {
    let (db, dir) = setup().await;
    let repo = db::SaveStateRepo::new(db);

    let first = save_state::save(&repo, &dir, "rom1", "mesen", Some(1), b"OLD")
        .await
        .unwrap();
    let second = save_state::save(&repo, &dir, "rom1", "mesen", Some(1), b"NEW")
        .await
        .unwrap();

    // caminho determinístico por slot (convenção RetroArch); o registro antigo
    // foi trocado e o conteúdo é o novo.
    assert_ne!(first.id, second.id);
    assert_eq!(std::fs::read(&second.file_path).unwrap(), b"NEW");
    let listed = save_state::list(&repo, "rom1").await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, second.id);

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn load_validates_core_and_returns_meta() {
    let (db, dir) = setup().await;
    let repo = db::SaveStateRepo::new(db);
    let meta = save_state::save(&repo, &dir, "rom1", "mesen", None, b"S")
        .await
        .unwrap();

    // core certo -> ok
    let ok = save_state::load_bytes(&repo, &meta.id, Some("mesen"))
        .await
        .unwrap();
    assert_eq!(ok.core_id, "mesen");

    // core errado -> CoreMismatch
    let err = save_state::load_bytes(&repo, &meta.id, Some("nestopia"))
        .await
        .unwrap_err();
    assert!(matches!(err, SaveError::CoreMismatch { .. }), "{err:?}");

    // sem core carregado -> NoCore
    assert!(matches!(
        save_state::load_bytes(&repo, &meta.id, None)
            .await
            .unwrap_err(),
        SaveError::NoCore
    ));

    // id inexistente -> NotFound
    assert!(matches!(
        save_state::load_bytes(&repo, "nope", Some("mesen"))
            .await
            .unwrap_err(),
        SaveError::NotFound
    ));

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn delete_removes_file_and_record() {
    let (db, dir) = setup().await;
    let repo = db::SaveStateRepo::new(db);
    let meta = save_state::save(&repo, &dir, "rom1", "mesen", Some(2), b"S")
        .await
        .unwrap();

    save_state::delete(&repo, &meta.id).await.unwrap();
    assert!(std::fs::metadata(&meta.file_path).is_err());
    assert!(save_state::list(&repo, "rom1").await.unwrap().is_empty());
    assert!(repo.get_state(&meta.id).await.unwrap().is_none());

    let _ = std::fs::remove_dir_all(dir);
}
