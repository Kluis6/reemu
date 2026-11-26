//! Catálogo de cores libretro (etapa 10): lista curada baixada do buildbot
//! oficial.
//!
//! O buildbot serve `<stem>.<ext>.zip` em
//! `https://buildbot.libretro.com/nightly/<os>/<arch>/latest/`. Baixar =
//! pegar o zip, extrair o dylib pra `<dados>/cores/`. `<stem>` (ex:
//! `fceumm_libretro`) é o mesmo id que `discover_cores` e `load_game` usam.
//!
//! `hw` diz o que o core exige de render:
//! - `Software` — buffer de pixels cru.
//! - `OpenGl` — renderiza num FBO; o frontend cria um contexto GL offscreen
//!   (etapa 02 passo 4) e traz o frame por interop dma_buf zero-cópia (padrão,
//!   `REEMU_GL_INTEROP=0` força readback; a fence `sync_file` do fim do frame
//!   substitui o `glFinish`). Precisa de `libEGL` + GPU.
//! - `Vulkan` — o core roda in-process e adota o `VkDevice` do compositor
//!   (etapa 12); o `VkImage` do scanout vira textura do wgpu sem cópia. Cai
//!   pro processo filho (GL/sw) se a negociação Vulkan não rolar.

use crate::error::CatalogError;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CoreHw {
    Software,
    OpenGl,
    /// Renderiza em Vulkan. O ReEmu roda esses cores in-process, adotando o
    /// `VkDevice` do compositor (etapa 12) — zero cópia. Cai pro processo filho
    /// (GL/sw) se a negociação Vulkan falhar.
    Vulkan,
}

impl CoreHw {
    pub fn as_str(self) -> &'static str {
        match self {
            CoreHw::Software => "software",
            CoreHw::OpenGl => "opengl",
            CoreHw::Vulkan => "vulkan",
        }
    }
}

pub struct CatalogEntry {
    /// Stem do arquivo, sem extensão (ex: `fceumm_libretro`).
    pub id: &'static str,
    pub name: &'static str,
    pub systems: &'static str,
    pub license: &'static str,
    pub hw: CoreHw,
}

const fn sw(
    id: &'static str,
    name: &'static str,
    systems: &'static str,
    license: &'static str,
) -> CatalogEntry {
    CatalogEntry {
        id,
        name,
        systems,
        license,
        hw: CoreHw::Software,
    }
}

const fn gl(
    id: &'static str,
    name: &'static str,
    systems: &'static str,
    license: &'static str,
) -> CatalogEntry {
    CatalogEntry {
        id,
        name,
        systems,
        license,
        hw: CoreHw::OpenGl,
    }
}

const fn vk(
    id: &'static str,
    name: &'static str,
    systems: &'static str,
    license: &'static str,
) -> CatalogEntry {
    CatalogEntry {
        id,
        name,
        systems,
        license,
        hw: CoreHw::Vulkan,
    }
}

pub const CATALOG: &[CatalogEntry] = &[
    // --- Nintendo 8/16 bits ---
    sw("fceumm_libretro", "FCEUmm", "NES / Famicom", "GPLv2"),
    sw("nestopia_libretro", "Nestopia UE", "NES / Famicom", "GPLv2"),
    sw("mesen_libretro", "Mesen", "NES / Famicom", "GPLv3"),
    sw(
        "mesen2_libretro",
        "Mesen 2 (MesenCE)",
        "NES / SNES / Game Boy / Color / GBA / PC Engine / SuperGrafx / CD / Master System / Game Gear / WonderSwan",
        "GPLv3",
    ),
    sw("quicknes_libretro", "QuickNES", "NES / Famicom", "LGPLv2.1"),
    sw(
        "snes9x_libretro",
        "Snes9x",
        "Super Nintendo",
        "Non-commercial",
    ),
    sw(
        "snes9x2010_libretro",
        "Snes9x 2010",
        "Super Nintendo",
        "Non-commercial",
    ),
    sw(
        "snes9x2005_libretro",
        "Snes9x 2005",
        "Super Nintendo",
        "Non-commercial",
    ),
    sw(
        "bsnes_mercury_performance_libretro",
        "bsnes-mercury Performance",
        "Super Nintendo",
        "GPLv3",
    ),
    sw(
        "bsnes_mercury_balanced_libretro",
        "bsnes-mercury Balanced",
        "Super Nintendo",
        "GPLv3",
    ),
    // --- Game Boy / GBA ---
    sw("gambatte_libretro", "Gambatte", "Game Boy / Color", "GPLv2"),
    sw("sameboy_libretro", "SameBoy", "Game Boy / Color", "MIT"),
    sw("tgbdual_libretro", "TGB Dual", "Game Boy / Color", "GPLv2"),
    sw(
        "DoubleCherryGB_libretro",
        "DoubleCherryGB",
        "Game Boy / Color",
        "AGPLv3",
    ),
    sw("gearboy_libretro", "Gearboy", "Game Boy / Color", "GPLv3"),
    sw(
        "mgba_libretro",
        "mGBA",
        "Game Boy Advance / GB / GBC",
        "MPL-2.0",
    ),
    sw("vba_next_libretro", "VBA Next", "Game Boy Advance", "GPLv2"),
    sw("gpsp_libretro", "gpSP", "Game Boy Advance", "GPLv2"),
    // --- Nintendo DS / Virtual Boy ---
    sw("desmume_libretro", "DeSmuME", "Nintendo DS", "GPLv2"),
    sw("melonds_libretro", "melonDS", "Nintendo DS", "GPLv3"),
    sw("mednafen_vb_libretro", "Beetle VB", "Virtual Boy", "GPLv2"),
    // --- Nintendo 64 (GL) ---
    gl(
        "mupen64plus_next_libretro",
        "Mupen64Plus-Next",
        "Nintendo 64",
        "GPLv3",
    ),
    gl(
        "parallel_n64_libretro",
        "ParaLLEl N64",
        "Nintendo 64",
        "GPLv3",
    ),
    // --- Nintendo GameCube (GL) ---
    gl(
        "dolphin_libretro",
        "Dolphin",
        "Nintendo GameCube",
        "GPLv2+",
    ),
    // --- Sega ---
    sw(
        "genesis_plus_gx_libretro",
        "Genesis Plus GX",
        "Mega Drive / Master System / Game Gear / SG-1000 / Sega CD",
        "Non-commercial",
    ),
    sw(
        "picodrive_libretro",
        "PicoDrive",
        "Mega Drive / 32X / Sega CD",
        "MAME-like",
    ),
    sw(
        "blastem_libretro",
        "BlastEm",
        "Mega Drive / Genesis",
        "GPLv3",
    ),
    sw(
        "gearsystem_libretro",
        "Gearsystem",
        "Master System / Game Gear / SG-1000",
        "GPLv3",
    ),
    gl("kronos_libretro", "Kronos", "Sega Saturn", "GPLv2"),
    sw(
        "mednafen_saturn_libretro",
        "Beetle Saturn",
        "Sega Saturn",
        "GPLv2",
    ),
    gl(
        "flycast_libretro",
        "Flycast",
        "Dreamcast / NAOMI / Atomiswave",
        "GPLv2",
    ),
    // --- NEC ---
    sw(
        "mednafen_pce_libretro",
        "Beetle PCE",
        "PC Engine / SuperGrafx / CD",
        "GPLv2",
    ),
    sw(
        "mednafen_pce_fast_libretro",
        "Beetle PCE Fast",
        "PC Engine / TurboGrafx-16",
        "GPLv2",
    ),
    sw(
        "mednafen_supergrafx_libretro",
        "Beetle SuperGrafx",
        "SuperGrafx",
        "GPLv2",
    ),
    sw("mednafen_pcfx_libretro", "Beetle PC-FX", "PC-FX", "GPLv2"),
    // --- Sony ---
    sw(
        "pcsx_rearmed_libretro",
        "PCSX-ReARMed",
        "PlayStation",
        "GPLv2",
    ),
    sw(
        "mednafen_psx_libretro",
        "Beetle PSX",
        "PlayStation",
        "GPLv2",
    ),
    vk(
        "mednafen_psx_hw_libretro",
        "Beetle PSX HW",
        "PlayStation",
        "GPLv2",
    ),
    gl(
        "swanstation_libretro",
        "SwanStation",
        "PlayStation",
        "GPLv3",
    ),
    gl("pcsx2_libretro", "LRPS2", "PlayStation 2", "GPLv3"),
    gl("ppsspp_libretro", "PPSSPP", "PSP", "GPLv2"),
    // --- SNK / Arcade ---
    sw(
        "fbneo_libretro",
        "FinalBurn Neo",
        "Neo Geo / CPS / arcade",
        "Non-commercial",
    ),
    sw(
        "fbalpha2012_libretro",
        "FB Alpha 2012",
        "Neo Geo / CPS / arcade",
        "Non-commercial",
    ),
    sw(
        "fbalpha2012_cps1_libretro",
        "FB Alpha 2012 CPS-1",
        "Arcade / Capcom CPS-1",
        "Non-commercial",
    ),
    sw(
        "mame2003_plus_libretro",
        "MAME 2003-Plus",
        "Arcade (0.78+)",
        "Non-commercial",
    ),
    sw(
        "mame2010_libretro",
        "MAME 2010",
        "Arcade (0.139)",
        "Non-commercial",
    ),
    sw("neocd_libretro", "NeoCD", "Neo Geo CD", "GPLv3"),
    sw(
        "mednafen_ngp_libretro",
        "Beetle NeoPop",
        "Neo Geo Pocket / Color",
        "GPLv2",
    ),
    // --- Atari ---
    sw(
        "hatari_libretro",
        "Hatari",
        "Atari ST / STE / TT / Falcon",
        "GPLv2",
    ),
    sw(
        "hatari2014_libretro",
        "Hatari 2014",
        "Atari ST / STE / TT / Falcon",
        "GPLv2",
    ),
    sw(
        "hatarib_libretro",
        "hatariB",
        "Atari ST / STE / TT / Falcon",
        "GPLv2",
    ),
    sw("stella2014_libretro", "Stella 2014", "Atari 2600", "GPLv2"),
    sw("stella_libretro", "Stella", "Atari 2600", "GPLv2"),
    sw("prosystem_libretro", "ProSystem", "Atari 7800", "GPLv2"),
    sw(
        "atari800_libretro",
        "Atari800",
        "Atari 8-bit / 5200",
        "GPLv2",
    ),
    sw("handy_libretro", "Handy", "Atari Lynx", "Zlib"),
    sw(
        "mednafen_lynx_libretro",
        "Beetle Lynx",
        "Atari Lynx",
        "GPLv2",
    ),
    sw(
        "virtualjaguar_libretro",
        "Virtual Jaguar",
        "Atari Jaguar",
        "GPLv3",
    ),
    // --- Bandai / outros portáteis ---
    sw(
        "mednafen_wswan_libretro",
        "Beetle WonderSwan",
        "WonderSwan / Color",
        "GPLv2",
    ),
    sw("pokemini_libretro", "PokeMini", "Pokémon Mini", "GPLv3"),
    sw("potator_libretro", "Potator", "Watara Supervision", "GPLv2"),
    // --- Home computers ---
    sw(
        "bluemsx_libretro",
        "blueMSX",
        "MSX / ColecoVision / SG-1000",
        "GPLv2",
    ),
    sw("fmsx_libretro", "fMSX", "MSX", "Non-commercial"),
    sw("openmsx_libretro", "openMSX", "MSX / MSX2 / MSX2+ / turbo R", "GPLv2"),
    sw("gearcoleco_libretro", "Gearcoleco", "ColecoVision", "GPLv3"),
    sw("vice_x64sc_libretro", "VICE x64sc", "Commodore 64", "GPLv2"),
    sw("puae_libretro", "PUAE", "Commodore Amiga", "GPLv2"),
    sw("px68k_libretro", "PX68k", "Sharp X68000", "GPLv2"),
    sw("dirksimple_libretro", "DirkSimple", "LaserDisc", "zlib"),
    gl("hypseushw_libretro", "HypseusHW", "LaserDisc", "GPLv3"),
    sw("fuse_libretro", "Fuse", "ZX Spectrum", "GPLv3"),
    sw("81_libretro", "EightyOne", "Sinclair ZX81", "GPLv3"),
    sw("cap32_libretro", "Caprice32", "Amstrad CPC", "GPLv2"),
    sw("dosbox_pure_libretro", "DOSBox-Pure", "MS-DOS", "GPLv2"),
    sw("jaxe_libretro", "JAXE", "CHIP-8 / Super CHIP / XO-CHIP", "MIT"),
    sw("ardens_libretro", "Ardens", "Arduboy / Arduboy FX", "MIT"),
    sw("arduous_libretro", "Arduous", "Arduboy", "GPLv3"),
    sw("uzem_libretro", "Uzem", "Uzebox", "MIT"),
    sw("vaporspec_libretro", "VaporSpec", "VaporSpec", "MIT"),
    sw("gametank_libretro", "GameTank", "GameTank", "MIT"),
    sw("sameduck_libretro", "SameDuck", "Mega Duck / Cougar Boy", "MIT"),
    sw("gw_libretro", "GW", "Handheld Electronic", "zlib"),
    sw(
        "amiarcadia_libretro",
        "AmiArcadia",
        "Emerson Arcadia 2001 / Interton VC 4000 / Elektor TV Games Computer",
        "Non-commercial",
    ),
    sw(
        "scummvm_libretro",
        "ScummVM",
        "ScummVM (adventure games)",
        "GPLv3",
    ),
    // --- Consoles diversos ---
    sw("opera_libretro", "Opera", "3DO", "LGPLv2.1"),
    sw("freeintv_libretro", "FreeIntv", "Intellivision", "GPLv3"),
    sw("o2em_libretro", "O2EM", "Magnavox Odyssey 2", "Artistic"),
    sw("vecx_libretro", "vecx", "Vectrex", "GPLv3"),
    sw("prboom_libretro", "PrBoom", "Doom (WAD)", "GPLv2"),
    sw(
        "tic80_libretro",
        "TIC-80",
        "TIC-80 (fantasy console)",
        "MIT",
    ),
    // --- gerados por scripts/gen_core_catalog.py (libretro-core-info) ---
    sw("crocods_libretro", "CrocoDS", "Amstrad CPC", "MIT"),
    sw("dice_libretro", "DICE", "Arcade", "GPLv3"),
    sw("hbmame_libretro", "HBMAME (Git)", "Arcade", "GPLv2+"),
    sw("mame_libretro", "MAME", "Arcade", "GPLv2+"),
    sw("mame2000_libretro", "MAME 2000 (0.37b5)", "Arcade", "MAME"),
    sw("mame2003_libretro", "MAME 2003 (0.78)", "Arcade", "MAME"),
    sw(
        "mame2003_midway_libretro",
        "MAME 2003 Midway (0.78)",
        "Arcade",
        "MAME",
    ),
    sw("mame2015_libretro", "MAME 2015 (0.160)", "Arcade", "MAME"),
    sw("mame2016_libretro", "MAME 2016 (0.174)", "Arcade", "GPLv2+"),
    sw("stella2023_libretro", "Stella 2023", "Atari 2600", "GPLv2"),
    sw("tia_libretro", "Tia", "Atari 2600", "GPLv2+"),
    sw("a5200_libretro", "a5200", "Atari 5200", "GPLv2"),
    sw("gearlynx_libretro", "Gearlynx", "Atari Lynx", "GPLv3"),
    sw("holani_libretro", "Holani", "Atari Lynx", "GPLv3"),
    sw(
        "jollycv_libretro",
        "JollyCV",
        "ColecoVision/CreatiVision/My Vision",
        "BSD-3-Clause, MIT",
    ),
    sw("amiberry_libretro", "Amiberry", "Commodore Amiga", "GPLv3"),
    sw("puae2021_libretro", "PUAE 2021", "Commodore Amiga", "GPLv2"),
    sw("vice_x128_libretro", "VICE x128", "Commodore C128", "GPLv2"),
    sw("frodo_libretro", "Frodo", "Commodore C64", "GPLv2"),
    sw("vice_x64_libretro", "VICE x64", "Commodore C64", "GPLv2"),
    sw(
        "vice_xscpu64_libretro",
        "VICE xscpu64",
        "Commodore C64 SuperCPU",
        "GPLv2",
    ),
    sw(
        "ep128emu_core_libretro",
        "ep128emu-core",
        "Enterprise 64/128",
        "GPLv2",
    ),
    sw(
        "geargrafx_libretro",
        "Geargrafx",
        "NEC PC Engine / SuperGrafx / CD",
        "GPLv3",
    ),
    sw(
        "desmume2015_libretro",
        "DeSmuME 2015",
        "Nintendo DS",
        "GPLv2",
    ),
    gl("melondsds_libretro", "melonDS DS", "Nintendo DS", "GPLv3+"),
    sw("noods_libretro", "NooDS", "Nintendo DS", "GPLv3"),
    sw(
        "fixgb_libretro",
        "fixGB",
        "Nintendo Game Boy / Color",
        "MIT",
    ),
    sw(
        "irogb_libretro",
        "IroGB",
        "Nintendo Game Boy / Color",
        "GPLv3",
    ),
    sw(
        "mednafen_gba_libretro",
        "Beetle GBA",
        "Nintendo Game Boy Advance",
        "GPLv2",
    ),
    sw(
        "meteor_libretro",
        "Meteor",
        "Nintendo Game Boy Advance",
        "GPLv3",
    ),
    sw(
        "vbam_libretro",
        "VBA-M",
        "Nintendo Game Boy Advance",
        "GPLv2",
    ),
    sw(
        "skyemu_libretro",
        "SkyEmu",
        "Nintendo Game Boy/GBA/NDS",
        "MIT",
    ),
    sw("fixnes_libretro", "fixNES", "Nintendo NES / Famicom", "MIT"),
    sw(
        "rustynes_libretro",
        "RustyNES",
        "Nintendo NES / Famicom",
        "GPLv3+",
    ),
    sw(
        "mednafen_snes_libretro",
        "Beetle bsnes",
        "Nintendo SNES / SFC",
        "GPLv2",
    ),
    sw(
        "mednafen_supafaust_libretro",
        "Beetle Supafaust",
        "Nintendo SNES / SFC",
        "GPLv2+",
    ),
    sw("bsnes_libretro", "bsnes", "Nintendo SNES / SFC", "GPLv3"),
    sw(
        "bsnes-jg_libretro",
        "bsnes-jg",
        "Nintendo SNES / SFC",
        "GPLv3",
    ),
    sw(
        "mesen-s_libretro",
        "Mesen-S",
        "SNES / Game Boy / Color",
        "GPLv3",
    ),
    sw(
        "bsnes2014_accuracy_libretro",
        "bsnes 2014 Accuracy",
        "Nintendo SNES / SFC",
        "GPLv3",
    ),
    sw(
        "bsnes2014_balanced_libretro",
        "bsnes 2014 Balanced",
        "Nintendo SNES / SFC",
        "GPLv3",
    ),
    sw(
        "bsnes2014_performance_libretro",
        "bsnes 2014 Performance",
        "Nintendo SNES / SFC",
        "GPLv3",
    ),
    sw(
        "bsnes_cplusplus98_libretro",
        "bsnes C++98 (v085)",
        "Nintendo SNES / SFC",
        "GPLv3",
    ),
    sw(
        "bsnes_hd_beta_libretro",
        "bsnes-hd beta",
        "Nintendo SNES / SFC",
        "GPLv3",
    ),
    sw(
        "bsnes_mercury_accuracy_libretro",
        "bsnes-mercury Accuracy",
        "Nintendo SNES / SFC",
        "GPLv3",
    ),
    sw(
        "nside_sfc_balanced_libretro",
        "nSide (Super Famicom Balanced)",
        "Nintendo SNES / SFC",
        "GPLv3",
    ),
    sw(
        "snes9x2002_libretro",
        "Snes9x 2002",
        "Nintendo SNES / SFC",
        "Non-commercial",
    ),
    sw(
        "snes9x2005_plus_libretro",
        "Snes9x 2005 Plus",
        "Nintendo SNES / SFC",
        "Non-commercial",
    ),
    sw(
        "cdi2015_libretro",
        "Philips CDi 2015",
        "Philips CDi",
        "GPLv2+",
    ),
    sw(
        "same_cdi_libretro",
        "SAME CDi (Git)",
        "Philips CDi",
        "GPLv2+",
    ),
    sw("clownmdemu_libretro", "ClownMDEmu", "Sega MD/CD", "AGPLv3"),
    sw("smsplus_libretro", "SMS Plus GX", "Sega MS/GG", "GPLv2"),
    sw(
        "genesis_plus_gx_wide_libretro",
        "Genesis Plus GX Wide",
        "Sega MS/GG/MD/CD",
        "Non-commercial",
    ),
    sw("ymir_libretro", "Emir", "Sega Saturn", "GPLv3"),
    gl(
        "yabasanshiro_libretro",
        "YabaSanshiro",
        "Sega Saturn",
        "GPLv2",
    ),
    sw("yabause_libretro", "Yabause", "Sega Saturn", "GPLv2"),
    sw(
        "race_libretro",
        "RACE",
        "SNK Neo Geo Pocket / Color",
        "GPLv2",
    ),
    sw("dosbox_libretro", "DOSBox", "DOS", "GPLv2"),
    sw("dosbox_core_libretro", "DOSBox-core", "DOS", "GPLv2"),
    sw("dosbox_svn_libretro", "DOSBox-SVN", "DOS", "GPLv2"),
];

pub fn find(core_id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|c| c.id == core_id)
}

/// Ids de sistema (os da varredura, `library_scan::systems`) que um core do
/// catálogo atende, tirados do texto `systems` dele. Serve pra escolher o
/// core de um jogo pelo SISTEMA antes da extensão — `.bin` é aceito por
/// dezenas de cores, e a ordem alfabética mandava jogo de Mega Drive pro
/// a5200. Core fora do catálogo → vazio.
pub fn system_ids(core_id: &str) -> Vec<&'static str> {
    find(core_id).map_or_else(Vec::new, |c| systems_from_text(c.systems))
}

fn systems_from_text(text: &str) -> Vec<&'static str> {
    let t = text.to_lowercase();
    // palavra inteira (separada por espaço, `/`, `(`…): "nes" não casa
    // "genesis", "md" não casa "mdx"
    let words: Vec<&str> = t
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| !w.is_empty())
        .collect();
    let word = |w: &str| words.contains(&w);
    let has = |s: &str| t.contains(s);
    // "Game Boy Advance" não é Game Boy
    let without_gba = t.replace("game boy advance", "");
    let mut out = Vec::new();
    let mut add = |cond: bool, id: &'static str| {
        if cond && !out.contains(&id) {
            out.push(id);
        }
    };
    add(has("snes") || has("super nintendo") || word("sfc"), "snes");
    add(word("nes") || has("famicom"), "nes");
    add(has("game boy advance") || word("gba"), "gba");
    let gb = without_gba.contains("game boy") || word("gb") || word("gbc");
    add(gb, "gb");
    add(gb, "gbc");
    add(has("nintendo ds") || word("nds"), "nds");
    add(has("virtual boy"), "vb");
    add(has("nintendo 64"), "n64");
    add(has("gamecube") || word("ngc"), "gamecube");
    add(
        has("mega drive") || has("genesis") || word("md"),
        "megadrive",
    );
    add(has("master system") || word("ms"), "mastersystem");
    add(has("game gear") || word("gg"), "gamegear");
    add(has("sg-1000"), "sg1000");
    add(
        has("sega") && (has("sega cd") || word("cd")) || has("mega-cd"),
        "segacd",
    );
    add(word("32x"), "sega32x");
    add(has("saturn"), "saturn");
    add(has("dreamcast"), "dreamcast");
    add(has("naomi"), "naomi");
    add(has("atomiswave"), "atomiswave");
    let pce = has("pc engine") || has("turbografx") || has("supergrafx");
    add(pce, "pcengine");
    add(pce && word("cd"), "pcenginecd");
    add(has("supergrafx"), "supergrafx");
    add(has("pc-fx"), "pcfx");
    add(has("playstation 2"), "ps2");
    add(has("playstation") && !has("playstation 2"), "psx");
    add(word("psp"), "psp");
    add(has("neo geo cd"), "neogeocd");
    add(has("neo geo pocket"), "ngp");
    add(
        has("arcade") || word("cps") || word("mame") || has("neo geo /"),
        "arcade",
    );
    add(has("atari 2600"), "atari2600");
    add(has("5200"), "atari5200");
    add(has("7800"), "atari7800");
    add(has("atari 8-bit"), "atari8bit");
    add(has("atari st"), "atarist");
    add(has("lynx"), "lynx");
    add(has("jaguar"), "jaguar");
    add(has("wonderswan"), "wonderswan");
    add(has("pokémon mini") || has("pokemon mini"), "pokemini");
    add(has("supervision"), "supervision");
    add(word("msx"), "msx");
    add(has("chip-8"), "chip8");
    add(word("arduboy"), "arduboy");
    add(word("uzebox"), "uzebox");
    add(word("vaporspec"), "vaporspec");
    add(word("gametank"), "gametank");
    add(has("mega duck") || has("cougar boy"), "megaduck");
    add(has("handheld electronic"), "handheld_electronic");
    add(has("arcadia 2001"), "arcadia");
    add(has("interton vc 4000"), "vc4000");
    add(has("elektor tv games computer"), "elektor");
    add(has("colecovision"), "coleco");
    add(has("commodore 64") || has("c64"), "c64");
    add(has("amiga"), "amiga");
    add(word("x68000"), "x68000");
    add(word("laserdisc"), "laserdisc");
    add(has("zx spectrum"), "zxspectrum");
    add(word("zx81") || has("zx 81"), "zx81");
    add(has("amstrad cpc"), "amstradcpc");
    add(word("dos") || has("ms-dos"), "dos");
    add(has("scummvm"), "scummvm");
    add(word("3do"), "3do");
    add(has("intellivision"), "intellivision");
    add(has("odyssey"), "odyssey2");
    add(has("vectrex"), "vectrex");
    add(word("cdi"), "cdi");
    out
}

pub(crate) fn buildbot_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "apple/osx"
    } else {
        "linux"
    }
}

pub(crate) fn dylib_ext() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

fn download_url(core_id: &str) -> String {
    format!(
        "https://buildbot.libretro.com/nightly/{}/x86_64/latest/{}.{}.zip",
        buildbot_os(),
        core_id,
        dylib_ext()
    )
}

/// Baixa o `<core_id>` do buildbot e extrai o dylib pra `cores_dir`.
/// Devolve o caminho instalado.
pub async fn download(cores_dir: &Path, core_id: &str) -> Result<PathBuf, CatalogError> {
    if find(core_id).is_none() {
        return Err(format!("`{core_id}` não está no catálogo").into());
    }
    let url = download_url(core_id);
    log::info!("baixando core: {url}");
    let resp = net::http::client()
        .get(&url)
        .send()
        .await
        .map_err(|e| CatalogError::http("download", e))?
        .error_for_status()
        .map_err(|e| CatalogError::http("download", e))?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| CatalogError::http("download", e))?;

    let cores_dir = cores_dir.to_path_buf();
    let so_name = format!("{core_id}.{}", dylib_ext());
    tokio::task::spawn_blocking(move || -> Result<PathBuf, CatalogError> {
        std::fs::create_dir_all(&cores_dir)?;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| CatalogError::Corrupt(format!("zip inválido: {e}")))?;
        let mut entry = zip
            .by_name(&so_name)
            .map_err(|_| CatalogError::Corrupt(format!("`{so_name}` não está no zip")))?;
        let tmp = cores_dir.join(format!("{so_name}.part"));
        let mut out = std::fs::File::create(&tmp)?;
        std::io::copy(&mut entry, &mut out)?;
        drop(out);
        install_keeping_previous(&cores_dir, &so_name, &tmp)
    })
    .await
    .map_err(|e| CatalogError::Other(e.to_string()))?
}

/// Pasta da versão anterior de cada core. A descoberta não entra em subpastas.
fn previous_dir(cores_dir: &Path) -> PathBuf {
    cores_dir.join("previous")
}

/// Põe `tmp` no lugar do core e guarda o que estava lá como versão anterior: o
/// buildbot só tem a última compilação, então é a única volta possível. Mover
/// funciona mesmo com a DLL carregada; sobrescrever, no Windows, não.
fn install_keeping_previous(
    cores_dir: &Path,
    so_name: &str,
    tmp: &Path,
) -> Result<PathBuf, CatalogError> {
    let dest = cores_dir.join(so_name);
    if !dest.is_file() {
        std::fs::rename(tmp, &dest)?;
        return Ok(dest);
    }
    let previous = previous_dir(cores_dir);
    std::fs::create_dir_all(&previous)?;
    let kept = previous.join(so_name);
    // a anterior da anterior não fica: uma cópia por core
    let aside = previous.join(format!("{so_name}.old"));
    let had_kept = kept.is_file();
    if had_kept {
        std::fs::rename(&kept, &aside)?;
    }
    let swapped = std::fs::rename(&dest, &kept).and_then(|()| std::fs::rename(tmp, &dest));
    if let Err(e) = swapped {
        // desfaz: o core instalado continua o que estava
        if !dest.is_file() {
            let _ = std::fs::rename(&kept, &dest);
        }
        if had_kept {
            let _ = std::fs::rename(&aside, &kept);
        }
        let _ = std::fs::remove_file(tmp);
        return Err(e.into());
    }
    let _ = std::fs::remove_file(&aside);
    Ok(dest)
}

/// Tem versão anterior guardada?
pub fn has_previous(cores_dir: &Path, core_id: &str) -> bool {
    previous_dir(cores_dir)
        .join(format!("{core_id}.{}", dylib_ext()))
        .is_file()
}

/// Volta para a versão anterior; a que estava instalada passa a ser a anterior.
pub fn restore_previous(cores_dir: &Path, core_id: &str) -> Result<(), CatalogError> {
    let so_name = format!("{core_id}.{}", dylib_ext());
    let kept = previous_dir(cores_dir).join(&so_name);
    if !kept.is_file() {
        return Err(CatalogError::NotFound("não há versão anterior".into()));
    }
    let tmp = cores_dir.join(format!("{so_name}.part"));
    std::fs::rename(&kept, &tmp)?;
    install_keeping_previous(cores_dir, &so_name, &tmp).map(|_| ())
}

/// Remove o dylib de um core instalado e a versão anterior dele.
pub fn remove(cores_dir: &Path, core_id: &str) -> Result<(), CatalogError> {
    let path = cores_dir.join(format!("{core_id}.{}", dylib_ext()));
    if !path.is_file() {
        return Err(CatalogError::NoCore("core não está instalado".into()));
    }
    std::fs::remove_file(&path)?;
    let _ =
        std::fs::remove_file(previous_dir(cores_dir).join(format!("{core_id}.{}", dylib_ext())));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_keeps_one_previous_and_restores() {
        let dir = std::env::temp_dir().join(format!("reemu-core-prev-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let so = format!("x_libretro.{}", dylib_ext());
        let install = |content: &str| {
            let tmp = dir.join("novo.part");
            std::fs::write(&tmp, content).unwrap();
            install_keeping_previous(&dir, &so, &tmp).unwrap();
        };
        let read = |p: PathBuf| std::fs::read_to_string(p).unwrap();

        install("v1");
        assert!(!has_previous(&dir, "x_libretro"));
        install("v2");
        install("v3");
        assert_eq!(read(dir.join(&so)), "v3");
        assert_eq!(read(previous_dir(&dir).join(&so)), "v2");
        assert_eq!(std::fs::read_dir(previous_dir(&dir)).unwrap().count(), 1);

        // voltar troca as duas, então dá para desfazer
        restore_previous(&dir, "x_libretro").unwrap();
        assert_eq!(read(dir.join(&so)), "v2");
        assert_eq!(read(previous_dir(&dir).join(&so)), "v3");

        remove(&dir, "x_libretro").unwrap();
        assert!(!has_previous(&dir, "x_libretro"));
        assert_eq!(
            restore_previous(&dir, "x_libretro")
                .unwrap_err()
                .to_string(),
            "não há versão anterior"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn catalog_ids_are_libretro_stems_and_unique() {
        let mut ids: Vec<&str> = CATALOG.iter().map(|c| c.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids duplicados no catálogo");
        assert!(CATALOG.iter().all(|c| c.id.ends_with("_libretro")));
        // nomes de sistema não-vazios
        assert!(CATALOG
            .iter()
            .all(|c| !c.systems.is_empty() && !c.name.is_empty()));
    }

    #[test]
    fn url_shape() {
        let u = download_url("fceumm_libretro");
        assert!(u.starts_with("https://buildbot.libretro.com/nightly/"));
        assert!(u.ends_with("/fceumm_libretro.so.zip") || u.ends_with("/fceumm_libretro.dll.zip"));
    }

    #[test]
    fn dolphin_maps_to_gamecube_opengl() {
        let entry = find("dolphin_libretro").expect("dolphin no catalogo");
        assert!(matches!(entry.hw, CoreHw::OpenGl));
        assert_eq!(system_ids("dolphin_libretro"), vec!["gamecube"]);
    }

    /// **Teste de fumaça do catálogo** (fora do `cargo test` normal —
    /// baixa ~1-2 GB). Pra cada core: baixa pelo mesmo `download` do app e
    /// abre com `reemu-core-host --probe` num processo à parte (core que
    /// cai no `retro_init` derruba só o probe). Roda no workflow
    /// `catalog-smoke.yml` (Linux e Windows); à mão:
    ///
    /// ```text
    /// cargo build -p core-host-desktop
    /// cargo test -p reemu-desktop --lib catalog_smoke -- --ignored --nocapture
    /// ```
    ///
    /// `REEMU_SMOKE_ONLY=fceumm_libretro,vbam_libretro` restringe a lista;
    /// `REEMU_CORE_HOST` aponta outro binário. `REEMU_HATARI_TOS` aponta
    /// o firmware local para a fumaça de Atari ST. `REEMU_PX68K_BIOS_DIR` aponta
    /// uma pasta com IPL e CGROM para o PX68k. Com `GITHUB_STEP_SUMMARY`
    /// escreve a tabela no resumo do job.
    /// Onde cada thread do probe travado está parada no kernel (Linux:
    /// `/proc/<pid>/task/*/{comm,wchan,syscall}`) — pra falha que só
    /// acontece no runner do CI dizer a causa sem precisar de gdb lá.
    fn hang_diagnostics(pid: u32) -> String {
        let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
            return String::new();
        };
        let read = |p: std::path::PathBuf| {
            std::fs::read_to_string(p)
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        let threads: Vec<String> = tasks
            .flatten()
            .map(|t| {
                let d = t.path();
                let syscall = read(d.join("syscall"));
                let nr = syscall.split(' ').next().unwrap_or("?").to_string();
                format!(
                    "{} (wchan {}, syscall {nr})",
                    read(d.join("comm")),
                    read(d.join("wchan"))
                )
            })
            .collect();
        format!(" — threads: {}", threads.join("; "))
    }

    #[test]
    fn system_ids_from_catalog_text() {
        let ids = |t| systems_from_text(t);
        assert_eq!(ids("NES / Famicom"), vec!["nes"]);
        assert_eq!(ids("Mega Drive / Genesis"), vec!["megadrive"]);
        assert_eq!(ids("Super Nintendo"), vec!["snes"]);
        assert_eq!(ids("Game Boy Advance / GB / GBC"), vec!["gba", "gb", "gbc"]);
        assert_eq!(ids("Nintendo Game Boy Advance"), vec!["gba"]);
        assert_eq!(ids("Game Boy / Color"), vec!["gb", "gbc"]);
        assert_eq!(system_ids("DoubleCherryGB_libretro"), vec!["gb", "gbc"]);
        assert_eq!(system_ids("hatari_libretro"), vec!["atarist"]);
        assert_eq!(system_ids("hatari2014_libretro"), vec!["atarist"]);
        assert_eq!(system_ids("hatarib_libretro"), vec!["atarist"]);
        assert_eq!(system_ids("px68k_libretro"), vec!["x68000"]);
        assert_eq!(system_ids("dirksimple_libretro"), vec!["laserdisc"]);
        assert_eq!(system_ids("hypseushw_libretro"), vec!["laserdisc"]);
        assert_eq!(system_ids("openmsx_libretro"), vec!["msx"]);
        assert_eq!(system_ids("jaxe_libretro"), vec!["chip8"]);
        assert_eq!(system_ids("ardens_libretro"), vec!["arduboy"]);
        assert_eq!(system_ids("arduous_libretro"), vec!["arduboy"]);
        assert_eq!(system_ids("uzem_libretro"), vec!["uzebox"]);
        assert_eq!(system_ids("vaporspec_libretro"), vec!["vaporspec"]);
        assert_eq!(system_ids("gametank_libretro"), vec!["gametank"]);
        assert_eq!(system_ids("sameduck_libretro"), vec!["megaduck"]);
        assert_eq!(system_ids("gw_libretro"), vec!["handheld_electronic"]);
        assert_eq!(
            system_ids("amiarcadia_libretro"),
            vec!["arcadia", "vc4000", "elektor"]
        );
        assert_eq!(system_ids("81_libretro"), vec!["zx81"]);
        assert_eq!(system_ids("mesen-s_libretro"), vec!["snes", "gb", "gbc"]);
        assert_eq!(
            system_ids("mesen2_libretro"),
            vec![
                "snes",
                "nes",
                "gba",
                "gb",
                "gbc",
                "mastersystem",
                "gamegear",
                "pcengine",
                "pcenginecd",
                "supergrafx",
                "wonderswan"
            ]
        );
        assert_eq!(
            ids("Mega Drive / Master System / Game Gear / SG-1000 / Sega CD"),
            vec!["megadrive", "mastersystem", "gamegear", "sg1000", "segacd"]
        );
        assert_eq!(ids("Sega MD/CD"), vec!["megadrive", "segacd"]);
        assert_eq!(ids("Atari 5200"), vec!["atari5200"]);
        assert_eq!(ids("Atari 2600"), vec!["atari2600"]);
        assert_eq!(ids("PlayStation"), vec!["psx"]);
        assert_eq!(ids("PlayStation 2"), vec!["ps2"]);
        assert_eq!(
            ids("Dreamcast / NAOMI / Atomiswave"),
            vec!["dreamcast", "naomi", "atomiswave"]
        );
        assert_eq!(
            ids("PC Engine / SuperGrafx / CD"),
            vec!["pcengine", "pcenginecd", "supergrafx"]
        );
        assert_eq!(ids("Neo Geo / CPS / arcade"), vec!["arcade"]);
        assert_eq!(system_ids("fbalpha2012_cps1_libretro"), vec!["arcade"]);
        // todo core do catálogo cai em pelo menos um sistema da varredura,
        // exceto os de sistemas que a varredura não tem (Doom, TIC-80,
        // Commodore 128, Enterprise)
        const SEM_SISTEMA: &[&str] = &[
            "prboom_libretro",
            "tic80_libretro",
            "vice_x128_libretro",
            "ep128emu_core_libretro",
        ];
        let sem: Vec<_> = CATALOG
            .iter()
            .filter(|c| !SEM_SISTEMA.contains(&c.id))
            .filter(|c| systems_from_text(c.systems).is_empty())
            .map(|c| (c.id, c.systems))
            .collect();
        assert!(sem.is_empty(), "sem sistema: {sem:?}");
    }

    /// Falhas conhecidas e explicadas: aparecem na tabela como
    /// "conhecido" e não derrubam o job. Tirar daqui quando resolver.
    const KNOWN_BROKEN: &[(&str, &str)] = &[(
        "ep128emu_core_libretro",
        "cai numa thread de emulação que o próprio core sobe no retro_init \
             (sem jogo, sem as ROMs opcionais do Enterprise) — binário sem símbolos",
    )];

    /// Só no Windows (no Linux rodam os 180 quadros). Desde 2026-09-28 a fumaça
    /// semanal ficava vermelha toda segunda por causa deles, e uma regressão
    /// real passaria batida (TASKS.md, "Stella trava no Windows na fumaça do
    /// catálogo"). Continuam na tabela como "conhecido"; a investigação segue
    /// no TASKS.
    const KNOWN_BROKEN_WINDOWS: &[(&str, &str)] = &[
        (
            "stella_libretro",
            "passa de 60 s rodando a ROM de teste só no Windows (desde o VFS v3, 2026-09-26)",
        ),
        (
            "stella2023_libretro",
            "passa de 60 s rodando a ROM de teste só no Windows (desde o VFS v3, 2026-09-26)",
        ),
    ];

    /// Motivo conhecido de falha de `core_id` neste sistema, se houver.
    fn known_broken(core_id: &str) -> Option<&'static str> {
        KNOWN_BROKEN
            .iter()
            .chain(if cfg!(windows) {
                KNOWN_BROKEN_WINDOWS
            } else {
                &[]
            })
            .find(|k| k.0 == core_id)
            .map(|k| k.1)
    }

    /// Roda o `reemu-core-host` com `args` (probe ou run) e espera até 60 s.
    /// Devolve o status (`None` = travou e foi morto), o stdout e, se travou,
    /// o diagnóstico das threads.
    fn run_host(
        host: &std::path::Path,
        core: &std::path::Path,
        args: &[&std::ffi::OsStr],
        out_path: &std::path::Path,
    ) -> (Option<std::process::ExitStatus>, String, String) {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};
        let mut cmd = Command::new(host);
        // mesmo tratamento do app pra core que pede pilha executável
        if let Some((k, v)) = core_loader_desktop::exec_stack_env(core) {
            cmd.env(k, v);
        }
        // stdout num ARQUIVO, não pipe: core que imprime muito enchia o pipe
        // (lido só no fim) e travava o processo
        let out_file = std::fs::File::create(out_path).expect("arquivo de saída");
        let mut child = cmd
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out_file))
            .stderr(Stdio::null())
            .spawn()
            .expect("rodar reemu-core-host");
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut hang_info = String::new();
        let status = loop {
            if let Some(st) = child.try_wait().unwrap() {
                break Some(st);
            }
            if Instant::now() > deadline {
                hang_info = hang_diagnostics(child.id());
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let out = std::fs::read(out_path)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let _ = std::fs::remove_file(out_path);
        (status, out, hang_info)
    }

    fn smoke_has_av_and_state(output: &str) -> bool {
        output.lines().any(|line| {
            let parts: Vec<_> = line.split('\t').collect();
            matches!(parts.as_slice(), ["REEMU_RUN_OK", "180", "180", audio, "ok"]
                if audio.parse::<usize>().is_ok_and(|n| n > 0))
        })
    }

    fn hatari_smoke(host: &Path, core: &Path, cores_dir: &Path, id: &str) -> String {
        use std::ffi::OsStr;
        use std::io::Write;

        let media = cores_dir.join(format!("{id}-media"));
        std::fs::create_dir_all(&media).expect("pasta de teste do Hatari");
        // Exercita o EmuTOS embutido no hatariB.
        if id != "hatarib_libretro" {
            let Some(tos) = std::env::var_os("REEMU_HATARI_TOS") else {
                return "boot de disquete pendente (REEMU_HATARI_TOS)".into();
            };
            std::fs::copy(tos, media.join("tos.img"))
                .expect("copiar o firmware de teste do Hatari");
        }
        let disk = media.join("smoke.st");
        let archive = media.join("smoke.zip");
        let image = crate::smoke_roms::atari_st_media();
        std::fs::write(&disk, &image).expect("gravar disquete Atari ST de teste");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        zip.start_file("smoke.st", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&image).unwrap();
        drop(zip.finish().unwrap());

        for path in [disk, archive] {
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new(id),
                path.as_os_str(),
                OsStr::new("360"),
                media.as_os_str(),
            ];
            let (status, output, detail) = run_host(host, core, &args, &media.join("run.txt"));
            let ran = output.lines().any(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                matches!(fields.as_slice(), ["REEMU_RUN_OK", "360", colored, audio, "ok" | "diferente"]
                    if colored.parse::<usize>().is_ok_and(|n| n > 0)
                        && audio.parse::<usize>().is_ok_and(|n| n > 0))
            });
            let booted = output.lines().any(|line| {
                matches!(
                    line,
                    "REEMU_RUN_CENTER\t238\t0\t238" | "REEMU_RUN_CENTER\t239\t0\t239"
                )
            });
            if !status.is_some_and(|s| s.success()) || !ran || !booted {
                return format!(
                    "FALHA: boot Atari ST ({}): {output}{detail}",
                    path.display()
                );
            }
        }
        "360 quadros com boot de .st e ZIP, áudio; save state restaura".into()
    }

    fn px68k_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;
        use std::io::Write;

        let Some(firmware) = std::env::var_os("REEMU_PX68K_BIOS_DIR").map(PathBuf::from) else {
            return "boot de disquete pendente (REEMU_PX68K_BIOS_DIR)".into();
        };
        let media = cores_dir.join("px68k-media");
        let keropi = media.join("keropi");
        std::fs::create_dir_all(&keropi).expect("pasta de teste do PX68k");
        for name in ["iplrom.dat", "cgrom.dat"] {
            std::fs::copy(firmware.join(name), keropi.join(name))
                .unwrap_or_else(|e| panic!("copiar {name} para a fumaça do PX68k: {e}"));
        }
        let (xdf, dim) = crate::smoke_roms::x68000_media();
        let xdf_path = media.join("boot.xdf");
        let dim_path = media.join("boot.dim");
        let zip_path = media.join("boot.zip");
        let playlist = media.join("boot.m3u");
        std::fs::write(&xdf_path, xdf).expect("gravar disquete XDF");
        std::fs::write(&dim_path, &dim).expect("gravar disquete DIM");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        zip.start_file("boot.dim", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&dim).unwrap();
        drop(zip.finish().unwrap());
        std::fs::write(&playlist, b"boot.dim\nboot.xdf\n").expect("gravar M3U");

        for path in [xdf_path, dim_path, zip_path, playlist] {
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("px68k_libretro"),
                path.as_os_str(),
                OsStr::new("360"),
                media.as_os_str(),
            ];
            let (status, output, detail) = run_host(host, core, &args, &media.join("run.txt"));
            let ran = output.lines().any(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                matches!(fields.as_slice(), ["REEMU_RUN_OK", "359" | "360", _, audio, "ok" | "diferente"]
                    if audio.parse::<usize>().is_ok_and(|n| n > 0))
            });
            let booted = output
                .lines()
                .any(|line| line == "REEMU_RUN_CENTER\t230\t0\t230");
            if !status.is_some_and(|s| s.success()) || !ran || !booted {
                return format!(
                    "FALHA: boot Sharp X68000 ({}): {output}{detail}",
                    path.display()
                );
            }
        }
        "359 quadros com boot de XDF, DIM, ZIP e M3U, áudio; save state restaura".into()
    }

    fn dirksimple_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;
        let Some(video) = std::env::var_os("REEMU_DIRKSIMPLE_OGV") else {
            return "vídeo de teste pendente (REEMU_DIRKSIMPLE_OGV)".into();
        };
        let system = cores_dir.join("dirksimple-system");
        let data = system.join("DirkSimple/data/games/reemu-smoke");
        std::fs::create_dir_all(&data).expect("criar pasta de teste do DirkSimple");
        std::fs::write(data.join("game.lua"), crate::smoke_roms::DIRKSIMPLE_LUA)
            .expect("gravar dados de teste do DirkSimple");
        for ext in ["ogv", "dirksimple"] {
            let media = cores_dir.join(format!("reemu-smoke.{ext}"));
            std::fs::copy(&video, &media).expect("copiar o vídeo de teste do DirkSimple");
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("dirksimple_libretro"),
                media.as_os_str(),
                OsStr::new("120"),
                system.as_os_str(),
                OsStr::new("--paced"),
            ];
            let (status, output, detail) =
                run_host(host, core, &args, &cores_dir.join("dirksimple.log"));
            let ran = output.lines().any(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                matches!(fields.as_slice(), ["REEMU_RUN_OK", "120", color, _, "ok" | "diferente"]
                    if color.parse::<u32>().is_ok_and(|n| n >= 60))
            });
            let advanced = output.lines().any(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                matches!(fields.as_slice(), ["REEMU_RUN_CENTER", red, green, blue]
                    if red.parse::<u8>().is_ok_and(|v| v > 220)
                    && green.parse::<u8>().is_ok_and(|v| v < 32)
                    && blue.parse::<u8>().is_ok_and(|v| v > 220))
            });
            if !status.is_some_and(|s| s.success())
                || !ran
                || !advanced
                || !output.lines().any(|line| line == "REEMU_RUN_AUDIBLE\ttrue")
            {
                return format!("FALHA: vídeo DirkSimple ({ext}): {output}{detail}");
            }
        }
        "120 quadros em OGV e DIRKSIMPLE, áudio e save state restaura".into()
    }

    fn hypseus_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;
        use std::io::Write;

        let Some(media) = std::env::var_os("REEMU_HYPSEUS_MEDIA_DIR").map(PathBuf::from) else {
            return "vídeo de teste pendente (REEMU_HYPSEUS_MEDIA_DIR)".into();
        };
        let root = cores_dir.join("Daphne");
        let frames = root.join("framefile");
        let roms = root.join("roms");
        let vldp = root.join("vldp/lair");
        for dir in [&frames, &roms, &vldp] {
            std::fs::create_dir_all(dir).expect("criar layout de teste Daphne");
        }
        for name in ["color.m2v", "color.ogg"] {
            std::fs::copy(media.join(name), vldp.join(name)).expect("copiar mídia de teste");
        }
        let framefile = frames.join("lair.txt");
        std::fs::write(&framefile, b"../vldp/lair\n0 color.m2v\n").unwrap();
        // Só na fixture, para carregar o programa original de teste em vez
        // dos bytes comerciais. CHIPS 2–4 são preenchimento de tamanho fixo.
        std::fs::write(roms.join("lair.commands"), b"-nocrc").unwrap();
        let rom_path = roms.join("lair.zip");
        let mut rom = zip::ZipWriter::new(std::fs::File::create(&rom_path).unwrap());
        let program = [
            0xf3, 0x31, 0xff, 0xa7, 0x3e, 0xfd, 0x32, 0x20, 0xe0, 0x18, 0xfe,
        ];
        for chip in 1..=4 {
            let mut data = [0; 8192];
            if chip == 1 {
                data[..program.len()].copy_from_slice(&program);
            }
            rom.start_file(
                format!("dl_f2_u{chip}.bin"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            rom.write_all(&data).unwrap();
        }
        rom.finish().unwrap();
        let system = root.join("system");
        std::fs::create_dir_all(&system).unwrap();
        for content in [&framefile, &rom_path] {
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("hypseushw_libretro"),
                content.as_os_str(),
                OsStr::new("120"),
                system.as_os_str(),
                OsStr::new("--paced"),
            ];
            let (status, output, detail) =
                run_host(host, core, &args, &cores_dir.join("hypseus.log"));
            let video = output.lines().any(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                matches!(fields.as_slice(), ["REEMU_RUN_CENTER", red, green, blue]
                    if red.parse::<u8>().is_ok_and(|r| r > 220)
                    && green.parse::<u8>().is_ok_and(|g| g < 35)
                    && blue.parse::<u8>().is_ok_and(|b| b > 220))
            });
            let audio = output.lines().any(|line| line == "REEMU_RUN_AUDIBLE\ttrue");
            if !status.is_some_and(|st| st.success()) || !video || !audio {
                return format!("FALHA: HypseusHW ({}) {output}{detail}", content.display());
            }
        }
        "120 quadros de emulação por framefile e ZIP, vídeo e áudio".into()
    }

    fn amiarcadia_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use crate::smoke_roms::{AMIARCADIA_BIN, AMIARCADIA_TVC};
        use std::ffi::OsStr;
        use std::io::Write;

        let system = cores_dir.join("amiarcadia-system");
        std::fs::create_dir_all(&system).unwrap();
        let bin = cores_dir.join("arcadia-smoke.bin");
        let tvc = cores_dir.join("elektor-smoke.tvc");
        let bin_zip = bin.with_extension("zip");
        let mixed_zip = tvc.with_extension("zip");
        std::fs::write(&bin, AMIARCADIA_BIN).unwrap();
        std::fs::write(&tvc, AMIARCADIA_TVC).unwrap();

        for (path, tvc_entry) in [(&bin_zip, false), (&mixed_zip, true)] {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("arcadia.bin", opts).unwrap();
            zip.write_all(AMIARCADIA_BIN).unwrap();
            if tvc_entry {
                zip.start_file("elektor.tvc", opts).unwrap();
                zip.write_all(AMIARCADIA_TVC).unwrap();
            }
            zip.finish().unwrap();
        }

        for (rom, expected_color) in [
            (bin, "0\t255\t255"),
            (bin_zip, "0\t255\t255"),
            (tvc, "156\t0\t0"),
            (mixed_zip, "156\t0\t0"),
        ] {
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("amiarcadia_libretro"),
                rom.as_os_str(),
                OsStr::new("180"),
                system.as_os_str(),
            ];
            let (status, output, detail) = run_host(host, core, &args, &system.join("run.txt"));
            if !status.is_some_and(|s| s.success())
                || !smoke_has_av_and_state(&output)
                || !output.lines().any(|line| line == "REEMU_RUN_AUDIBLE\ttrue")
                || !output
                    .lines()
                    .any(|line| line.strip_prefix("REEMU_RUN_CENTER\t") == Some(expected_color))
            {
                return format!("FALHA: AmiArcadia ({}): {output}{detail}", rom.display());
            }
        }
        "180 quadros de Arcadia e Elektor, mídia avulsa e ZIP, vídeo, áudio e save state ok".into()
    }

    fn eightyone_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;
        use std::io::Write;

        let system = cores_dir.join("zx81-system");
        std::fs::create_dir_all(&system).expect("criar pasta de sistema de teste do ZX81");
        let program = cores_dir.join("zx81-smoke.p");
        let archive = program.with_extension("zip");
        let image = crate::smoke_roms::zx81_program();
        std::fs::write(&program, &image).expect("gravar programa ZX81 de teste");

        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("pascal.p", opts).unwrap();
        zip.write_all(b"program example; begin end.").unwrap();
        zip.start_file("smoke.p", opts).unwrap();
        zip.write_all(&image).unwrap();
        zip.finish().unwrap();

        for path in [program, archive] {
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("81_libretro"),
                path.as_os_str(),
                OsStr::new("180"),
                system.as_os_str(),
            ];
            let (status, output, detail) = run_host(host, core, &args, &system.join("run.txt"));
            let booted = output
                .lines()
                .any(|line| line == "REEMU_RUN_CENTER\t0\t0\t0")
                && output
                    .lines()
                    .any(|line| line == "REEMU_RUN_SIZE\t320\t240");
            if !status.is_some_and(|s| s.success()) || !smoke_has_av_and_state(&output) || !booted {
                return format!("FALHA: boot ZX81 ({}): {output}{detail}", path.display());
            }
        }
        "180 quadros com programa ZX81 avulso e em ZIP, save state ok".into()
    }

    fn arduboy_smoke(host: &Path, core: &Path, cores_dir: &Path, id: &str) -> String {
        use std::ffi::OsStr;
        use std::io::Write;

        let media = cores_dir.join(format!("{id}-media"));
        std::fs::create_dir_all(&media).expect("criar pasta de teste do Arduboy");
        let hex = media.join("smoke.hex");
        let rom = crate::smoke_roms::ARDUBOY_HEX.as_bytes();
        std::fs::write(&hex, rom).expect("gravar programa AVR de teste");

        let mut paths = vec![hex];
        if id == "ardens_libretro" {
            let package = media.join("smoke.arduboy");
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&package).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("info.json", opts).unwrap();
            zip.write_all(br#"{"schemaVersion":2,"title":"ReEmu Smoke","binaries":[{"title":"ReEmu Smoke","filename":"smoke.hex","device":"Arduboy"}]}"#)
                .unwrap();
            zip.start_file("smoke.hex", opts).unwrap();
            zip.write_all(rom).unwrap();
            zip.finish().unwrap();
            paths.push(package);
        }

        for path in &paths {
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new(id),
                path.as_os_str(),
                OsStr::new("180"),
                media.as_os_str(),
            ];
            let (status, output, detail) = run_host(host, core, &args, &media.join("run.txt"));
            let booted = output.lines().any(|line| line == "REEMU_RUN_AUDIBLE\ttrue");
            if !status.is_some_and(|s| s.success()) || !smoke_has_av_and_state(&output) || !booted {
                return format!("FALHA: boot Arduboy ({}): {output}{detail}", path.display());
            }
        }
        let formats = if paths.len() == 2 {
            "HEX e pacote Arduboy"
        } else {
            "HEX"
        };
        format!("180 quadros com {formats}, áudio e save state ok")
    }

    fn uzem_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;
        let Some(rom) = std::env::var_os("REEMU_UZEM_ROM") else {
            return "boot de cartucho pendente (REEMU_UZEM_ROM)".into();
        };
        let args = [
            OsStr::new("--run"),
            cores_dir.as_os_str(),
            OsStr::new("uzem_libretro"),
            rom.as_os_str(),
            OsStr::new("180"),
            cores_dir.as_os_str(),
        ];
        let (status, output, detail) = run_host(host, core, &args, &cores_dir.join("uzem.log"));
        let ran = output.lines().any(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            matches!(fields.as_slice(), ["REEMU_RUN_OK", "180", video, _, "ok"]
                if video.parse::<u32>().is_ok_and(|frames| frames > 0))
        });
        if !status.is_some_and(|s| s.success())
            || !ran
            || !output.lines().any(|line| line == "REEMU_RUN_AUDIBLE\ttrue")
        {
            return format!("FALHA: boot Uzebox: {output}{detail}");
        }
        "180 quadros com cartucho UZE, áudio e save state ok".into()
    }

    fn vaporspec_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;

        let rom = cores_dir.join("smoke.vaporbin");
        std::fs::write(&rom, crate::smoke_roms::vaporspec_program())
            .expect("gravar o programa de teste do VaporSpec");
        let args = [
            OsStr::new("--run"),
            cores_dir.as_os_str(),
            OsStr::new("vaporspec_libretro"),
            rom.as_os_str(),
            OsStr::new("180"),
            cores_dir.as_os_str(),
        ];
        let (status, output, detail) =
            run_host(host, core, &args, &cores_dir.join("vaporspec.log"));
        let drew_red = output.lines().any(|line| {
            matches!(
                line,
                "REEMU_RUN_CENTER\t252\t0\t0" | "REEMU_RUN_CENTER\t255\t0\t0"
            )
        });
        if !status.is_some_and(|status| status.success())
            || !smoke_has_av_and_state(&output)
            || !drew_red
            || !output
                .lines()
                .any(|line| line == "REEMU_RUN_SIZE\t256\t192")
        {
            return format!("FALHA: boot VaporSpec: {output}{detail}");
        }
        "180 quadros com imagem e save state ok".into()
    }

    fn gametank_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;

        let Some(rom) = std::env::var_os("REEMU_GAMETANK_ROM") else {
            return "boot de cartucho pendente (REEMU_GAMETANK_ROM)".into();
        };
        let args = [
            OsStr::new("--run"),
            cores_dir.as_os_str(),
            OsStr::new("gametank_libretro"),
            rom.as_os_str(),
            OsStr::new("180"),
            cores_dir.as_os_str(),
        ];
        let (status, output, detail) = run_host(host, core, &args, &cores_dir.join("gametank.log"));
        if !status.is_some_and(|status| status.success())
            || !smoke_has_av_and_state(&output)
            || !output.lines().any(|line| line == "REEMU_RUN_AUDIBLE\ttrue")
        {
            return format!("FALHA: boot GameTank: {output}{detail}");
        }
        "180 quadros com imagem, áudio e save state ok".into()
    }

    fn sameduck_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;

        let game = crate::smoke_roms::megaduck_program();
        for ext in ["duck", "bin"] {
            let rom = cores_dir.join(format!("megaduck-smoke.{ext}"));
            std::fs::write(&rom, &game).expect("gravar o programa de teste do Mega Duck");
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("sameduck_libretro"),
                rom.as_os_str(),
                OsStr::new("180"),
                cores_dir.as_os_str(),
            ];
            let (status, output, detail) =
                run_host(host, core, &args, &cores_dir.join("sameduck.log"));
            if !status.is_some_and(|status| status.success())
                || !smoke_has_av_and_state(&output)
                || !output.lines().any(|line| line == "REEMU_RUN_AUDIBLE\ttrue")
                || !output
                    .lines()
                    .any(|line| line == "REEMU_RUN_CENTER\t170\t170\t170")
            {
                return format!("FALHA: boot do Mega Duck ({ext}): {output}{detail}");
            }
        }
        "180 quadros com DUCK e BIN, áudio e save state ok".into()
    }

    fn gw_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;

        let Some(content) = std::env::var_os("REEMU_GW_ROM") else {
            return "boot de simulador pendente (REEMU_GW_ROM)".into();
        };
        let args = [
            OsStr::new("--run"),
            cores_dir.as_os_str(),
            OsStr::new("gw_libretro"),
            content.as_os_str(),
            OsStr::new("180"),
            cores_dir.as_os_str(),
        ];
        let (status, output, detail) = run_host(host, core, &args, &cores_dir.join("gw.log"));
        let rendered = output.lines().any(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            matches!(fields.as_slice(), ["REEMU_RUN_OK", "180", "180", samples, "sem"]
                if samples.parse::<usize>().is_ok_and(|count| count > 0))
        });
        if !status.is_some_and(|status| status.success()) || !rendered {
            return format!("FALHA: boot Handheld Electronic: {output}{detail}");
        }
        "180 quadros com imagem, sem suporte a save state".into()
    }

    fn jaxe_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;
        use std::io::Write;

        let system = cores_dir.join("jaxe-system");
        std::fs::create_dir_all(&system).expect("criar pasta de sistema de teste do JAXE");
        let rom = crate::smoke_roms::CHIP8_ROM;
        let archive = cores_dir.join("jaxe-smoke.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        zip.start_file("jaxe-smoke.xo8", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(rom).unwrap();
        zip.finish().unwrap();

        for ext in ["ch8", "sc8", "xo8", "hc8", "zip"] {
            let content = archive.with_extension(ext);
            if ext != "zip" {
                std::fs::write(&content, rom).expect("gravar ROM de teste do JAXE");
            }
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("jaxe_libretro"),
                content.as_os_str(),
                OsStr::new("180"),
                system.as_os_str(),
            ];
            let (status, output, detail) = run_host(host, core, &args, &system.join("run.txt"));
            let booted = output
                .lines()
                .any(|line| line == "REEMU_RUN_CENTER\t255\t255\t255")
                && output.lines().any(|line| line == "REEMU_RUN_AUDIBLE\ttrue");
            if !status.is_some_and(|s| s.success()) || !smoke_has_av_and_state(&output) || !booted {
                return format!("FALHA: boot JAXE ({ext}): {output}{detail}");
            }
        }
        "180 quadros com CH8, SC8, XO8, HC8 e ZIP, áudio e save state ok".into()
    }

    fn openmsx_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;
        use std::io::Write;

        let media = cores_dir.join("openmsx-media");
        let system = media.join("system");
        std::fs::create_dir_all(&system).expect("criar pasta de sistema de teste do openMSX");
        let cart = media.join("smoke.mx2");
        let archive = media.join("smoke.zip");
        let image = crate::smoke_roms::msx_cartridge();
        std::fs::write(&cart, &image).expect("gravar cartucho MSX de teste");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        zip.start_file(
            "smoke.rom",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(&image).unwrap();
        zip.finish().unwrap();

        for content in [cart, archive] {
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("openmsx_libretro"),
                content.as_os_str(),
                OsStr::new("450"),
                system.as_os_str(),
            ];
            let (status, output, detail) = run_host(host, core, &args, &media.join("run.txt"));
            let ran = output.lines().any(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                matches!(
                    fields.as_slice(),
                    ["REEMU_RUN_OK", "450", _, _, "ok" | "diferente"]
                )
            });
            let booted = output.contains("REEMU_RUN_CENTER\t39\t39\t255")
                && output.contains("REEMU_RUN_AUDIBLE\ttrue");
            if !status.is_some_and(|s| s.success()) || !ran || !booted {
                return format!(
                    "FALHA: boot openMSX ({}): {output}{detail}",
                    content.display()
                );
            }
        }
        "450 quadros com cartucho MSX2 avulso e em ZIP, áudio; save state restaura".into()
    }

    fn mesen_sgb_smoke(host: &Path, core: &Path, cores_dir: &Path, id: &str) -> String {
        use std::ffi::OsStr;

        let media = cores_dir.join(format!("{id}-sgb"));
        std::fs::create_dir_all(&media).expect("pasta de teste do Super Game Boy");
        let (cart, firmware) = crate::smoke_roms::sgb2_media();
        let rom_path = media.join("smoke.gb");
        let firmware_path = media.join("SGB2.sfc");
        std::fs::write(&rom_path, cart).unwrap();
        std::fs::write(&firmware_path, &firmware).unwrap();
        let args = [
            OsStr::new("--run"),
            cores_dir.as_os_str(),
            OsStr::new(id),
            rom_path.as_os_str(),
            OsStr::new("180"),
            media.as_os_str(),
        ];
        let (status, output, detail) = run_host(host, core, &args, &media.join("run.txt"));
        if !status.is_some_and(|s| s.success())
            || !smoke_has_av_and_state(&output)
            || !output
                .lines()
                .any(|line| line == "REEMU_RUN_SIZE\t256\t239")
            || !output
                .lines()
                .any(|line| line == "REEMU_RUN_CENTER\t255\t0\t0")
        {
            return format!("FALHA: Super Game Boy 2: {output}{detail}");
        }
        if std::fs::read(firmware_path).as_deref().ok() != Some(firmware.as_slice()) {
            return "FALHA: Super Game Boy 2 deixou o firmware de teste alterado".into();
        }
        "Super Game Boy 2 com imagem, áudio e save state".into()
    }

    fn mesen2_disk_smoke(host: &Path, core: &Path, cores_dir: &Path) -> String {
        use std::ffi::OsStr;

        let media = cores_dir.join("mesen2-media");
        let system = media.join("system");
        let empty = media.join("empty");
        std::fs::create_dir_all(&system).expect("pasta de firmware de teste");
        std::fs::create_dir_all(&empty).expect("pasta de sistema vazia");
        let fds = media.join("smoke.fds");
        let cue = media.join("smoke.cue");
        let (disk, firmware) = crate::smoke_roms::fds_media();
        std::fs::write(&fds, disk).unwrap();
        std::fs::write(system.join("disksys.rom"), firmware).unwrap();
        std::fs::write(
            &cue,
            b"FILE \"track.bin\" BINARY\n  TRACK 01 MODE1/2048\n    INDEX 01 00:00:00\n",
        )
        .unwrap();
        std::fs::write(media.join("track.bin"), vec![0; 128 * 2048]).unwrap();
        let syscard = crate::smoke_roms::RomKind::PcEngine.build().repeat(32);
        std::fs::write(system.join("syscard3.pce"), &syscard).unwrap();

        let run = |rom: &Path, system: &Path| {
            let args = [
                OsStr::new("--run"),
                cores_dir.as_os_str(),
                OsStr::new("mesen2_libretro"),
                rom.as_os_str(),
                OsStr::new("180"),
                system.as_os_str(),
            ];
            run_host(host, core, &args, &media.join("run.txt"))
        };
        let (status, output, detail) = run(&cue, &empty);
        if status.and_then(|s| s.code()) != Some(2) || !output.contains("REEMU_RUN_ERR\t") {
            return format!("FALHA: PC Engine CD sem firmware: {output}{detail}");
        }
        let (status, output, detail) = run(&fds, &empty);
        if status.and_then(|s| s.code()) != Some(2)
            || !output
                .lines()
                .any(|line| line.starts_with("REEMU_RUN_ERR\t") && line.contains("disksys.rom"))
        {
            return format!("FALHA: FDS sem firmware: {output}{detail}");
        }
        for (name, rom) in [("FDS", &fds), ("PC Engine CD", &cue)] {
            let (status, output, detail) = run(rom, &system);
            if !status.is_some_and(|s| s.success()) || !smoke_has_av_and_state(&output) {
                return format!("FALHA: {name}: {output}{detail}");
            }
            if name == "FDS"
                && !output
                    .lines()
                    .any(|line| line == "REEMU_RUN_CENTER\t100\t176\t255")
            {
                return format!("FALHA: FDS não desenhou a cor do firmware de teste: {output}");
            }
        }
        if std::fs::read(system.join("syscard3.pce")).as_deref().ok() != Some(syscard.as_slice())
            || system
                .join("[BIOS] Super CD-ROM System (Japan) (v3.0).pce")
                .exists()
        {
            return "FALHA: PC Engine CD deixou o firmware de teste alterado".into();
        }
        "FDS e PC Engine CD com imagem, áudio e save state".into()
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "baixa o catálogo inteiro — rode com --ignored"]
    async fn catalog_smoke() {
        use std::fmt::Write as _;

        let host = std::env::var_os("REEMU_CORE_HOST")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // target/<perfil>/deps/<teste> → target/<perfil>/reemu-core-host
                let exe = std::env::current_exe().unwrap();
                let dir = exe.parent().unwrap().parent().unwrap();
                dir.join(format!("reemu-core-host{}", std::env::consts::EXE_SUFFIX))
            });
        assert!(
            host.is_file(),
            "{} não existe — rode `cargo build -p core-host-desktop` antes",
            host.display()
        );
        let only: Option<Vec<String>> = std::env::var("REEMU_SMOKE_ONLY")
            .ok()
            .map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
        let cores_dir = std::env::temp_dir().join(format!("reemu-smoke-{}", std::process::id()));

        // (core, sistemas, resultado do probe, detalhe, jogo)
        let mut rows: Vec<(&str, &str, &str, String, String)> = Vec::new();
        // (core, situação na tabela do aviso de atualização)
        let mut updates: Vec<(&str, String)> = Vec::new();
        for c in CATALOG {
            if only
                .as_ref()
                .is_some_and(|o| !o.iter().any(|id| id == c.id))
            {
                continue;
            }
            let path = match download(&cores_dir, c.id).await {
                Ok(p) => p,
                Err(e) => {
                    rows.push((c.id, c.systems, "download", e.to_string(), "—".into()));
                    continue;
                }
            };
            let out_path = cores_dir.join(format!("{}.probe.txt", c.id));
            let probe_args = [
                "--probe".as_ref(),
                cores_dir.as_os_str(),
                std::ffi::OsStr::new(c.id),
            ];
            let (status, out, hang_info) = run_host(&host, &path, &probe_args, &out_path);
            let line = out.lines().find(|l| l.starts_with("REEMU_PROBE_"));
            let probe_version = line
                .filter(|l| l.starts_with("REEMU_PROBE_OK"))
                .and_then(|l| l.split('\t').nth(2))
                .map(str::to_owned);
            if let Some(version) = &probe_version {
                if let Some(state) = crate::updates::table_check(c.id, version).await {
                    updates.push((c.id, state));
                }
            }
            let row = match (status, line) {
                (None, _) => ("travou", format!("mais de 60 s no retro_init{hang_info}")),
                (Some(st), Some(l)) if st.success() && l.starts_with("REEMU_PROBE_OK") => {
                    let f: Vec<&str> = l.split('\t').collect();
                    (
                        "ok",
                        format!("{} {}", f.get(1).unwrap_or(&""), f.get(2).unwrap_or(&"")),
                    )
                }
                (Some(_), Some(l)) => (
                    "erro",
                    l.trim_start_matches("REEMU_PROBE_ERR\t").to_string(),
                ),
                (Some(st), None) => ("caiu", format!("{st}")),
            };
            let row = match known_broken(c.id) {
                Some(why) if row.0 != "ok" => ("conhecido", format!("{}: {why}", row.1)),
                _ => row,
            };
            // Fase 2: com o core aberto, carrega uma ROM gerada do sistema
            // (`smoke_roms`) e roda alguns quadros.
            let game = match (row.0, crate::smoke_roms::RomKind::for_systems(c.systems)) {
                ("ok", Some(kind)) => {
                    let rom = cores_dir.join(format!("smoke.{}", kind.extension()));
                    std::fs::write(&rom, kind.build()).expect("gravar a ROM de teste");
                    let out_path = cores_dir.join(format!("{}.run.txt", c.id));
                    let run_args = [
                        "--run".as_ref(),
                        cores_dir.as_os_str(),
                        std::ffi::OsStr::new(c.id),
                        rom.as_os_str(),
                        std::ffi::OsStr::new("180"),
                    ];
                    let (status, out, hang) = run_host(&host, &path, &run_args, &out_path);
                    let line = out.lines().find(|l| l.starts_with("REEMU_RUN_"));
                    match (status, line) {
                        (None, _) => format!("FALHA: travou{hang}"),
                        (Some(_), Some(l)) if l.starts_with("REEMU_RUN_OK") => {
                            let f: Vec<&str> = l.split('\t').collect();
                            let n = |i: usize| f.get(i).and_then(|v| v.parse::<u64>().ok());
                            match (n(1), n(2), n(3)) {
                                (Some(0), _, _) => "FALHA: nenhum quadro de vídeo".into(),
                                (Some(v), Some(cor), Some(a)) => format!(
                                    "{v} quadros{}{}{}",
                                    if cor > 0 {
                                        ", com imagem"
                                    } else {
                                        ", tela preta"
                                    },
                                    if a > 0 { ", áudio" } else { "" },
                                    // save state: salvar, rodar, restaurar e
                                    // salvar de novo (`reemu-core-host --run`)
                                    match f.get(4).map(|s| s.trim()) {
                                        Some("ok") => "; save state ok",
                                        Some("diferente") => {
                                            "; save state restaura (estado não idêntico)"
                                        }
                                        Some("recusou") => "; save state recusado",
                                        Some("sem") => "; sem save state",
                                        _ => "",
                                    }
                                ),
                                _ => format!("FALHA: saída estranha: {l}"),
                            }
                        }
                        (Some(_), Some(l)) => {
                            format!("FALHA: {}", l.trim_start_matches("REEMU_RUN_ERR\t"))
                        }
                        (Some(st), None) => format!("FALHA: caiu ({st})"),
                    }
                }
                ("ok", None) => "sem ROM de teste".into(),
                _ => "—".into(),
            };
            let game = if c.id == "mesen2_libretro" && row.0 == "ok" && !game.starts_with("FALHA") {
                let disks = mesen2_disk_smoke(&host, &path, &cores_dir);
                if disks.starts_with("FALHA") {
                    disks
                } else {
                    format!("{game}; {disks}")
                }
            } else {
                game
            };
            let game = if matches!(c.id, "mesen2_libretro" | "mesen-s_libretro")
                && row.0 == "ok"
                && !game.starts_with("FALHA")
            {
                let sgb = mesen_sgb_smoke(&host, &path, &cores_dir, c.id);
                if sgb.starts_with("FALHA") {
                    sgb
                } else {
                    format!("{game}; {sgb}")
                }
            } else {
                game
            };
            let game = if matches!(
                c.id,
                "hatari_libretro" | "hatari2014_libretro" | "hatarib_libretro"
            ) && row.0 == "ok"
            {
                hatari_smoke(&host, &path, &cores_dir, c.id)
            } else {
                game
            };
            let game = if c.id == "px68k_libretro" && row.0 == "ok" {
                px68k_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "dirksimple_libretro" && row.0 == "ok" {
                dirksimple_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "hypseushw_libretro" && row.0 == "ok" {
                hypseus_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "openmsx_libretro" && row.0 == "ok" {
                openmsx_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "jaxe_libretro" && row.0 == "ok" {
                jaxe_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if matches!(c.id, "ardens_libretro" | "arduous_libretro") && row.0 == "ok" {
                arduboy_smoke(&host, &path, &cores_dir, c.id)
            } else {
                game
            };
            let game = if c.id == "uzem_libretro" && row.0 == "ok" {
                uzem_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "vaporspec_libretro" && row.0 == "ok" {
                vaporspec_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "gametank_libretro" && row.0 == "ok" {
                gametank_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "sameduck_libretro" && row.0 == "ok" {
                sameduck_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "gw_libretro" && row.0 == "ok" {
                gw_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "81_libretro" && row.0 == "ok" {
                eightyone_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = if c.id == "amiarcadia_libretro" && row.0 == "ok" {
                amiarcadia_smoke(&host, &path, &cores_dir)
            } else {
                game
            };
            let game = match known_broken(c.id) {
                Some(why) if game.starts_with("FALHA") => format!("conhecido: {why}"),
                _ => game,
            };
            rows.push((c.id, c.systems, row.0, row.1, game));
            // libera o disco (o MAME sozinho tem centenas de MB)
            let _ = std::fs::remove_file(&path);
        }
        let _ = std::fs::remove_dir_all(&cores_dir);

        let game_failed = |g: &str| g.starts_with("FALHA");
        let failed = rows
            .iter()
            .filter(|r| (r.2 != "ok" && r.2 != "conhecido") || game_failed(&r.4))
            .count();
        let ok = rows.iter().filter(|r| r.2 == "ok").count();
        let games = rows.iter().filter(|r| r.4.contains("quadros")).count();
        let states = rows
            .iter()
            .filter(|r| r.4.contains("save state ok") || r.4.contains("save state restaura"))
            .count();
        let mut md = format!(
            "## Fumaça do catálogo ({})\n\n{} de {} cores abriram; {} rodaram a ROM de teste; {} salvaram e restauraram o estado.\n\n| core | sistemas | resultado | detalhe | jogo |\n|---|---|---|---|---|\n",
            buildbot_os(),
            ok,
            rows.len(),
            games,
            states
        );
        // falhas primeiro
        rows.sort_by_key(|r| (r.2 == "ok" && !game_failed(&r.4), r.2 == "conhecido", r.0));
        for (id, sys, res, det, game) in &rows {
            let _ = writeln!(
                md,
                "| `{id}` | {sys} | {res} | {} | {} |",
                det.replace('|', "/"),
                game.replace('|', "/")
            );
        }
        let _ = writeln!(
            md,
            "\n## Aviso de atualização\n\n| core | situação |\n|---|---|"
        );
        updates.sort_by_key(|u| (!u.1.starts_with("FALHA"), u.0));
        for (id, state) in &updates {
            let _ = writeln!(md, "| `{id}` | {} |", state.replace('|', "/"));
        }
        let table_failed = updates.iter().filter(|u| u.1.starts_with("FALHA")).count();
        println!("{md}");
        if let Some(p) = std::env::var_os("GITHUB_STEP_SUMMARY") {
            let _ = std::fs::write(p, &md);
        }
        assert_eq!(
            failed, 0,
            "{failed} core(s) não abriram ou não rodaram a ROM de teste — ver a tabela acima"
        );
        assert_eq!(
            table_failed, 0,
            "{table_failed} core(s) com problema na tabela do aviso de atualização — ver acima"
        );
    }
}
