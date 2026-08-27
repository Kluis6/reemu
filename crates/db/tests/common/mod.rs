//! Helpers compartilhados pelos testes de integração.
//! (Cada arquivo de teste é um crate; nem todos usam todos os helpers.)
#![allow(dead_code)]

use db::{connect_in_memory, Db};
use sqlx::Executor;

/// Banco SQLite in-memory com a migration já aplicada e `foreign_keys` ON.
pub async fn mem_db() -> Db {
    connect_in_memory().await.expect("abrir db in-memory")
}

/// Insere uma `roms` mínima (save_states/save_ram têm FK -> roms).
pub async fn seed_rom(db: &Db, id: &str, system_id: &str) {
    db.execute(
        sqlx::query(
            "INSERT INTO roms (id, file_path, crc32, md5, system_id, added_at) \
             VALUES (?1, ?2, 'CRC', 'MD5', ?3, 0)",
        )
        .bind(id)
        .bind(format!("/roms/{id}.zip"))
        .bind(system_id),
    )
    .await
    .expect("seed rom");
}

/// Insere um `shader_presets` mínimo e devolve o id.
pub async fn seed_preset(db: &Db, id: &str) {
    db.execute(sqlx::query(
        "INSERT INTO shader_presets (id, name, source_path, format) VALUES (?1, ?2, ?3, 'slang')",
    )
    .bind(id)
    .bind(format!("preset {id}"))
    .bind(format!("/presets/{id}.slangp")))
    .await
    .expect("seed preset");
}

/// Insere um `decoration_packs` mínimo.
pub async fn seed_pack(db: &Db, id: &str) {
    db.execute(sqlx::query(
        "INSERT INTO decoration_packs (id, name, source, base_path) VALUES (?1, ?2, 'bundled', ?3)",
    )
    .bind(id)
    .bind(format!("pack {id}"))
    .bind(format!("/packs/{id}")))
    .await
    .expect("seed pack");
}

/// Insere uma atribuição de shader chain crua (sem passar pelo repo).
pub async fn seed_shader_assignment(
    db: &Db,
    id: &str,
    scope: &str,
    system_id: Option<&str>,
    rom_id: Option<&str>,
    preset_id: &str,
) {
    db.execute(
        sqlx::query(
            "INSERT INTO shader_chain_assignments (id, scope, system_id, rom_id, preset_id) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(id)
        .bind(scope)
        .bind(system_id)
        .bind(rom_id)
        .bind(preset_id),
    )
    .await
    .expect("seed shader assignment");
}

pub async fn seed_decoration_assignment(
    db: &Db,
    id: &str,
    scope: &str,
    system_id: Option<&str>,
    rom_id: Option<&str>,
    pack_id: &str,
    asset_path: &str,
) {
    db.execute(
        sqlx::query(
            "INSERT INTO decoration_assignments \
             (id, scope, system_id, rom_id, pack_id, asset_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id)
        .bind(scope)
        .bind(system_id)
        .bind(rom_id)
        .bind(pack_id)
        .bind(asset_path),
    )
    .await
    .expect("seed decoration assignment");
}
