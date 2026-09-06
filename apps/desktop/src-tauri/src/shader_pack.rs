//! Downloader do pacote `libretro/slang-shaders` (opção B do backlog de
//! shaders). Espelha `core_catalog.rs`: baixa o zip do GitHub, extrai pra
//! `<dados>/shaders/slang-shaders/` e a `ShaderLibrary` aponta pra lá.
//!
//! O zip tem um dir raiz `slang-shaders-master/` — a extração tira esse
//! prefixo. Troca atômica-ish: extrai num `.part` e renomeia no fim.

use std::path::{Path, PathBuf};
use tauri::ipc::Channel;

const ARCHIVE_URL: &str =
    "https://codeload.github.com/libretro/slang-shaders/zip/refs/heads/master";

/// `<dados>/shaders/slang-shaders`.
pub fn install_dir(shaders_dir: &Path) -> PathBuf {
    shaders_dir.join("slang-shaders")
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub received: u64,
    /// 0 se o servidor não mandou `Content-Length`.
    pub total: u64,
    /// `"download"` | `"extract"`.
    pub phase: &'static str,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
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
    log::info!("baixando pacote de shaders: {ARCHIVE_URL}");
    let mut resp = reqwest::get(ARCHIVE_URL)
        .await
        .map_err(|e| format!("download: {e}"))?
        .error_for_status()
        .map_err(|e| format!("download: {e}"))?;
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
    tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf, String> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| format!("zip inválido: {e}"))?;
        let tmp = dest.with_extension("part");
        let _ = std::fs::remove_dir_all(&tmp);
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(name) = f.enclosed_name() else {
                continue;
            };
            // tira o `slang-shaders-master/` da frente
            let rel: PathBuf = name.components().skip(1).collect();
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
    .map_err(|e| e.to_string())?
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
}
