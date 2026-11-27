//! Extração de ROM de dentro de um `.zip`/`.7z` na hora de carregar o core.
//!
//! O scan (`library-scan`) já cataloga o arquivo como uma ROM (system + hash
//! da entrada interna). Aqui, no load, a entrada principal é extraída para um
//! arquivo temporário (ou pasta temporária quando se trata de `.cue`/`.gdi`/`.m3u`
//! acompanhado de trilhas `.bin`/`.raw`) e removida automaticamente no `Drop`.

use domain::library::{is_zx81_program, system_for_st, ST_HEADER_LEN, ZX81_P_HEADER_LEN};
use sevenz_rust2::{ArchiveReader, Password};
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};

/// Extensões multi-arquivo de imagem de disco: exigem extrair os arquivos
/// irmãos (`.bin`, `.img`, `.raw`, `.iso`, `.wav`) na mesma pasta temporária
/// preservando os nomes relativos referenciados por `FILE "..."`.
const SHEET_EXTS: &[&str] = &["m3u", "cue", "gdi", "ccd"];

/// Extensões de ROM e mídia de disco reconhecidas dentro de `.zip`/`.7z`.
const ROM_EXTS: &[&str] = &[
    "nes", "fds", "unif", "unf", "sfc", "smc", "swc", "fig", "bs", "gb", "gbc", "gba", "srl",
    "n64", "z64", "v64", "ndd", "md", "smd", "gen", "sgd", "sms", "gg", "pce", "sgx", "a26", "a78",
    "lnx", "ws", "wsc", "ngp", "ngc", "32x", "vb", "col", "int", "nds", "dsi", "ids", "sg", "a52",
    "atr", "xfd", "atx", "xex", "j64", "jag", "min", "sv", "mx1", "mx2", "vec", "d64", "d71",
    "d81", "g64", "t64", "x64", "crt", "adf", "adz", "dms", "hdf", "tzx", "z80", "rzx", "szx",
    "scl", "trd", "cdt", "cpr", "m3u", "cue", "gdi", "ccd", "chd", "iso", "gcm", "rvz", "wia",
    "gcz", "cso", "pbp", "cdi", "img", "mdf", "nrg", "tgc", "msa", "stx", "ch8", "sc8", "xo8",
    "hc8", "t81", "uze", "vaporbin", "gtr", "duck", "mgw",
];

/// ROM extraída para um arquivo ou diretório temporário, apagado no `Drop`.
#[derive(Debug)]
pub struct ExtractedRom {
    path: PathBuf,
    cleanup_dir: Option<PathBuf>,
    /// Nome da entrada dentro do arquivo comprimido (ex.: `jogo.sfc`).
    entry: String,
}

impl ExtractedRom {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }
}

impl Drop for ExtractedRom {
    fn drop(&mut self) {
        if let Some(dir) = &self.cleanup_dir {
            let _ = std::fs::remove_dir_all(dir);
        } else {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn create_unique(temp_dir: &Path, ext: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    for n in 0u32.. {
        let p = temp_dir.join(format!("reemu-rom-{}-{n}.{ext}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&p)
        {
            Ok(f) => return Ok((p, f)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("u32 esgotado ao nomear a ROM temporária")
}

fn create_unique_dir(temp_dir: &Path) -> std::io::Result<PathBuf> {
    for n in 0u32.. {
        let dir = temp_dir.join(format!("reemu-disc-{}-{n}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("u32 esgotado ao nomear o diretório temporário de disco")
}

fn sanitize_rel_path(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let mut out = PathBuf::new();
    for comp in Path::new(&normalized).components() {
        match comp {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn is_sheet_ext(ext: &str) -> bool {
    SHEET_EXTS.iter().any(|s| ext.eq_ignore_ascii_case(s))
}

/// Valida o conteúdo das extensões ambíguas e devolve os bytes já lidos.
/// O leitor de 7z continua da mesma posição ao extrair a entrada.
fn rom_entry_prefix(
    name: &str,
    size: u64,
    mut reader: impl Read,
    required_extension: Option<&str>,
) -> Option<Vec<u8>> {
    let ext = Path::new(name).extension()?.to_str()?;
    if let Some(required) = required_extension {
        return ext.eq_ignore_ascii_case(required).then(Vec::new);
    }
    if ext.eq_ignore_ascii_case("st") {
        let mut header = [0; ST_HEADER_LEN];
        reader.read_exact(&mut header).ok()?;
        system_for_st(size, &header)?;
        return Some(header.to_vec());
    }
    if ext.eq_ignore_ascii_case("p") {
        let mut header = [0; ZX81_P_HEADER_LEN];
        reader.read_exact(&mut header).ok()?;
        return is_zx81_program(size, &header).then(|| header.to_vec());
    }
    ROM_EXTS
        .iter()
        .any(|r| ext.eq_ignore_ascii_case(r))
        .then(Vec::new)
}

/// `true` se `path` tem extensão `.zip`/`.7z`.
pub fn is_archive(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip") || e.eq_ignore_ascii_case("7z"))
}

fn is_7z(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("7z"))
}

/// Extrai a ROM ou imagem de disco de dentro de `archive_path`.
pub fn extract_rom(
    archive_path: &Path,
    temp_dir: &Path,
    required_extension: Option<&str>,
) -> std::io::Result<ExtractedRom> {
    if is_7z(archive_path) {
        extract_rom_7z(archive_path, temp_dir, required_extension)
    } else {
        extract_rom_zip(archive_path, temp_dir, required_extension)
    }
}

fn extract_rom_zip(
    zip_path: &Path,
    temp_dir: &Path,
    required_extension: Option<&str>,
) -> std::io::Result<ExtractedRom> {
    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Se não houver extensão obrigatória, prioriza `.m3u`/`.cue`/`.gdi`/`.ccd`
    // antes de imagens brutas para não abrir uma trilha secundária por engano.
    let mut sheet_idx: Option<usize> = None;
    if required_extension.is_none() {
        for &sheet in SHEET_EXTS {
            sheet_idx = (0..zip.len()).find(|&i| {
                let Ok(entry) = zip.by_index(i) else {
                    return false;
                };
                if !entry.is_file() {
                    return false;
                }
                let Ok(name) = entry.name() else {
                    return false;
                };
                Path::new(name.as_ref())
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(sheet))
            });
            if sheet_idx.is_some() {
                break;
            }
        }
    }

    let idx = sheet_idx.or_else(|| {
        (0..zip.len()).find(|&i| {
            let Ok(mut entry) = zip.by_index(i) else {
                return false;
            };
            if !entry.is_file() {
                return false;
            }
            let Ok(name) = entry.name() else {
                return false;
            };
            let name = name.into_owned();
            rom_entry_prefix(&name, entry.size(), &mut entry, required_extension).is_some()
        })
    });
    let Some(idx) = idx else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "nenhuma ROM reconhecida dentro do .zip (set de arcade?)",
        ));
    };

    let entry_name = {
        let entry = zip
            .by_index(idx)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        entry
            .name()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
            .into_owned()
    };
    let ext = Path::new(&entry_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("rom")
        .to_ascii_lowercase();

    if is_sheet_ext(&ext) {
        let Some(primary_rel) = sanitize_rel_path(&entry_name) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "caminho inválido dentro do .zip",
            ));
        };
        let disc_dir = create_unique_dir(temp_dir)?;
        let extract_res = (|| -> std::io::Result<PathBuf> {
            for i in 0..zip.len() {
                let mut f = zip
                    .by_index(i)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                if !f.is_file() {
                    continue;
                }
                let Ok(raw_name) = f.name() else {
                    continue;
                };
                let Some(rel) = sanitize_rel_path(raw_name.as_ref()) else {
                    continue;
                };
                let dest = disc_dir.join(rel);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&dest)?;
                std::io::copy(&mut f, &mut out)?;
                out.flush()?;
            }
            Ok(disc_dir.join(primary_rel))
        })();
        return match extract_res {
            Ok(primary_path) => Ok(ExtractedRom {
                path: primary_path,
                cleanup_dir: Some(disc_dir),
                entry: entry_name,
            }),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&disc_dir);
                Err(e)
            }
        };
    }

    let mut entry = zip
        .by_index(idx)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let (out_path, mut out) = create_unique(temp_dir, &ext)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let r = entry.read(&mut buf)?;
        if r == 0 {
            break;
        }
        out.write_all(&buf[..r])?;
    }
    out.flush()?;
    Ok(ExtractedRom {
        path: out_path,
        cleanup_dir: None,
        entry: entry_name,
    })
}

fn extract_rom_7z(
    archive_path: &Path,
    temp_dir: &Path,
    required_extension: Option<&str>,
) -> std::io::Result<ExtractedRom> {
    let mut r = ArchiveReader::open(archive_path, Password::empty())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    // Verifica no índice do 7z se há `.m3u`/`.cue`/`.gdi`/`.ccd` que exige extrair
    // também as trilhas irmãs `.bin`/`.raw` em subdiretório temporário.
    let sheet_entry_name: Option<String> = if required_extension.is_none() {
        SHEET_EXTS.iter().find_map(|&sheet| {
            r.archive()
                .files
                .iter()
                .filter(|f| !f.is_directory())
                .find(|f| {
                    Path::new(f.name())
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case(sheet))
                })
                .map(|f| f.name().to_string())
        })
    } else {
        required_extension
            .filter(|req| is_sheet_ext(req))
            .and_then(|req| {
                r.archive()
                    .files
                    .iter()
                    .filter(|f| !f.is_directory())
                    .find(|f| {
                        Path::new(f.name())
                            .extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| e.eq_ignore_ascii_case(req))
                    })
                    .map(|f| f.name().to_string())
            })
    };

    if let Some(sheet_name) = sheet_entry_name {
        let Some(primary_rel) = sanitize_rel_path(&sheet_name) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "caminho inválido dentro do .7z",
            ));
        };
        let disc_dir = create_unique_dir(temp_dir)?;
        let mut io_err: Option<std::io::Error> = None;
        let walk = r.for_each_entries(|entry, reader| {
            if entry.is_directory() {
                return Ok(true);
            }
            let Some(rel) = sanitize_rel_path(entry.name()) else {
                std::io::copy(reader, &mut std::io::sink())?;
                return Ok(true);
            };
            let dest = disc_dir.join(rel);
            let step = (|| -> std::io::Result<()> {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&dest)?;
                std::io::copy(reader, &mut out)?;
                out.flush()?;
                Ok(())
            })();
            if let Err(e) = step {
                io_err = Some(e);
                return Ok(false);
            }
            Ok(true)
        });
        if let Some(e) = io_err {
            let _ = std::fs::remove_dir_all(&disc_dir);
            return Err(e);
        }
        if let Err(e) = walk {
            let _ = std::fs::remove_dir_all(&disc_dir);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ));
        }
        return Ok(ExtractedRom {
            path: disc_dir.join(primary_rel),
            cleanup_dir: Some(disc_dir),
            entry: sheet_name,
        });
    }

    let mut result: Option<std::io::Result<(PathBuf, String)>> = None;
    r.for_each_entries(|entry, reader| {
        if result.is_some() || entry.is_directory() {
            return Ok(true);
        }
        let Some(prefix) =
            rom_entry_prefix(entry.name(), entry.size(), &mut *reader, required_extension)
        else {
            return Ok(true);
        };
        let ext = Path::new(entry.name())
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("rom")
            .to_ascii_lowercase();
        let name = entry.name().to_string();
        result = Some((|| -> std::io::Result<(PathBuf, String)> {
            let (out_path, mut out) = create_unique(temp_dir, &ext)?;
            out.write_all(&prefix)?;
            std::io::copy(reader, &mut out)?;
            out.flush()?;
            Ok((out_path, name))
        })());
        Ok(true)
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    match result {
        Some(Ok((path, entry))) => Ok(ExtractedRom {
            path,
            cleanup_dir: None,
            entry,
        }),
        Some(Err(e)) => Err(e),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "nenhuma ROM reconhecida dentro do .7z",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    fn scratch_path(ext: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "reemu-archive-test-{}-{}.{ext}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn extracts_rom_from_zip() {
        let zip_path = scratch_path("zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zw.start_file("Game (USA).sfc", opts).unwrap();
            zw.write_all(b"snes-rom-payload").unwrap();
            zw.finish().unwrap();
        }

        let extracted = extract_rom(&zip_path, &std::env::temp_dir(), None).unwrap();
        assert_eq!(extracted.path().extension().unwrap(), "sfc");
        assert_eq!(
            std::fs::read(extracted.path()).unwrap(),
            b"snes-rom-payload"
        );

        let _ = std::fs::remove_file(&zip_path);
    }

    #[test]
    fn extracts_cue_and_bin_siblings_from_zip_and_7z_and_cleans_up_on_drop() {
        let cue =
            b"FILE \"Track 1.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n";
        let bin = b"raw-psx-sector-payload";

        let zip_path = scratch_path("zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("Track 1.bin", opts).unwrap();
            zw.write_all(bin).unwrap();
            zw.start_file("Resident Evil 2.cue", opts).unwrap();
            zw.write_all(cue).unwrap();
            zw.finish().unwrap();
        }
        let temp_dir_zip = {
            let extracted = extract_rom(&zip_path, &std::env::temp_dir(), None).unwrap();
            assert_eq!(
                extracted.path().file_name().unwrap().to_str(),
                Some("Resident Evil 2.cue")
            );
            let dir = extracted.path().parent().unwrap().to_path_buf();
            assert_eq!(std::fs::read(dir.join("Track 1.bin")).unwrap(), bin);
            dir
        };
        assert!(
            !temp_dir_zip.exists(),
            "diretório temporário do zip removido no drop"
        );
        let _ = std::fs::remove_file(&zip_path);

        let sz_path = scratch_path("7z");
        {
            let mut w = sevenz_rust2::ArchiveWriter::create(&sz_path).unwrap();
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("Track 1.bin"),
                Some(std::io::Cursor::new(bin.to_vec())),
            )
            .unwrap();
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("THPS2.cue"),
                Some(std::io::Cursor::new(cue.to_vec())),
            )
            .unwrap();
            w.finish().unwrap();
        }
        let temp_dir_7z = {
            let extracted = extract_rom(&sz_path, &std::env::temp_dir(), None).unwrap();
            assert_eq!(
                extracted.path().file_name().unwrap().to_str(),
                Some("THPS2.cue")
            );
            let dir = extracted.path().parent().unwrap().to_path_buf();
            assert_eq!(std::fs::read(dir.join("Track 1.bin")).unwrap(), bin);
            dir
        };
        assert!(
            !temp_dir_7z.exists(),
            "diretório temporário do 7z removido no drop"
        );
        let _ = std::fs::remove_file(&sz_path);
    }

    #[test]
    fn extracts_sufami_cart_from_7z_without_dropping_the_header() {
        let mut cart = vec![0; 0x80000];
        cart[..14].copy_from_slice(b"BANDAI SFC-ADX");
        let archive = scratch_path("7z");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("Atari.st"),
                Some(std::io::Cursor::new(vec![0; 0x80000])),
            )
            .unwrap();
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("Sufami.ST"),
                Some(std::io::Cursor::new(&cart)),
            )
            .unwrap();
        drop(writer.finish().unwrap());
        let extracted = extract_rom(&archive, &std::env::temp_dir(), None).unwrap();
        assert_eq!(extracted.entry(), "Sufami.ST");
        assert_eq!(std::fs::read(extracted.path()).unwrap(), cart);
        let _ = std::fs::remove_file(&archive);
    }

    #[test]
    fn extracts_rom_from_7z() {
        let archive_path = scratch_path("7z");
        {
            let mut w = sevenz_rust2::ArchiveWriter::create(&archive_path).unwrap();
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("Game (USA).sfc"),
                Some(std::io::Cursor::new(b"snes-rom-payload".to_vec())),
            )
            .unwrap();
            w.finish().unwrap();
        }

        let extracted = extract_rom(&archive_path, &std::env::temp_dir(), None).unwrap();
        assert_eq!(extracted.path().extension().unwrap(), "sfc");
        assert_eq!(
            std::fs::read(extracted.path()).unwrap(),
            b"snes-rom-payload"
        );

        let _ = std::fs::remove_file(&archive_path);
    }

    #[test]
    fn sevenz_without_a_recognized_rom_entry_is_not_found() {
        let archive_path = scratch_path("7z");
        {
            let mut w = sevenz_rust2::ArchiveWriter::create(&archive_path).unwrap();
            w.push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("sfiii3n.06"),
                Some(std::io::Cursor::new(
                    b"chip-dump-not-a-cartridge-rom".to_vec(),
                )),
            )
            .unwrap();
            w.finish().unwrap();
        }

        let err = extract_rom(&archive_path, &std::env::temp_dir(), None).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

        let _ = std::fs::remove_file(&archive_path);
    }

    #[test]
    fn concurrent_extractions_get_distinct_files() {
        let zip_path = scratch_path("zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts: zip::write::SimpleFileOptions = Default::default();
            zw.start_file("Race (USA).gba", opts).unwrap();
            zw.write_all(b"gba-payload").unwrap();
            zw.finish().unwrap();
        }
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let p = zip_path.clone();
                std::thread::spawn(move || extract_rom(&p, &std::env::temp_dir(), None).unwrap())
            })
            .collect();
        let extracted: Vec<ExtractedRom> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let mut paths: Vec<_> = extracted.iter().map(|e| e.path().to_path_buf()).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), 8, "cada extração com seu próprio arquivo");
        for e in &extracted {
            assert_eq!(std::fs::read(e.path()).unwrap(), b"gba-payload");
        }
        let _ = std::fs::remove_file(&zip_path);
    }
}
