//! Inferência de sistema a partir da extensão do arquivo (e, quando a
//! extensão é ambígua, da pasta) — palpite pro agrupamento inicial da
//! biblioteca; o match real de metadata é por hash.

/// Extensões de imagem de disco — compartilhadas por vários sistemas (PS1,
/// PS2, Saturn, Dreamcast, PSP, Sega CD, PC Engine CD, PC-FX, 3DO...). A
/// extensão sozinha não dá pra saber qual; `system_for_extension` devolve o
/// balde genérico `"disc"` pra elas, e quem escaneia (`scan.rs`) tenta
/// desambiguar pelo nome da pasta antes de aceitar esse fallback (ver
/// `system_from_folder_name`). Deliberadamente **sem** `.bin` — usado por
/// lixo demais fora de contexto de disco pra arriscar reconhecer sozinho.
pub const AMBIGUOUS_DISC_EXTS: &[&str] = &[
    "iso", "cue", "chd", "pbp", "gdi", "cdi", "mdf", "m3u", "img", "ccd", "nrg", "cso",
];

/// `system_id` canônico pra uma extensão (sem o ponto, minúsculo). `None` se
/// não reconhecida. Pras extensões de disco (`AMBIGUOUS_DISC_EXTS`) isso é
/// só o fallback genérico — `scan.rs` tenta a pasta primeiro.
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
        "pce" => "pcengine",
        "sgx" => "supergrafx",
        "a26" => "atari2600",
        "a78" => "atari7800",
        "lnx" => "lynx",
        "ws" | "wsc" => "wonderswan",
        "ngp" | "ngc" => "ngp",
        "32x" => "sega32x",
        "vb" => "vb",
        "col" => "coleco",
        "int" => "intellivision",
        // Extensões exclusivas de um sistema, tiradas do `supported_extensions`
        // do `.info` oficial de cada core do catálogo (`libretro-core-info`).
        // As genéricas desses mesmos cores (`.bin`/`.rom`/`.dsk`/`.tap`/`.cas`)
        // ficam em `folder_only_exts`: só valem com a pasta dizendo o sistema.
        "nds" | "dsi" | "ids" => "nds",
        "sg" => "sg1000",
        "a52" => "atari5200",
        "atr" | "xfd" | "atx" | "xex" => "atari8bit",
        "j64" | "jag" => "jaguar",
        "min" => "pokemini",
        "sv" => "supervision",
        "mx1" | "mx2" => "msx",
        "vec" => "vectrex",
        "d64" | "d71" | "d81" | "g64" | "t64" | "x64" | "crt" => "c64",
        "adf" | "adz" | "dms" | "hdf" => "amiga",
        "tzx" | "z80" | "rzx" | "szx" | "scl" | "trd" => "zxspectrum",
        "cdt" | "cpr" => "amstradcpc",
        // `.gdi`/`.cdi` são GD-ROM → Dreamcast por padrão (NAOMI/Atomiswave a
        // pasta desambigua); `.pbp`/`.cso` são de PSP. As outras seguem ambíguas
        // (`scan.rs` tenta a pasta, depois fareja o conteúdo).
        "gdi" | "cdi" => "dreamcast",
        "pbp" | "cso" => "psp",
        "iso" | "cue" | "chd" | "mdf" | "m3u" | "img" | "ccd" | "nrg" => "disc",
        _ => return None,
    })
}

/// Nomes de pasta comuns em bibliotecas RetroBat/ES-DE e packs de bezel →
/// `system_id` canônico do ReEmu. Usado tanto pro scan de ROMs (desambiguar
/// disco/arcade) quanto pro scan de decoração (`decoration.rs`).
pub fn system_from_folder_name(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    let n = n.trim();
    Some(match n {
        "nes"
        | "famicom"
        | "fc"
        | "nintendo entertainment system"
        | "nintendo - nintendo entertainment system" => "nes",
        "snes"
        | "sfc"
        | "super famicom"
        | "super nintendo"
        | "supernintendo"
        | "super nintendo entertainment system"
        | "nintendo - super nintendo entertainment system" => "snes",
        "gb" | "gameboy" | "game boy" | "nintendo - game boy" => "gb",
        "gbc" | "game boy color" | "gameboycolor" | "nintendo - game boy color" => "gbc",
        "gba" | "game boy advance" | "gameboyadvance" | "nintendo - game boy advance" => "gba",
        "n64" | "nintendo 64" | "nintendo64" | "nintendo - nintendo 64" => "n64",
        "genesis"
        | "megadrive"
        | "mega drive"
        | "sega genesis"
        | "sega mega drive"
        | "sega - mega drive - genesis"
        | "md" => "megadrive",
        "sms"
        | "mastersystem"
        | "master system"
        | "sega master system"
        | "sega - master system - mark iii" => "mastersystem",
        "gg" | "gamegear" | "game gear" | "sega game gear" | "sega - game gear" => "gamegear",
        "32x" | "sega32x" | "sega 32x" | "sega - 32x" => "sega32x",
        "pce"
        | "pcengine"
        | "pc engine"
        | "turbografx"
        | "turbografx-16"
        | "turbografx 16"
        | "tg16"
        | "nec - pc engine - turbografx 16" => "pcengine",
        "atari2600" | "atari 2600" | "2600" | "atari - 2600" => "atari2600",
        "atari7800" | "atari 7800" | "7800" | "atari - 7800" => "atari7800",
        "lynx" | "atari lynx" | "atari - lynx" => "lynx",
        "wonderswan" | "ws" | "bandai - wonderswan" => "wonderswan",
        "ngp" | "neo geo pocket" | "neogeopocket" | "snk - neo geo pocket" => "ngp",
        "virtualboy" | "virtual boy" | "vb" | "nintendo - virtual boy" => "vb",
        "colecovision" | "coleco" | "coleco - colecovision" => "coleco",
        "intellivision" | "intv" | "mattel - intellivision" => "intellivision",
        "nds" | "nintendo ds" | "nintendods" | "nintendo - nintendo ds" | "dsi"
        | "nintendo dsi" | "nintendo - nintendo dsi" => "nds",
        "sg1000" | "sg-1000" | "sega sg-1000" | "sega - sg-1000" => "sg1000",
        "supergrafx" | "sgx" | "pc engine supergrafx" | "nec - pc engine supergrafx" => {
            "supergrafx"
        }
        "atari5200" | "atari 5200" | "5200" | "atari - 5200" => "atari5200",
        "atari800" | "atari8bit" | "atari 8-bit" | "atari 800" | "atarixl"
        | "atari - 8-bit family" => "atari8bit",
        "jaguar" | "atarijaguar" | "atari jaguar" | "atari - jaguar" => "jaguar",
        "pokemini" | "pokemon mini" | "nintendo - pokemon mini" => "pokemini",
        "supervision" | "watara supervision" | "watara - supervision" => "supervision",
        "msx" | "msx1" | "msx2" | "microsoft - msx" | "microsoft - msx2" => "msx",
        "vectrex" | "gce - vectrex" => "vectrex",
        "odyssey2" | "odyssey 2" | "videopac" | "magnavox - odyssey2" => "odyssey2",
        "c64" | "commodore 64" | "commodore64" | "commodore - 64" => "c64",
        "amiga" | "commodore amiga" | "commodore - amiga" => "amiga",
        "zxspectrum" | "zx spectrum" | "spectrum" | "sinclair - zx spectrum" => "zxspectrum",
        "amstradcpc" | "cpc" | "amstrad cpc" | "amstrad - cpc" => "amstradcpc",
        // --- disco (AMBIGUOUS_DISC_EXTS) ---
        "psx" | "playstation" | "ps1" | "sony - playstation" => "psx",
        "ps2" | "playstation2" | "playstation 2" | "sony - playstation 2" => "ps2",
        "saturn" | "sega saturn" | "sega - saturn" => "saturn",
        "dreamcast" | "dc" | "sega dreamcast" | "sega - dreamcast" => "dreamcast",
        "naomi" | "sega naomi" | "sega - naomi" | "naomi2" | "naomi 2" | "sega - naomi 2" => "naomi",
        "atomiswave" | "aw" | "sammy atomiswave" | "sammy - atomiswave" => "atomiswave",
        "psp" | "playstationportable" | "sony - playstation portable" => "psp",
        "segacd" | "mega-cd" | "megacd" | "sega cd" | "sega - mega-cd - sega cd" => "segacd",
        "pcenginecd" | "turbografxcd" | "turbografx-cd" | "tgcd" | "pce-cd" | "pcecd"
        | "nec - pc engine cd - turbografx-cd" => "pcenginecd",
        "neogeocd" | "neo geo cd" | "neogeo cd" | "ngcd" | "snk - neo geo cd" => "neogeocd",
        "cdi" | "cd-i" | "cdimono1" | "philips cd-i" | "philips - cd-i" => "cdi",
        "3do" | "the 3do company - 3do" => "3do",
        "pcfx" | "pc-fx" | "pc fx" | "nec - pc-fx" => "pcfx",
        // --- arcade ---
        "arcade" | "mame" | "fbneo" | "fba" | "finalburn neo" | "neogeo" | "neo geo" | "cps1"
        | "cps2" | "cps3" => "arcade",
        _ => return None,
    })
}

/// Extensões genéricas (usadas por vários sistemas, ou por arquivo que não é
/// ROM) que só contam como ROM de `system_id` quando uma pasta ancestral já
/// identifica esse sistema (ex: `<roms>/msx/jogo.rom`). Fora dessa pasta o
/// scan continua ignorando — um `.bin` solto não vira ROM de nada. Tiradas
/// do `supported_extensions` do `.info` oficial dos cores do catálogo.
pub fn folder_only_exts(system_id: &str) -> &'static [&'static str] {
    match system_id {
        "msx" => &["rom", "ri", "dsk", "cas"],
        "odyssey2" => &["bin"],
        "vectrex" => &["bin"],
        "supervision" => &["bin"],
        "sg1000" => &["bin", "rom"],
        "atari5200" => &["bin", "rom", "car"],
        "atari8bit" => &["bin", "rom", "car", "cas", "com"],
        "jaguar" => &["bin", "rom", "abs", "cof"],
        "c64" => &["prg", "p00", "tap"],
        "zxspectrum" => &["tap", "dsk", "sna", "dck"],
        "amstradcpc" => &["dsk", "sna", "tap"],
        _ => &[],
    }
}

/// Pasta do sistema no servidor de thumbnails da libretro
/// (`thumbnails.libretro.com`). `None` = sem cobertura conhecida. Nomes
/// conferidos contra os repositórios do org `libretro-thumbnails` no GitHub
/// (o `_` do nome do repo é o espaço da pasta real) — não de memória.
fn libretro_thumbnail_system(system_id: &str) -> Option<&'static str> {
    Some(match system_id {
        "nes" => "Nintendo - Nintendo Entertainment System",
        "snes" => "Nintendo - Super Nintendo Entertainment System",
        "gb" => "Nintendo - Game Boy",
        "gbc" => "Nintendo - Game Boy Color",
        "gba" => "Nintendo - Game Boy Advance",
        "n64" => "Nintendo - Nintendo 64",
        "vb" => "Nintendo - Virtual Boy",
        "megadrive" => "Sega - Mega Drive - Genesis",
        "mastersystem" => "Sega - Master System - Mark III",
        "gamegear" => "Sega - Game Gear",
        "sega32x" => "Sega - 32X",
        "saturn" => "Sega - Saturn",
        "dreamcast" => "Sega - Dreamcast",
        "naomi" => "Sega - Naomi",
        "atomiswave" => "Atomiswave",
        "segacd" => "Sega - Mega-CD - Sega CD",
        "neogeocd" => "SNK - Neo Geo CD",
        "cdi" => "Philips - CD-i",
        "pcengine" => "NEC - PC Engine - TurboGrafx 16",
        "pcenginecd" => "NEC - PC Engine CD - TurboGrafx-CD",
        "pcfx" => "NEC - PC-FX",
        "atari2600" => "Atari - 2600",
        "atari7800" => "Atari - 7800",
        "lynx" => "Atari - Lynx",
        "wonderswan" => "Bandai - WonderSwan",
        "ngp" => "SNK - Neo Geo Pocket",
        "coleco" => "Coleco - ColecoVision",
        "intellivision" => "Mattel - Intellivision",
        "3do" => "The 3DO Company - 3DO",
        "psx" => "Sony - PlayStation",
        "ps2" => "Sony - PlayStation 2",
        "psp" => "Sony - PlayStation Portable",
        "nds" => "Nintendo - Nintendo DS",
        "sg1000" => "Sega - SG-1000",
        "supergrafx" => "NEC - PC Engine SuperGrafx",
        "atari5200" => "Atari - 5200",
        "atari8bit" => "Atari - 8-bit Family",
        "jaguar" => "Atari - Jaguar",
        "pokemini" => "Nintendo - Pokemon Mini",
        "supervision" => "Watara - Supervision",
        "msx" => "Microsoft - MSX",
        "vectrex" => "GCE - Vectrex",
        "odyssey2" => "Magnavox - Odyssey2",
        "c64" => "Commodore - 64",
        "amiga" => "Commodore - Amiga",
        "zxspectrum" => "Sinclair - ZX Spectrum",
        "amstradcpc" => "Amstrad - CPC",
        // "arcade": sem cobertura de boxart 1:1 (MAME/FBNeo são sets separados
        // no thumbnails.libretro.com, não um sistema único) — fica sem, o
        // frontend já cai nas iniciais no `onerror`.
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
        assert!(libretro_boxart_url("arcade", "whatever").is_none());
        // caracteres proibidos viram _
        assert!(libretro_boxart_url("snes", "Tom & Jerry")
            .unwrap()
            .contains("Tom%20_%20Jerry"));
    }

    #[test]
    fn new_disc_systems_have_boxart_coverage() {
        for sys in ["psx", "ps2", "saturn", "dreamcast", "psp"] {
            assert!(
                libretro_boxart_url(sys, "Game").is_some(),
                "esperava cobertura de thumbnail pra {sys}"
            );
        }
    }

    #[test]
    fn maps_common_extensions() {
        assert_eq!(system_for_extension("nes"), Some("nes"));
        assert_eq!(system_for_extension("SFC"), Some("snes"));
        assert_eq!(system_for_extension("gba"), Some("gba"));
        assert_eq!(system_for_extension("md"), Some("megadrive"));
        assert_eq!(system_for_extension("vb"), Some("vb"));
        assert_eq!(system_for_extension("a78"), Some("atari7800"));
        assert_eq!(system_for_extension("col"), Some("coleco"));
        assert_eq!(system_for_extension("int"), Some("intellivision"));
        assert_eq!(system_for_extension("iso"), Some("disc")); // fallback genérico
        assert_eq!(system_for_extension("gdi"), Some("dreamcast")); // GD-ROM
        assert_eq!(system_for_extension("cdi"), Some("dreamcast"));
        assert_eq!(system_for_extension("pbp"), Some("psp"));
        assert_eq!(system_for_extension("xyz"), None);
    }

    #[test]
    fn ambiguous_disc_exts_resolve_to_a_disc_system() {
        // toda extensão de disco cai num sistema de disco conhecido (genérico
        // `disc`, ou já específico quando a extensão não é de fato ambígua).
        const DISC_SYSTEMS: &[&str] = &["disc", "dreamcast", "psp"];
        for ext in AMBIGUOUS_DISC_EXTS {
            let sys = system_for_extension(ext).unwrap_or("?");
            assert!(DISC_SYSTEMS.contains(&sys), "{ext} -> {sys}");
        }
    }

    #[test]
    fn folder_name_disambiguates_disc_systems() {
        assert_eq!(system_from_folder_name("psx"), Some("psx"));
        assert_eq!(system_from_folder_name("PlayStation"), Some("psx"));
        assert_eq!(system_from_folder_name("ps1"), Some("psx"));
        assert_eq!(system_from_folder_name("Saturn"), Some("saturn"));
        assert_eq!(system_from_folder_name("dreamcast"), Some("dreamcast"));
        assert_eq!(system_from_folder_name("dc"), Some("dreamcast"));
        assert_eq!(system_from_folder_name("naomi"), Some("naomi"));
        assert_eq!(system_from_folder_name("atomiswave"), Some("atomiswave"));
        assert_eq!(system_from_folder_name("neogeocd"), Some("neogeocd"));
        assert_eq!(system_from_folder_name("psp"), Some("psp"));
        assert_eq!(system_from_folder_name("segacd"), Some("segacd"));
        assert_eq!(system_from_folder_name("not-a-system"), None);
    }

    /// Todo sistema novo precisa das 3 pontas: extensão ou pasta que o
    /// reconhece, e pasta de capa no libretro-thumbnails.
    const CATALOG_SYSTEMS: &[&str] = &[
        "nds",
        "sg1000",
        "supergrafx",
        "atari5200",
        "atari8bit",
        "jaguar",
        "pokemini",
        "supervision",
        "msx",
        "vectrex",
        "odyssey2",
        "c64",
        "amiga",
        "zxspectrum",
        "amstradcpc",
    ];

    #[test]
    fn catalog_systems_are_recognized_and_have_boxart() {
        for sys in CATALOG_SYSTEMS {
            assert_eq!(system_from_folder_name(sys), Some(*sys), "pasta {sys}");
            assert!(libretro_boxart_url(sys, "Game").is_some(), "capa {sys}");
        }
    }

    #[test]
    fn maps_catalog_system_extensions() {
        assert_eq!(system_for_extension("nds"), Some("nds"));
        assert_eq!(system_for_extension("sg"), Some("sg1000"));
        assert_eq!(system_for_extension("sgx"), Some("supergrafx"));
        assert_eq!(system_for_extension("pce"), Some("pcengine"));
        assert_eq!(system_for_extension("a52"), Some("atari5200"));
        assert_eq!(system_for_extension("xex"), Some("atari8bit"));
        assert_eq!(system_for_extension("j64"), Some("jaguar"));
        assert_eq!(system_for_extension("min"), Some("pokemini"));
        assert_eq!(system_for_extension("sv"), Some("supervision"));
        assert_eq!(system_for_extension("mx2"), Some("msx"));
        assert_eq!(system_for_extension("vec"), Some("vectrex"));
        assert_eq!(system_for_extension("d64"), Some("c64"));
        assert_eq!(system_for_extension("adf"), Some("amiga"));
        assert_eq!(system_for_extension("tzx"), Some("zxspectrum"));
        assert_eq!(system_for_extension("cdt"), Some("amstradcpc"));
    }

    #[test]
    fn generic_exts_never_recognized_without_folder() {
        for sys in CATALOG_SYSTEMS {
            for ext in folder_only_exts(sys) {
                assert_eq!(system_for_extension(ext), None, "{sys}: .{ext}");
            }
        }
    }

    #[test]
    fn folder_name_recognizes_arcade_aliases() {
        for alias in ["arcade", "mame", "fbneo", "FBA", "neogeo", "cps2"] {
            assert_eq!(system_from_folder_name(alias), Some("arcade"), "{alias}");
        }
    }
}
