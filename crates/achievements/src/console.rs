//! Mapeamento de sistemas do ReEmu para `RC_CONSOLE_*` (`include/rc_consoles.h`)
//! e cálculo de hash oficial do RetroAchievements (`include/rc_hash.h`).

use crate::ffi;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::Path;

pub const RC_CONSOLE_MEGA_DRIVE: u32 = 1;
pub const RC_CONSOLE_NINTENDO_64: u32 = 2;
pub const RC_CONSOLE_SUPER_NINTENDO: u32 = 3;
pub const RC_CONSOLE_GAMEBOY: u32 = 4;
pub const RC_CONSOLE_GAMEBOY_ADVANCE: u32 = 5;
pub const RC_CONSOLE_GAMEBOY_COLOR: u32 = 6;
pub const RC_CONSOLE_NINTENDO: u32 = 7;
pub const RC_CONSOLE_PC_ENGINE: u32 = 8;
pub const RC_CONSOLE_SEGA_CD: u32 = 9;
pub const RC_CONSOLE_SEGA_32X: u32 = 10;
pub const RC_CONSOLE_MASTER_SYSTEM: u32 = 11;
pub const RC_CONSOLE_PLAYSTATION: u32 = 12;
pub const RC_CONSOLE_ATARI_LYNX: u32 = 13;
pub const RC_CONSOLE_NEOGEO_POCKET: u32 = 14;
pub const RC_CONSOLE_GAME_GEAR: u32 = 15;
pub const RC_CONSOLE_GAMECUBE: u32 = 16;
pub const RC_CONSOLE_ATARI_JAGUAR: u32 = 17;
pub const RC_CONSOLE_NINTENDO_DS: u32 = 18;
pub const RC_CONSOLE_WII: u32 = 19;
pub const RC_CONSOLE_PLAYSTATION_2: u32 = 21;
pub const RC_CONSOLE_MAGNAVOX_ODYSSEY2: u32 = 23;
pub const RC_CONSOLE_POKEMON_MINI: u32 = 24;
pub const RC_CONSOLE_ATARI_2600: u32 = 25;
pub const RC_CONSOLE_MS_DOS: u32 = 26;
pub const RC_CONSOLE_ARCADE: u32 = 27;
pub const RC_CONSOLE_VIRTUAL_BOY: u32 = 28;
pub const RC_CONSOLE_MSX: u32 = 29;
pub const RC_CONSOLE_COMMODORE_64: u32 = 30;
pub const RC_CONSOLE_ZX81: u32 = 31;
pub const RC_CONSOLE_SG1000: u32 = 33;
pub const RC_CONSOLE_AMIGA: u32 = 35;
pub const RC_CONSOLE_ATARI_ST: u32 = 36;
pub const RC_CONSOLE_AMSTRAD_PC: u32 = 37;
pub const RC_CONSOLE_SATURN: u32 = 39;
pub const RC_CONSOLE_DREAMCAST: u32 = 40;
pub const RC_CONSOLE_PSP: u32 = 41;
pub const RC_CONSOLE_CDI: u32 = 42;
pub const RC_CONSOLE_3DO: u32 = 43;
pub const RC_CONSOLE_COLECOVISION: u32 = 44;
pub const RC_CONSOLE_INTELLIVISION: u32 = 45;
pub const RC_CONSOLE_VECTREX: u32 = 46;
pub const RC_CONSOLE_PCFX: u32 = 49;
pub const RC_CONSOLE_ATARI_5200: u32 = 50;
pub const RC_CONSOLE_ATARI_7800: u32 = 51;
pub const RC_CONSOLE_X68K: u32 = 52;
pub const RC_CONSOLE_WONDERSWAN: u32 = 53;
pub const RC_CONSOLE_NEO_GEO_CD: u32 = 56;
pub const RC_CONSOLE_ZX_SPECTRUM: u32 = 59;
pub const RC_CONSOLE_GAME_AND_WATCH: u32 = 60;
pub const RC_CONSOLE_NINTENDO_3DS: u32 = 62;
pub const RC_CONSOLE_SUPERVISION: u32 = 63;
pub const RC_CONSOLE_MEGADUCK: u32 = 69;
pub const RC_CONSOLE_ARDUBOY: u32 = 71;
pub const RC_CONSOLE_WASM4: u32 = 72;
pub const RC_CONSOLE_PC_ENGINE_CD: u32 = 76;
pub const RC_CONSOLE_NINTENDO_DSI: u32 = 78;
pub const RC_CONSOLE_UZEBOX: u32 = 80;
pub const RC_CONSOLE_FAMICOM_DISK_SYSTEM: u32 = 81;

/// Converte o `system_id` do ReEmu para a constante oficial `RC_CONSOLE_*`.
/// Devolve `None` quando o sistema não possui conjunto no RetroAchievements.
pub fn console_id_for_system(system_id: &str) -> Option<u32> {
    match system_id {
        "megadrive" | "genesis" => Some(RC_CONSOLE_MEGA_DRIVE),
        "n64" => Some(RC_CONSOLE_NINTENDO_64),
        "snes" => Some(RC_CONSOLE_SUPER_NINTENDO),
        "gb" => Some(RC_CONSOLE_GAMEBOY),
        "gba" => Some(RC_CONSOLE_GAMEBOY_ADVANCE),
        "gbc" => Some(RC_CONSOLE_GAMEBOY_COLOR),
        "nes" => Some(RC_CONSOLE_NINTENDO),
        "fds" => Some(RC_CONSOLE_FAMICOM_DISK_SYSTEM),
        "pcengine" | "supergrafx" => Some(RC_CONSOLE_PC_ENGINE),
        "pcenginecd" => Some(RC_CONSOLE_PC_ENGINE_CD),
        "segacd" => Some(RC_CONSOLE_SEGA_CD),
        "sega32x" => Some(RC_CONSOLE_SEGA_32X),
        "mastersystem" => Some(RC_CONSOLE_MASTER_SYSTEM),
        "psx" | "ps1" => Some(RC_CONSOLE_PLAYSTATION),
        "lynx" => Some(RC_CONSOLE_ATARI_LYNX),
        "ngp" | "ngpc" => Some(RC_CONSOLE_NEOGEO_POCKET),
        "gamegear" => Some(RC_CONSOLE_GAME_GEAR),
        "gamecube" | "gc" => Some(RC_CONSOLE_GAMECUBE),
        "jaguar" => Some(RC_CONSOLE_ATARI_JAGUAR),
        "nds" => Some(RC_CONSOLE_NINTENDO_DS),
        "dsi" => Some(RC_CONSOLE_NINTENDO_DSI),
        "wii" => Some(RC_CONSOLE_WII),
        "ps2" => Some(RC_CONSOLE_PLAYSTATION_2),
        "odyssey2" => Some(RC_CONSOLE_MAGNAVOX_ODYSSEY2),
        "pokemini" => Some(RC_CONSOLE_POKEMON_MINI),
        "atari2600" => Some(RC_CONSOLE_ATARI_2600),
        "atari5200" | "atari8bit" => Some(RC_CONSOLE_ATARI_5200),
        "atari7800" => Some(RC_CONSOLE_ATARI_7800),
        "dos" => Some(RC_CONSOLE_MS_DOS),
        "arcade" | "naomi" | "atomiswave" => Some(RC_CONSOLE_ARCADE),
        "vb" => Some(RC_CONSOLE_VIRTUAL_BOY),
        "msx" => Some(RC_CONSOLE_MSX),
        "c64" => Some(RC_CONSOLE_COMMODORE_64),
        "zx81" => Some(RC_CONSOLE_ZX81),
        "sg1000" => Some(RC_CONSOLE_SG1000),
        "amiga" => Some(RC_CONSOLE_AMIGA),
        "atarist" => Some(RC_CONSOLE_ATARI_ST),
        "amstradcpc" => Some(RC_CONSOLE_AMSTRAD_PC),
        "saturn" => Some(RC_CONSOLE_SATURN),
        "dreamcast" => Some(RC_CONSOLE_DREAMCAST),
        "psp" => Some(RC_CONSOLE_PSP),
        "cdi" => Some(RC_CONSOLE_CDI),
        "3do" => Some(RC_CONSOLE_3DO),
        "coleco" => Some(RC_CONSOLE_COLECOVISION),
        "intellivision" => Some(RC_CONSOLE_INTELLIVISION),
        "vectrex" => Some(RC_CONSOLE_VECTREX),
        "pcfx" => Some(RC_CONSOLE_PCFX),
        "x68000" => Some(RC_CONSOLE_X68K),
        "wonderswan" | "wonderswancolor" => Some(RC_CONSOLE_WONDERSWAN),
        "neogeocd" => Some(RC_CONSOLE_NEO_GEO_CD),
        "zxspectrum" => Some(RC_CONSOLE_ZX_SPECTRUM),
        "handheld_electronic" => Some(RC_CONSOLE_GAME_AND_WATCH),
        "3ds" => Some(RC_CONSOLE_NINTENDO_3DS),
        "supervision" => Some(RC_CONSOLE_SUPERVISION),
        "megaduck" => Some(RC_CONSOLE_MEGADUCK),
        "arduboy" => Some(RC_CONSOLE_ARDUBOY),
        "wasm4" => Some(RC_CONSOLE_WASM4),
        "uzebox" => Some(RC_CONSOLE_UZEBOX),
        _ => None,
    }
}

/// Nome oficial do console no `rcheevos`.
pub fn console_name(console_id: u32) -> String {
    let ptr = unsafe { ffi::rc_console_name(console_id) };
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// Calcula o hash RetroAchievements de um buffer em memória para `console_id`.
pub fn hash_buffer(console_id: u32, buffer: &[u8]) -> Option<String> {
    let mut out = [0 as c_char; 33];
    let ok = unsafe {
        ffi::rc_hash_generate_from_buffer(
            out.as_mut_ptr(),
            console_id,
            buffer.as_ptr(),
            buffer.len(),
        )
    };
    if ok == 0 {
        return None;
    }
    let s = unsafe { CStr::from_ptr(out.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    (!s.is_empty()).then_some(s)
}

/// Informa se `console_id` representa uma plataforma baseada em disco óptico
/// no `rcheevos` (`rc_hash_disc.c`).
pub fn is_disc_console(console_id: u32) -> bool {
    matches!(
        console_id,
        RC_CONSOLE_SEGA_CD
            | RC_CONSOLE_PLAYSTATION
            | RC_CONSOLE_GAMECUBE
            | RC_CONSOLE_WII
            | RC_CONSOLE_PLAYSTATION_2
            | RC_CONSOLE_SATURN
            | RC_CONSOLE_DREAMCAST
            | RC_CONSOLE_PSP
            | RC_CONSOLE_CDI
            | RC_CONSOLE_3DO
            | RC_CONSOLE_PCFX
            | RC_CONSOLE_NEO_GEO_CD
            | RC_CONSOLE_PC_ENGINE_CD
    )
}

/// Converte o `core_id` libretro em um `RC_CONSOLE_*` de referência quando o
/// `system_id` salvo na biblioteca é genérico (`"disc"`).
pub fn console_id_for_core(core_id: &str) -> Option<u32> {
    let base = core_id.rsplit(['/', '\\']).next().unwrap_or(core_id);
    let stem = base
        .strip_suffix(".dll")
        .or_else(|| base.strip_suffix(".so"))
        .or_else(|| base.strip_suffix(".dylib"))
        .unwrap_or(base);
    match stem {
        "pcsx2_libretro" | "lrps2_libretro" | "play_libretro" => Some(RC_CONSOLE_PLAYSTATION_2),
        "dolphin_libretro" => Some(RC_CONSOLE_GAMECUBE),
        "pcsx_rearmed_libretro"
        | "swanstation_libretro"
        | "duckstation_libretro"
        | "mednafen_psx_libretro"
        | "mednafen_psx_hw_libretro" => Some(RC_CONSOLE_PLAYSTATION),
        "ppsspp_libretro" => Some(RC_CONSOLE_PSP),
        "flycast_libretro" => Some(RC_CONSOLE_DREAMCAST),
        "mednafen_saturn_libretro" | "yabasanshiro_libretro" | "kronos_libretro" => {
            Some(RC_CONSOLE_SATURN)
        }
        "genesis_plus_gx_libretro" | "genesis_plus_gx_wide_libretro" | "picodrive_libretro" => {
            Some(RC_CONSOLE_SEGA_CD)
        }
        "opera_libretro" => Some(RC_CONSOLE_3DO),
        "mednafen_pce_libretro" | "mednafen_pce_fast_libretro" => Some(RC_CONSOLE_PC_ENGINE_CD),
        "mednafen_pcfx_libretro" => Some(RC_CONSOLE_PCFX),
        "neocd_libretro" => Some(RC_CONSOLE_NEO_GEO_CD),
        _ => None,
    }
}

/// Calcula o hash RetroAchievements de um arquivo em disco para `console_id`,
/// incluindo suporte a `.chd`, `.cso`, `.gcz` e arquivos `.zip`/`.7z`.
pub fn hash_file(console_id: u32, path: &Path) -> Option<String> {
    crate::disc_io::hash_path_for_console(console_id, path)
}
