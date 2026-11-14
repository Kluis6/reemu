//! Downloader do pacote de slang shaders — os mesmos que o RetroArch usa.
//! Espelha o `catalog.rs` da crate `core-catalog`: baixa o zip, extrai pra
//! `<dados>/shaders/slang-shaders/` e a `ShaderLibrary` aponta pra lá.
//!
//! Fonte primária: `shaders_slang.zip` do buildbot da libretro (é o que o
//! "Online Updater → Update Slang Shaders" do RetroArch baixa — pré-empacotado,
//! sempre atual, sem dir raiz). Fallback: o zip do repo no GitHub, que vem com
//! um dir raiz `slang-shaders-master/`. A extração detecta e tira qualquer
//! prefixo de dir raiz único. Troca atômica-ish: extrai num `.part` e renomeia.

use std::path::{Path, PathBuf};
use tauri::ipc::Channel;

use net::repo_version::{head_commit, installed_at, is_outdated, now_secs, refresh_latest, Commit};

const ARCHIVE_URL: &str = "https://buildbot.libretro.com/assets/frontend/shaders_slang.zip";
/// Usado se o buildbot estiver fora do ar. Vem com dir raiz `slang-shaders-master/`.
const FALLBACK_URL: &str =
    "https://codeload.github.com/libretro/slang-shaders/zip/refs/heads/master";

/// `<dados>/shaders/slang-shaders`.
pub fn install_dir(shaders_dir: &Path) -> PathBuf {
    shaders_dir.join("slang-shaders")
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "ShaderPackProgress"))]
pub struct DownloadProgress {
    pub received: u64,
    /// 0 se o servidor não mandou `Content-Length`.
    pub total: u64,
    /// `"download"` | `"extract"`.
    #[cfg_attr(test, ts(type = "'download' | 'extract'"))]
    pub phase: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "ShaderPackStatus"))]
pub struct PackStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub preset_count: usize,
}

pub fn status(shaders_dir: &Path) -> PackStatus {
    let dir = install_dir(shaders_dir);
    if !dir.is_dir() {
        return PackStatus {
            installed: false,
            path: None,
            preset_count: 0,
        };
    }
    PackStatus {
        preset_count: count_slangp(&dir),
        path: Some(dir.to_string_lossy().into_owned()),
        installed: true,
    }
}

fn count_slangp(dir: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|x| x.to_str()) == Some("slangp") {
                n += 1;
            }
        }
    }
    n
}

/// Baixa e extrai o pacote. Emite `DownloadProgress` durante o download.
pub async fn download(
    shaders_dir: &Path,
    progress: Channel<DownloadProgress>,
) -> Result<PathBuf, String> {
    // o commit do `master` agora, registrado pra saber depois se o repositório
    // mudou. O zip do buildbot é refeito a partir do repositório (todo dia); o
    // do GitHub é o próprio `master`. Sem a API, o download segue sem registrar.
    let head = head_commit("libretro", "slang-shaders").await;
    if head.is_none() {
        log::warn!("shaders: commit do slang-shaders não consultado");
    }
    let mut resp = match fetch(ARCHIVE_URL).await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("buildbot de shaders falhou ({e}) — tentando o GitHub");
            fetch(FALLBACK_URL).await?
        }
    };
    let total = resp.content_length().unwrap_or(0);
    let mut bytes: Vec<u8> = Vec::with_capacity(total.max(16 * 1024 * 1024) as usize);
    let mut last = std::time::Instant::now();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("download: {e}"))? {
        bytes.extend_from_slice(&chunk);
        if last.elapsed().as_millis() >= 150 {
            let _ = progress.send(DownloadProgress {
                received: bytes.len() as u64,
                total,
                phase: "download",
            });
            last = std::time::Instant::now();
        }
    }
    let _ = progress.send(DownloadProgress {
        received: bytes.len() as u64,
        total,
        phase: "extract",
    });

    let dest = install_dir(shaders_dir);
    let path = tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf, String> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| format!("zip inválido: {e}"))?;
        let tmp = dest.with_extension("part");
        let _ = std::fs::remove_dir_all(&tmp);
        // O zip do buildbot extrai direto no conteúdo (`crt/`, `bezel/`…); o do
        // GitHub embrulha tudo num `slang-shaders-master/`. Detecta se TODAS as
        // entradas dividem o mesmo 1º componente e, se sim, tira esse prefixo.
        let strip_root = zip_common_root(&mut zip);
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            // link simbólico de dentro do zip não vira arquivo aqui
            if f.is_symlink() {
                continue;
            }
            let Some(name) = f.enclosed_name() else {
                continue;
            };
            let rel: PathBuf = if strip_root {
                name.components().skip(1).collect()
            } else {
                name
            };
            if rel.as_os_str().is_empty() {
                continue;
            }
            let out = tmp.join(&rel);
            if f.is_dir() {
                std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            } else {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
                std::io::copy(&mut f, &mut w).map_err(|e| e.to_string())?;
            }
        }
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(dest.parent().unwrap_or(Path::new("/")))
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &dest).map_err(|e| format!("mover pra {}: {e}", dest.display()))?;
        log::info!("pacote de shaders extraído em {}", dest.display());
        Ok(dest)
    })
    .await
    .map_err(|e| e.to_string())??;
    if let Some(head) = head {
        let v = Version {
            installed: Some(head.clone()),
            latest: Some(head),
            checked_at: now_secs(),
        };
        v.save(shaders_dir);
    }
    Ok(path)
}

/// `<dados>/shaders/slang-shaders.version.json`: fora da pasta do pacote, que
/// é apagada e trocada inteira a cada download.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Version {
    #[serde(default)]
    installed: Option<Commit>,
    #[serde(default)]
    latest: Option<Commit>,
    /// Epoch em segundos da última consulta.
    #[serde(default)]
    checked_at: u64,
}

impl Version {
    fn path(shaders_dir: &Path) -> PathBuf {
        shaders_dir.join("slang-shaders.version.json")
    }
    fn load(shaders_dir: &Path) -> Self {
        std::fs::read(Self::path(shaders_dir))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    fn save(&self, shaders_dir: &Path) {
        if let Ok(b) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(Self::path(shaders_dir), b);
        }
    }
}

/// Consulta o GitHub no máximo a cada 6 h (limite da API sem login).
const CHECK_EVERY_SECS: u64 = 6 * 60 * 60;

/// O pacote instalado tem versão nova no `libretro/slang-shaders`? `false`
/// sem pacote instalado. `force` consulta agora; sem rede, usa a última
/// consulta guardada.
pub async fn update_available(shaders_dir: &Path, force: bool) -> bool {
    let dir = install_dir(shaders_dir);
    if !dir.is_dir() {
        return false;
    }
    let mut v = Version::load(shaders_dir);
    if force || now_secs().saturating_sub(v.checked_at) >= CHECK_EVERY_SECS {
        let got = refresh_latest(
            "libretro",
            "slang-shaders",
            v.installed.as_ref(),
            v.latest.as_ref(),
        )
        .await;
        match got {
            Some(c) => {
                v.latest = Some(c);
                v.checked_at = now_secs();
                v.save(shaders_dir);
            }
            None => log::warn!("shaders: commit do slang-shaders não consultado"),
        }
    }
    let Some(latest) = &v.latest else {
        return false;
    };
    let dir_date = installed_at(&dir);
    is_outdated(v.installed.as_ref(), latest, dir_date.as_deref())
}

/// GET com status checado. `reqwest` já segue redirects (o buildbot manda um).
async fn fetch(url: &str) -> Result<reqwest::Response, String> {
    log::info!("baixando pacote de shaders: {url}");
    net::http::client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("download: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download: {e}"))
}

/// `true` se toda entrada do zip começa com o mesmo diretório de 1º nível
/// (ex.: `slang-shaders-master/`) — aí a extração tira esse prefixo.
fn zip_common_root<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>) -> bool {
    let mut root: Option<std::ffi::OsString> = None;
    for i in 0..zip.len() {
        let Ok(f) = zip.by_index(i) else { return false };
        let Some(name) = f.enclosed_name() else {
            return false;
        };
        let Some(first) = name.components().next() else {
            continue;
        };
        let first = first.as_os_str().to_os_string();
        match &root {
            None => root = Some(first),
            Some(r) if *r != first => return false,
            _ => {}
        }
    }
    root.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_dir_shape() {
        let d = install_dir(Path::new("/data/shaders"));
        assert!(d.ends_with("slang-shaders"));
    }

    #[test]
    fn status_of_missing_dir() {
        let st = status(Path::new("/nao/existe/xyz"));
        assert!(!st.installed);
        assert_eq!(st.preset_count, 0);
    }

    fn zip_of(names: &[&str]) -> zip::ZipArchive<std::io::Cursor<Vec<u8>>> {
        let mut buf = Vec::new();
        {
            let mut zw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            for n in names {
                zw.start_file(*n, opts).unwrap();
            }
            zw.finish().unwrap();
        }
        zip::ZipArchive::new(std::io::Cursor::new(buf)).unwrap()
    }

    #[test]
    fn detects_github_style_root_prefix() {
        let mut z = zip_of(&[
            "slang-shaders-master/crt/crt-geom.slangp",
            "slang-shaders-master/bezel/koko-aio/monitor.slangp",
        ]);
        assert!(zip_common_root(&mut z));
    }

    #[test]
    fn no_prefix_when_entries_at_root() {
        let mut z = zip_of(&["crt/crt-geom.slangp", "bezel/koko-aio/monitor.slangp"]);
        assert!(!zip_common_root(&mut z));
    }
}
