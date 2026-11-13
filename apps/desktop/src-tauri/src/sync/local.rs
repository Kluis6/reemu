//! A visão local do cloud save: quais arquivos de save a conta tem neste disco
//! e o hash de cada um. Só leitura: nada aqui cria, move ou apaga arquivo.
//!
//! - arquivos de save de cada ROM da biblioteca em `<save_dir>`, com o stem que
//!   a sessão usa (`emu_session::srm_path`);
//! - memory cards do PS2 em `<system>/pcsx2/memcards/*.ps2`;
//! - VMUs do Dreamcast em `<system>/dc/vmu_save_[a-d][1-8].bin`.
//!
//! Erro de leitura nunca vira "lista vazia": o motor aborta a passada, porque
//! uma lista curta demais faria a base parecer apagada. Só a pasta que não
//! existe (`NotFound`) é uma pasta vazia.

use super::platform::platform_for_system;
use domain::library::RomRepository;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Extensões de save que os cores gravam em `<save_dir>` (`GET_SAVE_DIRECTORY`).
const ROM_SAVE_EXTENSIONS: &[&str] = &[
    "srm", "sav", "rtc", "fs", "eep", "fla", "mcr", "mcd", "bsv", "mpk", "ldci", "sram",
];

/// Pasta (relativa à de sistema) dos cartões do PS2 e `core` da linha na nuvem.
const PS2_CORE: &str = "system/pcsx2/memcards";
/// Idem, para os VMUs do Dreamcast.
const DC_CORE: &str = "system/dc";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFile {
    /// `file:<plataforma>/<arquivo>` para os saves de ROM e
    /// `file:<plataforma>/<core>/<arquivo>` para cartões e VMUs.
    pub key: String,
    /// A ROM dona do arquivo; `None` para cartões e VMUs.
    pub rom_id: Option<String>,
    pub platform: &'static str,
    /// O campo `core` da linha na nuvem: a pasta, como a porta WebDAV do
    /// Alvanista arquiva esses arquivos. `None` para os saves de ROM.
    pub core: Option<&'static str>,
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

fn is_ps2_card(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ps2"))
}

fn io_err(path: &Path, e: &io::Error) -> String {
    format!("{}: {e}", path.display())
}

/// Arquivos regulares de `dir` cujo nome passa em `accept`, em ordem de nome.
/// Pasta inexistente é lista vazia; qualquer outro erro é erro.
fn system_files(
    dir: &Path,
    accept: impl Fn(&str) -> bool,
) -> Result<Vec<(String, PathBuf)>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(dir, &e)),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_err(dir, &e))?;
        let is_file = entry.file_type().map_err(|e| io_err(&entry.path(), &e))?;
        if !is_file.is_file() {
            continue;
        }
        if let Ok(name) = entry.file_name().into_string() {
            if accept(&name) {
                found.push((name, entry.path()));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Plataforma, `core` e filtro de nome de um grupo de arquivos de sistema.
type SystemGroup = (&'static str, &'static str, fn(&str) -> bool);

/// Os arquivos de save da conta que existem agora neste disco, na ordem das
/// ROMs da biblioteca e depois cartões e VMUs. Chave ou caminho repetido fica
/// uma vez só (duas ROMs de mesmo stem dividem o mesmo arquivo).
pub async fn account_saves(
    repo: &dyn RomRepository,
    save_dir: &Path,
    system_dir: &Path,
) -> Result<Vec<SaveFile>, String> {
    let roms = repo
        .list()
        .await
        .map_err(|e| format!("lista de ROMs: {e}"))?;
    let mut out = Vec::new();
    let mut seen_keys = HashSet::new();
    let mut seen_paths = HashSet::new();

    for rom in roms {
        let platform = platform_for_system(&rom.system_id);
        let srm = emu_session::srm_path(save_dir, &rom.file_path);
        let Some(srm_name) = srm.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // `srm_path` termina sempre em `.srm`; o stem pode ter pontos
        // ("Game v1.2"), então a troca de extensão é por texto, não por `Path`.
        let stem = srm_name.strip_suffix(".srm").unwrap_or(srm_name);
        for ext in ROM_SAVE_EXTENSIONS {
            let filename = format!("{stem}.{ext}");
            let path = save_dir.join(&filename);
            match std::fs::metadata(&path) {
                Ok(m) if m.is_file() => {}
                Ok(_) => continue,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io_err(&path, &e)),
            }
            let key = format!("file:{platform}/{filename}");
            if !seen_paths.insert(path.clone()) || !seen_keys.insert(key.clone()) {
                continue;
            }
            out.push(SaveFile {
                key,
                rom_id: Some(rom.id.clone()),
                platform,
                core: None,
                filename,
                path,
            });
        }
    }

    let groups: [SystemGroup; 2] = [
        ("ps2", PS2_CORE, is_ps2_card),
        ("dreamcast", DC_CORE, is_vmu_name),
    ];
    for (platform, core, accept) in groups {
        // `core` é "system/<pasta>": a pasta no disco é relativa à de sistema.
        let rel = core.strip_prefix("system/").unwrap_or(core);
        for (filename, path) in system_files(&system_dir.join(rel), accept)? {
            let key = format!("file:{platform}/{core}/{filename}");
            if seen_paths.insert(path.clone()) && seen_keys.insert(key.clone()) {
                out.push(SaveFile {
                    key,
                    rom_id: None,
                    platform,
                    core: Some(core),
                    filename,
                    path,
                });
            }
        }
    }
    Ok(out)
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

    async fn add_roms(repo: &RomsRepo, roms: &[(&str, &str, &str)]) {
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
    }

    async fn repo_with(roms: &[(&str, &str, &str)]) -> RomsRepo {
        let repo = RomsRepo::new(connect_in_memory().await.unwrap());
        add_roms(&repo, roms).await;
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
        std::fs::write(saves.join("Mario.sav"), b"mario").unwrap();
        std::fs::write(saves.join("Game v1.2.eep"), b"dotted stem").unwrap();
        std::fs::write(saves.join("Orphan.srm"), b"no rom").unwrap();
        std::fs::write(saves.join("Mario.txt"), b"wrong ext").unwrap();
        std::fs::write(cards.join("Mcd001.ps2"), b"card1").unwrap();
        std::fs::write(cards.join("Mcd002.PS2"), b"card2").unwrap();
        std::fs::write(cards.join("notes.txt"), b"x").unwrap();
        std::fs::write(dc.join("vmu_save_A1.bin"), b"vmu").unwrap();
        std::fs::write(dc.join("VMU_SAVE_D8.BIN"), b"vmu").unwrap();
        std::fs::write(dc.join("vmu_save_e1.bin"), b"no").unwrap();
        std::fs::write(dc.join("vmu_save_A9.bin"), b"no").unwrap();
        std::fs::write(dc.join("vmu_save_A1.bin.bak"), b"no").unwrap();

        // Uma ROM sem save em disco não aparece.
        let repo = repo_with(&[
            ("r1", "C:/roms/Zelda.gba", "gba"),
            ("r2", "C:/roms/Absent.nes", "nes"),
            ("r3", "C:/roms/Mario.gbc", "gbc"),
            ("r4", "C:/roms/Game v1.2.n64", "n64"),
        ])
        .await;

        let got = account_saves(&repo, &saves, &system).await.unwrap();
        let keys: Vec<&str> = got.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(
            keys,
            vec![
                "file:gba/Zelda.srm",
                "file:gbc/Mario.sav",
                "file:n64/Game v1.2.eep",
                "file:ps2/system/pcsx2/memcards/Mcd001.ps2",
                "file:ps2/system/pcsx2/memcards/Mcd002.PS2",
                "file:dreamcast/system/dc/VMU_SAVE_D8.BIN",
                "file:dreamcast/system/dc/vmu_save_A1.bin",
            ]
        );
        assert_eq!(got[0].rom_id.as_deref(), Some("r1"));
        assert_eq!(got[0].platform, "gba");
        assert_eq!(got[0].core, None);
        assert_eq!(got[0].filename, "Zelda.srm");
        assert_eq!(got[0].path, saves.join("Zelda.srm"));
        assert!(got[..3].iter().all(|s| s.core.is_none()));
        assert!(got[3..].iter().all(|s| s.rom_id.is_none()));
        assert!(got[3..5]
            .iter()
            .all(|s| s.core == Some("system/pcsx2/memcards")));
        assert!(got[5..].iter().all(|s| s.core == Some("system/dc")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn two_roms_with_the_same_stem_share_one_file() {
        let root = temp("dedup");
        let saves = root.join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("Game.srm"), b"x").unwrap();
        let repo = repo_with(&[
            ("r1", "C:/roms/Game.gba", "gba"),
            ("r2", "C:/roms/Game.nes", "nes"),
        ])
        .await;
        let got = account_saves(&repo, &saves, &root.join("system"))
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].key, "file:gba/Game.srm");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn missing_directories_are_an_empty_list() {
        let root = temp("empty");
        let repo = repo_with(&[("r1", "C:/roms/Zelda.gba", "gba")]).await;
        let got = account_saves(&repo, &root.join("saves"), &root.join("system"))
            .await
            .unwrap();
        assert!(got.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_failed_rom_listing_is_an_error_not_an_empty_list() {
        let root = temp("romfail");
        let db = connect_in_memory().await.unwrap();
        let repo = RomsRepo::new(db.clone());
        db.close().await;
        let got = account_saves(&repo, &root.join("saves"), &root.join("system")).await;
        assert!(got.is_err(), "{got:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_read_dir_error_other_than_not_found_is_an_error() {
        let root = temp("readdir");
        let system = root.join("system");
        std::fs::create_dir_all(system.join("pcsx2")).unwrap();
        // `memcards` existe mas é um arquivo: `read_dir` falha sem ser NotFound.
        std::fs::write(system.join("pcsx2").join("memcards"), b"not a dir").unwrap();
        let repo = repo_with(&[]).await;
        let got = account_saves(&repo, &root.join("saves"), &system).await;
        assert!(got.is_err(), "{got:?}");
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
