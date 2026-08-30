//! Catálogo de cores libretro (MVP da etapa 10): uma lista curada de cores
//! **software** (sem exigir contexto GL) baixados do buildbot oficial.
//!
//! O buildbot serve `<stem>.so.zip` em
//! `https://buildbot.libretro.com/nightly/<os>/<arch>/latest/`. Baixar =
//! pegar o zip, extrair o `.so` pra `<dados>/cores/`. `<stem>` (ex:
//! `fceumm_libretro`) é o mesmo id que `discover_cores` e `load_game` usam.

use std::path::{Path, PathBuf};

pub struct CatalogEntry {
    /// Stem do arquivo, sem `.so` (ex: `fceumm_libretro`).
    pub id: &'static str,
    pub name: &'static str,
    pub systems: &'static str,
    pub license: &'static str,
}

/// Só cores que rodam em software (o caminho GL ainda não existe — etapa 02
/// passo 4).
pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        id: "fceumm_libretro",
        name: "FCEUmm",
        systems: "NES / Famicom",
        license: "GPLv2",
    },
    CatalogEntry {
        id: "nestopia_libretro",
        name: "Nestopia UE",
        systems: "NES / Famicom",
        license: "GPLv2",
    },
    CatalogEntry {
        id: "snes9x_libretro",
        name: "Snes9x",
        systems: "Super Nintendo",
        license: "Non-commercial",
    },
    CatalogEntry {
        id: "snes9x2010_libretro",
        name: "Snes9x 2010",
        systems: "Super Nintendo",
        license: "Non-commercial",
    },
    CatalogEntry {
        id: "mesen_libretro",
        name: "Mesen",
        systems: "NES / Famicom",
        license: "GPLv3",
    },
    CatalogEntry {
        id: "gambatte_libretro",
        name: "Gambatte",
        systems: "Game Boy / Color",
        license: "GPLv2",
    },
    CatalogEntry {
        id: "mgba_libretro",
        name: "mGBA",
        systems: "Game Boy Advance / GB / GBC",
        license: "MPL-2.0",
    },
    CatalogEntry {
        id: "genesis_plus_gx_libretro",
        name: "Genesis Plus GX",
        systems: "Mega Drive / Master System / Game Gear / SG-1000",
        license: "Non-commercial",
    },
    CatalogEntry {
        id: "picodrive_libretro",
        name: "PicoDrive",
        systems: "Mega Drive / 32X / Sega CD",
        license: "MAME-like",
    },
    CatalogEntry {
        id: "mednafen_pce_fast_libretro",
        name: "Beetle PCE Fast",
        systems: "PC Engine / TurboGrafx-16",
        license: "GPLv2",
    },
    CatalogEntry {
        id: "mednafen_wswan_libretro",
        name: "Beetle WonderSwan",
        systems: "WonderSwan / Color",
        license: "GPLv2",
    },
    CatalogEntry {
        id: "mednafen_ngp_libretro",
        name: "Beetle NeoPop",
        systems: "Neo Geo Pocket / Color",
        license: "GPLv2",
    },
    CatalogEntry {
        id: "stella2014_libretro",
        name: "Stella 2014",
        systems: "Atari 2600",
        license: "GPLv2",
    },
    CatalogEntry {
        id: "prosystem_libretro",
        name: "ProSystem",
        systems: "Atari 7800",
        license: "GPLv2",
    },
    CatalogEntry {
        id: "handy_libretro",
        name: "Handy",
        systems: "Atari Lynx",
        license: "Zlib",
    },
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

/// Baixa o `<core_id>` do buildbot e extrai o `.so` pra `cores_dir`.
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

/// Remove o `.so` de um core instalado.
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
    }

    #[test]
    fn url_shape() {
        let u = download_url("fceumm_libretro");
        assert!(u.starts_with("https://buildbot.libretro.com/nightly/"));
        assert!(u.ends_with("/fceumm_libretro.so.zip") || u.ends_with("/fceumm_libretro.dll.zip"));
    }
}
