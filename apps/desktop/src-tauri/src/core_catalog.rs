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
//!   (etapa 02 passo 4) e traz o frame por readback (ou interop dma_buf com
//!   `REEMU_GL_INTEROP=1`). Precisa de `libEGL` + GPU.
//!
//! Cores exclusivamente Vulkan ficam de fora até a etapa 12.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CoreHw {
    Software,
    OpenGl,
}

impl CoreHw {
    pub fn as_str(self) -> &'static str {
        match self {
            CoreHw::Software => "software",
            CoreHw::OpenGl => "opengl",
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
    gl(
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
];

pub fn find(core_id: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|c| c.id == core_id)
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
}
