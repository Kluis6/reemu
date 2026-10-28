//! Medição do save state de um core real (doc 17, fase 0): tamanho do
//! `retro_serialize`, tempo de salvar e de restaurar, e os quirks. Base pra
//! decidir o rewind por sistema. Ignorado: precisa de core e ROM na máquina.
//!
//! Um core por processo (libretro), então uma execução por core:
//!
//! ```text
//! REEMU_MEASURE_CORE=<dll/so do core> REEMU_MEASURE_ROM=<rom> \
//!   [REEMU_MEASURE_SYSTEM=<pasta system/BIOS>] [REEMU_MEASURE_FRAMES=600] \
//!   cargo test -p core-loader-desktop --test measure_state -- --ignored --nocapture
//! ```
//!
//! Tempos em microssegundos inteiros: um core pode mudar o modo do FPU do
//! processo (o pcsx_rearmed deu tempos absurdos com `f64`).

use core_loader_desktop::DesktopCoreLoader;
use domain::core_loader::CoreId;
use domain::frame_source::FrameSource;
use std::path::PathBuf;
use std::time::Instant;

/// Quadros rodados antes de medir (padrão ~10 s de jogo, pra sair do boot).
const DEFAULT_WARMUP_FRAMES: u32 = 600;
const SAMPLES: usize = 10;

/// `µs` como `ms` com duas casas, sem ponto flutuante.
fn ms(us: u128) -> String {
    format!("{}.{:02}", us / 1000, (us % 1000) / 10)
}

#[tokio::test]
#[ignore = "precisa de um core e de uma ROM reais (REEMU_MEASURE_CORE/ROM)"]
async fn measure_save_state() {
    let (Some(core), Some(rom)) = (
        std::env::var_os("REEMU_MEASURE_CORE"),
        std::env::var_os("REEMU_MEASURE_ROM"),
    ) else {
        eprintln!("defina REEMU_MEASURE_CORE e REEMU_MEASURE_ROM");
        return;
    };
    let frames: u32 = std::env::var("REEMU_MEASURE_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WARMUP_FRAMES);
    let tmp = std::env::temp_dir().join(format!("reemu-measure-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let system = std::env::var_os("REEMU_MEASURE_SYSTEM")
        .map(PathBuf::from)
        .unwrap_or_else(|| tmp.clone());
    let ldr = DesktopCoreLoader::new(tmp.clone(), system, tmp.clone());
    let core_id = CoreId(core.to_string_lossy().into_owned());
    let name = std::path::Path::new(&core)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut c = ldr
        .load_core(&core_id, &rom.to_string_lossy())
        .await
        .expect("load do core");

    let t = Instant::now();
    for _ in 0..frames {
        let _ = c.next_frame();
    }
    let frame_us = t.elapsed().as_micros() / u128::from(frames.max(1));

    let mut sizes = Vec::new();
    let mut save_us = Vec::new();
    let mut load_us = Vec::new();
    for _ in 0..SAMPLES {
        let t = Instant::now();
        let state = c.serialize_state();
        save_us.push(t.elapsed().as_micros());
        let Some(state) = state else {
            println!(
                "MEDIDA core={name} quirks=0x{:x}: sem save state depois de {frames} quadros \
                 (serialize falhou ou tamanho 0)",
                core_loader_desktop::serialization_quirks()
            );
            return;
        };
        sizes.push(state.len());
        let t = Instant::now();
        let ok = c.restore_state(&state);
        load_us.push(t.elapsed().as_micros());
        assert!(ok, "restore do estado recém-salvo falhou");
        let _ = c.next_frame();
    }
    let avg = |v: &[u128]| v.iter().sum::<u128>() / v.len() as u128;
    let max = |v: &[u128]| v.iter().copied().max().unwrap_or(0);
    println!(
        "MEDIDA core={name} quirks=0x{:x} tamanho={} KB (min {} / max {} bytes) \
         salvar={} ms (máx {}) restaurar={} ms (máx {}) quadro={} ms",
        core_loader_desktop::serialization_quirks(),
        sizes.iter().max().unwrap() / 1024,
        sizes.iter().min().unwrap(),
        sizes.iter().max().unwrap(),
        ms(avg(&save_us)),
        ms(max(&save_us)),
        ms(avg(&load_us)),
        ms(max(&load_us)),
        ms(frame_us),
    );
}
