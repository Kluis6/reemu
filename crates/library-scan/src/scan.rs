//! Varredura de um diretório de ROMs → `domain::library::Rom` (por hash).

use crate::archive::{is_supported_archive, peek_archive};
use crate::identity::identify;
use crate::sector::ext_of;
use crate::systems::{
    folder_only_exts, folder_only_flat, system_for_extension, system_for_rom_entry,
    system_from_folder_name, AMBIGUOUS_DISC_EXTS,
};
use domain::library::{dirksimple_game_id, Rom, RomRepository, DIRKSIMPLE_OFFICIAL_GAMES};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use walkdir::WalkDir;

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

fn is_whole_archive(system_id: &str, ext: &str) -> bool {
    WHOLE_ARCHIVE_SYSTEMS.contains(&system_id) || (system_id == "x68000" && ext == "zip")
}

/// Calcula a identidade de um arquivo cujo `system_id` já está gravado na
/// biblioteca (reaproveitado pelo passo em segundo plano para linhas antigas).
pub fn identify_known_rom(path: &Path, system_id: &str) -> std::io::Result<crate::Identity> {
    let ext = ext_of(path);
    let archived_entry = if is_supported_archive(&ext) && !is_whole_archive(system_id, &ext) {
        let found = peek_archive(path, Some(system_id)).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "arquivo compactado sem entrada reconhecida",
            )
        })?;
        Some(found.entry)
    } else {
        None
    };
    identify(path, system_id, archived_entry.as_deref())
}

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
        && peek_archive(path, None).is_some_and(|a| {
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
    if is_mame_companion(path, &ext)
        || is_daphne_rom_companion(path, &ext)
        || is_unplayable_disc_dump(path, root, &ext, false)
    {
        return false;
    }
    system_for_rom_file(path, &ext).is_some()
        || is_supported_archive(&ext)
        || system_from_folder_ext(path, root, &ext).is_some()
}

fn system_for_rom_file(path: &Path, ext: &str) -> Option<&'static str> {
    if ext == "txt" {
        return is_daphne_framefile(path).then_some("laserdisc");
    }
    if ext == "ogv" {
        return dirksimple_game_id(path)
            .is_some_and(|id| DIRKSIMPLE_OFFICIAL_GAMES.contains(&id.as_str()))
            .then_some("laserdisc");
    }
    if !matches!(ext, "st" | "xdf" | "dim" | "p") {
        return system_for_extension(ext);
    }
    let file = std::fs::File::open(path).ok()?;
    system_for_rom_entry(ext, file.metadata().ok()?.len(), file)
}

fn named_dir(path: &Path, name: &str) -> bool {
    path.file_name()
        .is_some_and(|part| part.eq_ignore_ascii_case(name))
}

/// Reconhece o layout Daphne com uma pasta de vídeo na primeira linha e
/// pelo menos um registro de quadro apontando para vídeo MPEG.
fn is_daphne_framefile(path: &Path) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    if !named_dir(dir, "framefile")
        && !dir.parent().is_some_and(|parent| named_dir(parent, "vldp"))
        && !dir
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("daphne"))
    {
        return false;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut lines = BufReader::new(file.take(8192)).lines();
    if !lines
        .next()
        .is_some_and(|line| line.is_ok_and(|dir| !dir.trim().is_empty()))
    {
        return false;
    }
    for line in lines {
        let Ok(line) = line else { return false };
        let mut words = line.split_whitespace();
        let Some(frame) = words.next() else { continue };
        return frame.parse::<i32>().is_ok()
            && words.next().is_some_and(|media| {
                matches!(ext_of(Path::new(media)).as_str(), "m2v" | "mpg" | "mpeg")
            });
    }
    false
}

fn is_daphne_rom_companion(path: &Path, ext: &str) -> bool {
    if ext != "zip" {
        return false;
    }
    let Some(roms) = path.parent().filter(|dir| named_dir(dir, "roms")) else {
        return false;
    };
    let Some(root) = roms.parent() else {
        return false;
    };
    let Some(game) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let rooted_daphne = root
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(system_from_folder_name)
        == Some("laserdisc");
    if rooted_daphne && (root.join("framefile").is_dir() || root.join("vldp").is_dir()) {
        return true;
    }
    let name = format!("{game}.txt");
    [
        root.join("framefile").join(&name),
        root.join("vldp").join(game).join(&name),
        root.join(format!("{game}.daphne")).join(&name),
    ]
    .iter()
    .any(|file| is_daphne_framefile(file))
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
        if is_daphne_rom_companion(path, &ext) {
            continue;
        }

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
        let (system_id, archived_entry): (&str, Option<String>) = if AMBIGUOUS_DISC_EXTS
            .contains(&ext.as_str())
        {
            // Extensão de disco: (1) pasta ancestral (RetroBat/ES-DE); (2)
            // sistema específico da extensão (`.gdi`→dreamcast, `.pbp`→psp);
            // (3) assinatura no conteúdo do arquivo; (4) balde genérico.
            let sys = system_from_dirs(&ancestor_dirs(path, dir))
                .or_else(|| system_for_extension(&ext).filter(|s| *s != "disc"))
                .or_else(|| crate::disc_sniff::identify(path))
                .unwrap_or("disc");
            (sys, None)
        } else if ext == "hdf" && system_from_dirs(&ancestor_dirs(path, dir)) == Some("x68000") {
            ("x68000", None)
        } else if ext == "tzx" && system_from_dirs(&ancestor_dirs(path, dir)) == Some("zx81") {
            ("zx81", None)
        } else if let Some(sys) = system_for_rom_file(path, &ext) {
            (sys, None)
        } else if let Some(sys) = system_from_folder_ext(path, dir, &ext) {
            (sys, None)
        } else if is_supported_archive(&ext) {
            let folder_system = system_from_dirs(&ancestor_dirs(path, dir));
            if let Some(sys) = folder_system.filter(|s| WHOLE_ARCHIVE_SYSTEMS.contains(s)) {
                // Set de arcade/NAOMI/Atomiswave: sem "a ROM" dentro do
                // arquivo (chip dumps avulsos) — ele inteiro é a unidade,
                // hash do arquivo.
                (sys, None)
            } else {
                match peek_archive(path, folder_system) {
                    // O PX68k ordena e monta a mídia interna do ZIP. O
                    // arquivo inteiro é a unidade de jogo e identificação.
                    Some(a) if is_whole_archive(a.system_id, &ext) => (a.system_id, None),
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
