//! Um banco de antes da identidade (até a migration 23) passa pela 24 sem
//! perder nada: as linhas de antes ficam com as colunas novas em NULL.

mod common;

use common::{mem_db, seed_rom};
use db::{run_migrations, RomsRepo};
use domain::library::RomRepository;

const NEW_COLS: [&str; 5] = [
    "sha1",
    "serial",
    "ident_name",
    "ident_size",
    "identified_at",
];

#[tokio::test]
async fn an_existing_database_migrates_and_keeps_its_roms_with_null_identity() {
    let db = mem_db().await;
    // Volta a tabela para antes da 24: tira as colunas e o registro.
    for c in NEW_COLS {
        sqlx::query(&format!("ALTER TABLE roms DROP COLUMN {c}"))
            .execute(&db)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 24")
        .execute(&db)
        .await
        .unwrap();
    seed_rom(&db, "rom1", "nes").await;

    run_migrations(&db).await.unwrap();

    let cols: Vec<String> =
        sqlx::query_as::<_, (String,)>("SELECT name FROM pragma_table_info('roms')")
            .fetch_all(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|(n,)| n)
            .collect();
    for c in NEW_COLS {
        assert!(cols.contains(&c.to_owned()), "falta a coluna {c}");
    }

    let rom = RomsRepo::new(db.clone())
        .get("rom1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rom.crc32, "CRC");
    assert_eq!(rom.sha1, None);
    assert_eq!(rom.serial, None);
    assert_eq!(rom.ident_name, None);
    assert_eq!(rom.ident_size, None);
    assert_eq!(rom.identified_at, None);
}

#[tokio::test]
async fn identity_columns_round_trip_through_the_repo() {
    let db = mem_db().await;
    let repo = RomsRepo::new(db);
    let rom = domain::library::Rom {
        id: "r".into(),
        file_path: "/roms/a.nes".into(),
        crc32: "AAAAAAAA".into(),
        md5: "m".into(),
        system_id: "nes".into(),
        sha1: Some("s".into()),
        serial: Some("SLUS-00594".into()),
        ident_name: Some("a.nes".into()),
        ident_size: Some(40960),
        identified_at: Some(99),
        ..Default::default()
    };
    repo.add(&rom).await.unwrap();
    assert_eq!(repo.get("r").await.unwrap().unwrap(), rom);
}
