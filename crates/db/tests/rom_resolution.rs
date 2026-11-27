//! Persistência de `rom_resolution`, coluna `game_metadata.game_uuid` e
//! seleção de itens vencidos em `pending_due`.

mod common;

use common::{mem_db, seed_rom};
use db::{run_migrations, MetadataRepo, RomResolutionRepo};
use domain::metadata::{GameMetadata, MetadataRepository, ResolutionState, RomResolution};

#[tokio::test]
async fn an_existing_database_migrates_to_0025_without_losing_metadata() {
    let db = mem_db().await;
    sqlx::query("DROP TABLE rom_resolution")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE game_metadata DROP COLUMN game_uuid")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 25")
        .execute(&db)
        .await
        .unwrap();

    seed_rom(&db, "r1", "snes").await;
    sqlx::query(
        "INSERT INTO game_metadata (id, rom_id, title, provider_source) \
         VALUES ('r1', 'r1', 'Super Metroid', 'screenscraper')",
    )
    .execute(&db)
    .await
    .unwrap();

    run_migrations(&db).await.unwrap();

    let meta = MetadataRepo::new(db.clone())
        .get_metadata("r1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(meta.title, "Super Metroid");
    assert_eq!(meta.game_uuid, None);
}

#[tokio::test]
async fn resolution_round_trip_and_cascade_on_rom_delete() {
    let db = mem_db().await;
    seed_rom(&db, "r1", "nes").await;
    seed_rom(&db, "r2", "snes").await;
    let repo = RomResolutionRepo::new(db.clone());
    let meta_repo = MetadataRepo::new(db.clone());

    assert_eq!(repo.unresolved_rom_ids().await.unwrap(), vec!["r1", "r2"]);

    let row = RomResolution {
        rom_id: "r1".into(),
        state: ResolutionState::Found,
        game_uuid: Some("11111111-2222-3333-4444-555555555555".into()),
        matched_by: Some("sha1".into()),
        reason: None,
        retry_after: None,
        media_pending: false,
        answer_json: Some(
            r#"{"game_id":"11111111-2222-3333-4444-555555555555","title":"Contra"}"#.into(),
        ),
        resolved_at: 1_700_000_000,
    };
    repo.put(&row).await.unwrap();
    assert_eq!(repo.get("r1").await.unwrap(), Some(row.clone()));
    assert_eq!(repo.all_found().await.unwrap(), vec![row.clone()]);
    assert_eq!(
        repo.by_game_uuid("11111111-2222-3333-4444-555555555555")
            .await
            .unwrap(),
        Some(row)
    );
    assert_eq!(repo.unresolved_rom_ids().await.unwrap(), vec!["r2"]);

    meta_repo
        .upsert_metadata(&GameMetadata {
            rom_id: "r1".into(),
            title: "Contra".into(),
            cover_url: Some("https://cdn.alvanista.com/contra.png".into()),
            provider_source: Some("alvanista".into()),
            game_uuid: Some("11111111-2222-3333-4444-555555555555".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    let got_meta = meta_repo.get_metadata("r1").await.unwrap().unwrap();
    assert_eq!(
        got_meta.game_uuid.as_deref(),
        Some("11111111-2222-3333-4444-555555555555")
    );
    let titles = meta_repo.titles_and_covers().await.unwrap();
    assert_eq!(titles.get("r1"), Some(&("Contra".to_owned(), true)));

    sqlx::query("DELETE FROM roms WHERE id = 'r1'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(repo.get("r1").await.unwrap(), None);
}

#[tokio::test]
async fn pending_due_selects_only_rows_ready_for_retry_and_respects_seven_day_media_window() {
    let db = mem_db().await;
    for id in [
        "p_due",
        "p_future",
        "m_due",
        "m_future",
        "m_expired",
        "f_done",
        "u_none",
        "u_lookup_due",
        "manual",
    ] {
        seed_rom(&db, id, "nes").await;
    }
    let repo = RomResolutionRepo::new(db);
    let now: i64 = 1_000_000;

    let put = |rom_id: &str,
               state: ResolutionState,
               reason: Option<&str>,
               retry_after: Option<i64>,
               media_pending: bool,
               resolved_at: i64| {
        let repo = repo.clone();
        let row = RomResolution {
            rom_id: rom_id.to_owned(),
            state,
            game_uuid: matches!(state, ResolutionState::Found | ResolutionState::Manual)
                .then(|| "uuid-1".to_owned()),
            matched_by: None,
            reason: reason.map(str::to_owned),
            retry_after,
            media_pending,
            answer_json: None,
            resolved_at,
        };
        async move { repo.put(&row).await.unwrap() }
    };

    put(
        "p_due",
        ResolutionState::Pending,
        None,
        Some(now),
        false,
        now - 1800,
    )
    .await;
    put(
        "p_future",
        ResolutionState::Pending,
        None,
        Some(now + 60),
        false,
        now,
    )
    .await;
    put(
        "m_due",
        ResolutionState::Found,
        None,
        Some(now - 10),
        true,
        now - 3600,
    )
    .await;
    put(
        "m_future",
        ResolutionState::Found,
        None,
        Some(now + 3600),
        true,
        now - 60,
    )
    .await;
    // Mais de 7 dias desde resolved_at: fica com o que já tem e sai da fila.
    put(
        "m_expired",
        ResolutionState::Found,
        None,
        Some(now - 10),
        true,
        now - 7 * 86_400,
    )
    .await;
    put(
        "f_done",
        ResolutionState::Found,
        None,
        None,
        false,
        now - 100,
    )
    .await;
    put(
        "u_none",
        ResolutionState::Unknown,
        Some("not_indexed"),
        None,
        false,
        now - 100,
    )
    .await;
    put(
        "u_lookup_due",
        ResolutionState::Unknown,
        Some("lookup_failed"),
        Some(now),
        false,
        now - 86_400,
    )
    .await;
    put(
        "manual",
        ResolutionState::Manual,
        None,
        None,
        false,
        now - 100,
    )
    .await;

    assert_eq!(
        repo.pending_due(now).await.unwrap(),
        vec!["m_due", "p_due", "u_lookup_due"]
    );
    assert_eq!(repo.counts().await.unwrap(), (5, 2, 2));
    let skip = repo.skip_own_scraper_ids().await.unwrap();
    assert!(skip.contains("p_due") && skip.contains("p_future"));
    assert!(skip.contains("f_done") && skip.contains("manual"));
    assert!(!skip.contains("u_none") && !skip.contains("u_lookup_due"));
}
