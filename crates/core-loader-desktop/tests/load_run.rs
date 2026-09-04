//! Loader ponta a ponta contra o core-fake (`fixtures/testcore.c`,
//! compilado pelo build.rs). Sem core real nem ROM real.
//!
//! A API libretro é um-core-por-processo (estado global), então os testes
//! são serializados por `TEST_LOCK`.

use core_loader_desktop::DesktopCoreLoader;
use domain::core_loader::{CoreId, CoreLoadError, CoreLoader, LoadedCore, RenderBackend};
use domain::frame_source::{FrameOrigin, FrameSource, SoftwarePixelFormat};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::{Mutex, MutexGuard};

/// Caminho do core-fake, vindo do build.rs.
const TESTCORE: &str = env!("REEMU_TESTCORE");

// libretro é um-core-por-processo → serializa os testes. `tokio::sync::Mutex`
// (não o std) porque a guarda cruza `.await`.
static TEST_LOCK: Mutex<()> = Mutex::const_new(());
static NONCE: AtomicU32 = AtomicU32::new(0);

async fn guard() -> MutexGuard<'static, ()> {
    TEST_LOCK.lock().await
}

fn loader() -> DesktopCoreLoader {
    let tmp = std::env::temp_dir();
    DesktopCoreLoader::new(tmp.clone(), tmp.clone(), tmp)
}

fn write_rom(bytes: &[u8]) -> std::path::PathBuf {
    let n = NONCE.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("reemu-test-{}-{n}.bin", std::process::id()));
    std::fs::write(&p, bytes).unwrap();
    p
}

fn core_id() -> CoreId {
    CoreId(TESTCORE.into())
}

#[tokio::test]
async fn software_core_loads_runs_and_produces_frames() {
    let _lock = guard().await;
    let rom = write_rom(b"dummy content");
    let ldr = loader();

    let mut core = ldr
        .load_core(&core_id(), rom.to_str().unwrap())
        .await
        .expect("load do core-fake");

    let av = core.system_av_info();
    assert_eq!(av.geometry.base_width, 64);
    assert_eq!(av.geometry.base_height, 48);
    assert_eq!(av.timing.fps, 60.0);
    assert_eq!(av.timing.sample_rate, 32000.0);
    assert_eq!(
        core.render_requirements().render_backend,
        RenderBackend::Software
    );

    let mut colors = vec![];
    for _ in 0..3 {
        let frame = core.next_frame().expect("frame do core software");
        assert_eq!(frame.metadata.native_width, 64);
        assert_eq!(frame.metadata.native_height, 48);
        match frame.origin {
            FrameOrigin::SoftwareRawBuffer {
                data,
                pitch,
                format,
            } => {
                assert_eq!(format, SoftwarePixelFormat::Rgb565);
                assert_eq!(pitch, 64 * 2);
                assert_eq!(data.len(), 64 * 48 * 2);
                colors.push((data[0], data[1]));
            }
            FrameOrigin::HardwareTexture(_) => panic!("core software não deveria dar textura HW"),
        }
    }
    assert!(
        colors[0] != colors[1] && colors[1] != colors[2],
        "cada frame do core-fake muda de cor: {colors:?}"
    );

    // áudio acumulado é drenável (o core-fake emite silêncio)
    assert!(!core.drain_audio().is_empty());
    assert!(core.drain_audio().is_empty(), "drain esvazia");

    assert_eq!(
        ldr.known_render_requirements(&core_id())
            .unwrap()
            .render_backend,
        RenderBackend::Software
    );

    drop(core);
    let _ = std::fs::remove_file(rom);
}

#[tokio::test]
async fn save_state_round_trip_and_pending_hook() {
    let _lock = guard().await;
    let rom = write_rom(b"x");
    let ldr = loader();
    let mut core = ldr
        .load_core(&core_id(), rom.to_str().unwrap())
        .await
        .unwrap();

    core.next_frame();
    core.next_frame();
    let snap = core.serialize_state().expect("serialize");

    core.next_frame();
    core.next_frame();
    core.next_frame();
    assert!(core.restore_state(&snap), "unserialize");

    // ponto de extensão pra etapa 08
    assert!(core.poll_save_state().is_none());
    core.request_save_state();
    assert!(core.poll_save_state().is_some());
    assert!(core.poll_save_state().is_none());

    drop(core);
    let _ = std::fs::remove_file(rom);
}

#[tokio::test]
async fn save_ram_round_trip() {
    let _lock = guard().await;
    let rom = write_rom(b"x");
    let ldr = loader();
    let mut core = ldr
        .load_core(&core_id(), rom.to_str().unwrap())
        .await
        .unwrap();

    // O testcore expõe 64 bytes de SRAM pré-carregada (0xA5, 0x5A, ...).
    let initial = core.save_ram().expect("testcore tem save RAM");
    assert_eq!(initial.len(), 64);
    assert_eq!(&initial[..2], &[0xA5, 0x5A]);

    let mut new_ram = vec![0u8; 64];
    new_ram[0] = 0x42;
    new_ram[63] = 0x99;
    assert!(core.restore_save_ram(&new_ram));
    assert_eq!(core.save_ram().unwrap(), new_ram);

    // tamanho errado é rejeitado
    assert!(!core.restore_save_ram(&[0u8; 8]));

    drop(core);
    let _ = std::fs::remove_file(rom);
}

#[tokio::test]
async fn core_options_schema_and_runtime_set() {
    use core_loader_desktop::{core_option_values, core_options, set_core_option};

    let _lock = guard().await;
    let rom = write_rom(b"x");
    let ldr = loader();
    let mut core = ldr
        .load_core(&core_id(), rom.to_str().unwrap())
        .await
        .unwrap();

    // O testcore declara `testcore_speed` e `testcore_mark` via SET_VARIABLES.
    let schema = core_options();
    assert_eq!(schema.len(), 2);
    let mark = schema
        .iter()
        .find(|o| o.option_key == "testcore_mark")
        .unwrap();
    assert_eq!(mark.default_value, "A");
    assert_eq!(core_option_values().get("testcore_mark").unwrap(), "A");

    // Troca em runtime; o core espelha o 1º char do valor em SRAM[2].
    assert!(set_core_option("testcore_mark", "C"));
    assert!(!set_core_option("testcore_mark", "Z")); // valor inválido
    assert!(!set_core_option("inexistente", "A"));

    core.next_frame();
    assert_eq!(core.save_ram().unwrap()[2], b'C');

    drop(core);
    let _ = std::fs::remove_file(rom);
}

#[tokio::test]
async fn hw_render_core_detects_requirements_and_negotiates() {
    let _lock = guard().await;
    let rom = write_rom(b"HW payload"); // "HW" -> core-fake declara SET_HW_RENDER
    let ldr = loader();

    let res = ldr.load_core(&core_id(), rom.to_str().unwrap()).await;
    // Com EGL disponível (CI/desktop) o contexto GL é criado e o core carrega;
    // sem libEGL, recusa com HwRenderUnsupported. Nenhum outro erro é aceitável.
    match &res {
        Ok(_) => {}
        Err(CoreLoadError::HwRenderUnsupported(_)) => {}
        Err(e) => panic!("erro inesperado num core GL: {e:?}"),
    }
    drop(res); // libera o guard (Ok segura o DesktopCore; Err já soltou)

    let reqs = ldr
        .known_render_requirements(&core_id())
        .expect("requisitos detectados");
    assert_eq!(reqs.render_backend, RenderBackend::OpenGl);
    assert_eq!(reqs.gl_version_min.as_deref(), Some("3.3"));
    assert_eq!(reqs.gl_profile.as_deref(), Some("core"));
    assert!(reqs.needs_depth_stencil);

    // slot global liberado — dá pra carregar de novo
    let ok = write_rom(b"plain");
    let core = ldr
        .load_core(&core_id(), ok.to_str().unwrap())
        .await
        .unwrap();
    drop(core);

    let _ = std::fs::remove_file(rom);
    let _ = std::fs::remove_file(ok);
}

#[tokio::test]
async fn missing_core_is_not_found() {
    let _lock = guard().await;
    let ldr = loader();
    let res = ldr
        .load_core(&CoreId("/nao/existe/foo_libretro.so".into()), "/tmp/x")
        .await;
    assert!(matches!(res, Err(CoreLoadError::NotFound(_))));
    drop(res);
}

#[tokio::test]
async fn second_load_while_one_is_active_fails() {
    let _lock = guard().await;
    let rom = write_rom(b"a");
    let ldr = loader();
    let core = ldr
        .load_core(&core_id(), rom.to_str().unwrap())
        .await
        .unwrap();

    let res = ldr.load_core(&core_id(), rom.to_str().unwrap()).await;
    assert!(matches!(res, Err(CoreLoadError::LoadFailed(_))));
    drop(res);

    drop(core);
    let _ = std::fs::remove_file(rom);
}

/// `.zip` sem nenhuma entrada com extensão de ROM de cartucho reconhecida —
/// o caso de um set de arcade (MAME/FBNeo, só chip dumps avulsos). Antes
/// desse fix, `open_core` devolvia `LoadFailed` (`extract_rom` não achava o
/// que extrair); agora cai pro caminho do `.zip` original em vez de travar.
#[tokio::test]
async fn zip_without_a_recognized_rom_entry_falls_back_to_the_zip_path() {
    use std::io::Write as _;

    let _lock = guard().await;
    let n = NONCE.fetch_add(1, Ordering::Relaxed);
    let zip_path =
        std::env::temp_dir().join(format!("reemu-arcade-test-{}-{n}.zip", std::process::id()));
    {
        let f = std::fs::File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zw.start_file("sfiii3n.06", opts).unwrap();
        zw.write_all(b"chip-dump-not-a-cartridge-rom").unwrap();
        zw.finish().unwrap();
    }

    let ldr = loader();
    let core = ldr
        .load_core(&core_id(), zip_path.to_str().unwrap())
        .await
        .expect("não devia travar — devia cair pro caminho original do .zip");
    drop(core);
    let _ = std::fs::remove_file(zip_path);
}
