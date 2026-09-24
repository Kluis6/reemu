//! Extração de ROM de dentro de um `.zip`/`.7z` na hora de carregar o core.
//!
//! O scan (`library-scan`) já cataloga o arquivo como uma ROM (system + hash
//! da entrada interna). Aqui, no load, a 1ª entrada com extensão de ROM é
//! extraída pra um arquivo temporário — o caminho vai pro core; alguns cores
//! exigem `need_fullpath`, e mesmo os que não exigem preferem um `path` real
//! pro nome.

use sevenz_rust2::{ArchiveReader, Password};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Extensões de ROM crua pra achar a entrada certa dentro do zip. Espelha as
/// extensões de cartucho de `library-scan::system_for_extension` — sem elas
/// aqui, uma ROM zipada é catalogada pelo scan mas não é extraída no load.
/// (Este crate não depende do `library-scan`; manter as duas em sincronia.)
const ROM_EXTS: &[&str] = &[
    "nes", "fds", "unif", "unf", "sfc", "smc", "swc", "fig", "gb", "gbc", "gba", "srl", "n64",
    "z64", "v64", "ndd", "md", "smd", "gen", "sgd", "sms", "gg", "pce", "sgx", "a26", "a78", "lnx",
    "ws", "wsc", "ngp", "ngc", "32x", "vb", "col", "int", "nds", "dsi", "ids", "sg", "a52", "atr",
    "xfd", "atx", "xex", "j64", "jag", "min", "sv", "mx1", "mx2", "vec", "d64", "d71", "d81",
    "g64", "t64", "x64", "crt", "adf", "adz", "dms", "hdf", "tzx", "z80", "rzx", "szx", "scl",
    "trd", "cdt", "cpr", "cue", "chd", "iso", "pbp",
];

/// ROM extraída pra um arquivo temporário — apagado no `Drop`.
#[derive(Debug)]
pub struct ExtractedRom {
    path: PathBuf,
}

impl ExtractedRom {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ExtractedRom {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn is_rom_entry(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ROM_EXTS.iter().any(|r| e.eq_ignore_ascii_case(r)))
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

/// Extrai a 1ª ROM de dentro de `archive_path` pra
/// `<temp>/reemu-rom-<pid>-<n>.<ext>`.
pub fn extract_rom(archive_path: &Path, temp_dir: &Path) -> std::io::Result<ExtractedRom> {
    if is_7z(archive_path) {
        extract_rom_7z(archive_path, temp_dir)
    } else {
        extract_rom_zip(archive_path, temp_dir)
    }
}

fn extract_rom_zip(zip_path: &Path, temp_dir: &Path) -> std::io::Result<ExtractedRom> {
    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let idx = (0..zip.len()).find(|&i| {
        zip.by_index(i)
            .map(|e| e.is_file() && is_rom_entry(e.name()))
            .unwrap_or(false)
    });
    let Some(idx) = idx else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "nenhuma ROM reconhecida dentro do .zip (set de arcade?)",
        ));
    };

    let mut entry = zip
        .by_index(idx)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let ext = Path::new(entry.name())
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("rom")
        .to_ascii_lowercase();

    let mut n = 0u32;
    let out_path = loop {
        let p = temp_dir.join(format!("reemu-rom-{}-{n}.{ext}", std::process::id()));
        if !p.exists() {
            break p;
        }
        n += 1;
    };

    let mut out = std::fs::File::create(&out_path)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let r = entry.read(&mut buf)?;
        if r == 0 {
            break;
        }
        out.write_all(&buf[..r])?;
    }
    out.flush()?;
    Ok(ExtractedRom { path: out_path })
}

fn extract_rom_7z(archive_path: &Path, temp_dir: &Path) -> std::io::Result<ExtractedRom> {
    let mut r = ArchiveReader::open(archive_path, Password::empty())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    let mut n = 0u32;
    let mut result: Option<std::io::Result<PathBuf>> = None;
    r.for_each_entries(|entry, reader| {
        if result.is_some() || entry.is_directory() || !is_rom_entry(entry.name()) {
            return Ok(true);
        }
        let ext = Path::new(entry.name())
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("rom")
            .to_ascii_lowercase();
        let out_path = loop {
            let p = temp_dir.join(format!("reemu-rom-{}-{n}.{ext}", std::process::id()));
            if !p.exists() {
                break p;
            }
            n += 1;
        };
        result = Some((|| -> std::io::Result<PathBuf> {
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(reader, &mut out)?;
            out.flush()?;
            Ok(out_path)
        })());
        Ok(true)
    })
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

    match result {
        Some(Ok(path)) => Ok(ExtractedRom { path }),
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

        let extracted = extract_rom(&zip_path, &std::env::temp_dir()).unwrap();
        assert_eq!(extracted.path().extension().unwrap(), "sfc");
        assert_eq!(
            std::fs::read(extracted.path()).unwrap(),
            b"snes-rom-payload"
        );

        let _ = std::fs::remove_file(&zip_path);
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

        let extracted = extract_rom(&archive_path, &std::env::temp_dir()).unwrap();
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

        let err = extract_rom(&archive_path, &std::env::temp_dir()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

        let _ = std::fs::remove_file(&archive_path);
    }
}
