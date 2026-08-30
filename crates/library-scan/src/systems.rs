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

/// Pasta do sistema no servidor de thumbnails da libretro
/// (`thumbnails.libretro.com`). `None` = sem cobertura conhecida.
fn libretro_thumbnail_system(system_id: &str) -> Option<&'static str> {
    Some(match system_id {
        "nes" => "Nintendo - Nintendo Entertainment System",
        "snes" => "Nintendo - Super Nintendo Entertainment System",
        "gb" => "Nintendo - Game Boy",
        "gbc" => "Nintendo - Game Boy Color",
        "gba" => "Nintendo - Game Boy Advance",
        "n64" => "Nintendo - Nintendo 64",
        "megadrive" => "Sega - Mega Drive - Genesis",
        "mastersystem" => "Sega - Master System - Mark III",
        "gamegear" => "Sega - Game Gear",
        "sega32x" => "Sega - 32X",
        "pcengine" => "NEC - PC Engine - TurboGrafx 16",
        "atari2600" => "Atari - 2600",
        "lynx" => "Atari - Lynx",
        "wonderswan" => "Bandai - WonderSwan",
        "ngp" => "SNK - Neo Geo Pocket",
        _ => return None,
    })
}

/// Convenção de nome de arquivo da libretro: alguns caracteres viram `_`.
fn sanitize_thumb_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '&' | '*' | '/' | ':' | '`' | '<' | '>' | '?' | '\\' | '|' => '_',
            _ => c,
        })
        .collect()
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .flat_map(|b| {
            if b.is_ascii_alphanumeric()
                || matches!(b, b'-' | b'_' | b'.' | b'(' | b')' | b'!' | b',')
            {
                vec![b as char]
            } else {
                format!("%{b:02X}").chars().collect()
            }
        })
        .collect()
}

/// URL do boxart da libretro pra `(system_id, título da ROM)`. O título deve
/// ser o nome do arquivo sem extensão (ex: `Super Mario Bros. (World)`); casa
/// melhor se as ROMs seguem a nomenclatura No-Intro. `None` se o sistema não
/// tem cobertura. Não faz requisição — o `<img>` do frontend carrega (e cai
/// num placeholder no `onerror`).
pub fn libretro_boxart_url(system_id: &str, rom_title: &str) -> Option<String> {
    let sys = libretro_thumbnail_system(system_id)?;
    Some(format!(
        "https://thumbnails.libretro.com/{}/Named_Boxarts/{}.png",
        percent_encode(sys),
        percent_encode(&sanitize_thumb_name(rom_title)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxart_url_shape() {
        let u = libretro_boxart_url("nes", "Super Mario Bros. (World)").unwrap();
        assert_eq!(
            u,
            "https://thumbnails.libretro.com/Nintendo%20-%20Nintendo%20Entertainment%20System/Named_Boxarts/Super%20Mario%20Bros.%20(World).png"
        );
        assert!(libretro_boxart_url("disc", "whatever").is_none());
        // caracteres proibidos viram _
        assert!(libretro_boxart_url("snes", "Tom & Jerry")
            .unwrap()
            .contains("Tom%20_%20Jerry"));
    }

    #[test]
    fn maps_common_extensions() {
        assert_eq!(system_for_extension("nes"), Some("nes"));
        assert_eq!(system_for_extension("SFC"), Some("snes"));
        assert_eq!(system_for_extension("gba"), Some("gba"));
        assert_eq!(system_for_extension("md"), Some("megadrive"));
        assert_eq!(system_for_extension("xyz"), None);
    }
}
