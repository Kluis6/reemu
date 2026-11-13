mod common;

use common::mem_db;
use db::SyncRepo;
use domain::sync::{BaseRow, ConflictRow};

fn base(key: &str, kind: &str) -> BaseRow {
    BaseRow {
        key: key.into(),
        kind: kind.into(),
        remote_save_id: "s1".into(),
        remote_version_id: "v1".into(),
        remote_hash: "aa".into(),
        local_hash: "bb".into(),
        local_path: "/saves/a.srm".into(),
        rom_crc32: Some("DEADBEEF".into()),
        synced_at: 10,
        wiped_at: None,
        backup_path: None,
    }
}

fn conflict(key: &str, rom: Option<&str>) -> ConflictRow {
    ConflictRow {
        key: key.into(),
        local_hash: "l".into(),
        local_mtime: 1,
        local_size: 2,
        remote_save_id: "s".into(),
        remote_version_id: "v".into(),
        remote_hash: "r".into(),
        remote_device_label: Some("Deck".into()),
        remote_client_mtime: None,
        remote_size: 3,
        remote_deleted: false,
        rom_id: rom.map(Into::into),
        seen_at: 5,
    }
}

#[tokio::test]
async fn base_round_trips_a_row() {
    let repo = SyncRepo::new(mem_db().await);
    let row = base("file:nes/a.srm", "file");
    repo.base_put(&row).await.unwrap();
    assert_eq!(
        repo.base_get("file:nes/a.srm").await.unwrap(),
        Some(row.clone())
    );
    let mut newer = row;
    newer.remote_version_id = "v2".into();
    newer.rom_crc32 = None;
    repo.base_put(&newer).await.unwrap();
    assert_eq!(repo.base_get("file:nes/a.srm").await.unwrap(), Some(newer));
    repo.base_delete("file:nes/a.srm").await.unwrap();
    assert_eq!(repo.base_get("file:nes/a.srm").await.unwrap(), None);
}

#[tokio::test]
async fn base_all_filters_by_kind() {
    let repo = SyncRepo::new(mem_db().await);
    repo.base_put(&base("file:nes/a.srm", "file"))
        .await
        .unwrap();
    repo.base_put(&base("state:nes/core/a.state", "state"))
        .await
        .unwrap();
    let files = repo.base_all("file").await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].key, "file:nes/a.srm");
    assert_eq!(repo.base_all("config").await.unwrap().len(), 0);
}

#[tokio::test]
async fn base_mark_wiped_sets_the_column() {
    let repo = SyncRepo::new(mem_db().await);
    repo.base_put(&base("file:nes/a.srm", "file"))
        .await
        .unwrap();
    repo.base_mark_wiped("file:nes/a.srm", 99, Some("/data/sync-backups/x/a.srm"))
        .await
        .unwrap();
    let got = repo.base_get("file:nes/a.srm").await.unwrap().unwrap();
    assert_eq!(got.wiped_at, Some(99));
    assert_eq!(
        got.backup_path.as_deref(),
        Some("/data/sync-backups/x/a.srm")
    );
    // Uma sincronização nova grava a base de novo e apaga a marca e o lugar.
    repo.base_put(&base("file:nes/a.srm", "file"))
        .await
        .unwrap();
    let got = repo.base_get("file:nes/a.srm").await.unwrap().unwrap();
    assert_eq!((got.wiped_at, got.backup_path), (None, None));
}

#[tokio::test]
async fn clear_all_empties_base_and_conflicts() {
    let repo = SyncRepo::new(mem_db().await);
    repo.base_put(&base("file:nes/a.srm", "file"))
        .await
        .unwrap();
    repo.base_put(&base("state:nes/core/a.state", "state"))
        .await
        .unwrap();
    repo.conflict_put(&conflict("file:nes/a.srm", None))
        .await
        .unwrap();
    repo.clear_all().await.unwrap();
    assert!(repo.base_all("file").await.unwrap().is_empty());
    assert!(repo.base_all("state").await.unwrap().is_empty());
    assert!(repo.conflicts_all().await.unwrap().is_empty());
}

#[tokio::test]
async fn conflicts_round_trip_and_filter_by_rom() {
    let repo = SyncRepo::new(mem_db().await);
    repo.conflict_put(&conflict("file:nes/a.srm", Some("rom1")))
        .await
        .unwrap();
    // O lado da nuvem que é uma remoção no site volta como tal.
    let deleted = ConflictRow {
        remote_deleted: true,
        ..conflict("file:nes/b.srm", None)
    };
    repo.conflict_put(&deleted).await.unwrap();
    let all = repo.conflicts_all().await.unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&deleted));
    let for_rom = repo.conflicts_for_rom("rom1").await.unwrap();
    assert_eq!(for_rom, vec![conflict("file:nes/a.srm", Some("rom1"))]);
    repo.conflict_delete("file:nes/a.srm").await.unwrap();
    assert_eq!(repo.conflicts_all().await.unwrap().len(), 1);
}
