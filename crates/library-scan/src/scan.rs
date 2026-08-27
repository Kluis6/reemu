//! Varredura de um diretório de ROMs → `domain::library::Rom` (por hash).

use crate::hash::FileRomHasher;
use crate::systems::system_for_extension;
use domain::library::{Rom, RomRepository};
use std::path::Path;
use walkdir::WalkDir;

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

/// Varre `dir` recursivamente e adiciona ao `repo` as ROMs ainda não
/// catalogadas (dedup por `file_path`). Extensões desconhecidas são puladas.
pub async fn scan_into<R: RomRepository + ?Sized>(
    repo: &R,
    dir: &Path,
    now_unix: i64,
) -> Result<ScanReport, ScanError> {
    let mut report = ScanReport::default();

    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(system_id) = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(system_for_extension)
        else {
            report.skipped_unrecognized += 1;
            continue;
        };
        report.found += 1;

        let path_str = path.to_string_lossy();
        if repo.find_by_path(&path_str).await?.is_some() {
            report.skipped_known += 1;
            continue;
        }

        let hash = match FileRomHasher::hash_file(&path_str) {
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
        })
        .await?;
        report.added += 1;
    }

    Ok(report)
}
