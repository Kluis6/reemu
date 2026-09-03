//! Extração de ROM de dentro de um `.zip` na hora de carregar o core.
//!
//! O scan (`library-scan`) já cataloga o `.zip` como uma ROM (system + hash da
//! entrada interna). Aqui, no load, a 1ª entrada com extensão de ROM é extraída
//! pra um arquivo temporário — o caminho vai pro core; alguns cores exigem
//! `need_fullpath`, e mesmo os que não exigem preferem um `path` real pro nome.

use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Extensões de ROM crua (subconjunto — o suficiente pra achar a entrada certa
/// dentro do zip). Espelha `library-scan::system_for_extension`.
const ROM_EXTS: &[&str] = &[
    "nes", "fds", "unif", "unf", "sfc", "smc", "swc", "fig", "gb", "gbc", "gba", "srl", "n64",
    "z64", "v64", "ndd", "md", "smd", "gen", "sgd", "sms", "gg", "pce", "sgx", "a26", "lnx", "ws",
    "wsc", "ngp", "ngc", "32x", "cue", "chd", "iso", "pbp",
];

/// ROM extraída pra um arquivo temporário — apagado no `Drop`.
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

/// `true` se `path` tem extensão `.zip`.
pub fn is_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

/// Extrai a 1ª ROM de dentro de `zip_path` pra `<temp>/reemu-rom-<pid>-<n>.<ext>`.
pub fn extract_rom(zip_path: &Path, temp_dir: &Path) -> std::io::Result<ExtractedRom> {
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
            "nenhuma ROM reconhecida dentro do .zip",
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
