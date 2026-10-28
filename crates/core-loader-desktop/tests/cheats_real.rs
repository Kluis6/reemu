//! Cheat por código num core real (doc 17, fase 1). Ignorado: precisa do
//! snes9x e de uma ROM de SNES na máquina.
//!
//! ```text
//! REEMU_CHEAT_CORE=<snes9x_libretro> REEMU_CHEAT_ROM=<rom .sfc/.smc> \
//!   cargo test -p core-loader-desktop --test cheats_real -- --ignored --nocapture
//! ```

use core_loader_desktop::DesktopCoreLoader;
use domain::cheats::CheatCode;
use domain::core_loader::CoreId;
use domain::frame_source::FrameSource;

/// Pro Action Replay do SNES: `7E0100AB` = byte `0xAB` no endereço `$7E:0100`,
/// que é o offset `0x0100` da WRAM (`RETRO_MEMORY_SYSTEM_RAM` do snes9x).
const CODE: &str = "7E0100AB";
const OFFSET: usize = 0x0100;

#[tokio::test]
#[ignore = "precisa do snes9x e de uma ROM de SNES (REEMU_CHEAT_CORE/ROM)"]
async fn code_cheat_writes_ram_and_turns_off() {
    let (Some(core), Some(rom)) = (
        std::env::var_os("REEMU_CHEAT_CORE"),
        std::env::var_os("REEMU_CHEAT_ROM"),
    ) else {
        eprintln!("defina REEMU_CHEAT_CORE e REEMU_CHEAT_ROM");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("reemu-cheat-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let ldr = DesktopCoreLoader::new(tmp.clone(), tmp.clone(), tmp);
    let mut c = ldr
        .load_core(
            &CoreId(core.to_string_lossy().into_owned()),
            &rom.to_string_lossy(),
        )
        .await
        .expect("load do snes9x");
    for _ in 0..60 {
        let _ = c.next_frame();
    }
    c.set_cheats(&[CheatCode {
        code: CODE.into(),
        enabled: true,
    }]);
    for _ in 0..5 {
        let _ = c.next_frame();
    }
    let ram = c.system_ram().expect("WRAM do snes9x");
    assert_eq!(
        ram[OFFSET], 0xAB,
        "cheat ligado: o core escreve 0xAB a cada quadro"
    );

    // desligado: o core para de escrever, e o jogo pode mudar o byte de novo
    c.set_cheats(&[CheatCode {
        code: CODE.into(),
        enabled: false,
    }]);
    let _ = c.next_frame();
    println!(
        "cheat aplicado: WRAM[0x{OFFSET:04x}] = 0x{:02x}",
        ram[OFFSET]
    );
}
