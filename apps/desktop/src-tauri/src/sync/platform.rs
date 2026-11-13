//! O mapa dos ids de sistema do ReEmu para a plataforma de save da nuvem.
//!
//! Os valores são os de `SavePlatform` do GameKave (`gamekave-saves`). Um
//! sistema sem valor exato vai para `other`: o servidor guarda e conta na
//! cota igual, nunca recusa.

// Ainda sem chamador: o motor de sync (próxima task) passa a usar.
#[allow(dead_code)]
pub fn platform_for_system(system_id: &str) -> &'static str {
    match system_id {
        "psx" => "ps1",
        "ps2" => "ps2",
        "snes" => "snes",
        "nes" => "nes",
        "gb" => "gb",
        "gbc" => "gbc",
        "gba" => "gba",
        "n64" => "n64",
        "megadrive" => "megadrive",
        "mastersystem" => "mastersystem",
        "gamegear" => "gamegear",
        "saturn" => "saturn",
        "dreamcast" => "dreamcast",
        "atari2600" => "atari2600",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_systems_and_defaults_to_other() {
        for (system, platform) in [
            ("psx", "ps1"),
            ("ps2", "ps2"),
            ("snes", "snes"),
            ("nes", "nes"),
            ("gb", "gb"),
            ("gbc", "gbc"),
            ("gba", "gba"),
            ("n64", "n64"),
            ("megadrive", "megadrive"),
            ("mastersystem", "mastersystem"),
            ("gamegear", "gamegear"),
            ("saturn", "saturn"),
            ("dreamcast", "dreamcast"),
            ("atari2600", "atari2600"),
        ] {
            assert_eq!(platform_for_system(system), platform, "{system}");
        }
        for other in ["psp", "nds", "dos", "arcade", "neogeocd", "pcengine", ""] {
            assert_eq!(platform_for_system(other), "other", "{other}");
        }
    }
}
