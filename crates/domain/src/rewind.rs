//! Regras de domínio do Rewind em tempo real (docs/ai-context/17-cheats-rewind.md,
//! fase 3): orçamento padrão de RAM, verificação de quirks de serialização do
//! `libretro.h` e intervalo de captura (`stride`) por sistema/core.

use crate::core_loader::serialization_quirk as q;

/// Orçamento padrão do buffer circular de rewind em RAM (128 MiB).
pub const DEFAULT_REWIND_BUDGET_MB: u32 = 128;
pub const MIN_REWIND_BUDGET_MB: u32 = 32;
pub const MAX_REWIND_BUDGET_MB: u32 = 1024;

/// Motivo pelo qual os quirks declarados pelo core impedem o rewind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewindQuirkRejection {
    /// `RETRO_SERIALIZATION_QUIRK_INCOMPLETE`: não confiável para recursos sensíveis a quadro.
    Incomplete,
    /// `RETRO_SERIALIZATION_QUIRK_SINGLE_SESSION`: recusado na política do rewind.
    SingleSession,
}

/// Verifica se os quirks de serialização do core permitem o rewind.
///
/// - `INCOMPLETE` e `SINGLE_SESSION` desativam o rewind limpa e imediatamente.
/// - `MUST_INITIALIZE` é aceito (as primeiras chamadas a `retro_serialize` podem
///   falhar e o buffer simplesmente aguarda até o core terminar de iniciar).
/// - `CORE_VARIABLE_SIZE` / `FRONT_VARIABLE_SIZE` são aceitos e tratados pelo
///   delta XOR com tamanho variável.
pub fn check_rewind_quirks(quirks: u64) -> Result<(), RewindQuirkRejection> {
    if quirks & q::INCOMPLETE != 0 {
        return Err(RewindQuirkRejection::Incomplete);
    }
    if quirks & q::SINGLE_SESSION != 0 {
        return Err(RewindQuirkRejection::SingleSession);
    }
    Ok(())
}

/// Devolve o intervalo de captura em quadros (`Some(stride)`) para `(system_id, core_id)`,
/// ou `None` quando o sistema/core tem estados pesados demais de GPU/RAM (Dreamcast,
/// PSP, PS2, GameCube, LaserDisc) e fica com rewind desativado.
///
/// - `1` quadro: consoles 8/16-bit e portáteis leves (NES, FDS, SNES, Mega Drive,
///   Master System, Game Gear, GB, GBC, GBA, PC Engine, Atari 2600, Neo Geo Pocket,
///   WonderSwan e similares).
/// - `3` quadros: sistemas intermediários (PS1, N64, NDS, Saturn, Sega CD, 32X,
///   Virtual Boy e similares).
pub fn rewind_stride_for(system_id: &str, core_id: &str) -> Option<u32> {
    let sys = system_id.trim().to_ascii_lowercase();
    let core = core_id
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(core_id)
        .to_ascii_lowercase();

    // Cores pesados ou com estado de dezenas de MB / contexto de vídeo pesado.
    let heavy_core = [
        "flycast",
        "ppsspp",
        "pcsx2",
        "lrps2",
        "play_",
        "dolphin",
        "hypseus",
        "dirksimple",
    ]
    .iter()
    .any(|pat| core.contains(pat));
    if heavy_core {
        return None;
    }

    match sys.as_str() {
        "dreamcast" | "naomi" | "atomiswave" | "psp" | "ps2" | "gamecube" | "wii" | "laserdisc" => {
            None
        }
        "nes"
        | "fds"
        | "snes"
        | "megadrive"
        | "mastersystem"
        | "sms"
        | "gamegear"
        | "sg1000"
        | "gb"
        | "gbc"
        | "gba"
        | "pcengine"
        | "pce"
        | "supergrafx"
        | "atari2600"
        | "atari5200"
        | "atari7800"
        | "atari8bit"
        | "lynx"
        | "ngp"
        | "wonderswan"
        | "coleco"
        | "intellivision"
        | "vectrex"
        | "odyssey2"
        | "pokemini"
        | "supervision"
        | "megaduck"
        | "arduboy"
        | "uzebox"
        | "chip8"
        | "vaporspec"
        | "gametank"
        | "handheld_electronic"
        | "zx81"
        | "zxspectrum"
        | "c64"
        | "amstradcpc"
        | "msx" => Some(1),
        "psx" | "n64" | "nds" | "saturn" | "segacd" | "sega32x" | "32x" | "vb" | "virtualboy"
        | "pcenginecd" | "pcfx" | "neogeocd" | "3do" | "cdi" | "jaguar" | "arcade" | "amiga"
        | "atarist" | "x68000" | "dos" | "scummvm" => Some(3),
        // Fallback quando `system_id` é vazio ou genérico (`disc`), inferido pelo core.
        _ => {
            let medium_core = [
                "psx",
                "pcsx",
                "swanstation",
                "mednafen_psx",
                "beetle_psx",
                "mupen64",
                "parallel_n64",
                "melonds",
                "desmume",
                "saturn",
                "yabause",
                "kronos",
                "picodrive",
                "mednafen_vb",
                "beetle_vb",
                "opera",
                "fbneo",
                "mame",
            ]
            .iter()
            .any(|pat| core.contains(pat));
            if medium_core || sys == "disc" {
                Some(3)
            } else {
                Some(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quirk_rules_reject_incomplete_and_single_session_only() {
        assert_eq!(check_rewind_quirks(0), Ok(()));
        assert_eq!(
            check_rewind_quirks(q::MUST_INITIALIZE | q::CORE_VARIABLE_SIZE | q::PLATFORM_DEPENDENT),
            Ok(())
        );
        assert_eq!(
            check_rewind_quirks(q::INCOMPLETE),
            Err(RewindQuirkRejection::Incomplete)
        );
        assert_eq!(
            check_rewind_quirks(q::SINGLE_SESSION),
            Err(RewindQuirkRejection::SingleSession)
        );
    }

    #[test]
    fn stride_matches_system_tiers_and_disables_heavy_systems() {
        for light in [
            "nes",
            "fds",
            "snes",
            "megadrive",
            "mastersystem",
            "SMS",
            "gamegear",
            "gb",
            "gbc",
            "gba",
            "pce",
            "pcengine",
            "atari2600",
            "ngp",
            "wonderswan",
        ] {
            assert_eq!(
                rewind_stride_for(light, "some_libretro"),
                Some(1),
                "{light}"
            );
        }

        for medium in [
            "psx",
            "n64",
            "nds",
            "saturn",
            "segacd",
            "32x",
            "sega32x",
            "vb",
            "virtualboy",
        ] {
            assert_eq!(
                rewind_stride_for(medium, "some_libretro"),
                Some(3),
                "{medium}"
            );
        }

        for heavy in ["dreamcast", "psp", "ps2", "gamecube"] {
            assert_eq!(rewind_stride_for(heavy, "some_libretro"), None, "{heavy}");
        }

        assert_eq!(rewind_stride_for("disc", "flycast_libretro"), None);
        assert_eq!(rewind_stride_for("disc", "ppsspp_libretro"), None);
        assert_eq!(rewind_stride_for("disc", "pcsx_rearmed_libretro"), Some(3));
        assert_eq!(rewind_stride_for("", "reemu-testcore"), Some(1));
    }
}
