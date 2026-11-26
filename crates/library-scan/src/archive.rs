//! Suporte a ROMs dentro de `.zip`/`.7z`. O scan olha a 1ª entrada com
//! extensão reconhecida; o hash (CRC32/MD5) é o da ROM **crua** descomprimida,
//! não a do arquivo — é o que casa com o ScreenScraper. O loader
//! (`core-loader-desktop`) extrai pra um arquivo temporário na hora de carregar.

use crate::hash::{FileRomHasher, RomHashes};
use crate::systems::{folder_only_exts, system_for_rom_entry, X68000_DISK_EXTS};
use sevenz_rust2::{ArchiveReader, Password};
use std::io::BufReader;
#[cfg(test)]
use std::io::Read;
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
pub fn peek_archive(path: &Path, folder_system: Option<&str>) -> Option<ArchivedRom> {
    let seven_zip = is_7z(path);
    let peek = |required_extension| {
        if seven_zip {
            peek_7z(
                path,
                folder_system.filter(|system| *system != "msx"),
                required_extension,
            )
        } else {
            peek_zip(path, folder_system, required_extension)
        }
    };
    if folder_system == Some("megaduck") {
        // DUCK tem prioridade sobre arquivos BIN na mesma coleção.
        if let Some(rom) = peek(Some("duck")) {
            return Some(rom);
        }
    }
    peek(None)
}

fn archive_entry_system(
    ext: &str,
    size: u64,
    reader: impl std::io::Read,
    folder_system: Option<&str>,
) -> Option<&'static str> {
    if folder_system == Some("x68000")
        && X68000_DISK_EXTS.iter().any(|x| ext.eq_ignore_ascii_case(x))
    {
        return Some("x68000");
    }
    if folder_system == Some("msx")
        && folder_only_exts("msx")
            .iter()
            .any(|x| ext.eq_ignore_ascii_case(x))
    {
        return Some("msx");
    }
    if folder_system == Some("zx81") && ext.eq_ignore_ascii_case("tzx") {
        return Some("zx81");
    }
    if folder_system == Some("megaduck") && ext.eq_ignore_ascii_case("bin") {
        return Some("megaduck");
    }
    // As mídias do DirkSimple podem ter vários GB. O core abre o vídeo solto.
    // O Ardens abre o pacote .arduboy diretamente, sem uma camada ZIP ou 7z externa.
    system_for_rom_entry(ext, size, reader)
        .filter(|system| !matches!(*system, "laserdisc" | "arduboy"))
}

/// Bytes descomprimidos de uma entrada do arquivo (`.zip`/`.7z`), inteiros
/// na memória. Só os testes usam (a varredura hasheia aos poucos, com
/// [`hash_archive_entry`]).
#[cfg(test)]
pub(crate) fn read_archive_entry(path: &Path, entry: &str) -> std::io::Result<Vec<u8>> {
    if is_7z(path) {
        read_7z_entry(path, entry)
    } else {
        read_zip_entry(path, entry)
    }
}

/// CRC32, MD5 e SHA1 de uma entrada, descomprimida aos poucos direto no
/// hash: nunca inteira na memória (uma imagem de disco pode ter 1,5 GiB).
/// `name` é o nome que vai no resultado. Para depois de `cap` bytes
/// descomprimidos, valha o índice o que valer: `Ok(None)`, sem hash.
pub(crate) fn hash_archive_entry(
    path: &Path,
    entry: &str,
    name: &str,
    cap: u64,
) -> std::io::Result<Option<RomHashes>> {
    if !is_7z(path) {
        let file = std::fs::File::open(path)?;
        let mut zip = zip::ZipArchive::new(BufReader::new(file))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let f = zip
            .by_name(entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
        return FileRomHasher::hash_reader_capped(f, name, cap);
    }
    let mut r = ArchiveReader::open(path, Password::empty())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    // Com mais de uma thread o LZMA2 descomprime cada pedaço inteiro num
    // `Vec` (`Lzma2ReaderMt`); com uma, vai aos poucos pelo dicionário.
    r.set_thread_count(1);
    let mut out = None;
    r.for_each_entries(|e, reader| {
        if out.is_some() {
            return Ok(false);
        }
        if e.name() == entry {
            out = Some(FileRomHasher::hash_reader_capped(reader, name, cap));
            return Ok(false);
        }
        // As entradas de um bloco vêm em sequência: a de antes é lida (e
        // jogada fora) pra chegar na seguinte.
        std::io::copy(reader, &mut std::io::sink())?;
        Ok(true)
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    out.unwrap_or_else(|| Err(std::io::ErrorKind::NotFound.into()))
}

/// Tamanho descomprimido de uma entrada, lido do índice do arquivo (nada é
/// descomprimido).
pub(crate) fn archive_entry_size(path: &Path, entry: &str) -> std::io::Result<u64> {
    if is_7z(path) {
        let r = ArchiveReader::open(path, Password::empty())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
        r.archive()
            .files
            .iter()
            .find(|f| f.name() == entry)
            .map(|f| f.size())
            .ok_or_else(|| std::io::ErrorKind::NotFound.into())
    } else {
        let file = std::fs::File::open(path)?;
        let mut zip = zip::ZipArchive::new(BufReader::new(file))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let f = zip
            .by_name(entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
        Ok(f.size())
    }
}

fn peek_zip(
    path: &Path,
    folder_system: Option<&str>,
    required_extension: Option<&str>,
) -> Option<ArchivedRom> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).ok()?;
    for i in 0..zip.len() {
        let mut e = zip.by_index(i).ok()?;
        if !e.is_file() {
            continue;
        }
        // zip 9: nome que não se lê de forma consistente vira erro; essa
        // entrada é pulada.
        let Ok(name) = e.name() else {
            continue;
        };
        let name = name.into_owned();
        if let Some(sys) = Path::new(&name)
            .extension()
            .and_then(|x| x.to_str())
            .filter(|ext| {
                required_extension.is_none_or(|required| ext.eq_ignore_ascii_case(required))
            })
            .and_then(|ext| archive_entry_system(ext, e.size(), &mut e, folder_system))
        {
            return Some(ArchivedRom {
                entry: name,
                system_id: sys,
            });
        }
    }
    None
}

#[cfg(test)]
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

fn peek_7z(
    path: &Path,
    folder_system: Option<&str>,
    required_extension: Option<&str>,
) -> Option<ArchivedRom> {
    let mut r = ArchiveReader::open(path, Password::empty()).ok()?;
    let mut found = None;
    r.for_each_entries(|e, reader| {
        if found.is_none() && !e.is_directory() {
            if let Some(sys) = Path::new(e.name())
                .extension()
                .and_then(|x| x.to_str())
                .filter(|ext| {
                    required_extension.is_none_or(|required| ext.eq_ignore_ascii_case(required))
                })
                .and_then(|ext| archive_entry_system(ext, e.size(), reader, folder_system))
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

#[cfg(test)]
fn read_7z_entry(path: &Path, entry: &str) -> std::io::Result<Vec<u8>> {
    let mut r = ArchiveReader::open(path, Password::empty())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    r.read_file(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))
}
