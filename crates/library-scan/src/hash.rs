//! Hash de ROM (CRC32 + MD5). Matching por hash é a única forma de match
//! automático (ver `docs/ai-context/09`) — nome de arquivo não conta.
//!
//! Header skip: alguns dumps carregam um header do emulador que não faz
//! parte da ROM. Cobrimos o caso clássico do iNES (`.nes`, 16 bytes) —
//! outros formatos entram conforme necessário.

use domain::metadata::{RomHash, RomHashService};
use md5::{Digest, Md5};
use std::fs::File;
use std::io::{BufReader, Read};

const INES_MAGIC: &[u8; 4] = b"NES\x1a";
const INES_HEADER_LEN: u64 = 16;

pub struct FileRomHasher;

impl FileRomHasher {
    pub fn hash_file(path: &str) -> std::io::Result<RomHash> {
        let mut reader = BufReader::new(File::open(path)?);

        // Detecta e pula o header iNES.
        let mut head = [0u8; 4];
        let n = read_up_to(&mut reader, &mut head)?;
        let skip = n == 4 && &head == INES_MAGIC;

        let mut crc = crc32fast::Hasher::new();
        let mut md5 = Md5::new();

        if !skip {
            crc.update(&head[..n]);
            md5.update(&head[..n]);
        } else {
            // consome o resto do header (já lemos 4 dos 16 bytes)
            let mut rest = [0u8; (INES_HEADER_LEN - 4) as usize];
            read_exact_or_eof(&mut reader, &mut rest)?;
        }

        let mut buf = [0u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buf)?;
            if read == 0 {
                break;
            }
            crc.update(&buf[..read]);
            md5.update(&buf[..read]);
        }

        Ok(RomHash {
            crc32: format!("{:08X}", crc.finalize()),
            md5: format!("{:x}", md5.finalize()),
        })
    }
}

impl RomHashService for FileRomHasher {
    fn compute(&self, file_path: &str) -> Result<RomHash, String> {
        Self::hash_file(file_path).map_err(|e| format!("{file_path}: {e}"))
    }
}

fn read_up_to(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

fn read_exact_or_eof(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<()> {
    let _ = read_up_to(r, buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str, bytes: &[u8]) -> String {
        let p = std::env::temp_dir().join(format!("reemu-hash-{}-{name}", std::process::id()));
        std::fs::File::create(&p).unwrap().write_all(bytes).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn known_crc32_and_md5() {
        // "hello world" -> CRC32 0x0D4A1185, MD5 5eb63bbbe01eeed093cb22bb8f5acdc3
        let f = tmp("hello", b"hello world");
        let h = FileRomHasher::hash_file(&f).unwrap();
        assert_eq!(h.crc32, "0D4A1185");
        assert_eq!(h.md5, "5eb63bbbe01eeed093cb22bb8f5acdc3");
        let _ = std::fs::remove_file(f);
    }

    #[test]
    fn ines_header_is_skipped() {
        let body = b"ROMBODY-1234567890";
        let mut with_header = Vec::new();
        with_header.extend_from_slice(b"NES\x1a");
        with_header.extend_from_slice(&[0u8; 12]); // resto do header de 16 bytes
        with_header.extend_from_slice(body);

        let a = FileRomHasher::hash_file(&tmp("nes-hdr", &with_header)).unwrap();
        let b = FileRomHasher::hash_file(&tmp("nes-body", body)).unwrap();
        assert_eq!(a.crc32, b.crc32, "hash deve ignorar o header iNES");
        assert_eq!(a.md5, b.md5);
    }

    #[test]
    fn non_ines_hashed_whole() {
        let data = b"NOT-A-NES-FILE";
        let a = FileRomHasher::hash_file(&tmp("plain", data)).unwrap();
        // sem header -> mesmo hash de um CRC/MD5 direto
        let mut c = crc32fast::Hasher::new();
        c.update(data);
        assert_eq!(a.crc32, format!("{:08X}", c.finalize()));
    }
}
