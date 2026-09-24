//! Arquivos de sistema que não são BIOS: hoje só a pasta `PPSSPP/` que o core
//! de PSP usa (fontes, atlas da interface do sistema, `compat.ini`…). Sem ela
//! alguns jogos mostram texto quebrado nos diálogos do sistema.
//!
//! Ao contrário de BIOS (copyright da fabricante — o ReEmu nunca baixa), isto
//! é GPL e sai do buildbot da libretro: o mesmo `assets/system/PPSSPP.zip`
//! que o "Core System Files Downloader" do RetroArch baixa. Raiz do zip =
//! `PPSSPP/`, extraída direto em `<system_dir>/`.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

const PPSSPP_URL: &str = "https://buildbot.libretro.com/assets/system/PPSSPP.zip";
/// Arquivo que só existe com a pasta completa — marca de "instalado".
const PPSSPP_MARKER: &str = "PPSSPP/ppge_atlas.zim";

pub fn ppsspp_installed(system_dir: &Path) -> bool {
    system_dir.join(PPSSPP_MARKER).is_file()
}

/// Baixa e instala `<system_dir>/PPSSPP/`. Extrai numa pasta temporária e só
/// então troca pela atual — download interrompido não deixa pasta pela
/// metade. Devolve quantos arquivos foram extraídos.
pub async fn download_ppsspp(system_dir: &Path) -> Result<usize, String> {
    log::info!("system files: baixando {PPSSPP_URL}");
    let resp = reqwest::Client::builder()
        .user_agent("reemu/0.1")
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?
        .get(PPSSPP_URL)
        .send()
        .await
        .map_err(|e| format!("rede: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("buildbot HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("download: {e}"))?;
    let dir = system_dir.to_path_buf();
    tokio::task::spawn_blocking(move || install_zip(&dir, &bytes))
        .await
        .map_err(|e| e.to_string())?
}

fn install_zip(system_dir: &Path, bytes: &[u8]) -> Result<usize, String> {
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("zip: {e}"))?;
    let staging = system_dir.join(".PPSSPP.part");
    let _ = std::fs::remove_dir_all(&staging);
    let mut files = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        // Só `PPSSPP/...`, sem `..` nem caminho absoluto (zip malicioso não
        // escreve fora da pasta).
        let Some(rel) = entry.enclosed_name().and_then(|p| strip_root(&p)) else {
            continue;
        };
        let dest = staging.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&dest, buf).map_err(|e| e.to_string())?;
        files += 1;
    }
    if !staging.join("ppge_atlas.zim").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("pacote do PPSSPP incompleto (sem ppge_atlas.zim)".into());
    }
    let final_dir = system_dir.join("PPSSPP");
    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&staging, &final_dir).map_err(|e| e.to_string())?;
    log::info!("system files: PPSSPP instalado ({files} arquivos)");
    Ok(files)
}

/// `PPSSPP/a/b` → `a/b`. Qualquer coisa fora de `PPSSPP/` → `None`.
fn strip_root(p: &Path) -> Option<PathBuf> {
    let mut comps = p.components();
    match comps.next()? {
        Component::Normal(first) if first == "PPSSPP" => {}
        _ => return None,
    }
    let rest: PathBuf = comps.collect();
    (!rest.as_os_str().is_empty()).then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut zw = zip::ZipWriter::new(&mut out);
            for (name, data) in entries {
                zw.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zw.write_all(data).unwrap();
            }
            zw.finish().unwrap();
        }
        out.into_inner()
    }

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "reemu-sysfiles-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn installs_under_system_dir_and_ignores_foreign_paths() {
        let dir = tmp();
        let z = zip_with(&[
            ("PPSSPP/ppge_atlas.zim", b"atlas"),
            ("PPSSPP/flash0/font/jpn0.pgf", b"font"),
            ("outra-coisa/x.txt", b"fora"),
        ]);
        assert!(!ppsspp_installed(&dir));
        assert_eq!(install_zip(&dir, &z).unwrap(), 2);
        assert!(ppsspp_installed(&dir));
        assert!(dir.join("PPSSPP/flash0/font/jpn0.pgf").is_file());
        assert!(!dir.join("outra-coisa").exists());
        assert!(
            !dir.join(".PPSSPP.part").exists(),
            "sem sobra da pasta temporária"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn incomplete_pack_keeps_the_installed_one() {
        let dir = tmp();
        install_zip(&dir, &zip_with(&[("PPSSPP/ppge_atlas.zim", b"v1")])).unwrap();
        let bad = zip_with(&[("PPSSPP/compat.ini", b"x")]);
        assert!(install_zip(&dir, &bad).is_err());
        assert_eq!(std::fs::read(dir.join(PPSSPP_MARKER)).unwrap(), b"v1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strip_root_rejects_anything_outside_ppsspp() {
        assert_eq!(
            strip_root(Path::new("PPSSPP/a/b")),
            Some(PathBuf::from("a/b"))
        );
        assert_eq!(strip_root(Path::new("PPSSPP")), None);
        assert_eq!(strip_root(Path::new("x/PPSSPP/a")), None);
    }

    /// Baixa o pacote REAL do buildbot (rede, ~11 MB). Fora do CI:
    /// `cargo test -p reemu-desktop --lib real_ppsspp -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn real_ppsspp_pack_installs() {
        let dir = tmp();
        let n = download_ppsspp(&dir).await.expect("baixar e instalar");
        assert!(n > 100, "só {n} arquivos");
        assert!(ppsspp_installed(&dir));
        assert!(dir.join("PPSSPP/compat.ini").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
