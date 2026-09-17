//! Suporte a ROMs dentro de `.zip`/`.7z`. O scan olha a 1ª entrada com
//! extensão reconhecida; o hash (CRC32/MD5) é o da ROM **crua** descomprimida,
//! não a do arquivo — é o que casa com o ScreenScraper. O loader
//! (`core-loader-desktop`) extrai pra um arquivo temporário na hora de carregar.

use crate::systems::system_for_extension;
use sevenz_rust2::{ArchiveReader, Password};
use std::io::{BufReader, Read};
use std::path::Path;

/// Uma ROM localizada dentro de um arquivo comprimido.
pub struct ArchivedRom {
    /// Nome da entrada dentro do arquivo.
    pub entry: String,
    /// `system_id` inferido da extensão da entrada.
    pub system_id: &'static str,
}

/// `true` se a extensão é um formato de arquivo que o scan sabe abrir.
pub fn is_supported_archive(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("7z")
}

fn is_7z(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("7z"))
}

/// Primeira entrada do arquivo (`.zip`/`.7z`) com extensão de ROM reconhecida.
pub fn peek_archive(path: &Path) -> Option<ArchivedRom> {
    if is_7z(path) {
        peek_7z(path)
    } else {
        peek_zip(path)
    }
}

/// Bytes descomprimidos de uma entrada do arquivo (`.zip`/`.7z`).
pub fn read_archive_entry(path: &Path, entry: &str) -> std::io::Result<Vec<u8>> {
    if is_7z(path) {
        read_7z_entry(path, entry)
    } else {
        read_zip_entry(path, entry)
    }
}

fn peek_zip(path: &Path) -> Option<ArchivedRom> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).ok()?;
    for i in 0..zip.len() {
        let e = zip.by_index(i).ok()?;
        if !e.is_file() {
            continue;
        }
        let name = e.name().to_string();
        if let Some(sys) = Path::new(&name)
            .extension()
            .and_then(|x| x.to_str())
            .and_then(system_for_extension)
        {
            return Some(ArchivedRom {
                entry: name,
                system_id: sys,
            });
        }
    }
    None
}

fn read_zip_entry(path: &Path, entry: &str) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let mut f = zip
        .by_name(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
    let mut buf = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut buf)?;
    Ok(buf)
}

fn peek_7z(path: &Path) -> Option<ArchivedRom> {
    let mut r = ArchiveReader::open(path, Password::empty()).ok()?;
    let mut found = None;
    r.for_each_entries(|e, _reader| {
        if found.is_none() && !e.is_directory() {
            if let Some(sys) = Path::new(e.name())
                .extension()
                .and_then(|x| x.to_str())
                .and_then(system_for_extension)
            {
                found = Some(ArchivedRom {
                    entry: e.name().to_string(),
                    system_id: sys,
                });
            }
        }
        Ok(true)
    })
    .ok()?;
    found
}

fn read_7z_entry(path: &Path, entry: &str) -> std::io::Result<Vec<u8>> {
    let mut r = ArchiveReader::open(path, Password::empty())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    r.read_file(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))
}
