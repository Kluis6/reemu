//! Inferência de sistema a partir da extensão do arquivo. Só um palpite pro
//! agrupamento inicial da biblioteca — o match real de metadata é por hash.

/// `system_id` canônico pra uma extensão (sem o ponto, minúsculo). `None` se
/// não reconhecida.
pub fn system_for_extension(ext: &str) -> Option<&'static str> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "nes" | "fds" | "unif" | "unf" => "nes",
        "sfc" | "smc" | "swc" | "fig" => "snes",
        "gb" => "gb",
        "gbc" => "gbc",
        "gba" | "srl" => "gba",
        "n64" | "z64" | "v64" | "ndd" => "n64",
        "md" | "smd" | "gen" | "sgd" => "megadrive",
        "sms" => "mastersystem",
        "gg" => "gamegear",
        "pce" | "sgx" => "pcengine",
        "a26" => "atari2600",
        "lnx" => "lynx",
        "ws" | "wsc" => "wonderswan",
        "ngp" | "ngc" => "ngp",
        "32x" => "sega32x",
        "cue" | "chd" | "iso" | "pbp" => "disc", // PS1/Saturn/etc — precisa do usuário
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_extensions() {
        assert_eq!(system_for_extension("nes"), Some("nes"));
        assert_eq!(system_for_extension("SFC"), Some("snes"));
        assert_eq!(system_for_extension("gba"), Some("gba"));
        assert_eq!(system_for_extension("md"), Some("megadrive"));
        assert_eq!(system_for_extension("xyz"), None);
    }
}
