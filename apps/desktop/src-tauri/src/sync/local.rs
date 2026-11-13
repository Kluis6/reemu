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
use domain::library::{Rom, RomRepository};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
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
    /// O CRC32 dessa ROM (D113: fica ao lado da base).
    pub rom_crc32: Option<String>,
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

/// Um rótulo curto e estável para o log no lugar do nome de um save ou de
/// uma chave (NFR-4: o log, que vai para o diagnóstico, nunca leva o nome de
/// uma ROM): os 8 primeiros hex do SHA-256 do texto.
pub fn log_tag(name: &str) -> String {
    let hex: String = Sha256::digest(name.as_bytes())
        .iter()
        .take(4)
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("#{hex}")
}

/// Erro de um arquivo de save: a pasta e o rótulo do nome, nunca o nome.
fn file_err(path: &Path, e: &io::Error) -> String {
    let dir = path.parent().unwrap_or(Path::new(""));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    format!("{} {}: {e}", dir.display(), log_tag(&name))
}

/// Arquivos regulares de `dir` cujo nome passa em `accept`, em ordem de nome.
/// Pasta inexistente é lista vazia; qualquer outro erro é erro.
fn system_files(
    dir: &Path,
    accept: impl Fn(&str) -> bool,
) -> Result<Vec<(String, PathBuf)>, String> {
    #[cfg(test)]
    probe::hit(dir);
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(dir, &e)),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_err(dir, &e))?;
        let is_file = entry.file_type().map_err(|e| file_err(&entry.path(), &e))?;
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

const SYSTEM_GROUPS: [SystemGroup; 2] = [
    ("ps2", PS2_CORE, is_ps2_card),
    ("dreamcast", DC_CORE, is_vmu_name),
];

/// As pastas dos cartões e VMUs dentro da pasta de sistema.
pub fn system_dirs(system_dir: &Path) -> Vec<PathBuf> {
    SYSTEM_GROUPS
        .iter()
        .map(|(_, core, _)| system_dir.join(core.strip_prefix("system/").unwrap_or(core)))
        .collect()
}

/// Nome de arquivo sem pasta: nada de `/`, `\`, `:` nem `..`. Um nome que vem
/// da nuvem nunca escolhe a pasta onde o arquivo vai parar.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':', '\0'])
        && Path::new(name).file_name().is_some_and(|n| n == name)
}

/// Plataforma, `core` (só cartões e VMUs) e nome de arquivo de uma chave
/// `file:<plataforma>/<arquivo>` ou `file:<plataforma>/<core>/<arquivo>`.
pub fn parse_key(key: &str) -> Option<(&str, Option<&str>, &str)> {
    let rest = key.strip_prefix("file:")?;
    let (platform, path) = rest.split_once('/')?;
    Some(match path.rsplit_once('/') {
        Some((core, filename)) => (platform, Some(core), filename),
        None => (platform, None, path),
    })
}

/// Uma ROM da biblioteca, pelo que o cloud save precisa dela.
#[derive(Debug, Clone)]
struct RomRef {
    id: String,
    crc32: String,
    platform: &'static str,
}

/// O que `account_saves` achou: os arquivos de save em disco agora e, para
/// cada ROM da biblioteca, onde o save dela fica (exista ou não), para um
/// save que só a nuvem tem saber onde descer.
#[derive(Debug, Default)]
pub struct LocalSaves {
    /// Os arquivos em disco, na ordem das ROMs da biblioteca e depois cartões
    /// e VMUs. Chave ou caminho repetido fica uma vez só (duas ROMs de mesmo
    /// stem dividem o mesmo arquivo).
    pub files: Vec<SaveFile>,
    save_dir: PathBuf,
    system_dir: PathBuf,
    /// Stem do save -> as ROMs com esse stem, na ordem da biblioteca.
    stems: HashMap<String, Vec<RomRef>>,
    /// Id da ROM -> plataforma da nuvem.
    platforms: HashMap<String, &'static str>,
}

impl LocalSaves {
    /// Onde fica, neste disco, o save de ROM chamado `filename`: um por
    /// plataforma das ROMs com esse stem, exista o arquivo ou não. Nome com
    /// pasta ou extensão que não é de save não tem lugar.
    pub fn rom_slots(&self, filename: &str) -> Vec<SaveFile> {
        let Some((stem, ext)) = filename.rsplit_once('.') else {
            return Vec::new();
        };
        if !plain_name(filename) || !ROM_SAVE_EXTENSIONS.contains(&ext) {
            return Vec::new();
        }
        let mut platforms = HashSet::new();
        self.stems
            .get(stem)
            .into_iter()
            .flatten()
            .filter(|r| platforms.insert(r.platform))
            .map(|r| SaveFile {
                key: format!("file:{}/{filename}", r.platform),
                rom_id: Some(r.id.clone()),
                rom_crc32: Some(r.crc32.clone()),
                platform: r.platform,
                core: None,
                filename: filename.to_owned(),
                path: self.save_dir.join(filename),
            })
            .collect()
    }

    /// Onde fica um cartão do PS2 ou um VMU (`core` é a pasta, como na nuvem).
    /// `None` para nome que não é de cartão ou VMU, ou que traz pasta.
    pub fn system_slot(&self, core: &str, filename: &str) -> Option<SaveFile> {
        if !plain_name(filename) {
            return None;
        }
        let (platform, core, accept) = SYSTEM_GROUPS.iter().find(|g| g.1 == core)?;
        if !accept(filename) {
            return None;
        }
        let rel = core.strip_prefix("system/").unwrap_or(core);
        Some(SaveFile {
            key: format!("file:{platform}/{core}/{filename}"),
            rom_id: None,
            rom_crc32: None,
            platform,
            core: Some(core),
            filename: filename.to_owned(),
            path: self.system_dir.join(rel).join(filename),
        })
    }

    /// O arquivo de uma chave, exista ou não; `None` quando a chave não é de
    /// nenhuma ROM da biblioteca nem de cartão ou VMU.
    pub fn slot_for_key(&self, key: &str) -> Option<SaveFile> {
        let (platform, core, filename) = parse_key(key)?;
        match core {
            Some(core) => self
                .system_slot(core, filename)
                .filter(|s| s.platform == platform),
            None => self
                .rom_slots(filename)
                .into_iter()
                .find(|s| s.platform == platform),
        }
    }

    /// A plataforma da nuvem de uma ROM da biblioteca.
    pub fn rom_platform(&self, rom_id: &str) -> Option<&'static str> {
        self.platforms.get(rom_id).copied()
    }
}

/// O stem dos saves de uma ROM: o mesmo que a sessão usa para o `.srm`.
fn save_stem(save_dir: &Path, rom: &Rom) -> Option<String> {
    let srm = emu_session::srm_path(save_dir, &rom.file_path);
    let name = srm.file_name()?.to_str()?;
    // `srm_path` termina sempre em `.srm`; o stem pode ter pontos
    // ("Game v1.2"), então a troca de extensão é por texto, não por `Path`.
    Some(name.strip_suffix(".srm").unwrap_or(name).to_owned())
}

/// Onde os saves de uma ROM ficam neste disco, existam ou não: um por
/// extensão de save, na ordem de `ROM_SAVE_EXTENSIONS`.
pub fn rom_slot_files(rom: &Rom, save_dir: &Path) -> Vec<SaveFile> {
    let Some(stem) = save_stem(save_dir, rom) else {
        return Vec::new();
    };
    let platform = platform_for_system(&rom.system_id);
    ROM_SAVE_EXTENSIONS
        .iter()
        .map(|ext| {
            let filename = format!("{stem}.{ext}");
            SaveFile {
                key: format!("file:{platform}/{filename}"),
                rom_id: Some(rom.id.clone()),
                rom_crc32: Some(rom.crc32.clone()),
                platform,
                core: None,
                path: save_dir.join(&filename),
                filename,
            }
        })
        .collect()
}

/// Os cartões e VMUs em disco da plataforma `platform` (nenhum para as
/// plataformas sem arquivo de sistema). Erro de leitura é erro.
pub fn system_saves(system_dir: &Path, platform: &str) -> Result<Vec<SaveFile>, String> {
    let mut out = Vec::new();
    for (p, core, accept) in SYSTEM_GROUPS {
        if p != platform {
            continue;
        }
        // `core` é "system/<pasta>": a pasta no disco é relativa à de sistema.
        let rel = core.strip_prefix("system/").unwrap_or(core);
        for (filename, path) in system_files(&system_dir.join(rel), accept)? {
            out.push(SaveFile {
                key: format!("file:{p}/{core}/{filename}"),
                rom_id: None,
                rom_crc32: None,
                platform: p,
                core: Some(core),
                filename,
                path,
            });
        }
    }
    Ok(out)
}

/// O nome como o disco compara: sem maiúsculas no Windows e no macOS (os
/// sistemas de arquivos de lá não as distinguem), exato no resto.
fn fold(name: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        name.to_lowercase()
    } else {
        name.to_owned()
    }
}

/// Os nomes (já em `fold`) dos arquivos regulares de `dir`, numa leitura só
/// da pasta. Pasta inexistente é conjunto vazio; outro erro é erro.
fn file_names(dir: &Path) -> Result<HashSet<String>, String> {
    #[cfg(test)]
    probe::hit(dir);
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(HashSet::new()),
        Err(e) => return Err(io_err(dir, &e)),
    };
    let mut names = HashSet::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_err(dir, &e))?;
        let kind = entry.file_type().map_err(|e| file_err(&entry.path(), &e))?;
        let regular = if kind.is_symlink() {
            // Um link vale pelo que ele aponta, como no `metadata` de um caminho.
            match std::fs::metadata(entry.path()) {
                Ok(m) => m.is_file(),
                Err(e) if e.kind() == io::ErrorKind::NotFound => false,
                Err(e) => return Err(file_err(&entry.path(), &e)),
            }
        } else {
            kind.is_file()
        };
        if let (true, Ok(name)) = (regular, entry.file_name().into_string()) {
            names.insert(fold(&name));
        }
    }
    Ok(names)
}

/// O arquivo existe e é regular? Só "não existe" vale `false`; outro erro é
/// erro.
fn is_file_at(path: &Path) -> Result<bool, String> {
    #[cfg(test)]
    probe::hit(path.parent().unwrap_or(path));
    match std::fs::metadata(path) {
        Ok(m) => Ok(m.is_file()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(file_err(path, &e)),
    }
}

/// Quantas vezes cada pasta foi consultada (só nos testes): prova que a
/// passada de uma ROM não lê a biblioteca inteira e que a passada completa lê
/// a pasta de saves uma vez.
#[cfg(test)]
pub(crate) mod probe {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    static HITS: Mutex<Option<HashMap<PathBuf, usize>>> = Mutex::new(None);

    pub fn hit(dir: &Path) {
        let mut hits = HITS.lock().unwrap_or_else(|p| p.into_inner());
        *hits
            .get_or_insert_with(HashMap::new)
            .entry(dir.to_owned())
            .or_default() += 1;
    }

    pub fn count(dir: &Path) -> usize {
        let hits = HITS.lock().unwrap_or_else(|p| p.into_inner());
        hits.as_ref().and_then(|h| h.get(dir).copied()).unwrap_or(0)
    }
}

/// Um arquivo por caminho e por chave: o primeiro que chega fica (duas ROMs
/// de mesmo stem dividem o mesmo arquivo).
#[derive(Default)]
struct Seen {
    keys: HashSet<String>,
    paths: HashSet<PathBuf>,
}

impl Seen {
    fn push(&mut self, files: &mut Vec<SaveFile>, file: SaveFile) {
        if self.paths.insert(file.path.clone()) && self.keys.insert(file.key.clone()) {
            files.push(file);
        }
    }
}

async fn list_roms(repo: &dyn RomRepository) -> Result<Vec<Rom>, String> {
    repo.list().await.map_err(|e| format!("lista de ROMs: {e}"))
}

/// O mapa da biblioteca (stem e plataforma de cada ROM), sem tocar o disco.
fn index(roms: &[Rom], save_dir: &Path, system_dir: &Path) -> LocalSaves {
    let mut saves = LocalSaves {
        save_dir: save_dir.to_owned(),
        system_dir: system_dir.to_owned(),
        ..LocalSaves::default()
    };
    for rom in roms {
        let platform = platform_for_system(&rom.system_id);
        saves.platforms.insert(rom.id.clone(), platform);
        if let Some(stem) = save_stem(save_dir, rom) {
            saves.stems.entry(stem).or_default().push(RomRef {
                id: rom.id.clone(),
                crc32: rom.crc32.clone(),
                platform,
            });
        }
    }
    saves
}

/// Leitura de disco fora das tarefas do Tokio: o prazo de antes de abrir um
/// jogo vale no meio dela, e o app não para enquanto ela roda.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| format!("leitura dos saves interrompida: {e}"))?
}

/// O mapa da biblioteca, sem arquivo nenhum: onde o save de cada chave fica.
/// Só o SQLite, sem tocar o disco.
pub async fn library(
    repo: &dyn RomRepository,
    save_dir: &Path,
    system_dir: &Path,
) -> Result<LocalSaves, String> {
    let roms = list_roms(repo).await?;
    Ok(index(&roms, save_dir, system_dir))
}

/// Os arquivos de save da conta que existem agora neste disco (`files`) e o
/// mapa de onde fica o save de cada ROM. A pasta de saves é lida uma vez, não
/// um arquivo por ROM. Erro de leitura é erro: o motor aborta a passada.
pub async fn account_saves(
    repo: &dyn RomRepository,
    save_dir: &Path,
    system_dir: &Path,
) -> Result<LocalSaves, String> {
    let roms = list_roms(repo).await?;
    let (save_dir, system_dir) = (save_dir.to_owned(), system_dir.to_owned());
    blocking(move || {
        let mut saves = index(&roms, &save_dir, &system_dir);
        let names = file_names(&save_dir)?;
        let mut seen = Seen::default();
        for rom in &roms {
            for file in rom_slot_files(rom, &save_dir) {
                if names.contains(&fold(&file.filename)) {
                    seen.push(&mut saves.files, file);
                }
            }
        }
        for (platform, _, _) in SYSTEM_GROUPS {
            for file in system_saves(&system_dir, platform)? {
                seen.push(&mut saves.files, file);
            }
        }
        Ok(saves)
    })
    .await
}

/// Só os saves de uma ROM: as vagas dela em `<save_dir>` e os cartões e VMUs
/// da plataforma dela. O mapa da biblioteca vem inteiro (do SQLite), mas o
/// disco só é consultado nesses lugares. A chave de cada arquivo é a mesma
/// que a passada completa daria (a primeira ROM da biblioteca com aquele
/// stem). `None` quando a ROM não está na biblioteca.
pub async fn rom_saves(
    repo: &dyn RomRepository,
    save_dir: &Path,
    system_dir: &Path,
    rom_id: &str,
) -> Result<Option<LocalSaves>, String> {
    let roms = list_roms(repo).await?;
    let (save_dir, system_dir, rom_id) = (
        save_dir.to_owned(),
        system_dir.to_owned(),
        rom_id.to_owned(),
    );
    blocking(move || {
        let mut saves = index(&roms, &save_dir, &system_dir);
        let Some(rom) = roms.iter().find(|r| r.id == rom_id) else {
            return Ok(None);
        };
        let mut seen = Seen::default();
        for slot in rom_slot_files(rom, &save_dir) {
            let file = saves
                .rom_slots(&slot.filename)
                .into_iter()
                .next()
                .unwrap_or(slot);
            if is_file_at(&file.path)? {
                seen.push(&mut saves.files, file);
            }
        }
        for file in system_saves(&system_dir, platform_for_system(&rom.system_id))? {
            seen.push(&mut saves.files, file);
        }
        Ok(Some(saves))
    })
    .await
}

/// O que listar na nuvem para os saves de uma ROM: os nomes das vagas dela e
/// o `core` dos cartões e VMUs da plataforma dela.
pub fn rom_listing(rom: &Rom, save_dir: &Path) -> (Vec<String>, Vec<&'static str>) {
    let names = rom_slot_files(rom, save_dir)
        .into_iter()
        .map(|f| f.filename)
        .collect();
    let platform = platform_for_system(&rom.system_id);
    let cores = SYSTEM_GROUPS
        .iter()
        .filter(|g| g.0 == platform)
        .map(|g| g.1)
        .collect();
    (names, cores)
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

        let got = account_saves(&repo, &saves, &system).await.unwrap().files;
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
            .unwrap()
            .files;
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].key, "file:gba/Game.srm");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn slots_say_where_a_cloud_save_lands_even_without_a_file() {
        let root = temp("slots");
        let saves = root.join("saves");
        let system = root.join("system");
        let repo = repo_with(&[
            ("r1", "C:/roms/Zelda.gba", "gba"),
            ("r2", "C:/roms/Zelda.nes", "nes"),
            ("r3", "C:/roms/Chrono.sfc", "snes"),
        ])
        .await;
        let local = account_saves(&repo, &saves, &system).await.unwrap();
        assert!(local.files.is_empty());

        // Duas ROMs de mesmo stem: uma vaga por plataforma, o mesmo caminho.
        let slots = local.rom_slots("Zelda.srm");
        let keys: Vec<&str> = slots.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, vec!["file:gba/Zelda.srm", "file:nes/Zelda.srm"]);
        assert!(slots.iter().all(|s| s.path == saves.join("Zelda.srm")));
        assert_eq!(slots[0].rom_id.as_deref(), Some("r1"));
        assert_eq!(slots[0].rom_crc32.as_deref(), Some("CRC"));
        // Extensão que não é de save, nome com pasta ou sem ROM: sem lugar.
        assert!(local.rom_slots("Zelda.txt").is_empty());
        assert!(local.rom_slots("../Zelda.srm").is_empty());
        assert!(local.rom_slots("sub/Zelda.srm").is_empty());
        assert!(local.rom_slots("Absent.srm").is_empty());

        let card = local
            .system_slot("system/pcsx2/memcards", "Mcd001.ps2")
            .unwrap();
        assert_eq!(card.key, "file:ps2/system/pcsx2/memcards/Mcd001.ps2");
        assert_eq!(
            card.path,
            system.join("pcsx2").join("memcards").join("Mcd001.ps2")
        );
        assert_eq!(card.core, Some("system/pcsx2/memcards"));
        let vmu = local.system_slot("system/dc", "vmu_save_A1.bin").unwrap();
        assert_eq!(vmu.platform, "dreamcast");
        assert_eq!(vmu.path, system.join("dc").join("vmu_save_A1.bin"));
        // Um nome da nuvem nunca escolhe a pasta.
        for (core, name) in [
            ("system/pcsx2/memcards", "../../evil.ps2"),
            ("system/pcsx2/memcards", "..\\evil.ps2"),
            ("system/pcsx2/memcards", "C:evil.ps2"),
            ("system/pcsx2/memcards", "notes.txt"),
            ("system/dc", "vmu_save_e1.bin"),
            ("system/other", "Mcd001.ps2"),
        ] {
            assert!(local.system_slot(core, name).is_none(), "{core} {name}");
        }

        assert_eq!(
            local.slot_for_key("file:snes/Chrono.srm").unwrap().rom_id,
            Some("r3".into())
        );
        assert!(local.slot_for_key("file:gba/Chrono.srm").is_none());
        assert_eq!(
            local
                .slot_for_key("file:ps2/system/pcsx2/memcards/Mcd001.ps2")
                .unwrap()
                .path,
            card.path
        );
        assert!(local
            .slot_for_key("file:dreamcast/system/pcsx2/memcards/Mcd001.ps2")
            .is_none());
        assert!(local.slot_for_key("state:snes/x/Chrono.state").is_none());
        assert_eq!(local.rom_platform("r3"), Some("snes"));
        assert_eq!(local.rom_platform("nope"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_key_splits_platform_core_and_name() {
        assert_eq!(
            parse_key("file:gba/Zelda.srm"),
            Some(("gba", None, "Zelda.srm"))
        );
        assert_eq!(
            parse_key("file:ps2/system/pcsx2/memcards/Mcd001.ps2"),
            Some(("ps2", Some("system/pcsx2/memcards"), "Mcd001.ps2"))
        );
        assert_eq!(parse_key("state:snes/x/a.state"), None);
        assert_eq!(parse_key("file:nothing"), None);
    }

    /// I-3: a passada completa lê a pasta de saves uma vez (não 12 consultas
    /// por ROM), e a de uma ROM consulta só as 12 vagas dela, com a mesma
    /// chave que a completa daria.
    #[tokio::test]
    async fn the_full_scan_reads_the_save_dir_once_and_a_rom_scan_only_its_slots() {
        let root = temp("probe");
        let saves = root.join("saves");
        let system = root.join("system");
        std::fs::create_dir_all(&saves).unwrap();
        let mut roms = Vec::new();
        for i in 0..40 {
            roms.push((format!("r{i:02}"), format!("C:/roms/Game{i:02}.gba")));
            std::fs::write(saves.join(format!("Game{i:02}.srm")), b"x").unwrap();
        }
        // Mesmo stem em outra plataforma: a chave é a da primeira ROM.
        roms.push(("rn".into(), "C:/roms/Game07.nes".into()));
        let repo = RomsRepo::new(connect_in_memory().await.unwrap());
        for (id, path) in &roms {
            let system = if path.ends_with(".nes") { "nes" } else { "gba" };
            add_roms(&repo, &[(id.as_str(), path.as_str(), system)]).await;
        }

        let before = probe::count(&saves);
        let all = account_saves(&repo, &saves, &system).await.unwrap();
        assert_eq!(all.files.len(), 40);
        assert_eq!(probe::count(&saves) - before, 1, "uma leitura da pasta");

        let before = probe::count(&saves);
        let one = rom_saves(&repo, &saves, &system, "rn")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(probe::count(&saves) - before, 12, "só as vagas da ROM");
        let keys: Vec<&str> = one.files.iter().map(|f| f.key.as_str()).collect();
        assert_eq!(keys, vec!["file:gba/Game07.srm"]);
        assert_eq!(one.files[0].rom_id.as_deref(), Some("r07"));
        assert!(all.files.contains(&one.files[0]));
        // Fora da biblioteca: nada a ler.
        assert!(rom_saves(&repo, &saves, &system, "nope")
            .await
            .unwrap()
            .is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// No Windows o disco não distingue maiúsculas: `zelda.SRM` é o save da
    /// ROM `Zelda`, nas duas leituras.
    #[cfg(windows)]
    #[tokio::test]
    async fn on_windows_a_save_name_matches_without_case() {
        let root = temp("case");
        let saves = root.join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("zelda.SRM"), b"z").unwrap();
        let repo = repo_with(&[("r1", "C:/roms/Zelda.gba", "gba")]).await;
        let all = account_saves(&repo, &saves, &root.join("system"))
            .await
            .unwrap();
        assert_eq!(all.files.len(), 1);
        assert_eq!(all.files[0].key, "file:gba/Zelda.srm");
        let one = rom_saves(&repo, &saves, &root.join("system"), "r1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(one.files, all.files);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn missing_directories_are_an_empty_list() {
        let root = temp("empty");
        let repo = repo_with(&[("r1", "C:/roms/Zelda.gba", "gba")]).await;
        let got = account_saves(&repo, &root.join("saves"), &root.join("system"))
            .await
            .unwrap()
            .files;
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
        assert!(got.is_err());
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
        assert!(got.is_err());
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
