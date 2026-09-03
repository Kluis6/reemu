//! Varredura de um diretório de ROMs → `domain::library::Rom` (por hash).

use crate::archive::{is_supported_archive, peek_zip, read_zip_entry};
use crate::hash::FileRomHasher;
use crate::systems::system_for_extension;
use domain::library::{Rom, RomRepository};
use std::io::Cursor;
use std::path::Path;
use walkdir::WalkDir;

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

        // ROM crua ou dentro de um .zip. Pro zip, o `system_id` e o hash vêm
        // da entrada interna (o CRC precisa ser o da ROM, não o do arquivo).
        let (system_id, archived_entry): (&str, Option<String>) =
            if let Some(sys) = system_for_extension(&ext) {
                (sys, None)
            } else if is_supported_archive(&ext) {
                match peek_zip(path) {
                    Some(a) => (a.system_id, Some(a.entry)),
                    None => {
                        report.skipped_unrecognized += 1;
                        continue;
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
