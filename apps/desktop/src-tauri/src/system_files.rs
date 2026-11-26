//! Pacotes de dados do Buildbot para PPSSPP e DirkSimple, instalados sob
//! `<system_dir>/`. O vídeo de LaserDisc é fornecido pela biblioteca local.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const PPSSPP_URL: &str = "https://buildbot.libretro.com/assets/system/PPSSPP.zip";
const DOLPHIN_URL: &str = "https://buildbot.libretro.com/assets/system/Dolphin.zip";
const DIRKSIMPLE_URL: &str = "https://buildbot.libretro.com/assets/system/DirkSimple.zip";
const DIRKSIMPLE_MAX_BYTES: usize = 2 * 1024 * 1024;
const DIRKSIMPLE_MAX_ENTRIES: usize = 128;
static DIRKSIMPLE_INSTALL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static DOLPHIN_INSTALL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Arquivo que só existe com a pasta completa — marca de "instalado".
const PPSSPP_MARKER: &str = "PPSSPP/ppge_atlas.zim";
const DOLPHIN_MARKER: &str = "dolphin-emu/Sys/codehandler.bin";

pub fn ppsspp_installed(system_dir: &Path) -> bool {
    system_dir.join(PPSSPP_MARKER).is_file()
}

pub fn dolphin_installed(system_dir: &Path) -> bool {
    system_dir.join(DOLPHIN_MARKER).is_file()
}

/// Baixa e instala `<system_dir>/PPSSPP/`. Extrai numa pasta temporária e só
/// então troca pela atual — download interrompido não deixa pasta pela
/// metade. Devolve quantos arquivos foram extraídos.
pub async fn download_ppsspp(system_dir: &Path) -> Result<usize, String> {
    let resp = download_pack(PPSSPP_URL).await?;
    let bytes = resp.bytes().await.map_err(|e| format!("download: {e}"))?;
    let dir = system_dir.to_path_buf();
    tokio::task::spawn_blocking(move || install_zip(&dir, &bytes))
        .await
        .map_err(|e| e.to_string())?
}

/// Baixa e instala `<system_dir>/dolphin-emu/Sys/` do buildbot oficial da
/// libretro, preservando qualquer `IPL.bin` opcional importado pelo usuário.
pub async fn download_dolphin(system_dir: &Path) -> Result<usize, String> {
    let _install = DOLPHIN_INSTALL.lock().await;
    let resp = download_pack(DOLPHIN_URL).await?;
    let bytes = resp.bytes().await.map_err(|e| format!("download: {e}"))?;
    let dir = system_dir.to_path_buf();
    tokio::task::spawn_blocking(move || install_dolphin_zip(&dir, &bytes))
        .await
        .map_err(|e| e.to_string())?
}

pub async fn ensure_dolphin(system_dir: &Path) -> Result<(), String> {
    if dolphin_installed(system_dir) {
        return Ok(());
    }
    download_dolphin(system_dir).await.map(|_| ())
}

async fn download_pack(url: &str) -> Result<reqwest::Response, String> {
    log::info!("system files: baixando {url}");
    let resp = reqwest::Client::builder()
        .user_agent(net::http::USER_AGENT)
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("rede: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("buildbot HTTP {}", resp.status()));
    }
    Ok(resp)
}

fn dirksimple_script(root: &Path, game: &str) -> bool {
    let folder = root.join("data/games").join(game);
    ["game.lua", "game.luac"]
        .iter()
        .any(|name| folder.join(name).is_file())
}

pub fn needs_dirksimple_data(system_dir: &Path, path: &Path) -> bool {
    domain::library::dirksimple_game_id(path).is_some_and(|id| {
        domain::library::DIRKSIMPLE_OFFICIAL_GAMES.contains(&id.as_str())
            && !dirksimple_script(&system_dir.join("DirkSimple"), &id)
    })
}

pub async fn ensure_dirksimple(system_dir: &Path) -> Result<(), String> {
    let _install = DIRKSIMPLE_INSTALL.lock().await;
    let root = system_dir.join("DirkSimple");
    if domain::library::DIRKSIMPLE_OFFICIAL_GAMES
        .iter()
        .all(|game| dirksimple_script(&root, game))
    {
        return Ok(());
    }
    let mut resp = download_pack(DIRKSIMPLE_URL).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("download: {e}"))? {
        if bytes.len() + chunk.len() > DIRKSIMPLE_MAX_BYTES {
            return Err("pacote do DirkSimple excedeu o tamanho esperado".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    tokio::task::spawn_blocking(move || install_dirksimple_zip(&root, &bytes))
        .await
        .map_err(|e| e.to_string())?
}

fn install_dirksimple_zip(root: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if zip.len() > DIRKSIMPLE_MAX_ENTRIES {
        return Err("pacote do DirkSimple excedeu o número esperado de arquivos".into());
    }
    let mut entries = Vec::new();
    let mut remaining = DIRKSIMPLE_MAX_BYTES;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|e| e.to_string())?;
        if entry.is_dir() || entry.is_symlink() {
            continue;
        }
        let Some(path) = entry
            .enclosed_name()
            .and_then(|p| strip_root(&p, "DirkSimple"))
        else {
            continue;
        };
        let mut data = Vec::new();
        entry
            .take(remaining as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        remaining = remaining
            .checked_sub(data.len())
            .ok_or("pacote do DirkSimple excedeu o tamanho descompactado esperado")?;
        entries.push((path, data));
    }
    for game in domain::library::DIRKSIMPLE_OFFICIAL_GAMES {
        let script = Path::new("data/games").join(game).join("game.lua");
        if !entries
            .iter()
            .any(|(path, data)| *path == script && !data.is_empty())
        {
            return Err(format!(
                "pacote do DirkSimple incompleto (sem {})",
                script.display()
            ));
        }
    }

    // Grava os scripts por último: uma interrupção durante cópia dos assets
    // mantém o pacote pendente e o próximo download preenche o que faltou.
    entries.sort_by_key(|(path, _)| path.ends_with("game.lua"));
    for (path, data) in entries {
        let dest = root.join(path);
        if dest.is_file() {
            continue;
        }
        let parent = dest
            .parent()
            .expect("asset do DirkSimple tem diretório pai");
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(&data).map_err(|e| e.to_string())?;
        match file.persist_noclobber(&dest) {
            Ok(_) => {}
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists && dest.is_file() => {}
            Err(e) => return Err(e.error.to_string()),
        }
    }
    Ok(())
}

fn install_zip(system_dir: &Path, bytes: &[u8]) -> Result<usize, String> {
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("zip: {e}"))?;
    let staging = system_dir.join(".PPSSPP.part");
    let _ = std::fs::remove_dir_all(&staging);
    let mut files = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        // link simbólico de dentro do zip não vira arquivo aqui
        if entry.is_symlink() {
            continue;
        }
        // Só `PPSSPP/...`, sem `..` nem caminho absoluto (zip malicioso não
        // escreve fora da pasta).
        let Some(rel) = entry.enclosed_name().and_then(|p| strip_root(&p, "PPSSPP")) else {
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

fn install_dolphin_zip(system_dir: &Path, bytes: &[u8]) -> Result<usize, String> {
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("zip: {e}"))?;
    std::fs::create_dir_all(system_dir).map_err(|e| e.to_string())?;
    let staging = system_dir.join(".dolphin-emu.part");
    let _ = std::fs::remove_dir_all(&staging);
    let mut files = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_symlink() {
            continue;
        }
        let Some(rel) = entry
            .enclosed_name()
            .and_then(|p| strip_root(&p, "dolphin-emu"))
        else {
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
    if !staging.join("Sys/codehandler.bin").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("pacote do Dolphin incompleto (sem Sys/codehandler.bin)".into());
    }
    let final_dir = system_dir.join("dolphin-emu");
    // Preserva BIOS IPL opcionais que o usuário já tenha importado.
    for region in ["USA", "EUR", "JAP"] {
        let existing_ipl = final_dir.join("Sys/GC").join(region).join("IPL.bin");
        if existing_ipl.is_file() {
            let staged_ipl = staging.join("Sys/GC").join(region).join("IPL.bin");
            if let Some(parent) = staged_ipl.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(&existing_ipl, &staged_ipl);
        }
    }
    let _ = std::fs::remove_dir_all(&final_dir);
    std::fs::rename(&staging, &final_dir).map_err(|e| e.to_string())?;
    log::info!("system files: Dolphin Sys instalado ({files} arquivos)");
    Ok(files)
}

/// `raiz/a/b` → `a/b`, só com componentes de caminho simples.
fn strip_root(p: &Path, root: &str) -> Option<PathBuf> {
    let rest = p.strip_prefix(root).ok()?;
    (!rest.as_os_str().is_empty() && rest.components().all(|c| matches!(c, Component::Normal(_))))
        .then(|| rest.to_path_buf())
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
    fn dirksimple_pack_retries_without_overwriting_local_game_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("DirkSimple");
        let lair = root.join("data/games/lair/game.lua");
        std::fs::create_dir_all(lair.parent().unwrap()).unwrap();
        std::fs::write(&lair, b"local script").unwrap();
        let archive = zip_with(&[
            ("DirkSimple/data/games/lair/game.lua", b"upstream lair"),
            ("DirkSimple/data/games/cliff/game.lua", b"upstream cliff"),
            ("DirkSimple/data/games/cliff/icon.png", b"icon"),
            ("DirkSimple/../foreign", b"foreign"),
        ]);
        assert!(needs_dirksimple_data(dir.path(), Path::new("cliff.ogv")));
        install_dirksimple_zip(&root, &archive).unwrap();
        install_dirksimple_zip(&root, &archive).unwrap();
        assert_eq!(std::fs::read(lair).unwrap(), b"local script");
        assert!(root.join("data/games/cliff/icon.png").is_file());
        assert!(!dir.path().join("foreign").exists());
        assert!(!needs_dirksimple_data(dir.path(), Path::new("CLIFF.OGV")));
    }

    #[test]
    fn strip_root_rejects_anything_outside_ppsspp() {
        assert_eq!(
            strip_root(Path::new("PPSSPP/a/b"), "PPSSPP"),
            Some(PathBuf::from("a/b"))
        );
        assert_eq!(strip_root(Path::new("PPSSPP"), "PPSSPP"), None);
        assert_eq!(strip_root(Path::new("x/PPSSPP/a"), "PPSSPP"), None);
        assert_eq!(
            strip_root(Path::new("DirkSimple/../escape"), "DirkSimple"),
            None
        );
    }

    #[test]
    fn dolphin_pack_installs_and_preserves_optional_ipl() {
        let dir = tempfile::tempdir().unwrap();
        let sys = dir.path();
        let ipl = sys.join("dolphin-emu/Sys/GC/USA/IPL.bin");
        std::fs::create_dir_all(ipl.parent().unwrap()).unwrap();
        std::fs::write(&ipl, b"user-ipl").unwrap();

        let z = zip_with(&[
            ("dolphin-emu/Sys/codehandler.bin", b"code"),
            ("dolphin-emu/Sys/GameSettings/GALE01.ini", b"[Core]"),
            ("fora/x.txt", b"ignorado"),
        ]);
        assert!(!dolphin_installed(sys));
        assert_eq!(install_dolphin_zip(sys, &z).unwrap(), 2);
        assert!(dolphin_installed(sys));
        assert_eq!(std::fs::read(&ipl).unwrap(), b"user-ipl");
        assert!(!sys.join("fora").exists());
        assert!(!sys.join(".dolphin-emu.part").exists());

        let bad = zip_with(&[("dolphin-emu/Sys/GameSettings/GALE01.ini", b"sem-marker")]);
        assert!(install_dolphin_zip(sys, &bad).is_err());
        assert!(dolphin_installed(sys));
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
