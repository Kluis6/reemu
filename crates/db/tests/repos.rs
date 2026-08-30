//! Testes de integração dos repositórios não-cascata: core options,
//! audio config (linha única), installed cores (upsert de render), roms.

mod common;

use common::mem_db;
use db::{
    AudioConfigRepo, CoreOptionsRepo, InstalledCoresRepo, MetadataRepo, RomsRepo, ShaderChainRepo,
};
use domain::audio::{AudioConfig, AudioConfigRepository};
use domain::core_loader::{
    CoreRenderRequirements, InstalledCore, InstalledCoreRepository, RenderBackend,
};
use domain::core_options::{CoreOptionDefinition, CoreOptionType, CoreOptionsStore};
use domain::library::{Rom, RomRepository};
use domain::metadata::{MatchStatus, MetadataConfig, MetadataRepository, ScrapeCandidate};
use domain::shader_chain::{
    AssignmentScope, ShaderChainResolver, ShaderChainStore, ShaderFormat, ShaderPreset,
};

fn rom(id: &str, crc: &str, system: &str) -> Rom {
    Rom {
        id: id.into(),
        file_path: format!("/roms/{id}.zip"),
        crc32: crc.into(),
        md5: format!("md5-{id}"),
        system_id: system.into(),
        added_at: 1_700_000_000,
        last_played_at: None,
    }
}

#[tokio::test]
async fn roms_crud_and_lookups() {
    let repo = RomsRepo::new(mem_db().await);
    repo.add(&rom("r1", "AABBCCDD", "nes")).await.unwrap();
    repo.add(&rom("r2", "AABBCCDD", "nes")).await.unwrap(); // mesmo CRC
    repo.add(&rom("r3", "11223344", "snes")).await.unwrap();

    assert_eq!(repo.get("r1").await.unwrap().unwrap().system_id, "nes");
    assert!(repo.get("nao-existe").await.unwrap().is_none());

    assert_eq!(repo.find_by_crc32("AABBCCDD").await.unwrap().len(), 2);
    assert_eq!(
        repo.find_by_path("/roms/r3.zip").await.unwrap().unwrap().id,
        "r3"
    );
    assert_eq!(repo.list_by_system("nes").await.unwrap().len(), 2);

    repo.remove("r1").await.unwrap();
    assert_eq!(repo.find_by_crc32("AABBCCDD").await.unwrap().len(), 1);
}

fn preset(id: &str, builtin: bool) -> ShaderPreset {
    ShaderPreset {
        id: id.into(),
        name: id.into(),
        source_path: id.into(),
        format: ShaderFormat::Slang,
        is_builtin: builtin,
        includes_bezel: false,
    }
}

#[tokio::test]
async fn shader_chain_assignment_cascade_and_replace() {
    let db = mem_db().await;
    let roms = RomsRepo::new(db.clone());
    roms.add(&rom("r1", "AA", "snes")).await.unwrap();
    let sc = ShaderChainRepo::new(db);

    sc.upsert_preset(&preset("crt", true)).await.unwrap();
    sc.upsert_preset(&preset("lcd", true)).await.unwrap();
    assert_eq!(sc.list_presets().await.unwrap().len(), 2);

    // sem atribuição → None
    assert!(sc.resolve("snes", Some("r1")).await.unwrap().is_none());

    // default
    sc.set_assignment(AssignmentScope::Default, None, None, "crt")
        .await
        .unwrap();
    assert_eq!(
        sc.resolve("snes", Some("r1"))
            .await
            .unwrap()
            .unwrap()
            .preset_id,
        "crt"
    );

    // rom vence default
    sc.set_assignment(AssignmentScope::Rom, None, Some("r1"), "lcd")
        .await
        .unwrap();
    assert_eq!(
        sc.resolve("snes", Some("r1"))
            .await
            .unwrap()
            .unwrap()
            .preset_id,
        "lcd"
    );

    // trocar a atribuição do escopo (não duplica)
    sc.set_assignment(AssignmentScope::Default, None, None, "lcd")
        .await
        .unwrap();
    sc.set_assignment(AssignmentScope::Default, None, None, "crt")
        .await
        .unwrap();

    // limpar rom → volta pro default
    sc.clear_assignment(AssignmentScope::Rom, None, Some("r1"))
        .await
        .unwrap();
    assert_eq!(
        sc.resolve("snes", Some("r1"))
            .await
            .unwrap()
            .unwrap()
            .preset_id,
        "crt"
    );
}

#[tokio::test]
async fn shader_parameter_overrides_roundtrip() {
    let db = mem_db().await;
    RomsRepo::new(db.clone())
        .add(&rom("r1", "AA", "snes"))
        .await
        .unwrap();
    let sc = ShaderChainRepo::new(db);
    sc.upsert_preset(&preset("crt", true)).await.unwrap();

    // sem atribuição no escopo → erro claro
    assert!(sc
        .set_parameter_override(AssignmentScope::Rom, None, Some("r1"), "SCANLINE", "0.4")
        .await
        .is_err());

    sc.set_assignment(AssignmentScope::Rom, None, Some("r1"), "crt")
        .await
        .unwrap();
    sc.set_parameter_override(AssignmentScope::Rom, None, Some("r1"), "SCANLINE", "0.4")
        .await
        .unwrap();
    sc.set_parameter_override(AssignmentScope::Rom, None, Some("r1"), "SCANLINE", "0.7")
        .await
        .unwrap(); // upsert, não duplica

    let a = sc.resolve("snes", Some("r1")).await.unwrap().unwrap();
    assert_eq!(
        a.parameter_overrides.get("SCANLINE").map(String::as_str),
        Some("0.7")
    );

    sc.clear_parameter_overrides(AssignmentScope::Rom, None, Some("r1"))
        .await
        .unwrap();
    let a = sc.resolve("snes", Some("r1")).await.unwrap().unwrap();
    assert!(a.parameter_overrides.is_empty());
}

fn candidate(title: &str, exact: bool) -> ScrapeCandidate {
    ScrapeCandidate {
        provider: "screenscraper".into(),
        external_id: "123".into(),
        title: title.into(),
        description: Some("desc".into()),
        cover_url: Some("http://x/c.png".into()),
        release_date: Some("1990".into()),
        genre: Some("Platform".into()),
        exact_hash_match: exact,
    }
}

#[tokio::test]
async fn metadata_config_scrape_and_pending_review() {
    let db = mem_db().await;
    RomsRepo::new(db.clone())
        .add(&rom("r1", "AA", "snes"))
        .await
        .unwrap();
    RomsRepo::new(db.clone())
        .add(&rom("r2", "BB", "nes"))
        .await
        .unwrap();
    let m = MetadataRepo::new(db);

    // config singleton
    assert_eq!(m.get_config().await.unwrap().provider, "screenscraper");
    m.set_config(&MetadataConfig {
        provider: "screenscraper".into(),
        screenscraper_user: Some("u".into()),
        screenscraper_password: Some("p".into()),
    })
    .await
    .unwrap();
    assert_eq!(
        m.get_config().await.unwrap().screenscraper_user.as_deref(),
        Some("u")
    );

    // fila = as 2 ROMs sem match
    assert_eq!(m.rom_ids_without_match().await.unwrap().len(), 2);

    // hash exato → auto (metadata aplicada na hora)
    m.record_match(
        "r1",
        &candidate("Super Mario World", true),
        MatchStatus::AutoMatched,
    )
    .await
    .unwrap();
    m.upsert_metadata(&domain::metadata::GameMetadata {
        rom_id: "r1".into(),
        title: "Super Mario World".into(),
        description: Some("desc".into()),
        cover_url: None,
        release_date: Some("1990".into()),
        genre: Some("Platform".into()),
        provider_source: Some("screenscraper".into()),
    })
    .await
    .unwrap();
    assert_eq!(
        m.get_metadata("r1").await.unwrap().unwrap().title,
        "Super Mario World"
    );

    // match por nome → pending; não aplica metadata até revisão
    m.record_match(
        "r2",
        &candidate("Some NES Game", false),
        MatchStatus::PendingReview,
    )
    .await
    .unwrap();
    assert!(m.get_metadata("r2").await.unwrap().is_none());
    let pending = m.list_pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].rom_id, "r2");
    assert_eq!(pending[0].candidate.title, "Some NES Game");

    // aceitar → metadata aplicada, sai da fila de pendências
    m.resolve_pending("r2", true).await.unwrap();
    assert_eq!(
        m.get_metadata("r2").await.unwrap().unwrap().title,
        "Some NES Game"
    );
    assert!(m.list_pending().await.unwrap().is_empty());

    // já não há ROM sem match
    assert!(m.rom_ids_without_match().await.unwrap().is_empty());
}

#[tokio::test]
async fn roms_duplicate_path_is_error() {
    let repo = RomsRepo::new(mem_db().await);
    repo.add(&rom("r1", "AA", "nes")).await.unwrap();
    let mut dup = rom("r2", "BB", "nes");
    dup.file_path = "/roms/r1.zip".into();
    assert!(repo.add(&dup).await.is_err());
}

#[tokio::test]
async fn audio_config_is_single_row_get_update() {
    let repo = AudioConfigRepo::new(mem_db().await);

    // migration semeia o default
    let cfg = repo.get().await.unwrap();
    assert!(cfg.rate_control_enabled);
    assert!((cfg.rate_control_delta - 0.005).abs() < 1e-6);
    assert_eq!(cfg.output_device_id, None);

    let updated = AudioConfig {
        output_device_id: Some("alsa:card0".into()),
        output_device_name: Some("Placa X".into()),
        rate_control_enabled: false,
        rate_control_delta: 0.01,
        sample_rate_preference: Some(48_000),
    };
    repo.update(&updated).await.unwrap();

    let cfg = repo.get().await.unwrap();
    assert_eq!(cfg, updated);
}

#[tokio::test]
async fn installed_cores_register_is_idempotent_and_render_upsert_separate() {
    let repo = InstalledCoresRepo::new(mem_db().await);

    let mut core = InstalledCore {
        core_id: "mesen".into(),
        version: "1.0".into(),
        installed_at: 100,
        render_requirements: None,
    };
    repo.register(&core).await.unwrap();

    // re-registrar (ex: update do core) não explode e atualiza a versão
    core.version = "1.1".into();
    core.installed_at = 200;
    repo.register(&core).await.unwrap();
    let got = repo.get("mesen").await.unwrap().unwrap();
    assert_eq!(got.version, "1.1");
    assert!(got.render_requirements.is_none());

    // primeiro load detecta os requisitos -> upsert só nesses campos
    let reqs = CoreRenderRequirements {
        render_backend: RenderBackend::OpenGl,
        gl_version_min: Some("3.3".into()),
        gl_profile: Some("core".into()),
        needs_depth_stencil: true,
    };
    repo.set_render_requirements("mesen", &reqs).await.unwrap();

    let got = repo.get("mesen").await.unwrap().unwrap();
    assert_eq!(got.version, "1.1"); // identidade preservada
    assert_eq!(got.render_requirements.as_ref().unwrap(), &reqs);

    // set_render_requirements em core não registrado é erro
    assert!(repo
        .set_render_requirements("fantasma", &reqs)
        .await
        .is_err());
}

#[tokio::test]
async fn installed_cores_render_backend_maps_to_lowercase_db_value() {
    let db = mem_db().await;
    let repo = InstalledCoresRepo::new(db.clone());
    repo.register(&InstalledCore {
        core_id: "flycast".into(),
        version: "2".into(),
        installed_at: 1,
        render_requirements: None,
    })
    .await
    .unwrap();
    repo.set_render_requirements(
        "flycast",
        &CoreRenderRequirements {
            render_backend: RenderBackend::Vulkan,
            gl_version_min: None,
            gl_profile: None,
            needs_depth_stencil: false,
        },
    )
    .await
    .unwrap();

    // valor cru no banco tem que bater com o CHECK ('software'|'opengl'|'vulkan')
    let raw: (String,) =
        sqlx::query_as("SELECT render_backend FROM installed_cores WHERE core_id = 'flycast'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(raw.0, "vulkan");
}

#[tokio::test]
async fn core_options_schema_replace_get_set() {
    let db = mem_db().await;
    // schema tem FK -> o core precisa existir
    let cores = InstalledCoresRepo::new(db.clone());
    cores
        .register(&InstalledCore {
            core_id: "pcsx2".into(),
            version: "1".into(),
            installed_at: 1,
            render_requirements: None,
        })
        .await
        .unwrap();

    let opts = CoreOptionsRepo::new(db.clone());
    let defs = vec![
        CoreOptionDefinition {
            option_key: "internal_res".into(),
            display_name: "Resolução interna".into(),
            option_type: CoreOptionType::Combo {
                choices: vec!["1x".into(), "2x".into(), "4x".into()],
            },
            default_value: "1x".into(),
        },
        CoreOptionDefinition {
            option_key: "widescreen".into(),
            display_name: "Widescreen".into(),
            option_type: CoreOptionType::Bool,
            default_value: "false".into(),
        },
        CoreOptionDefinition {
            option_key: "audio_latency".into(),
            display_name: "Latência de áudio".into(),
            option_type: CoreOptionType::Range {
                min: 0.0,
                max: 100.0,
                step: 5.0,
            },
            default_value: "60".into(),
        },
    ];
    opts.replace_schema("pcsx2", &defs).await.unwrap();

    let loaded = opts.schema_for("pcsx2").await.unwrap();
    assert_eq!(loaded.len(), 3);
    let combo = loaded
        .iter()
        .find(|d| d.option_key == "internal_res")
        .unwrap();
    match &combo.option_type {
        CoreOptionType::Combo { choices } => assert_eq!(choices, &["1x", "2x", "4x"]),
        other => panic!("esperava Combo, veio {other:?}"),
    }
    let range = loaded
        .iter()
        .find(|d| d.option_key == "audio_latency")
        .unwrap();
    match range.option_type {
        CoreOptionType::Range { min, max, step } => {
            assert_eq!((min, max, step), (0.0, 100.0, 5.0));
        }
        ref other => panic!("esperava Range, veio {other:?}"),
    }

    // valor default até o usuário setar
    assert!(opts
        .get_value("pcsx2", "internal_res")
        .await
        .unwrap()
        .is_none());
    opts.set_value("pcsx2", "internal_res", "4x").await.unwrap();
    opts.set_value("pcsx2", "internal_res", "2x").await.unwrap(); // upsert
    assert_eq!(
        opts.get_value("pcsx2", "internal_res")
            .await
            .unwrap()
            .as_deref(),
        Some("2x")
    );

    // values_for devolve todos os valores escolhidos do core
    opts.set_value("pcsx2", "frame_skip", "1").await.unwrap();
    let all = opts.values_for("pcsx2").await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all.get("internal_res").map(String::as_str), Some("2x"));
    assert_eq!(all.get("frame_skip").map(String::as_str), Some("1"));
    assert!(opts.values_for("desconhecido").await.unwrap().is_empty());

    // replace_schema substitui, não acumula
    opts.replace_schema("pcsx2", &defs[..1]).await.unwrap();
    assert_eq!(opts.schema_for("pcsx2").await.unwrap().len(), 1);
}

#[tokio::test]
async fn core_options_schema_requires_registered_core() {
    let opts = CoreOptionsRepo::new(mem_db().await);
    let res = opts
        .replace_schema(
            "core-fantasma",
            &[CoreOptionDefinition {
                option_key: "k".into(),
                display_name: "K".into(),
                option_type: CoreOptionType::Bool,
                default_value: "false".into(),
            }],
        )
        .await;
    assert!(res.is_err(), "FK deveria barrar schema de core inexistente");
}
