//! Download automático de bezels do **The Bezel Project**
//! (github.com/thebezelproject). Cada sistema tem um repo `bezelproject-<X>`
//! no formato *overlay do RetroArch* — `retroarch/overlay/GameBezels/<Sis>/`
//! com um `.png` (+ `.cfg`) por jogo, mais uma moldura de sistema solta.
//!
//! Estratégia: baixa o zip do branch `master`, extrai só a subárvore
//! `retroarch/overlay/` — remapeada pra uma estrutura que o
//! `library_scan::scan_decoration_pack` classifica sozinho — em
//! `<dados>/decorations/bezelproject/<system_id>/`, e re-importa a árvore
//! inteira como o (único) pack de decoração. Assim vários sistemas convivem
//! sob a mesma raiz sem mexer no schema "um pack ativo".

use std::io::Write;
use std::path::{Path, PathBuf};

use tauri::ipc::Channel;

use net::repo_version::{head_commit, installed_at, is_outdated, now_secs, refresh_latest, Commit};

/// `system_id` do ReEmu → repo `bezelproject-<X>` (família overlay/RetroArch).
/// Só sistemas que a família publica; PSP/PS2/DS não têm bezel (widescreen).
const CATALOG: &[(&str, &str)] = &[
    ("nes", "bezelproject-NES"),
    ("snes", "bezelproject-SNES"),
    ("n64", "bezelproject-N64"),
    ("gb", "bezelproject-GB"),
    ("gbc", "bezelproject-GBC"),
    ("gba", "bezelproject-GBA"),
    ("vb", "bezelproject-Virtualboy"),
    ("megadrive", "bezelproject-MegaDrive"),
    ("mastersystem", "bezelproject-MasterSystem"),
    ("gamegear", "bezelproject-GameGear"),
    ("sega32x", "bezelproject-Sega32X"),
    ("segacd", "bezelproject-SegaCD"),
    ("saturn", "bezelproject-Saturn"),
    ("dreamcast", "bezelproject-Dreamcast"),
    ("naomi", "bezelproject-Naomi"),
    ("atomiswave", "bezelproject-Atomiswave"),
    ("neogeocd", "bezelproject-NG-CD"),
    ("cdi", "bezelproject-CDiMono1"),
    ("psx", "bezelproject-PSX"),
    ("pcengine", "bezelproject-PCEngine"),
    ("pcenginecd", "bezelproject-PCE-CD"),
    ("pcfx", "bezelproject-PCFX"),
    ("atari2600", "bezelproject-Atari2600"),
    ("atari7800", "bezelproject-Atari7800"),
    ("lynx", "bezelproject-AtariLynx"),
    ("wonderswan", "bezelproject-WonderSwan"),
    ("ngp", "bezelproject-NGP"),
    ("coleco", "bezelproject-ColecoVision"),
    ("intellivision", "bezelproject-Intellivision"),
    ("3do", "bezelproject-3DO"),
    ("arcade", "bezelproject-MAME"),
];

pub fn repo_for(system_id: &str) -> Option<&'static str> {
    CATALOG
        .iter()
        .find(|(s, _)| *s == system_id)
        .map(|(_, r)| *r)
}

/// `<dados>/decorations/bezelproject` — raiz do pack acumulado.
pub fn pack_root(decorations_dir: &Path) -> PathBuf {
    decorations_dir.join("bezelproject")
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "BezelCatalogItem"))]
pub struct CatalogItem {
    pub system_id: String,
    /// `true` se já há `<pack_root>/<system_id>/` com conteúdo.
    pub installed: bool,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "BezelProgress"))]
pub struct DownloadProgress {
    pub system_id: String,
    pub received: u64,
    /// 0 se o servidor não mandou `Content-Length`.
    pub total: u64,
    /// `"download"` | `"extract"` | `"import"`.
    #[cfg_attr(test, ts(type = "'download' | 'extract' | 'import'"))]
    pub phase: &'static str,
}

/// Catálogo completo com o estado de cada sistema.
pub fn catalog(decorations_dir: &Path) -> Vec<CatalogItem> {
    let root = pack_root(decorations_dir);
    CATALOG
        .iter()
        .map(|(sid, _)| CatalogItem {
            system_id: (*sid).to_string(),
            installed: dir_has_files(&root.join(sid)),
        })
        .collect()
}

fn dir_has_files(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|mut rd| rd.next().is_some())
        .unwrap_or(false)
}

fn archive_url(repo: &str) -> String {
    format!("https://codeload.github.com/thebezelproject/{repo}/zip/refs/heads/master")
}

async fn fetch(url: &str) -> Result<reqwest::Response, String> {
    net::http::client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download: {e}"))
}

/// Baixa o pack de `system_id` e re-importa a árvore. Devolve o total de
/// atribuições de decoração gravadas (todos os sistemas baixados).
pub async fn download(
    decorations_dir: &Path,
    pool: &db::Db,
    system_id: &str,
    progress: Channel<DownloadProgress>,
) -> Result<usize, String> {
    let repo = repo_for(system_id)
        .ok_or_else(|| format!("`{system_id}` não tem bezel no The Bezel Project"))?;
    let root = pack_root(decorations_dir);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;

    // 0. o commit do `master` agora: é o que o zip vai trazer, e fica
    //    registrado pra saber depois se o repositório mudou. Sem a API (limite
    //    do GitHub, sem rede), o download segue sem registrar.
    let head = head_commit("thebezelproject", repo).await;
    if head.is_none() {
        log::warn!("bezel: commit de {repo} não consultado");
    }

    // 1. baixa o zip pra um arquivo temporário (packs passam de 500 MB — não
    //    dá pra segurar tudo na RAM como o slang-shaders).
    let url = archive_url(repo);
    log::info!("bezel: baixando {url}");
    let mut resp = fetch(&url).await?;
    let total = resp.content_length().unwrap_or(0);
    let zip_tmp = root.join(format!(".{system_id}.zip.part"));
    let mut file = std::fs::File::create(&zip_tmp).map_err(|e| e.to_string())?;
    let mut received = 0u64;
    let mut last = std::time::Instant::now();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("download: {e}"))? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        received += chunk.len() as u64;
        if last.elapsed().as_millis() >= 200 {
            let _ = progress.send(DownloadProgress {
                system_id: system_id.to_string(),
                received,
                total,
                phase: "download",
            });
            last = std::time::Instant::now();
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    let _ = progress.send(DownloadProgress {
        system_id: system_id.to_string(),
        received,
        total,
        phase: "extract",
    });

    // 2. extrai `retroarch/overlay/**` remapeado pra `<root>/<system_id>/`.
    let dest = root.join(system_id);
    let sid = system_id.to_string();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let f = std::fs::File::open(&zip_tmp).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(f).map_err(|e| format!("zip inválido: {e}"))?;
        let tmp = dest.with_extension("part");
        let _ = std::fs::remove_dir_all(&tmp);
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            // pasta ou link simbólico: nada a gravar
            if !entry.is_file() {
                continue;
            }
            let Some(name) = entry.enclosed_name() else {
                continue;
            };
            let Some(rel) = remap_entry(name.as_ref(), &sid) else {
                continue;
            };
            let out = tmp.join(&rel);
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut w).map_err(|e| e.to_string())?;
        }
        let _ = std::fs::remove_file(&zip_tmp);
        std::fs::remove_dir_all(&dest).ok();
        std::fs::rename(&tmp, &dest).map_err(|e| format!("mover pra {}: {e}", dest.display()))?;
        log::info!("bezel: pack de {sid} extraído em {}", dest.display());
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    if let Some(head) = head {
        let mut v = Versions::load(&root);
        v.installed.insert(system_id.to_string(), head.clone());
        v.latest.insert(system_id.to_string(), head);
        v.save(&root);
    }

    // 3. re-importa a árvore inteira (todos os sistemas já baixados).
    let _ = progress.send(DownloadProgress {
        system_id: system_id.to_string(),
        received,
        total,
        phase: "import",
    });
    crate::decoration::import_pack(pool, &root).await
}

/// `<pack_root>/.versions.json`: o commit de cada sistema baixado e a última
/// consulta ao GitHub. O scan de decorações só lê `.png`, então o arquivo não
/// atrapalha a importação.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Versions {
    #[serde(default)]
    installed: std::collections::BTreeMap<String, Commit>,
    #[serde(default)]
    latest: std::collections::BTreeMap<String, Commit>,
    /// Epoch em segundos da última consulta (todas as instaladas).
    #[serde(default)]
    checked_at: u64,
}

impl Versions {
    fn path(root: &Path) -> PathBuf {
        root.join(".versions.json")
    }
    fn load(root: &Path) -> Self {
        std::fs::read(Self::path(root))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    fn save(&self, root: &Path) {
        if let Ok(b) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(Self::path(root), b);
        }
    }
}

/// A API do GitHub sem login aceita 60 consultas por hora por IP; com o
/// resultado guardado por 6 h, abrir Molduras não gasta consultas.
const CHECK_EVERY_SECS: u64 = 6 * 60 * 60;

/// Sistemas baixados com versão nova no The Bezel Project. Consulta o GitHub
/// no máximo a cada 6 h (`force` ignora o prazo); sem rede, usa a última
/// consulta guardada.
pub async fn updates(decorations_dir: &Path, force: bool) -> Vec<String> {
    let root = pack_root(decorations_dir);
    let mut v = Versions::load(&root);
    let installed: Vec<(&str, &str)> = CATALOG
        .iter()
        .filter(|(sid, _)| dir_has_files(&root.join(sid)))
        .map(|(s, r)| (*s, *r))
        .collect();
    if force || now_secs().saturating_sub(v.checked_at) >= CHECK_EVERY_SECS {
        let mut ok = true;
        for (sid, repo) in &installed {
            let got = refresh_latest(
                "thebezelproject",
                repo,
                v.installed.get(*sid),
                v.latest.get(*sid),
            )
            .await;
            match got {
                Some(c) => {
                    v.latest.insert((*sid).to_string(), c);
                }
                None => {
                    log::warn!("bezel: commit de {repo} não consultado");
                    ok = false;
                }
            }
        }
        if ok {
            v.checked_at = now_secs();
        }
        v.save(&root);
    }
    installed
        .iter()
        .filter(|(sid, _)| {
            v.latest.get(*sid).is_some_and(|latest| {
                let dir_date = installed_at(&root.join(sid));
                is_outdated(v.installed.get(*sid), latest, dir_date.as_deref())
            })
        })
        .map(|(sid, _)| (*sid).to_string())
        .collect()
}

/// Traduz uma entrada do zip do Bezel Project pra o caminho relativo dentro de
/// `<pack_root>/`. Só `retroarch/overlay/**`; devolve `None` pro resto.
///
/// - `…/retroarch/overlay/GameBezels/<qualquer>/<...>` → `<sid>/GameBezels/<sid>/<...>`
/// - `…/retroarch/overlay/<Sistema>.png|.cfg` (arquivo solto) → `<sid>/<sid>.<ext>`
/// - qualquer outra coisa sob `overlay/` → `<sid>/<resto>`
fn remap_entry(name: &Path, sid: &str) -> Option<PathBuf> {
    let comps: Vec<&std::ffi::OsStr> = name.iter().collect();
    // pula o dir raiz do zip (`bezelproject-X-master/`)
    let after_root = comps.get(1..)?;
    let rest = match after_root {
        [a, b, tail @ ..]
            if a.eq_ignore_ascii_case("retroarch") && b.eq_ignore_ascii_case("overlay") =>
        {
            tail
        }
        _ => return None,
    };
    match rest {
        [] => None,
        // moldura de sistema solta
        [file] => {
            let ext = Path::new(file).extension()?.to_str()?.to_ascii_lowercase();
            (ext == "png" || ext == "cfg").then(|| PathBuf::from(sid).join(format!("{sid}.{ext}")))
        }
        // GameBezels/<qualquer>/<...>  → normaliza a pasta de sistema pro sid
        [g, _sysdir, tail @ ..] if g.eq_ignore_ascii_case("gamebezels") && !tail.is_empty() => {
            let mut p = PathBuf::from(sid).join("GameBezels").join(sid);
            for c in tail {
                p.push(c);
            }
            Some(p)
        }
        _ => {
            let mut p = PathBuf::from(sid);
            for c in rest {
                p.push(c);
            }
            Some(p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_maps_reemu_ids() {
        assert_eq!(repo_for("snes"), Some("bezelproject-SNES"));
        assert_eq!(repo_for("arcade"), Some("bezelproject-MAME"));
        assert_eq!(repo_for("psp"), None);
    }

    #[test]
    fn remaps_game_bezel_to_system_folder() {
        let p = Path::new("bezelproject-SNES-master/retroarch/overlay/GameBezels/SNES/Super Mario World (USA).png");
        assert_eq!(
            remap_entry(p, "snes").unwrap(),
            PathBuf::from("snes/GameBezels/snes/Super Mario World (USA).png")
        );
    }

    #[test]
    fn remaps_loose_system_frame_to_default_name() {
        let p = Path::new(
            "bezelproject-SNES-master/retroarch/overlay/Super-Nintendo-Entertainment-System.png",
        );
        assert_eq!(
            remap_entry(p, "snes").unwrap(),
            PathBuf::from("snes/snes.png")
        );
    }

    #[test]
    fn ignores_non_overlay_paths() {
        let p = Path::new("bezelproject-SNES-master/retroarch/config/Snes9x/Foo.cfg");
        assert!(remap_entry(p, "snes").is_none());
        let p2 = Path::new("bezelproject-SNES-master/README.md");
        assert!(remap_entry(p2, "snes").is_none());
    }
}
