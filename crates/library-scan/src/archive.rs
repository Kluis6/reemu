//! Suporte a ROMs dentro de `.zip`. O scan olha a 1ª entrada com extensão
//! reconhecida; o hash (CRC32/MD5) é o da ROM **crua** descomprimida, não o do
//! `.zip` — é o que casa com o ScreenScraper. O loader (`core-loader-desktop`)
//! extrai pra um arquivo temporário na hora de carregar.

use crate::systems::system_for_extension;
use std::io::{BufReader, Read};
use std::path::Path;

/// Uma ROM localizada dentro de um arquivo comprimido.
pub struct ArchivedRom {
    /// Nome da entrada dentro do `.zip`.
    pub entry: String,
    /// `system_id` inferido da extensão da entrada.
    pub system_id: &'static str,
}

/// `true` se a extensão é um formato de arquivo que o scan sabe abrir.
pub fn is_supported_archive(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("zip")
}

/// Primeira entrada do `.zip` com extensão de ROM reconhecida.
pub fn peek_zip(path: &Path) -> Option<ArchivedRom> {
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

/// Bytes descomprimidos de uma entrada do `.zip`.
pub fn read_zip_entry(path: &Path, entry: &str) -> std::io::Result<Vec<u8>> {
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
