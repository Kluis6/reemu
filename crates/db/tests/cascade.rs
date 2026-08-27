//! Resolução em cascata rom -> system -> default, para shader chain e
//! decoração. Cobre os três escopos e o caso "nenhum encontrado".

mod common;

use common::*;
use db::{DecorationRepo, ShaderChainRepo};
use domain::decoration::DecorationResolver;
use domain::shader_chain::{AssignmentScope, ShaderChainResolver};

#[tokio::test]
async fn shader_resolve_prefers_rom_then_system_then_default() {
    let db = mem_db().await;
    seed_preset(&db, "p_default").await;
    seed_preset(&db, "p_system").await;
    seed_preset(&db, "p_rom").await;
    seed_shader_assignment(&db, "a_default", "default", None, None, "p_default").await;
    seed_shader_assignment(&db, "a_system", "system", Some("nes"), None, "p_system").await;
    seed_shader_assignment(&db, "a_rom", "rom", None, Some("rom-42"), "p_rom").await;

    let repo = ShaderChainRepo::new(db.clone());

    // rom ganha quando existe
    let hit = repo.resolve("nes", Some("rom-42")).await.unwrap().unwrap();
    assert_eq!(hit.preset_id, "p_rom");
    assert_eq!(hit.scope, AssignmentScope::Rom);

    // sem atribuição de rom -> cai pro system
    let hit = repo
        .resolve("nes", Some("rom-sem-nada"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(hit.preset_id, "p_system");
    assert_eq!(hit.scope, AssignmentScope::System);

    // sem rom e sem system -> default
    let hit = repo.resolve("snes", None).await.unwrap().unwrap();
    assert_eq!(hit.preset_id, "p_default");
    assert_eq!(hit.scope, AssignmentScope::Default);
}

#[tokio::test]
async fn shader_resolve_none_when_no_assignment_anywhere() {
    let db = mem_db().await;
    let repo = ShaderChainRepo::new(db);
    let got = repo.resolve("nes", Some("rom-1")).await.unwrap();
    assert!(got.is_none());
}

#[tokio::test]
async fn shader_resolve_includes_parameter_overrides() {
    let db = mem_db().await;
    seed_preset(&db, "p1").await;
    seed_shader_assignment(&db, "a1", "system", Some("md"), None, "p1").await;
    sqlx::query(
        "INSERT INTO shader_parameter_overrides (id, assignment_id, parameter_key, value) \
         VALUES ('o1', 'a1', 'curvature', '0.03'), ('o2', 'a1', 'scanlines', '1.0')",
    )
    .execute(&db)
    .await
    .unwrap();

    let repo = ShaderChainRepo::new(db);
    let hit = repo.resolve("md", None).await.unwrap().unwrap();
    assert_eq!(
        hit.parameter_overrides.get("curvature").map(String::as_str),
        Some("0.03")
    );
    assert_eq!(
        hit.parameter_overrides.get("scanlines").map(String::as_str),
        Some("1.0")
    );
}

#[tokio::test]
async fn decoration_resolve_cascade_and_none() {
    let db = mem_db().await;
    seed_pack(&db, "bezels").await;
    seed_decoration_assignment(&db, "d_def", "default", None, None, "bezels", "default.png").await;
    seed_decoration_assignment(
        &db,
        "d_sys",
        "system",
        Some("nes"),
        None,
        "bezels",
        "nes.png",
    )
    .await;
    seed_decoration_assignment(
        &db,
        "d_rom",
        "rom",
        None,
        Some("rom-7"),
        "bezels",
        "rom7.png",
    )
    .await;

    let repo = DecorationRepo::new(db.clone());

    assert_eq!(
        repo.resolve("nes", Some("rom-7"))
            .await
            .unwrap()
            .unwrap()
            .asset_path,
        "rom7.png"
    );
    assert_eq!(
        repo.resolve("nes", Some("outra-rom"))
            .await
            .unwrap()
            .unwrap()
            .asset_path,
        "nes.png"
    );
    assert_eq!(
        repo.resolve("gb", None).await.unwrap().unwrap().asset_path,
        "default.png"
    );

    let empty = mem_db().await;
    let repo = DecorationRepo::new(empty);
    assert!(repo.resolve("nes", Some("x")).await.unwrap().is_none());
}

#[tokio::test]
async fn assignment_shape_check_is_enforced_by_schema() {
    // 'default' com system_id preenchido viola o CHECK de forma.
    let db = mem_db().await;
    seed_preset(&db, "p").await;
    let bad = sqlx::query(
        "INSERT INTO shader_chain_assignments (id, scope, system_id, rom_id, preset_id) \
         VALUES ('x', 'default', 'nes', NULL, 'p')",
    )
    .execute(&db)
    .await;
    assert!(
        bad.is_err(),
        "CHECK deveria rejeitar 'default' com system_id"
    );
}

#[tokio::test]
async fn only_one_default_assignment_allowed() {
    let db = mem_db().await;
    seed_preset(&db, "p1").await;
    seed_preset(&db, "p2").await;
    seed_shader_assignment(&db, "a1", "default", None, None, "p1").await;
    let dup = sqlx::query(
        "INSERT INTO shader_chain_assignments (id, scope, system_id, rom_id, preset_id) \
         VALUES ('a2', 'default', NULL, NULL, 'p2')",
    )
    .execute(&db)
    .await;
    assert!(dup.is_err(), "índice parcial deveria impedir 2º 'default'");
}
