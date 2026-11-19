//! Varredura de um diretório de ROMs → `domain::library::Rom` (por hash).

use crate::archive::{archive_entry_size, hash_archive_entry, is_supported_archive, peek_archive};
use crate::hash::{file_name_of, FileRomHasher, RomHashes};
use crate::sector::{open_file, read_sheet, sheet_data_track, Area, Opener};
use crate::systems::{
    folder_only_exts, folder_only_flat, system_for_extension, system_from_folder_name,
    AMBIGUOUS_DISC_EXTS,
};
use domain::library::{Rom, RomRepository};
use std::io::BufReader;
use std::path::Path;
use walkdir::WalkDir;

/// O que identifica um arquivo da biblioteca (D138): os hashes, quando o
/// arquivo é lido inteiro, e sempre o nome e o tamanho do que identifica (a
/// entrada de dentro do `.zip`/`.7z`, a 1ª trilha de dados de um
/// `.cue`/`.gdi`, ou o próprio arquivo), mais o serial do disco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// `None` quando o que identifica não foi lido inteiro.
    pub hashes: Option<Digests>,
    /// Nome (só o arquivo, sem pasta) do que identifica.
    pub name: String,
    /// Tamanho em bytes do que identifica (sem o header iNES).
    pub size: u64,
    /// Serial do disco, lido da imagem solta (`None` em cartucho, dentro de
    /// `.zip`/`.7z`, ou quando não se lê).
    pub serial: Option<String>,
    /// A trilha que o `.cue`/`.gdi` cita não estava lá, ou o texto dele não
    /// se leu: o nome e o tamanho são os do próprio cue, por enquanto. A
    /// varredura grava a linha com `identified_at` nulo, e o passo em segundo
    /// plano calcula de novo quando a trilha aparecer.
    pub provisional: bool,
}

/// CRC32 (maiúsculas), MD5 e SHA1 (minúsculas) de uma leitura só.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digests {
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
}

impl From<RomHashes> for Identity {
    fn from(h: RomHashes) -> Self {
        Identity {
            hashes: Some(Digests {
                crc32: h.crc32,
                md5: h.md5,
                sha1: h.sha1,
            }),
            name: h.name,
            size: h.size,
            serial: None,
            provisional: false,
        }
    }
}

/// Contêineres comprimidos identificados só pelo serial, pelo nome e pelo
/// tamanho (D138): o hash de um `.cso` ou `.chd` não casa com nada que o
/// ScreenScraper indexa e custaria ler o disco inteiro. Nunca são lidos
/// inteiros (NFR-5). O `.iso` não está aqui: segue o [`DISC_HASH_MAX`] como
/// as outras imagens, porque o de tamanho de CD o ScreenScraper conhece pelo
/// hash.
const SERIAL_ONLY_EXTS: &[&str] = &["cso", "chd"];

/// Maior imagem de disco lida inteira pro hash (um `.iso`, a trilha de um
/// `.cue`/`.gdi`, ou um `.img`, `.pbp`, `.cdi`, `.mdf`, `.nrg` solto): 1,5
/// GiB, na mesma leitura única do CRC32, do MD5 e do SHA1. Cabe a
/// maior trilha de CD (~999 MiB crus num CD de 99 minutos: 445.500 setores
/// de 2352 bytes) e a trilha 3 de um
/// GD-ROM de Dreamcast (até 504.150 setores de 2352 bytes, 1.185.760.800
/// bytes), que o catálogo e o ScreenScraper conhecem pelo hash. Um DVD (PS2,
/// até 8,5 GB) passa disso e fica com o serial, o nome e o tamanho, como o
/// `.cso` e o `.chd`.
const DISC_HASH_MAX: u64 = 1536 * 1024 * 1024;
const _: () = assert!(
    DISC_HASH_MAX >= (549_150 - 45_000) * 2352 && DISC_HASH_MAX < 2 * 1024 * 1024 * 1024,
    "cabe a trilha 3 do GD-ROM e nunca uma imagem de vários GB"
);

/// A identidade de `path` como a varredura calcula (o passo em segundo plano
/// recalcula as linhas antigas com ela). `archived_entry`: a ROM de dentro de
/// um `.zip`/`.7z`, se for o caso.
pub fn identify(
    path: &Path,
    system_id: &str,
    archived_entry: Option<&str>,
) -> std::io::Result<Identity> {
    identify_with(path, system_id, archived_entry, &open_file, DISC_HASH_MAX)
}

/// [`identify`] com outro jeito de abrir os arquivos e outro limite de
/// imagem lida inteira (os testes contam o que é lido sem gravar GBs).
fn identify_with(
    path: &Path,
    system_id: &str,
    archived_entry: Option<&str>,
    open: Opener,
    hash_max: u64,
) -> std::io::Result<Identity> {
    let mut id = match archived_entry {
        Some(entry) => archived_identity(path, entry, hash_max)?,
        None => file_identity(path, system_id, open, hash_max)?,
    };
    // Serial do disco (D138), só pros sistemas de disco e só da imagem solta
    // (de dentro de um `.zip`/`.7z` seria descomprimir o disco).
    id.serial = archived_entry
        .is_none()
        .then(|| crate::serial::read_serial_with(path, system_id, open))
        .flatten();
    Ok(id)
}

/// Identidade de um arquivo solto. `.cue` e `.gdi` são identificados pela
/// 1ª trilha de dados (a trilha 3 do GD-ROM no Dreamcast, quando há), não
/// pelo texto deles: é o hash da trilha que o Redump, o ScreenScraper e o
/// catálogo conhecem (D138). A trilha só é alcançada dentro da pasta do
/// cue ([`crate::disc_sniff::sibling`]) e se for arquivo comum; sem ela, o
/// nome e o tamanho do próprio cue, sem hash (provisório se a trilha só não
/// está lá ainda, ou se o texto não se leu). `.cso` e `.chd` nunca são
/// lidos ([`SERIAL_ONLY_EXTS`]); `.iso`, imagem ou trilha acima de
/// `hash_max` também não.
fn file_identity(
    path: &Path,
    system_id: &str,
    open: Opener,
    hash_max: u64,
) -> std::io::Result<Identity> {
    let ext = ext_of(path);
    match ext.as_str() {
        "cue" | "gdi" => {
            let Some(text) = read_sheet(path, open) else {
                return provisional(path);
            };
            match sheet_data_track(path, &text, Area::for_system(system_id)) {
                Some(track) if track.file.is_file() => disc_identity(&track.file, open, hash_max),
                // A trilha citada não está lá (ainda: o `.bin` pode chegar
                // depois do `.cue`).
                Some(_) => provisional(path),
                // Sem trilha de dados, ou citando fora da pasta: não muda.
                None => unhashed(path),
            }
        }
        e if SERIAL_ONLY_EXTS.contains(&e) => unhashed(path),
        e if AMBIGUOUS_DISC_EXTS.contains(&e) => disc_identity(path, open, hash_max),
        _ => hash_whole(path, open),
    }
}

/// Imagem de disco (ou trilha dela): lida inteira só até `hash_max`.
fn disc_identity(path: &Path, open: Opener, hash_max: u64) -> std::io::Result<Identity> {
    let id = unhashed(path)?;
    if id.size > hash_max {
        return Ok(id);
    }
    hash_whole(path, open)
}

/// A ROM de dentro de um `.zip`/`.7z`. Um `.cso`/`.chd` lá dentro, ou outra
/// imagem de disco (o `.iso` também) acima de `hash_max`, fica com o nome e
/// o tamanho do índice do arquivo, sem descomprimir. O resto é descomprimido aos poucos
/// direto no hash, nunca inteiro na memória.
fn archived_identity(path: &Path, entry: &str, hash_max: u64) -> std::io::Result<Identity> {
    let name = file_name_of(entry);
    let ext = ext_of(Path::new(entry));
    if AMBIGUOUS_DISC_EXTS.contains(&ext.as_str()) {
        let size = archive_entry_size(path, entry)?;
        if SERIAL_ONLY_EXTS.contains(&ext.as_str()) || size > hash_max {
            return Ok(Identity {
                hashes: None,
                name,
                size,
                serial: None,
                provisional: false,
            });
        }
    }
    Ok(hash_archive_entry(path, entry, &name)?.into())
}

/// Nome e tamanho de `path`, sem ler o conteúdo.
fn unhashed(path: &Path) -> std::io::Result<Identity> {
    Ok(Identity {
        hashes: None,
        name: file_name_of(&path.to_string_lossy()),
        size: std::fs::metadata(path)?.len(),
        serial: None,
        provisional: false,
    })
}

/// Como [`unhashed`], marcado pra ser calculado de novo.
fn provisional(path: &Path) -> std::io::Result<Identity> {
    Ok(Identity {
        provisional: true,
        ..unhashed(path)?
    })
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// CRC32, MD5 e SHA1 do arquivo inteiro, numa leitura só.
fn hash_whole(path: &Path, open: Opener) -> std::io::Result<Identity> {
    let name = file_name_of(&path.to_string_lossy());
    Ok(FileRomHasher::hash_reader(BufReader::new(open(path)?), &name)?.into())
}

/// Pastas ancestrais de `path` (relativas a `root`), da mais próxima da raiz
/// pra mais próxima do arquivo — ex: `<root>/psx/Game (USA).iso` → `["psx"]`.
/// Mesma técnica de `decoration.rs::classify` pra achar o "nome de sistema"
/// mais perto do arquivo numa biblioteca organizada por pasta (RetroBat/ES-DE).
fn ancestor_dirs(path: &Path, root: &Path) -> Vec<String> {
    // O nome da PRÓPRIA pasta raiz também conta: quem adiciona
    // `…/roms/atari2600` (e não `…/roms`) como fonte tem o sistema no nome da
    // raiz — sem isto, `.bin` de 2600 e os sets de NAOMI eram ignorados.
    let mut dirs: Vec<String> = root
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| vec![n.to_string()])
        .unwrap_or_default();
    if let Some(rel) = path.strip_prefix(root).ok().and_then(|rel| rel.parent()) {
        dirs.extend(rel.iter().filter_map(|c| c.to_str()).map(str::to_string));
    }
    dirs
}

/// Tenta achar um `system_id` nas pastas ancestrais, da mais próxima do
/// arquivo pra mais longe. `None` se nenhuma bater em `system_from_folder_name`.
fn system_from_dirs(dirs: &[String]) -> Option<&'static str> {
    dirs.iter().rev().find_map(|d| system_from_folder_name(d))
}

/// Sistema da pasta ancestral, se `ext` for uma das extensões genéricas que
/// ele aceita (`folder_only_exts` — ex: `.rom` dentro de `<roms>/msx/`).
fn system_from_folder_ext(path: &Path, root: &Path, ext: &str) -> Option<&'static str> {
    let dirs = ancestor_dirs(path, root);
    let sys = system_from_dirs(&dirs).filter(|s| folder_only_exts(s).contains(&ext))?;
    // `folder_only_flat` (DOS): só arquivo direto na pasta do sistema.
    if folder_only_flat(sys) && dirs.last().and_then(|d| system_from_folder_name(d)) != Some(sys) {
        return None;
    }
    Some(sys)
}

/// Sistemas em que o arquivo comprimido INTEIRO é o jogo (set do MAME:
/// dumps de chip avulsos, sem "a ROM" dentro): o sistema vem da pasta e o
/// core recebe o `.zip`/`.7z` como está (flycast/MAME aceitam `zip|7z`).
const WHOLE_ARCHIVE_SYSTEMS: &[&str] = &["arcade", "naomi", "atomiswave"];

/// `.chd` que acompanha um set do MAME: fica numa subpasta com o id do jogo
/// ao lado do `.zip` (`naomi/azumanga/gdl-0018.chd` + `naomi/azumanga.zip`,
/// "the chd file in a subdirectory of the roms folder named after the mame
/// ID" — docs.libretro.com, library/flycast). O jogo se abre pelo `.zip`; o
/// `.chd` sozinho não dá boot, então não vira entrada própria.
fn is_mame_companion(path: &Path, ext: &str) -> bool {
    if ext != "chd" {
        return false;
    }
    let (Some(dir), Some(id)) = (path.parent(), path.parent().and_then(|d| d.file_name())) else {
        return false;
    };
    let Some(parent) = dir.parent() else {
        return false;
    };
    ["zip", "7z"].iter().any(|e| {
        let mut set = std::ffi::OsString::from(id);
        set.push(".");
        set.push(e);
        parent.join(set).is_file()
    })
}

/// Sets de arcade que não são jogo: BIOS de placa e dispositivos que os
/// cores (FBNeo, MAME) carregam junto com o jogo — `neogeo.zip` pros jogos de
/// Neo Geo, `pgm.zip` pros de PGM, `qsound.zip` pros de CPS2… Ficam na pasta
/// dos jogos (é onde o core procura), mas abrir um deles não roda nada.
/// Nomes dos sets no FBNeo e no MAME; a lista não é completa — com o `.dat`
/// (próxima etapa) o próprio arquivo diz quem é BIOS/dispositivo.
const ARCADE_NON_GAMES: &[&str] = &[
    "neogeo",
    "neocdz",
    "pgm",
    "decocass",
    "skns",
    "stvbios",
    "cpzn1",
    "cpzn2",
    "isgsm",
    "bubsys",
    "coleco",
    "fdsbios",
    "msx",
    "spectrum",
    "spec128",
    "spec1282a",
    "ngp",
    "qsound",
    "nmk004",
    "ym2608",
    "ym2413",
    "namcoc69",
    "namcoc70",
    "namcoc75",
    "namco51",
    "namco52",
    "namco53",
    "namco54",
    "midssio",
    "cvs",
    "megatech",
    "megaplay",
    "playch10",
    "nss",
    "konamigx",
    "hng64",
    "naomi",
    "naomi2",
    "awbios",
    "hod2bios",
    "f355bios",
    "f355dlx",
    "airlbios",
    "crysbios",
    "sys246",
    "sys256",
    "sys573",
    "taitofx1",
    "taitogn",
    "coh1000a",
    "coh1000c",
    "coh1000t",
    "coh1001l",
    "coh1002e",
    "coh1002m",
    "coh1002v",
    "coh3002c",
    "coh3002t",
    "konamigv",
    "chihiro",
    "triforce",
    "mpu4",
    "aleck64",
    "n64",
    "pgm2",
    "segasp",
    "cubo",
    "galgbios",
    "ar_bios",
    "gq863",
    "konendev",
    "sammymdl",
];

/// Set de BIOS/dispositivo de arcade (ver [`ARCADE_NON_GAMES`]): `.zip`/`.7z`
/// numa pasta de arcade com o nome de um deles.
fn is_arcade_non_game(path: &Path, root: &Path, ext: &str) -> bool {
    if !is_supported_archive(ext) || system_from_dirs(&ancestor_dirs(path, root)) != Some("arcade")
    {
        return false;
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ARCADE_NON_GAMES.contains(&stem.as_str())
}

/// Sistemas que o flycast só roda no formato do MAME: o `.zip`/`.7z` do set,
/// com o `.chd` numa subpasta de mesmo nome nos jogos em GD-ROM ("Run NAOMI
/// GD-ROM format games stored in MAME zip + chd format by running the zip
/// file" — docs.libretro.com, library/flycast). Imagem de disco solta (`.cue`,
/// `.chd`, `.gdi`…) ou dentro de um arquivo (cópia do Redump) o flycast abre
/// como disco de Dreamcast, e o jogo não roda.
const MAME_ONLY_SYSTEMS: &[&str] = &["naomi", "atomiswave"];

fn is_disc_ext(ext: &str) -> bool {
    AMBIGUOUS_DISC_EXTS.contains(&ext)
}

/// Sistema de `MAME_ONLY_SYSTEMS` vindo da pasta, se houver.
fn mame_only_system(path: &Path, root: &Path) -> Option<&'static str> {
    system_from_dirs(&ancestor_dirs(path, root)).filter(|s| MAME_ONLY_SYSTEMS.contains(s))
}

/// Imagem de disco (solta ou dentro de `.zip`/`.7z`) numa pasta de NAOMI ou
/// Atomiswave: não é jogo que o core rode (ver `MAME_ONLY_SYSTEMS`).
/// `peek` controla se abre o arquivo comprimido pra olhar dentro (a
/// contagem rápida da barra de progresso não abre).
fn is_unplayable_disc_dump(path: &Path, root: &Path, ext: &str, peek: bool) -> bool {
    if mame_only_system(path, root).is_none() {
        return false;
    }
    if is_disc_ext(ext) {
        return true;
    }
    peek && is_supported_archive(ext)
        && peek_archive(path).is_some_and(|a| {
            Path::new(&a.entry)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| is_disc_ext(&e.to_ascii_lowercase()))
        })
}

/// Extensão reconhecida (ROM crua, arquivo comprimido suportado, ou
/// extensão genérica dentro da pasta de um sistema que a aceita).
fn recognized(path: &Path, root: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    if is_mame_companion(path, &ext) || is_unplayable_disc_dump(path, root, &ext, false) {
        return false;
    }
    system_for_extension(&ext).is_some()
        || is_supported_archive(&ext)
        || system_from_folder_ext(path, root, &ext).is_some()
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("erro de repositório: {0}")]
    Repo(#[from] domain::error::RepoError),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub found: usize,
    pub added: usize,
    pub skipped_known: usize,
    pub skipped_unrecognized: usize,
    pub errors: usize,
    /// Já catalogadas com outro sistema, corrigidas pela regra atual.
    pub reclassified: usize,
    /// Catalogadas antes mas que não são jogo (`.chd` de set do MAME, BIOS
    /// de arcade).
    pub removed: usize,
    /// `.zip`/`.7z` fora de pasta de arcade sem cartucho reconhecível dentro
    /// — em geral um set de arcade numa pasta com outro nome (a tela avisa).
    pub unrecognized_archives: usize,
}

/// Progresso da varredura (arquivo `current` de `total` reconhecidos).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProgress {
    pub current: usize,
    pub total: usize,
    pub file: String,
}

/// Conta rápido quantos arquivos com extensão reconhecida existem em `dir`
/// (só `stat`, sem ler conteúdo) — pro total da barra de progresso.
pub fn count_roms(dir: &Path) -> usize {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| recognized(e.path(), dir))
        .count()
}

/// Varre `dir` recursivamente e adiciona ao `repo` as ROMs ainda não
/// catalogadas (dedup por `file_path`). `on_progress` é chamado a cada
/// arquivo reconhecido (passe `|_| {}` se não quiser).
pub async fn scan_into<R, F>(
    repo: &R,
    dir: &Path,
    now_unix: i64,
    mut on_progress: F,
) -> Result<ScanReport, ScanError>
where
    R: RomRepository + ?Sized,
    F: FnMut(ScanProgress),
{
    let mut report = ScanReport::default();
    let total = count_roms(dir);

    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = ext_of(path);

        // Parte de um set do MAME (`.chd` ao lado do `.zip`): não é jogo; se
        // uma varredura antiga catalogou, sai da biblioteca.
        // Idem pra imagem de disco em pasta de NAOMI/Atomiswave (o flycast
        // só roda o formato do MAME): sai da biblioteca se já estava.
        // BIOS/dispositivo de arcade (`neogeo.zip`…): não é jogo; sai também.
        if is_mame_companion(path, &ext)
            || is_unplayable_disc_dump(path, dir, &ext, true)
            || is_arcade_non_game(path, dir, &ext)
        {
            if let Some(old) = repo.find_by_path(&path.to_string_lossy()).await? {
                repo.remove(&old.id).await?;
                report.removed += 1;
            }
            continue;
        }

        // ROM crua, dentro de um .zip/.7z, ou imagem de disco (extensão
        // ambígua entre vários sistemas — PS1/PS2/Saturn/Dreamcast/PSP/...).
        // Pro arquivo de cartucho, o `system_id`/hash vêm da entrada interna
        // (o CRC precisa ser o da ROM, não o do arquivo); pro arquivo de
        // arcade (MAME/FBNeo — sem entrada de cartucho reconhecível dentro)
        // e pra disco, o arquivo INTEIRO é a "ROM".
        let (system_id, archived_entry): (&str, Option<String>) =
            if AMBIGUOUS_DISC_EXTS.contains(&ext.as_str()) {
                // Extensão de disco: (1) pasta ancestral (RetroBat/ES-DE); (2)
                // sistema específico da extensão (`.gdi`→dreamcast, `.pbp`→psp);
                // (3) assinatura no conteúdo do arquivo; (4) balde genérico.
                let sys = system_from_dirs(&ancestor_dirs(path, dir))
                    .or_else(|| system_for_extension(&ext).filter(|s| *s != "disc"))
                    .or_else(|| crate::disc_sniff::identify(path))
                    .unwrap_or("disc");
                (sys, None)
            } else if let Some(sys) = system_for_extension(&ext) {
                (sys, None)
            } else if let Some(sys) = system_from_folder_ext(path, dir, &ext) {
                (sys, None)
            } else if is_supported_archive(&ext) {
                let whole = system_from_dirs(&ancestor_dirs(path, dir))
                    .filter(|s| WHOLE_ARCHIVE_SYSTEMS.contains(s));
                if let Some(sys) = whole {
                    // Set de arcade/NAOMI/Atomiswave: sem "a ROM" dentro do
                    // arquivo (chip dumps avulsos) — ele inteiro é a unidade,
                    // hash do arquivo.
                    (sys, None)
                } else {
                    match peek_archive(path) {
                        Some(a) => (a.system_id, Some(a.entry)),
                        None => {
                            report.skipped_unrecognized += 1;
                            report.unrecognized_archives += 1;
                            continue;
                        }
                    }
                }
            } else {
                report.skipped_unrecognized += 1;
                continue;
            };
        report.found += 1;
        on_progress(ScanProgress {
            current: report.found,
            total,
            file: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string(),
        });

        let path_str = path.to_string_lossy();
        if let Some(old) = repo.find_by_path(&path_str).await? {
            // Já catalogada; se a regra de identificação mudou (ex.: NAOMI
            // que entrou como `disc`), corrige o sistema sem perder nada.
            if old.system_id != system_id {
                repo.set_system(&old.id, system_id).await?;
                report.reclassified += 1;
            } else {
                report.skipped_known += 1;
            }
            continue;
        }

        let id = match identify(path, system_id, archived_entry.as_deref()) {
            Ok(id) => id,
            Err(e) => {
                // Nunca o caminho nem o nome (são do disco da pessoa).
                log::warn!("biblioteca: um arquivo não foi lido ({:?})", e.kind());
                report.errors += 1;
                continue;
            }
        };
        // Sem hash, `crc32` e `md5` ficam vazios (as colunas são NOT NULL) e
        // `sha1` fica nulo; o nome, o tamanho e o serial identificam.
        let (crc32, md5, sha1) = match id.hashes {
            Some(d) => (d.crc32, d.md5, Some(d.sha1)),
            None => (String::new(), String::new(), None),
        };

        repo.add(&Rom {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: path_str.into_owned(),
            crc32,
            md5,
            system_id: system_id.to_string(),
            added_at: now_unix,
            last_played_at: None,
            is_favorite: false,
            user_title: None,
            sha1,
            serial: id.serial,
            ident_name: Some(id.name),
            ident_size: i64::try_from(id.size).ok(),
            identified_at: (!id.provisional).then_some(now_unix),
        })
        .await?;
        report.added += 1;
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::read_archive_entry;
    use crate::sector::{ReadSeek, READ_BUDGET};
    use std::io::{Cursor, Read, Seek, SeekFrom, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    /// Arquivo aberto que soma num contador tudo o que se lê dele.
    struct Counting {
        inner: Box<dyn ReadSeek>,
        read: Arc<AtomicU64>,
    }

    impl Read for Counting {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.read.fetch_add(n as u64, Ordering::Relaxed);
            Ok(n)
        }
    }

    impl Seek for Counting {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }

    /// A identidade de um arquivo solto e quantos bytes foram lidos, de
    /// todos os arquivos (a imagem, o cue, a trilha), pra chegar nela.
    fn identify_counting(path: &Path, system: &str, hash_max: u64) -> (Identity, u64) {
        let read = Arc::new(AtomicU64::new(0));
        let open = |p: &Path| -> std::io::Result<Box<dyn ReadSeek>> {
            Ok(Box::new(Counting {
                inner: open_file(p)?,
                read: read.clone(),
            }))
        };
        let id = identify_with(path, system, None, &open, hash_max).unwrap();
        (id, read.load(Ordering::Relaxed))
    }

    fn digests_of(bytes: &[u8]) -> Digests {
        Identity::from(FileRomHasher::hash_reader(Cursor::new(bytes), "x").unwrap())
            .hashes
            .unwrap()
    }

    fn put(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    /// Dois arquivos de trilha de bytes diferentes (o cue diz o modo de cada um).
    fn tracks(dir: &Path) -> (Vec<u8>, Vec<u8>) {
        let t1 = vec![0xA5u8; 2352 * 4];
        let t2: Vec<u8> = (0..2352 * 8).map(|i| (i % 251) as u8).collect();
        put(dir, "Jogo (Track 1).bin", &t1);
        put(dir, "Jogo (Track 2).bin", &t2);
        (t1, t2)
    }

    #[test]
    fn cue_hashes_first_data_track() {
        let dir = tempfile::tempdir().unwrap();
        let (t1, t2) = tracks(dir.path());

        // Áudio primeiro: pulado; vale a 1ª trilha MODE1/ ou MODE2/.
        let text = "FILE \"Jogo (Track 1).bin\" BINARY\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\nFILE \"Jogo (Track 2).bin\" BINARY\n  TRACK 02 MODE2/2352\n    INDEX 00 00:00:00\n    INDEX 01 00:02:00\n";
        let cue = put(dir.path(), "Jogo.cue", text.as_bytes());
        let id = identify(&cue, "psx", None).unwrap();
        assert_eq!(id.name, "Jogo (Track 2).bin");
        assert_eq!(id.size, t2.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&t2)));
        assert_ne!(
            id.hashes,
            Some(digests_of(text.as_bytes())),
            "o texto do cue não é hasheado"
        );

        // Dados primeiro: a trilha 1 vale, mesmo com áudio depois.
        let text = "FILE \"Jogo (Track 1).bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\nFILE \"Jogo (Track 2).bin\" BINARY\n  TRACK 02 AUDIO\n    INDEX 01 00:00:00\n";
        let cue = put(dir.path(), "Dados.cue", text.as_bytes());
        let id = identify(&cue, "saturn", None).unwrap();
        assert_eq!(id.name, "Jogo (Track 1).bin");
        assert_eq!(id.size, t1.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&t1)));
    }

    #[test]
    fn gdi_hashes_track_3_for_dreamcast_when_present_else_first_data() {
        let dir = tempfile::tempdir().unwrap();
        let t1: Vec<u8> = vec![0x01; 2352 * 3];
        let t3: Vec<u8> = (0..2352 * 5).map(|i| (i % 13) as u8).collect();
        put(dir.path(), "track01.bin", &t1);
        put(dir.path(), "track02.raw", &[0x02; 2352 * 2]);
        put(dir.path(), "track 03.bin", &t3);

        let gdi = put(
            dir.path(),
            "Jogo.gdi",
            b"3\n1 0 4 2352 track01.bin 0\n2 450 0 2352 track02.raw 0\n3 45000 4 2352 \"track 03.bin\" 0\n",
        );
        let id = identify(&gdi, "dreamcast", None).unwrap();
        assert_eq!(id.name, "track 03.bin");
        assert_eq!(id.size, t3.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&t3)));
        // O mesmo gdi como outro sistema (pasta que diz outra coisa): a área
        // de alta densidade é só do Dreamcast, vale a 1ª trilha de dados.
        let id = identify(&gdi, "psx", None).unwrap();
        assert_eq!(id.name, "track01.bin");
        assert_eq!(id.hashes, Some(digests_of(&t1)));

        // Sem a trilha 3 (sem área de alta densidade): a 1ª trilha de dados.
        let gdi = put(
            dir.path(),
            "Curto.gdi",
            b"2\n1 0 4 2352 track01.bin 0\n2 450 0 2352 track02.raw 0\n",
        );
        let id = identify(&gdi, "dreamcast", None).unwrap();
        assert_eq!(id.name, "track01.bin");
        assert_eq!(id.hashes, Some(digests_of(&t1)));
    }

    #[test]
    fn missing_track_file_falls_back_to_name_and_size_of_cue() {
        let root = tempfile::tempdir().unwrap();
        let lib = root.path().join("lib");
        std::fs::create_dir_all(lib.join("Pasta.bin")).unwrap();
        // Existe, mas fora da pasta do cue: não é alcançado.
        put(root.path(), "Fora.bin", &[0x11; 4096]);
        let cue = |name: &str, file: &str, mode: &str| {
            put(
                &lib,
                name,
                format!("FILE \"{file}\" BINARY\n  TRACK 01 {mode}\n    INDEX 01 00:00:00\n")
                    .as_bytes(),
            )
        };
        // `provisional`: a trilha pode aparecer depois (calcula de novo); o
        // resto não muda com o tempo.
        for (sheet, provisional, why) in [
            (
                cue("Sumiu.cue", "Sumiu.bin", "MODE2/2352"),
                true,
                "trilha que não existe",
            ),
            (
                cue("Pasta.cue", "Pasta.bin", "MODE2/2352"),
                true,
                "trilha que é pasta",
            ),
            (
                cue("Sobe.cue", "../Fora.bin", "MODE2/2352"),
                false,
                "trilha fora da pasta",
            ),
            (cue("Musica.cue", "Pasta.bin", "AUDIO"), false, "só áudio"),
            (
                put(&lib, "Gd.gdi", b"1\n1 0 4 2352 nada.bin 0\n"),
                true,
                "gdi sem a trilha",
            ),
        ] {
            let id = identify(&sheet, "psx", None).unwrap();
            let own = sheet.file_name().unwrap().to_string_lossy().into_owned();
            assert_eq!(id.name, own, "{why}");
            assert_eq!(id.size, std::fs::metadata(&sheet).unwrap().len(), "{why}");
            assert_eq!(id.hashes, None, "{why}");
            assert_eq!(id.provisional, provisional, "{why}");
        }
    }

    /// Cue gravado com BOM (Bloco de Notas antigo) e cue/gdi em Windows-1252
    /// citando `Pokémon.bin`: a trilha é achada e hasheada.
    #[test]
    fn sheet_with_bom_or_cp1252_finds_its_track() {
        let dir = tempfile::tempdir().unwrap();
        let track: Vec<u8> = (0..2352 * 4).map(|i| (i % 19) as u8).collect();
        put(dir.path(), "Pokémon.bin", &track);
        let cue = "FILE \"Pokémon.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";

        let mut bom = b"\xEF\xBB\xBF".to_vec();
        bom.extend_from_slice(cue.as_bytes());
        let cp1252 =
            b"FILE \"Pok\xE9mon.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
        let gdi = b"1\n1 0 4 2352 \"Pok\xE9mon.bin\" 0\n";
        for (sheet, bytes, system) in [
            ("Bom.cue", &bom[..], "psx"),
            ("Antigo.cue", &cp1252[..], "psx"),
            ("Antigo.gdi", &gdi[..], "dreamcast"),
        ] {
            let path = put(dir.path(), sheet, bytes);
            let id = identify(&path, system, None).unwrap();
            assert_eq!(id.name, "Pokémon.bin", "{sheet}");
            assert_eq!(id.hashes, Some(digests_of(&track)), "{sheet}");
            assert!(!id.provisional, "{sheet}");
        }
    }

    /// Cue cujo texto não se lê, ou cuja trilha ainda não chegou: provisório.
    /// Quando a trilha aparece, a mesma conta dá a identidade dela.
    #[test]
    fn unreadable_sheet_or_missing_track_is_identified_again_later() {
        let dir = tempfile::tempdir().unwrap();
        let text =
            "FILE \"Jogo (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
        let cue = put(dir.path(), "Jogo.cue", text.as_bytes());

        let id = identify(&cue, "psx", None).unwrap();
        assert!(id.provisional, "trilha que ainda não chegou");
        assert_eq!((id.name.as_str(), id.hashes.as_ref()), ("Jogo.cue", None));

        let track: Vec<u8> = (0..2352 * 4).map(|i| (i % 17) as u8).collect();
        put(dir.path(), "Jogo (Track 1).bin", &track);
        let id = identify(&cue, "psx", None).unwrap();
        assert!(!id.provisional);
        assert_eq!(id.name, "Jogo (Track 1).bin");
        assert_eq!(id.hashes, Some(digests_of(&track)));

        // O texto do cue não se lê (o arquivo existe, a leitura falha).
        let refuse = |p: &Path| -> std::io::Result<Box<dyn ReadSeek>> {
            if p.extension().is_some_and(|e| e == "cue") {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            }
            open_file(p)
        };
        let id = identify_with(&cue, "psx", None, &refuse, DISC_HASH_MAX).unwrap();
        assert!(id.provisional, "texto que não se lê");
        assert_eq!((id.name.as_str(), id.hashes), ("Jogo.cue", None));
    }

    /// Zip com uma entrada guardada sem compressão cujos dados foram
    /// estragados: ler a entrada falha (o CRC não bate).
    fn corrupt_zip(dir: &Path, entry: &str) -> PathBuf {
        let zip_path = dir.join(format!("{entry}.zip"));
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            zw.start_file(
                entry,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zw.write_all(&vec![0x5A; 64 * 1024]).unwrap();
            zw.finish().unwrap();
        }
        let mut bytes = std::fs::read(&zip_path).unwrap();
        let data = bytes.windows(64).position(|w| w == [0x5A; 64]).unwrap();
        bytes[data + 1000] ^= 0xFF;
        std::fs::write(&zip_path, &bytes).unwrap();
        assert!(read_archive_entry(&zip_path, entry).is_err());
        zip_path
    }

    /// Imagem de Saturn com `size` bytes: o cabeçalho do setor 0 e o resto
    /// preenchido.
    fn saturn_image(size: usize) -> Vec<u8> {
        let mut img = vec![0x5Au8; size];
        img[..16].copy_from_slice(b"SEGA SEGASATURN ");
        img[0x20..0x2A].copy_from_slice(b"MK-81022  ");
        img
    }

    /// `.cso` e `.chd` em qualquer tamanho, e o `.iso` acima do limite: o
    /// nome, o tamanho e o serial, sem ler a imagem inteira.
    #[test]
    fn cso_chd_and_iso_above_the_cap_are_never_hashed_whole() {
        let dir = tempfile::tempdir().unwrap();
        // Maior que o orçamento do serial (2 MiB): ler inteiro passaria dele.
        const SIZE: usize = 3 * 1024 * 1024;
        // Abaixo do tamanho da imagem: o `.iso` fica acima do limite.
        const CAP: u64 = 1024 * 1024;
        let iso = put(dir.path(), "Jogo (USA).iso", &saturn_image(SIZE));
        let cso = put(dir.path(), "Jogo (USA).cso", &vec![0x5A; SIZE]);
        let chd = put(dir.path(), "Jogo (USA).chd", &vec![0x5A; SIZE]);

        for (path, cap) in [(&iso, CAP), (&cso, DISC_HASH_MAX), (&chd, DISC_HASH_MAX)] {
            // Sistema sem serial: a identidade não lê um byte.
            let (id, read) = identify_counting(path, "disc", cap);
            assert_eq!(read, 0, "{path:?}");
            assert_eq!(id.hashes, None, "{path:?}");
            assert_eq!(id.size, SIZE as u64);
            assert_eq!(
                id.name,
                path.file_name().unwrap().to_string_lossy().into_owned()
            );
        }
        // Com o sistema, só o que o serial lê, dentro do orçamento.
        for (path, system, cap) in [
            (&iso, "saturn", CAP),
            (&cso, "psp", DISC_HASH_MAX),
            (&chd, "psx", DISC_HASH_MAX),
        ] {
            let (id, read) = identify_counting(path, system, cap);
            assert!(read <= READ_BUDGET, "{system}: leu {read} bytes");
            assert_eq!(id.hashes, None, "{system}");
        }
        let (id, _) = identify_counting(&iso, "saturn", CAP);
        assert_eq!(id.serial.as_deref(), Some("MK-81022"));

        // Dentro de um `.zip`: o tamanho vem do índice e nada é
        // descomprimido (ler a entrada estragada falharia).
        let zip_chd = corrupt_zip(dir.path(), "Jogo (USA).chd");
        let id = identify(&zip_chd, "psx", Some("Jogo (USA).chd")).unwrap();
        assert_eq!(id.hashes, None);
        assert_eq!((id.name.as_str(), id.size), ("Jogo (USA).chd", 64 * 1024));
        let zip_iso = corrupt_zip(dir.path(), "Jogo (USA).iso");
        let id = identify_with(
            &zip_iso,
            "saturn",
            Some("Jogo (USA).iso"),
            &open_file,
            32 * 1024,
        )
        .unwrap();
        assert_eq!(id.hashes, None, "iso acima do limite dentro do zip");
        assert_eq!((id.name.as_str(), id.size), ("Jogo (USA).iso", 64 * 1024));

        // E de um `.7z`, também pelo índice.
        let sz = dir.path().join("Jogo.7z");
        let mut w = sevenz_rust2::ArchiveWriter::create(&sz).unwrap();
        w.push_archive_entry(
            sevenz_rust2::ArchiveEntry::new_file("Jogo (USA).chd"),
            Some(Cursor::new(vec![0x5A; 5000])),
        )
        .unwrap();
        w.finish().unwrap();
        let id = identify(&sz, "psx", Some("Jogo (USA).chd")).unwrap();
        assert_eq!(id.hashes, None);
        assert_eq!((id.name.as_str(), id.size), ("Jogo (USA).chd", 5000));
    }

    /// Um `.iso` até o limite tem o hash (CRC32, MD5 e SHA1 numa leitura só)
    /// e o serial; um byte acima, só o serial, o nome e o tamanho. Dentro de
    /// um `.zip`, o mesmo limite, hasheado aos poucos.
    #[test]
    fn iso_up_to_the_cap_is_hashed_with_its_serial() {
        let dir = tempfile::tempdir().unwrap();
        const SIZE: usize = 3 * 1024 * 1024;
        let img = saturn_image(SIZE);
        let iso = put(dir.path(), "Jogo (USA).iso", &img);

        // Abaixo do limite de verdade, e exatamente no limite.
        for cap in [DISC_HASH_MAX, SIZE as u64] {
            let (id, read) = identify_counting(&iso, "saturn", cap);
            assert_eq!(id.hashes, Some(digests_of(&img)), "limite {cap}");
            assert_eq!(id.serial.as_deref(), Some("MK-81022"), "limite {cap}");
            assert_eq!((id.name.as_str(), id.size), ("Jogo (USA).iso", SIZE as u64));
            // Uma leitura inteira pro hash, mais o pouco que o serial lê.
            let whole = SIZE as u64;
            assert!((whole..=whole + READ_BUDGET).contains(&read), "leu {read}");
        }

        // Um byte acima do limite: sem a leitura inteira.
        let (id, read) = identify_counting(&iso, "saturn", SIZE as u64 - 1);
        assert_eq!(id.hashes, None);
        assert_eq!(id.serial.as_deref(), Some("MK-81022"));
        assert_eq!(id.size, SIZE as u64);
        assert!(read <= READ_BUDGET, "leu {read}");

        let zip_path = dir.path().join("Jogo.zip");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            zw.start_file(
                "Jogo (USA).iso",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
            zw.write_all(&img).unwrap();
            zw.finish().unwrap();
        }
        let id = identify(&zip_path, "saturn", Some("Jogo (USA).iso")).unwrap();
        assert_eq!(id.hashes, Some(digests_of(&img)));
        assert_eq!(id.size, SIZE as u64);
        let id = identify_with(
            &zip_path,
            "saturn",
            Some("Jogo (USA).iso"),
            &open_file,
            SIZE as u64 - 1,
        )
        .unwrap();
        assert_eq!((id.hashes, id.size), (None, SIZE as u64));
    }

    /// Trilha ou imagem acima do limite: o nome e o tamanho dela, sem ler.
    /// O limite de verdade (1,5 GiB) está preso por um `assert!` de compilação
    /// junto do [`DISC_HASH_MAX`]; aqui ele vem pequeno pra não gravar GBs.
    #[test]
    fn disc_image_above_the_limit_is_identified_by_name_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let track: Vec<u8> = (0..64 * 1024).map(|i| (i % 7) as u8).collect();
        put(dir.path(), "Jogo (Track 1).bin", &track);
        let text = "FILE \"Jogo (Track 1).bin\" BINARY
  TRACK 01 MODE2/2352
    INDEX 01 00:00:00
";
        let cue = put(dir.path(), "Jogo.cue", text.as_bytes());

        let (id, read) = identify_counting(&cue, "disc", 32 * 1024);
        assert_eq!(id.name, "Jogo (Track 1).bin", "a trilha, não o cue");
        assert_eq!(id.size, track.len() as u64);
        assert_eq!(id.hashes, None);
        assert_eq!(read, text.len() as u64, "só o texto do cue foi lido");
        // No limite: a trilha é lida e hasheada.
        let (id, _) = identify_counting(&cue, "disc", track.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&track)));

        // Imagem solta (`.img`, `.pbp`...) segue a mesma regra.
        let img = put(dir.path(), "Jogo.img", &track);
        let (id, read) = identify_counting(&img, "disc", 32 * 1024);
        assert_eq!((id.hashes, id.size, read), (None, track.len() as u64, 0));
        let (id, _) = identify_counting(&img, "disc", track.len() as u64);
        assert_eq!(id.hashes, Some(digests_of(&track)));
    }
}
