//! Varredura de um diretório de ROMs → `domain::library::Rom` (por hash).

use crate::archive::{is_supported_archive, peek_archive, read_archive_entry};
use crate::hash::FileRomHasher;
use crate::systems::{
    folder_only_exts, folder_only_flat, system_for_extension, system_from_folder_name,
    AMBIGUOUS_DISC_EXTS,
};
use domain::library::{Rom, RomRepository};
use std::io::Cursor;
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

/// Extensão reconhecida (ROM crua, arquivo comprimido suportado, ou
/// extensão genérica dentro da pasta de um sistema que a aceita).
fn recognized(path: &Path, root: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    if is_mame_companion(path, &ext) {
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
    /// Catalogadas antes mas que não são jogo (`.chd` de set do MAME).
    pub removed: usize,
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
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        // Parte de um set do MAME (`.chd` ao lado do `.zip`): não é jogo; se
        // uma varredura antiga catalogou, sai da biblioteca.
        if is_mame_companion(path, &ext) {
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

        let hash = match &archived_entry {
            None => FileRomHasher::hash_file(&path_str),
            Some(entry) => read_archive_entry(path, entry)
                .and_then(|bytes| FileRomHasher::hash_reader(Cursor::new(bytes))),
        };
        let hash = match hash {
            Ok(h) => h,
            Err(e) => {
                log::warn!("hash {path_str}: {e}");
                report.errors += 1;
                continue;
            }
        };

        repo.add(&Rom {
            id: uuid::Uuid::new_v4().to_string(),
            file_path: path_str.into_owned(),
            crc32: hash.crc32,
            md5: hash.md5,
            system_id: system_id.to_string(),
            added_at: now_unix,
            last_played_at: None,
            is_favorite: false,
            user_title: None,
        })
        .await?;
        report.added += 1;
    }

    Ok(report)
}
