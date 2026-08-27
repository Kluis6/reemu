//! Testes de integração dos repositórios não-cascata: core options,
//! audio config (linha única), installed cores (upsert de render), roms.

mod common;

use common::mem_db;
use db::{AudioConfigRepo, CoreOptionsRepo, InstalledCoresRepo, RomsRepo};
use domain::audio::{AudioConfig, AudioConfigRepository};
use domain::core_loader::{
    CoreRenderRequirements, InstalledCore, InstalledCoreRepository, RenderBackend,
};
use domain::core_options::{CoreOptionDefinition, CoreOptionType, CoreOptionsStore};
use domain::library::{Rom, RomRepository};

fn rom(id: &str, crc: &str, system: &str) -> Rom {
    Rom {
        id: id.into(),
        file_path: format!("/roms/{id}.zip"),
        crc32: crc.into(),
        md5: format!("md5-{id}"),
        system_id: system.into(),
        added_at: 1_700_000_000,
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
