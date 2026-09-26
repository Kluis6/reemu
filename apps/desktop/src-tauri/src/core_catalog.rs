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
    sw("gearcoleco_libretro", "Gearcoleco", "ColecoVision", "GPLv3"),
    sw("vice_x64sc_libretro", "VICE x64sc", "Commodore 64", "GPLv2"),
    sw("puae_libretro", "PUAE", "Commodore Amiga", "GPLv2"),
    sw("fuse_libretro", "Fuse", "ZX Spectrum", "GPLv3"),
    sw("cap32_libretro", "Caprice32", "Amstrad CPC", "GPLv2"),
    sw("dosbox_pure_libretro", "DOSBox-Pure", "MS-DOS", "GPLv2"),
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
    add(has("lynx"), "lynx");
    add(has("jaguar"), "jaguar");
    add(has("wonderswan"), "wonderswan");
    add(has("pokémon mini") || has("pokemon mini"), "pokemini");
    add(has("supervision"), "supervision");
    add(word("msx"), "msx");
    add(has("colecovision"), "coleco");
    add(has("commodore 64") || has("c64"), "c64");
    add(has("amiga"), "amiga");
    add(has("zx spectrum"), "zxspectrum");
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

fn buildbot_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "apple/osx"
    } else {
        "linux"
    }
}

fn dylib_ext() -> &'static str {
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
pub async fn download(cores_dir: &Path, core_id: &str) -> Result<PathBuf, String> {
    if find(core_id).is_none() {
        return Err(format!("`{core_id}` não está no catálogo"));
    }
    let url = download_url(core_id);
    log::info!("baixando core: {url}");
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("download: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download: {e}"))?;
    let bytes = resp.bytes().await.map_err(|e| format!("download: {e}"))?;

    let cores_dir = cores_dir.to_path_buf();
    let so_name = format!("{core_id}.{}", dylib_ext());
    tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf, String> {
        std::fs::create_dir_all(&cores_dir).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| format!("zip inválido: {e}"))?;
        let mut entry = zip
            .by_name(&so_name)
            .map_err(|_| format!("`{so_name}` não está no zip"))?;
        let dest = cores_dir.join(&so_name);
        let tmp = cores_dir.join(format!("{so_name}.part"));
        let mut out = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        drop(out);
        std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
        Ok(dest)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Remove o dylib de um core instalado.
pub fn remove(cores_dir: &Path, core_id: &str) -> Result<(), String> {
    let path = cores_dir.join(format!("{core_id}.{}", dylib_ext()));
    if !path.is_file() {
        return Err("core não está instalado".into());
    }
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    /// `REEMU_CORE_HOST` aponta outro binário. Com `GITHUB_STEP_SUMMARY`
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
            vec!["pcengine", "pcenginecd"]
        );
        assert_eq!(ids("Neo Geo / CPS / arcade"), vec!["arcade"]);
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
                    rows.push((c.id, c.systems, "download", e, "—".into()));
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
            let row = match KNOWN_BROKEN.iter().find(|k| k.0 == c.id) {
                Some((_, why)) if row.0 != "ok" => ("conhecido", format!("{}: {why}", row.1)),
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
                                    "{v} quadros{}{}",
                                    if cor > 0 {
                                        ", com imagem"
                                    } else {
                                        ", tela preta"
                                    },
                                    if a > 0 { ", áudio" } else { "" }
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
            let game = match KNOWN_BROKEN.iter().find(|k| k.0 == c.id) {
                Some((_, why)) if game.starts_with("FALHA") => format!("conhecido: {why}"),
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
        let mut md = format!(
            "## Fumaça do catálogo ({})\n\n{} de {} cores abriram; {} rodaram a ROM de teste.\n\n| core | sistemas | resultado | detalhe | jogo |\n|---|---|---|---|---|\n",
            buildbot_os(),
            ok,
            rows.len(),
            games
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
        println!("{md}");
        if let Some(p) = std::env::var_os("GITHUB_STEP_SUMMARY") {
            let _ = std::fs::write(p, &md);
        }
        assert_eq!(
            failed, 0,
            "{failed} core(s) não abriram ou não rodaram a ROM de teste — ver a tabela acima"
        );
    }
}
