//! Hash de ROM (CRC32 + MD5 + SHA1, numa leitura só). Matching por hash é a única forma de match
//! automático (ver `docs/ai-context/09`) — nome de arquivo não conta.
//!
//! Header skip: alguns dumps carregam um header do emulador que não faz
//! parte da ROM. Cobrimos o caso clássico do iNES (`.nes`, 16 bytes) —
//! outros formatos entram conforme necessário.

use domain::metadata::{RomHash, RomHashService};
use md5::{Digest, Md5};
use sha1::Sha1;
use std::fs::File;
use std::io::{BufReader, Read};

const INES_MAGIC: &[u8; 4] = b"NES\x1a";
const INES_HEADER_LEN: u64 = 16;

/// O que uma leitura produz: os três hashes (CRC32 em maiúsculas, MD5 e SHA1
/// em minúsculas), o tamanho do que entrou neles (sem o header iNES, como os
/// hashes sem header do ScreenScraper) e o nome do que foi lido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomHashes {
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
    pub size: u64,
    /// Nome (só o arquivo, sem pasta) do que foi lido: a entrada de dentro do
    /// `.zip`/`.7z`, ou o próprio arquivo.
    pub name: String,
}

pub struct FileRomHasher;

impl FileRomHasher {
    pub fn hash_file(path: &str) -> std::io::Result<RomHashes> {
        let name = file_name_of(path);
        Self::hash_reader(BufReader::new(File::open(path)?), &name)
    }

    /// Como [`hash_file`], mas sobre um leitor qualquer — pra ROM de dentro de
    /// um arquivo comprimido (o CRC precisa ser o da ROM crua, não do `.zip`).
    /// `name` é o nome do que o leitor entrega (a entrada interna).
    pub fn hash_reader<R: Read>(mut reader: R, name: &str) -> std::io::Result<RomHashes> {
        // Detecta e pula o header iNES.
        let mut head = [0u8; 4];
        let n = read_up_to(&mut reader, &mut head)?;
        let skip = n == 4 && &head == INES_MAGIC;

        let mut crc = crc32fast::Hasher::new();
        let mut md5 = Md5::new();
        let mut sha1 = Sha1::new();
        let mut size = 0u64;

        if !skip {
            crc.update(&head[..n]);
            md5.update(&head[..n]);
            sha1.update(&head[..n]);
            size += n as u64;
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
            sha1.update(&buf[..read]);
            size += read as u64;
        }

        Ok(RomHashes {
            crc32: format!("{:08X}", crc.finalize()),
            md5: format!("{:x}", md5.finalize()),
            sha1: format!("{:x}", sha1.finalize()),
            size,
            name: name.to_owned(),
        })
    }
}

impl RomHashService for FileRomHasher {
    fn compute(&self, file_path: &str) -> Result<RomHash, String> {
        Self::hash_file(file_path)
            .map(|h| RomHash {
                crc32: h.crc32,
                md5: h.md5,
            })
            .map_err(|e| format!("{file_path}: {e}"))
    }
}

/// Último componente de um caminho (separador `/` ou `\`, venha de onde vier).
pub(crate) fn file_name_of(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
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
        assert_eq!(h.size, 11);
        let _ = std::fs::remove_file(f);
    }

    #[test]
    fn sha1_matches_known_vector() {
        let h = FileRomHasher::hash_reader(std::io::Cursor::new(b"abc"), "abc.bin").unwrap();
        assert_eq!(h.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(h.size, 3);
        assert_eq!(h.name, "abc.bin");
    }

    #[test]
    fn nes_header_is_skipped_for_all_three() {
        let body = b"ROMBODY-1234567890";
        let mut with_header = Vec::new();
        with_header.extend_from_slice(b"NES\x1a");
        with_header.extend_from_slice(&[0u8; 12]);
        with_header.extend_from_slice(body);

        let a = FileRomHasher::hash_file(&tmp("nes3-hdr", &with_header)).unwrap();
        let b = FileRomHasher::hash_file(&tmp("nes3-body", body)).unwrap();
        assert_eq!(a.crc32, b.crc32);
        assert_eq!(a.md5, b.md5);
        assert_eq!(a.sha1, b.sha1);
        assert_eq!(a.size, body.len() as u64, "size exclui o header iNES");
    }

    #[test]
    fn file_name_is_the_last_path_component() {
        assert_eq!(file_name_of(r"C:\roms\nes\Game.nes"), "Game.nes");
        assert_eq!(file_name_of("/roms/nes/Game.nes"), "Game.nes");
        assert_eq!(file_name_of("Game.nes"), "Game.nes");
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
