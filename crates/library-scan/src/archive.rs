//! Suporte a ROMs dentro de `.zip`/`.7z`. O scan olha a 1ª entrada com
//! extensão reconhecida; o hash (CRC32/MD5) é o da ROM **crua** descomprimida,
//! não a do arquivo — é o que casa com o ScreenScraper. O loader
//! (`core-loader-desktop`) extrai pra um arquivo temporário na hora de carregar.

use crate::hash::{FileRomHasher, RomHashes};
use crate::systems::{folder_only_exts, system_for_rom_entry, X68000_DISK_EXTS};
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
    if required_sheet_priority(path, seven_zip) {
        for &sheet in ARCHIVE_SHEET_PRIORITY {
            if let Some(rom) = peek(Some(sheet)) {
                return Some(rom);
            }
        }
    }
    peek(None)
}

fn required_sheet_priority(path: &Path, seven_zip: bool) -> bool {
    if seven_zip {
        ArchiveReader::open(path, Password::empty())
            .ok()
            .is_some_and(|r| {
                r.archive().files.iter().any(|f| {
                    !f.is_directory()
                        && Path::new(f.name())
                            .extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| {
                                ARCHIVE_SHEET_PRIORITY
                                    .iter()
                                    .any(|s| e.eq_ignore_ascii_case(s))
                            })
                })
            })
    } else {
        std::fs::File::open(path)
            .ok()
            .and_then(|f| zip::ZipArchive::new(BufReader::new(f)).ok())
            .is_some_and(|mut z| {
                (0..z.len()).any(|i| {
                    z.by_index(i).ok().is_some_and(|e| {
                        e.is_file()
                            && e.name().ok().is_some_and(|n| {
                                Path::new(n.as_ref())
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .is_some_and(|ext| {
                                        ARCHIVE_SHEET_PRIORITY
                                            .iter()
                                            .any(|s| ext.eq_ignore_ascii_case(s))
                                    })
                            })
                    })
                })
            })
    }
}

const ARCHIVE_DISC_FOLDER_SYSTEMS: &[&str] = &[
    "psx",
    "ps2",
    "psp",
    "saturn",
    "segacd",
    "dreamcast",
    "gamecube",
    "wii",
    "3do",
    "pcenginecd",
    "pcfx",
    "neogeocd",
    "cdi",
];

const ARCHIVE_SHEET_PRIORITY: &[&str] = &["m3u", "cue", "gdi", "ccd"];

fn archive_disc_folder_system(folder_system: Option<&str>) -> Option<&'static str> {
    let f = folder_system?;
    ARCHIVE_DISC_FOLDER_SYSTEMS
        .iter()
        .copied()
        .find(|s| *s == f)
}

fn sniff_archived_disc_prefix(mut reader: impl std::io::Read) -> Option<&'static str> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut filled = 0usize;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => break,
        }
    }
    if filled == 0 {
        return None;
    }
    let head = &buf[..filled];
    if head.len() >= 0x20
        && head[0x18..0x1C] != [0x5D, 0x1C, 0x9E, 0xA3]
        && head[0x1C..0x20] == [0xC2, 0x33, 0x9F, 0x3D]
    {
        return Some("gamecube");
    }
    if (head.starts_with(b"RVZ\x01") || head.starts_with(b"WIA\x01"))
        && head.len() >= 0xD8
        && u32::from_be_bytes([head[0x48], head[0x49], head[0x4A], head[0x4B]]) == 1
        && head[0x58 + 0x1C..0x58 + 0x20] == [0xC2, 0x33, 0x9F, 0x3D]
    {
        return Some("gamecube");
    }
    crate::disc_sniff::sniff_header_bytes(head)
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
    if matches!(ext.to_ascii_lowercase().as_str(), "gcm" | "gcz" | "tgc") {
        return Some("gamecube");
    }
    if matches!(ext.to_ascii_lowercase().as_str(), "rvz" | "wia") {
        return sniff_archived_disc_prefix(reader)
            .or_else(|| archive_disc_folder_system(folder_system))
            .or(Some("disc"));
    }
    if matches!(ext.to_ascii_lowercase().as_str(), "iso" | "img") {
        return archive_disc_folder_system(folder_system)
            .or_else(|| sniff_archived_disc_prefix(reader))
            .or(Some("disc"));
    }
    // As mídias do DirkSimple podem ter vários GB. O core abre o vídeo solto.
    // O Ardens abre o pacote .arduboy diretamente, sem uma camada ZIP ou 7z externa.
    let resolved = system_for_rom_entry(ext, size, reader)
        .filter(|system| !matches!(*system, "laserdisc" | "arduboy"))?;
    if resolved == "disc" {
        return Some(archive_disc_folder_system(folder_system).unwrap_or("disc"));
    }
    Some(resolved)
}

/// Lê até `max_bytes` descomprimidos de `entry` dentro de um `.zip`/`.7z`.
/// Usado pelo cálculo de hash do RetroAchievements (`rcheevos` limita buffers
/// de ROM a 64 MiB), sem precisar gravar arquivo temporário em disco.
pub fn read_archive_entry_capped(
    path: &Path,
    entry: &str,
    max_bytes: usize,
) -> std::io::Result<Vec<u8>> {
    if is_7z(path) {
        read_7z_entry_capped(path, entry, max_bytes)
    } else {
        read_zip_entry_capped(path, entry, max_bytes)
    }
}

/// Bytes descomprimidos de uma entrada do arquivo (`.zip`/`.7z`), inteiros
/// na memória. Só os testes usam (a varredura hasheia aos poucos, com
/// [`hash_archive_entry`]).
#[cfg(test)]
pub(crate) fn read_archive_entry(path: &Path, entry: &str) -> std::io::Result<Vec<u8>> {
    read_archive_entry_capped(path, entry, usize::MAX)
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

fn read_zip_entry_capped(path: &Path, entry: &str, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let f = zip
        .by_name(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e))?;
    let cap = usize::try_from(f.size())
        .unwrap_or(max_bytes)
        .min(max_bytes);
    let mut buf = Vec::with_capacity(cap);
    f.take(max_bytes as u64).read_to_end(&mut buf)?;
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

fn read_7z_entry_capped(path: &Path, entry: &str, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    let mut r = ArchiveReader::open(path, Password::empty())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    r.set_thread_count(1);
    let mut out = None;
    r.for_each_entries(|e, reader| {
        if out.is_some() {
            return Ok(false);
        }
        if e.name() == entry {
            let cap = usize::try_from(e.size())
                .unwrap_or(max_bytes)
                .min(max_bytes);
            let mut buf = Vec::with_capacity(cap);
            out = Some(
                reader
                    .take(max_bytes as u64)
                    .read_to_end(&mut buf)
                    .map(|_| buf),
            );
            return Ok(false);
        }
        std::io::copy(reader, &mut std::io::sink())?;
        Ok(true)
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    out.unwrap_or_else(|| Err(std::io::ErrorKind::NotFound.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn zip_disc_archive_prefers_cue_over_bin_and_keeps_disc_folder_system() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("Resident Evil 2 (USA) (Disc 1).zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("Resident Evil 2 (Track 1).bin", opts)
                .unwrap();
            zw.write_all(&vec![0u8; 2352 * 4]).unwrap();
            zw.start_file("Resident Evil 2.cue", opts).unwrap();
            zw.write_all(
                b"FILE \"Resident Evil 2 (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
            )
            .unwrap();
            zw.finish().unwrap();
        }

        let rom = peek_archive(&zip_path, Some("psx")).expect("archived cue");
        assert_eq!(rom.entry, "Resident Evil 2.cue");
        assert_eq!(rom.system_id, "psx");
    }

    #[test]
    fn zip_gamecube_iso_sniffs_magic_without_folder_hint() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("Spawn Armageddon.zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            let mut header = vec![0u8; 2048];
            header[0..6].copy_from_slice(b"GPWE69");
            header[0x1C..0x20].copy_from_slice(&[0xC2, 0x33, 0x9F, 0x3D]);
            zw.start_file("Spawn Armageddon.iso", opts).unwrap();
            zw.write_all(&header).unwrap();
            zw.finish().unwrap();
        }

        let rom = peek_archive(&zip_path, None).expect("archived gc iso");
        assert_eq!(rom.entry, "Spawn Armageddon.iso");
        assert_eq!(rom.system_id, "gamecube");
    }
}
