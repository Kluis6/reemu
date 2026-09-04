//! Varredura de um diretório de ROMs → `domain::library::Rom` (por hash).

use crate::archive::{is_supported_archive, peek_zip, read_zip_entry};
use crate::hash::FileRomHasher;
use crate::systems::{system_for_extension, system_from_folder_name, AMBIGUOUS_DISC_EXTS};
use domain::library::{Rom, RomRepository};
use std::io::Cursor;
use std::path::Path;
use walkdir::WalkDir;

/// Pastas ancestrais de `path` (relativas a `root`), da mais próxima da raiz
/// pra mais próxima do arquivo — ex: `<root>/psx/Game (USA).iso` → `["psx"]`.
/// Mesma técnica de `decoration.rs::classify` pra achar o "nome de sistema"
/// mais perto do arquivo numa biblioteca organizada por pasta (RetroBat/ES-DE).
fn ancestor_dirs(path: &Path, root: &Path) -> Vec<String> {
    path.strip_prefix(root)
        .ok()
        .and_then(|rel| rel.parent())
        .map(|p| {
            p.iter()
                .filter_map(|c| c.to_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Tenta achar um `system_id` nas pastas ancestrais, da mais próxima do
/// arquivo pra mais longe. `None` se nenhuma bater em `system_from_folder_name`.
fn system_from_dirs(dirs: &[String]) -> Option<&'static str> {
    dirs.iter().rev().find_map(|d| system_from_folder_name(d))
}

/// Extensão reconhecida (ROM crua ou arquivo comprimido suportado).
fn recognized(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| system_for_extension(e).is_some() || is_supported_archive(e))
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
        .filter(|e| recognized(e.path()))
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

        // ROM crua, dentro de um .zip, ou imagem de disco (extensão
        // ambígua entre vários sistemas — PS1/PS2/Saturn/Dreamcast/PSP/...).
        // Pro zip de cartucho, o `system_id`/hash vêm da entrada interna (o
        // CRC precisa ser o da ROM, não o do arquivo); pro zip de arcade
        // (MAME/FBNeo — sem entrada de cartucho reconhecível dentro) e pra
        // disco, o arquivo INTEIRO é a "ROM".
        let (system_id, archived_entry): (&str, Option<String>) =
            if AMBIGUOUS_DISC_EXTS.contains(&ext.as_str()) {
                // Extensão de disco: tenta a pasta ancestral (biblioteca
                // organizada por sistema, RetroBat/ES-DE); sem sinal de
                // pasta, cai no balde genérico de sempre.
                let sys = system_from_dirs(&ancestor_dirs(path, dir))
                    .unwrap_or_else(|| system_for_extension(&ext).unwrap_or("disc"));
                (sys, None)
            } else if let Some(sys) = system_for_extension(&ext) {
                (sys, None)
            } else if is_supported_archive(&ext) {
                let is_arcade = system_from_dirs(&ancestor_dirs(path, dir)) == Some("arcade");
                if is_arcade {
                    // Set de arcade: sem "a ROM" dentro do zip (chip dumps
                    // avulsos) — o zip inteiro é a unidade, hash do arquivo.
                    ("arcade", None)
                } else {
                    match peek_zip(path) {
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
        if repo.find_by_path(&path_str).await?.is_some() {
            report.skipped_known += 1;
            continue;
        }

        let hash = match &archived_entry {
            None => FileRomHasher::hash_file(&path_str),
            Some(entry) => read_zip_entry(path, entry)
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
        })
        .await?;
        report.added += 1;
    }

    Ok(report)
}
