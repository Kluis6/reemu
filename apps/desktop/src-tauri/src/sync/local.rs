//! A visão local do cloud save: quais arquivos de save a conta tem neste disco
//! e o hash de cada um. Só leitura: nada aqui cria, move ou apaga arquivo.
//!
//! - `.srm` de cada ROM da biblioteca, no caminho que a sessão usa
//!   (`emu_session::srm_path`, o mesmo cálculo do stem);
//! - memory cards do PS2 em `<system>/pcsx2/memcards/*.ps2`;
//! - VMUs do Dreamcast em `<system>/dc/vmu_save_[a-d][1-8].bin`.

use super::platform::platform_for_system;
use domain::library::RomRepository;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Extensões de save por ROM. O ReEmu só grava `.srm` (save RAM do libretro);
/// não há outra lista no código.
const ROM_SAVE_EXTENSIONS: &[&str] = &["srm"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFile {
    /// `file:<plataforma>/<arquivo>`, ou `file:<plataforma>/system/<arquivo>`
    /// para os arquivos da pasta de sistema (cartões e VMUs).
    pub key: String,
    /// A ROM dona do arquivo; `None` para cartões e VMUs.
    pub rom_id: Option<String>,
    pub platform: &'static str,
    pub filename: String,
    pub path: PathBuf,
}

/// SHA-256 (hex minúsculo), tamanho em bytes e mtime (segundos Unix).
pub fn sha256_file(path: &Path) -> io::Result<(String, u64, i64)> {
    let mut file = std::fs::File::open(path)?;
    let mtime = file
        .metadata()?
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    let hex = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok((hex, size, mtime))
}

fn is_vmu_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(rest) = lower
        .strip_prefix("vmu_save_")
        .and_then(|r| r.strip_suffix(".bin"))
    else {
        return false;
    };
    let b = rest.as_bytes();
    b.len() == 2 && (b'a'..=b'd').contains(&b[0]) && (b'1'..=b'8').contains(&b[1])
}

/// Arquivos regulares de `dir` cujo nome passa em `accept`, em ordem de nome.
fn system_files(dir: &Path, accept: impl Fn(&str) -> bool) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            accept(&name).then(|| (name, e.path()))
        })
        .collect();
    found.sort();
    found
}

/// Plataforma, pasta e filtro de nome de um grupo de arquivos da pasta de sistema.
type SystemGroup = (&'static str, PathBuf, fn(&str) -> bool);

/// Os arquivos de save da conta que existem agora neste disco, na ordem das
/// ROMs da biblioteca e depois cartões e VMUs. Chave repetida fica uma vez só.
pub async fn account_saves(
    repo: &dyn RomRepository,
    save_dir: &Path,
    system_dir: &Path,
) -> Vec<SaveFile> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for rom in repo.list().await.unwrap_or_default() {
        let platform = platform_for_system(&rom.system_id);
        for ext in ROM_SAVE_EXTENSIONS {
            let srm = emu_session::srm_path(save_dir, &rom.file_path);
            let path = srm.with_extension(ext);
            if !path.is_file() {
                continue;
            }
            let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let key = format!("file:{platform}/{filename}");
            if !seen.insert(key.clone()) {
                continue;
            }
            out.push(SaveFile {
                key,
                rom_id: Some(rom.id.clone()),
                platform,
                filename: filename.to_string(),
                path,
            });
        }
    }

    let groups: [SystemGroup; 2] = [
        ("ps2", system_dir.join("pcsx2").join("memcards"), |n| {
            Path::new(n)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("ps2"))
        }),
        ("dreamcast", system_dir.join("dc"), is_vmu_name),
    ];
    for (platform, dir, accept) in groups {
        for (filename, path) in system_files(&dir, accept) {
            let key = format!("file:{platform}/system/{filename}");
            if seen.insert(key.clone()) {
                out.push(SaveFile {
                    key,
                    rom_id: None,
                    platform,
                    filename,
                    path,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::{connect_in_memory, RomsRepo};
    use domain::library::Rom;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("reemu-sync-local-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    async fn repo_with(roms: &[(&str, &str, &str)]) -> RomsRepo {
        let repo = RomsRepo::new(connect_in_memory().await.unwrap());
        for (id, path, system) in roms {
            repo.add(&Rom {
                id: (*id).into(),
                file_path: (*path).into(),
                crc32: "CRC".into(),
                md5: "MD5".into(),
                system_id: (*system).into(),
                added_at: 0,
                last_played_at: None,
                is_favorite: false,
                user_title: None,
            })
            .await
            .unwrap();
        }
        repo
    }

    #[tokio::test]
    async fn account_saves_lists_srm_by_rom_stem_and_cards_by_pattern() {
        let root = temp("list");
        let saves = root.join("saves");
        let system = root.join("system");
        std::fs::create_dir_all(&saves).unwrap();
        let cards = system.join("pcsx2").join("memcards");
        let dc = system.join("dc");
        std::fs::create_dir_all(&cards).unwrap();
        std::fs::create_dir_all(&dc).unwrap();

        std::fs::write(saves.join("Zelda.srm"), b"zelda").unwrap();
        std::fs::write(saves.join("Orphan.srm"), b"no rom").unwrap();
        std::fs::write(saves.join("Mario.sav"), b"wrong ext").unwrap();
        std::fs::write(cards.join("Mcd001.ps2"), b"card1").unwrap();
        std::fs::write(cards.join("Mcd002.ps2"), b"card2").unwrap();
        std::fs::write(cards.join("notes.txt"), b"x").unwrap();
        std::fs::write(dc.join("vmu_save_A1.bin"), b"vmu").unwrap();
        std::fs::write(dc.join("vmu_save_d8.bin"), b"vmu").unwrap();
        std::fs::write(dc.join("vmu_save_e1.bin"), b"no").unwrap();
        std::fs::write(dc.join("vmu_save_A9.bin"), b"no").unwrap();
        std::fs::write(dc.join("vmu_save_A1.bin.bak"), b"no").unwrap();

        // Uma ROM sem save em disco não aparece.
        let repo = repo_with(&[
            ("r1", "C:/roms/Zelda.gba", "gba"),
            ("r2", "C:/roms/Absent.nes", "nes"),
        ])
        .await;

        let got = account_saves(&repo, &saves, &system).await;
        let keys: Vec<&str> = got.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "file:gba/Zelda.srm",
                "file:ps2/system/Mcd001.ps2",
                "file:ps2/system/Mcd002.ps2",
                "file:dreamcast/system/vmu_save_A1.bin",
                "file:dreamcast/system/vmu_save_d8.bin",
            ]
        );
        assert_eq!(got[0].rom_id.as_deref(), Some("r1"));
        assert_eq!(got[0].platform, "gba");
        assert_eq!(got[0].filename, "Zelda.srm");
        assert_eq!(got[0].path, saves.join("Zelda.srm"));
        assert!(got[1..].iter().all(|s| s.rom_id.is_none()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn account_saves_is_empty_when_nothing_exists() {
        let root = temp("empty");
        let repo = repo_with(&[("r1", "C:/roms/Zelda.gba", "gba")]).await;
        let got = account_saves(&repo, &root.join("saves"), &root.join("system")).await;
        assert!(got.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sha256_file_reports_hash_size_and_mtime() {
        let root = temp("sha");
        let p = root.join("a.srm");
        std::fs::write(&p, b"abc").unwrap();
        let (hash, size, mtime) = sha256_file(&p).unwrap();
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(size, 3);
        assert!(mtime > 1_600_000_000);
        assert!(sha256_file(&root.join("missing")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
