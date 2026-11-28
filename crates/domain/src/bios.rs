//! Arquivos de sistema (BIOS) que alguns cores libretro exigem ou aceitam
//! opcionalmente, além da ROM em si. Tabela pura — sem I/O; quem confere o
//! que está de fato no disco é o adapter (`apps/desktop/src-tauri/src/bios.rs`).
//!
//! Convenção de pasta = a mesma do RetroArch: os arquivos ficam em
//! `<system_dir>/[subfolder/]<filename>`, onde `system_dir` é o que o core lê
//! via `RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY`.
//!
//! Nomes de arquivo e MD5 conferidos em `docs.libretro.com/library/<core>/`
//! (não de memória — ver `docs/ai-context/REFERENCES.md`): Beetle PSX,
//! Kronos (Saturn), Flycast (Dreamcast), FBNeo, Genesis Plus GX + PicoDrive
//! (Sega CD), Beetle PCE Fast (PC Engine CD), Beetle PC-FX. Os nomes também
//! batem com as strings dos próprios `.so` desses cores. Amiga (PUAE), Atari
//! 5200/8-bit (atari800 + a5200), MSX (fMSX) e DS (melonDS/DeSmuME) foram
//! conferidos nos mesmos docs E nos `firmwareN_path`/`firmwareN_opt` do
//! `libretro-core-info` (o a5200 e o melonDS DS não têm página nos docs).
//!
//! `required` só é `true` quando TODOS os cores do sistema no catálogo
//! exigem o arquivo — o `RomDetail` avisa quando falta um obrigatório, e um
//! aviso falso pra quem usa o outro core confunde mais do que ajuda. O
//! requisito de um core específico é consultado por
//! `bios_required_for_core`.
//!
//! PSP não entra aqui: o PPSSPP não usa BIOS, e sim a pasta `assets` do
//! projeto PPSSPP (GPL) em `<system_dir>/PPSSPP/` — outro tipo de coisa.

/// Um arquivo de sistema esperado por um core, pra um `system_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiosFile {
    pub filename: &'static str,
    /// Subpasta dentro de `system_dir` (ex: `"dc"`, `"kronos"`, `"fbneo"`).
    /// `None` = direto na raiz de `system_dir`.
    pub subfolder: Option<&'static str>,
    /// MD5s aceitos (qualquer um vale — cores diferentes documentam
    /// revisões diferentes do mesmo arquivo, ex: `bios_CD_U.bin`). Vazio =
    /// só confere presença (ex: arcade, onde cada jogo pede um BIOS
    /// diferente).
    pub md5: &'static [&'static str],
    /// O core recusa rodar sem isso (`true`) ou tem fallback HLE (`false`).
    pub required: bool,
    /// Região/variante ou observação (mostrado na UI).
    pub note: &'static str,
    /// Não vazio = entrada "qualquer um destes": vários dumps equivalentes
    /// (nome, MD5) na pasta `subfolder`, e basta um (o PS2: qualquer BIOS em
    /// `pcsx2/bios/`). `filename` vira só a chave da entrada (`"*"`).
    pub any_of: &'static [(&'static str, &'static str)],
}

const PSX: &[BiosFile] = &[
    BiosFile {
        filename: "scph5500.bin",
        subfolder: None,
        md5: &["8dd7d5296a650fac7319bce665a6a53c"],
        required: false,
        note: "NTSC-J (Japão) — Beetle PSX cai pro OpenBIOS embutido se faltar",
        any_of: &[],
    },
    BiosFile {
        filename: "scph5501.bin",
        subfolder: None,
        md5: &["490f666e1afb15b7362b406ed1cea246"],
        required: false,
        note: "NTSC-U (EUA) — Beetle PSX cai pro OpenBIOS embutido se faltar",
        any_of: &[],
    },
    BiosFile {
        filename: "scph5502.bin",
        subfolder: None,
        md5: &["32736f17079d0b2b7024407c39bd3050"],
        required: false,
        note: "PAL (Europa) — Beetle PSX cai pro OpenBIOS embutido se faltar",
        any_of: &[],
    },
];

const SATURN: &[BiosFile] = &[BiosFile {
    filename: "saturn_bios.bin",
    subfolder: Some("kronos"),
    md5: &["af5828fdff51384f99b3c4926be27762"],
    required: true,
    note: "Kronos NÃO faz HLE — precisa de um BIOS real de Saturn pra rodar",
    any_of: &[],
}];

const DREAMCAST: &[BiosFile] = &[BiosFile {
    filename: "dc_boot.bin",
    subfolder: Some("dc"),
    md5: &["e10c53c2f8b90bab96ead2d368858623"],
    required: false,
    note: "Flycast tem opção \"Enable HLE BIOS\" — roda sem, com menos precisão",
    any_of: &[],
}];

/// NAOMI / NAOMI 2 no Flycast: BIOS do MAME em `dc/` (docs.libretro.com,
/// library/flycast — "BIOS from MAME"). Opcionais pra Dreamcast, mas o jogo
/// de NAOMI não roda sem a da placa; as de jogo específico só valem pra ele.
/// Sem MD5: os sets do MAME variam de versão.
const NAOMI: &[BiosFile] = &[
    BiosFile {
        filename: "naomi.zip",
        subfolder: Some("dc"),
        md5: &[],
        required: true,
        note: "BIOS da placa NAOMI (set do MAME) — os jogos de NAOMI não rodam sem",
        any_of: &[],
    },
    BiosFile {
        filename: "naomi2.zip",
        subfolder: Some("dc"),
        md5: &[],
        required: false,
        note: "BIOS da NAOMI 2 (set do MAME) — só pros jogos de NAOMI 2",
        any_of: &[],
    },
    BiosFile {
        filename: "hod2bios.zip",
        subfolder: Some("dc"),
        md5: &[],
        required: false,
        note: "BIOS própria de The House of the Dead 2",
        any_of: &[],
    },
    BiosFile {
        filename: "f355bios.zip",
        subfolder: Some("dc"),
        md5: &[],
        required: false,
        note: "BIOS de Ferrari F355 Challenge (twin/deluxe)",
        any_of: &[],
    },
    BiosFile {
        filename: "f355dlx.zip",
        subfolder: Some("dc"),
        md5: &[],
        required: false,
        note: "BIOS de Ferrari F355 Challenge (deluxe)",
        any_of: &[],
    },
    BiosFile {
        filename: "airlbios.zip",
        subfolder: Some("dc"),
        md5: &[],
        required: false,
        note: "BIOS de Airline Pilots (deluxe)",
        any_of: &[],
    },
];

const ATOMISWAVE: &[BiosFile] = &[BiosFile {
    filename: "awbios.zip",
    subfolder: Some("dc"),
    md5: &[],
    required: true,
    note: "BIOS da placa Atomiswave (set do MAME) — os jogos não rodam sem",
    any_of: &[],
}];

const ARCADE: &[BiosFile] = &[
    BiosFile {
        filename: "neogeo.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "Jogos de Neo Geo (MVS/AES) do FBNeo pedem isso",
        any_of: &[],
    },
    BiosFile {
        filename: "neocdz.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "Neo Geo CD — precisa também do neogeo.zip",
        any_of: &[],
    },
    BiosFile {
        filename: "coleco.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "ColecoVision via FBNeo",
        any_of: &[],
    },
    BiosFile {
        filename: "pgm.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "PGM System (IGS) via FBNeo",
        any_of: &[],
    },
    BiosFile {
        filename: "decocass.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "DECO Cassette System via FBNeo",
        any_of: &[],
    },
    BiosFile {
        filename: "fdsbios.zip",
        subfolder: Some("fbneo"),
        md5: &[],
        required: false,
        note: "Famicom Disk System via FBNeo",
        any_of: &[],
    },
];

const SEGA_CD: &[BiosFile] = &[
    BiosFile {
        filename: "bios_CD_U.bin",
        subfolder: None,
        // 1º: Genesis Plus GX; 2º: PicoDrive — revisões diferentes, as duas
        // documentadas como válidas.
        md5: &[
            "854b9150240a198070150e4566ae1290",
            "2efd74e3232ff260e371b99f84024f7f",
        ],
        required: false,
        note: "Sega CD (EUA) — obrigatório pros jogos dos EUA",
        any_of: &[],
    },
    BiosFile {
        filename: "bios_CD_E.bin",
        subfolder: None,
        md5: &["e66fa1dc5820d254611fdcdba0662372"],
        required: false,
        note: "Mega-CD (Europa) — obrigatório pros jogos europeus",
        any_of: &[],
    },
    BiosFile {
        filename: "bios_CD_J.bin",
        subfolder: None,
        md5: &["278a9397d192149e84e820ac621a8edd"],
        required: false,
        note: "Mega-CD (Japão) — obrigatório pros jogos japoneses",
        any_of: &[],
    },
];

const PC_ENGINE_CD: &[BiosFile] = &[
    BiosFile {
        filename: "syscard3.pce",
        subfolder: None,
        md5: &["38179df8f4ac870017db21ebcbf53114"],
        required: true,
        note: "Super CD-ROM² System 3.x — o padrão dos cores Beetle PCE",
        any_of: &[],
    },
    BiosFile {
        filename: "syscard2.pce",
        subfolder: None,
        md5: &[],
        required: false,
        note: "CD-ROM² System 2.x — alternativa (opção \"CD BIOS\" do core)",
        any_of: &[],
    },
    BiosFile {
        filename: "syscard1.pce",
        subfolder: None,
        md5: &[],
        required: false,
        note: "CD-ROM² System 1.x — alternativa (opção \"CD BIOS\" do core)",
        any_of: &[],
    },
    BiosFile {
        filename: "gexpress.pce",
        subfolder: None,
        md5: &[],
        required: false,
        note: "Game Express CD Card — só pros jogos da Games Express",
        any_of: &[],
    },
];

const PC_FX: &[BiosFile] = &[BiosFile {
    filename: "pcfx.rom",
    subfolder: None,
    md5: &["08e36edbea28a017f79f8d4f7ff9b6d7"],
    required: true,
    note: "BIOS v1.00 — o Beetle PC-FX não roda sem",
    any_of: &[],
}];

const GAMECUBE: &[BiosFile] = &[
    BiosFile {
        filename: "USA/IPL.bin",
        subfolder: Some("dolphin-emu/Sys/GC"),
        md5: &[
            "b17148254a5799684c7d783206504926",
            "fc924a7c879b661abc37cec4f018fdf3",
        ],
        required: false,
        note: "IPL NTSC-U (EUA) - opcional; util para fontes do sistema e abertura do GameCube",
        any_of: &[],
    },
    BiosFile {
        filename: "EUR/IPL.bin",
        subfolder: Some("dolphin-emu/Sys/GC"),
        md5: &[
            "db92574caab77a7ec99d4605fd6f2450",
            "339848a0b7c2124cf155276c1e79cbd0",
        ],
        required: false,
        note: "IPL PAL (Europa) - opcional; util para fontes do sistema e abertura do GameCube",
        any_of: &[],
    },
    BiosFile {
        filename: "JAP/IPL.bin",
        subfolder: Some("dolphin-emu/Sys/GC"),
        md5: &[
            "b17148254a5799684c7d783206504926",
            "fc924a7c879b661abc37cec4f018fdf3",
        ],
        required: false,
        note: "IPL NTSC-J (Japao) - opcional; util para fontes do sistema e abertura do GameCube",
        any_of: &[],
    },
];

const AMIGA: &[BiosFile] = &[
    BiosFile {
        filename: "kick34005.A500",
        subfolder: None,
        md5: &["82a21c1890cae844b3df741f2762d48d"],
        required: false,
        note: "Kickstart 1.3 (A500) — o da maioria dos jogos. PUAE usa AROS embutido se faltar",
        any_of: &[],
    },
    BiosFile {
        filename: "kick37175.A500",
        subfolder: None,
        md5: &["dc10d7bdd1b6f450773dfb558477c230"],
        required: false,
        note: "Kickstart 2.04 (A500+)",
        any_of: &[],
    },
    BiosFile {
        filename: "kick40063.A600",
        subfolder: None,
        md5: &["e40a5dfb3d017ba8779faba30cbd1c8e"],
        required: false,
        note: "Kickstart 3.1 (A600)",
        any_of: &[],
    },
    BiosFile {
        filename: "kick40068.A1200",
        subfolder: None,
        md5: &["646773759326fbac3b2311fd8c8793ee"],
        required: false,
        note: "Kickstart 3.1 (A1200) — jogos AGA",
        any_of: &[],
    },
    BiosFile {
        filename: "kick40060.CD32",
        subfolder: None,
        // KS sozinho ou KS + extended num arquivo só — os docs listam os dois
        md5: &[
            "5f8924d013dd57a89cf349f4cdedc6b1",
            "f2f241bf094168cfb9e7805dc2856433",
        ],
        required: false,
        note: "Kickstart 3.1 do CD32 (sozinho ou já combinado com o extended)",
        any_of: &[],
    },
    BiosFile {
        filename: "kick40060.CD32.ext",
        subfolder: None,
        md5: &["bb72565701b1b6faece07d68ea5da639"],
        required: false,
        note: "CD32 extended ROM (dispensável se o kick40060.CD32 já é o combinado)",
        any_of: &[],
    },
];
const ATARI_5200: &[BiosFile] = &[BiosFile {
    filename: "5200.rom",
    subfolder: None,
    md5: &["281f20ea4320404ec820fb7ec0693b38"],
    required: false,
    note: "atari800 exige; o a5200 roda sem",
    any_of: &[],
}];
const ATARI_ST: &[BiosFile] = &[BiosFile {
    filename: "tos.img",
    subfolder: None,
    md5: &[
        "c1c57ce48e8ee4135885cee9e63a68a2", // Atari TOS 1.02 US
        "693255fbec212bde1d0054a94d6ffb28", // EmuTOS 1.4, ST/US, 192 KiB
    ],
    required: false,
    note: "Hatari e Hatari 2014 exigem TOS. O hatariB inclui EmuTOS",
    any_of: &[],
}];
pub const X68000_IPL_NAMES: &[&str] =
    &["iplrom.dat", "iplrom30.dat", "iplromco.dat", "iplromxv.dat"];
pub const X68000_FONT_NAMES: &[&str] = &["cgrom.dat", "cgrom.tmp"];
pub const X68000_IPL_SIZE: u64 = 131_072;
pub const X68000_FONT_SIZE: u64 = 786_432;
const X68000: &[BiosFile] = &[
    BiosFile {
        filename: "iplrom.dat",
        subfolder: Some("keropi"),
        md5: &["7fd4caabac1d9169e289f0f7bbf71d8e"],
        required: true,
        note: "IPL do PX68k. Também pode usar uma das revisões alternativas",
        any_of: &[],
    },
    BiosFile {
        filename: "cgrom.dat",
        subfolder: Some("keropi"),
        md5: &[
            "cb0a5cfcf7247a7eab74bb2716260269", // CGROM original
            "ed8956b00f1847f93b566ce9d41aef4d", // Shinonome, gerado pelo WebX68k
        ],
        required: true,
        note: "ROM de fontes do PX68k. Aceita a fonte Shinonome gerada pelo WebX68k",
        any_of: &[],
    },
    BiosFile {
        filename: "iplrom30.dat",
        subfolder: Some("keropi"),
        md5: &["f373003710ab4322642f527f567e020a"],
        required: false,
        note: "IPL alternativo do X68030",
        any_of: &[],
    },
    BiosFile {
        filename: "iplromco.dat",
        subfolder: Some("keropi"),
        md5: &["cc78d4f4900f622bd6de1aed7f52592f"],
        required: false,
        note: "IPL alternativo do X68000 Compact",
        any_of: &[],
    },
    BiosFile {
        filename: "iplromxv.dat",
        subfolder: Some("keropi"),
        md5: &["0617321daa182c3f3d6f41fd02fb3275"],
        required: false,
        note: "IPL alternativo do X68000 XVI",
        any_of: &[],
    },
];
const ATARI_8BIT: &[BiosFile] = &[
    BiosFile {
        filename: "ATARIXL.ROM",
        subfolder: None,
        md5: &["06daac977823773a3eea3422fd26a703"],
        required: false,
        note: "OS do Atari XL/XE — o atari800 pede pra jogos de XL/XE",
        any_of: &[],
    },
    BiosFile {
        filename: "ATARIBAS.ROM",
        subfolder: None,
        md5: &["0bac0c6a50104045d902df4503a4c30b"],
        required: false,
        note: "Interpretador BASIC",
        any_of: &[],
    },
    BiosFile {
        filename: "ATARIOSA.ROM",
        subfolder: None,
        md5: &["eb1f32f5d9f382db1bbfb8d7f9cb343a"],
        required: false,
        note: "OS do Atari 400/800 (PAL)",
        any_of: &[],
    },
    BiosFile {
        filename: "ATARIOSB.ROM",
        subfolder: None,
        md5: &["a3e8d617c95d08031fe1b20d541434b2"],
        required: false,
        note: "OS do Atari 400/800 (NTSC)",
        any_of: &[],
    },
];
const MSX: &[BiosFile] = &[
    BiosFile {
        filename: "MSX.ROM",
        subfolder: None,
        md5: &["364a1a579fe5cb8dba54519bcfcdac0d"],
        required: false,
        note: "MSX1 — fMSX exige; o blueMSX usa as pastas Machines/Databases",
        any_of: &[],
    },
    BiosFile {
        filename: "MSX2.ROM",
        subfolder: None,
        md5: &["ec3a01c91f24fbddcbcab0ad301bc9ef"],
        required: false,
        note: "MSX2 — fMSX exige",
        any_of: &[],
    },
    BiosFile {
        filename: "MSX2EXT.ROM",
        subfolder: None,
        md5: &["2183c2aff17cf4297bdb496de78c2e8a"],
        required: false,
        note: "MSX2 ExtROM — fMSX exige",
        any_of: &[],
    },
    BiosFile {
        filename: "MSX2P.ROM",
        subfolder: None,
        md5: &["847cc025ffae665487940ff2639540e5"],
        required: false,
        note: "MSX2+ — fMSX exige",
        any_of: &[],
    },
    BiosFile {
        filename: "MSX2PEXT.ROM",
        subfolder: None,
        md5: &["7c8243c71d8f143b2531f01afa6a05dc"],
        required: false,
        note: "MSX2+ ExtROM — fMSX exige",
        any_of: &[],
    },
    BiosFile {
        filename: "DISK.ROM",
        subfolder: None,
        md5: &["80dcd1ad1a4cf65d64b7ba10504e8190"],
        required: false,
        note: "DiskROM/BDOS — jogos em disquete no fMSX",
        any_of: &[],
    },
];
const NDS: &[BiosFile] = &[
    BiosFile {
        filename: "bios7.bin",
        subfolder: None,
        md5: &["df692a80a5b1bc90728bc3dfc76cd948"],
        required: false,
        note: "ARM7 — melonDS tem BIOS livre embutida; DeSmuME só usa com a opção de BIOS externa",
        any_of: &[],
    },
    BiosFile {
        filename: "bios9.bin",
        subfolder: None,
        md5: &["a392174eb3e572fed6447e956bde4b25"],
        required: false,
        note: "ARM9 — mesma situação do bios7.bin",
        any_of: &[],
    },
    BiosFile {
        filename: "firmware.bin",
        subfolder: None,
        // o MD5 muda de dump pra dump (guarda configurações do console)
        md5: &[],
        required: false,
        note: "Firmware do DS — opcional; o MD5 varia por console",
        any_of: &[],
    },
];

const SNES: &[BiosFile] = &[
    BiosFile {
        filename: "BS-X.bin",
        subfolder: None,
        md5: &[
            "fed4d8242cfbed61343d53d48432aced", // Japão, rev. 1
            "4ed9648505ab33a4daec93707b16caba", // tradução DRM-Free do System.dat
        ],
        required: false,
        note: "Firmware BS-X para Satellaview",
        any_of: &[],
    },
    BiosFile {
        filename: "SufamiTurbo.sfc",
        subfolder: None,
        md5: &["d3a44ba7d42a74d3ac58cb9c14c6a5ca"],
        required: false,
        note: "Firmware Sufami Turbo para o Mesen2",
        any_of: &[],
    },
];

/// Arquivos de sistema conhecidos pra um `system_id`: a tabela à mão (com
/// as notas conferidas core a core) e, depois, o que só a gerada tem
/// (`bios_generated.rs`, de `scripts/gen_bios_table.py` — System.dat +
/// libretro-core-info). Vazio = o sistema não precisa de nada além da ROM.
pub fn bios_files_for_system(system_id: &str) -> Vec<BiosFile> {
    let mut out = handwritten(system_id).to_vec();
    out.extend(
        crate::bios_generated::GENERATED
            .iter()
            .filter(|(s, _)| *s == system_id)
            .map(|(_, f)| *f),
    );
    out
}

/// Requisito específico para um arquivo opcional na visão por plataforma.
pub fn bios_required_for_core(system_id: &str, filename: &str, core_id: &str) -> bool {
    matches!(
        (system_id, filename, core_id),
        (
            "atarist",
            "tos.img",
            "hatari_libretro" | "hatari2014_libretro"
        )
    )
}

/// Todos os `system_id` com algum BIOS conhecido (os da tabela à mão
/// primeiro, na ordem dela) — pro painel de Configurações varrer tudo.
pub fn known_systems() -> Vec<&'static str> {
    let mut out = HANDWRITTEN_SYSTEMS.to_vec();
    out.extend_from_slice(crate::bios_generated::GENERATED_SYSTEMS);
    out
}

fn handwritten(system_id: &str) -> &'static [BiosFile] {
    match system_id {
        "snes" => SNES,
        "psx" => PSX,
        "saturn" => SATURN,
        "dreamcast" => DREAMCAST,
        "naomi" => NAOMI,
        "atomiswave" => ATOMISWAVE,
        "arcade" => ARCADE,
        "segacd" => SEGA_CD,
        "pcenginecd" => PC_ENGINE_CD,
        "pcfx" => PC_FX,
        "amiga" => AMIGA,
        "atari5200" => ATARI_5200,
        "atarist" => ATARI_ST,
        "x68000" => X68000,
        "atari8bit" => ATARI_8BIT,
        "msx" => MSX,
        "nds" => NDS,
        "gamecube" => GAMECUBE,
        _ => &[],
    }
}

/// Sistemas da tabela à mão (ver `known_systems` pra lista completa).
const HANDWRITTEN_SYSTEMS: &[&str] = &[
    "psx",
    "saturn",
    "dreamcast",
    "naomi",
    "atomiswave",
    "arcade",
    "segacd",
    "pcenginecd",
    "pcfx",
    "amiga",
    "atari5200",
    "atarist",
    "x68000",
    "atari8bit",
    "msx",
    "nds",
    "snes",
    "gamecube",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psx_has_three_region_variants_all_optional() {
        let files = handwritten("psx");
        assert_eq!(files.len(), 3);
        assert!(files.iter().all(|f| !f.required));
        assert!(files.iter().all(|f| !f.md5.is_empty()));
    }

    #[test]
    fn saturn_bios_is_required() {
        let files = handwritten("saturn");
        assert_eq!(files.len(), 1);
        assert!(files[0].required);
        assert_eq!(files[0].subfolder, Some("kronos"));
    }

    #[test]
    fn sega_cd_us_accepts_both_documented_revisions() {
        let us = bios_files_for_system("segacd")
            .into_iter()
            .find(|f| f.filename == "bios_CD_U.bin")
            .unwrap();
        assert_eq!(us.md5.len(), 2);
    }

    #[test]
    fn pc_engine_cd_and_pc_fx_require_their_default_bios() {
        let pce = bios_files_for_system("pcenginecd");
        assert!(pce
            .iter()
            .any(|f| f.filename == "syscard3.pce" && f.required));
        assert_eq!(pce.iter().filter(|f| f.required).count(), 1);
        assert!(bios_files_for_system("pcfx")[0].required);
    }

    #[test]
    fn systems_with_multiple_cores_only_warn_when_all_require_it() {
        // cada um desses tem um core no catálogo que roda sem o arquivo
        for sys in ["amiga", "atari5200", "atari8bit", "atarist", "msx", "nds"] {
            assert!(
                bios_files_for_system(sys).iter().all(|f| !f.required),
                "{sys}: nenhum arquivo deveria ser obrigatório"
            );
        }
    }

    #[test]
    fn atari_st_external_tos_requirement_depends_on_the_core() {
        assert!(bios_required_for_core(
            "atarist",
            "tos.img",
            "hatari_libretro"
        ));
        assert!(bios_required_for_core(
            "atarist",
            "tos.img",
            "hatari2014_libretro"
        ));
        assert!(!bios_required_for_core(
            "atarist",
            "tos.img",
            "hatarib_libretro"
        ));
    }

    #[test]
    fn psp_has_no_bios_entry() {
        assert!(bios_files_for_system("psp").is_empty());
    }

    #[test]
    fn documented_md5s_are_lowercase_hex() {
        for sys in known_systems() {
            for f in bios_files_for_system(sys) {
                for h in f.md5.iter().chain(f.any_of.iter().map(|(_, h)| h)) {
                    assert_eq!(h.len(), 32, "{sys}/{}", f.filename);
                    assert!(
                        h.chars()
                            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                        "{sys}/{}",
                        f.filename
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_system_has_no_bios() {
        assert!(bios_files_for_system("whatever").is_empty());
        // cartucho puro: nada além da ROM
        assert!(bios_files_for_system("atari2600").is_empty());
    }

    #[test]
    fn ps2_is_one_any_of_entry_required() {
        let ps2: Vec<_> = bios_files_for_system("ps2")
            .into_iter()
            .filter(|f| !f.any_of.is_empty())
            .collect();
        assert_eq!(ps2.len(), 1);
        assert!(ps2[0].required);
        assert_eq!(ps2[0].subfolder, Some("pcsx2/bios"));
        assert!(ps2[0].any_of.len() > 10);
    }

    #[test]
    fn generated_does_not_repeat_handwritten_files() {
        for sys in known_systems() {
            let mut seen = std::collections::HashSet::new();
            for f in bios_files_for_system(sys) {
                assert!(
                    seen.insert((f.subfolder, f.filename)),
                    "{sys}: {:?}/{} repetido",
                    f.subfolder,
                    f.filename
                );
            }
        }
    }

    #[test]
    fn odyssey2_only_requires_the_default_bios() {
        let req: Vec<_> = bios_files_for_system("odyssey2")
            .into_iter()
            .filter(|f| f.required)
            .map(|f| f.filename)
            .collect();
        assert_eq!(req, vec!["o2rom.bin"]);
    }

    #[test]
    fn known_systems_all_resolve_to_non_empty() {
        for sys in known_systems() {
            assert!(
                !bios_files_for_system(sys).is_empty(),
                "{sys} devia ter pelo menos 1 BiosFile"
            );
        }
    }
}
