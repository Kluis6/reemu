//! O mínimo de ISO9660 pra achar um arquivo pelo caminho: descritor de volume
//! primário (setor 16), diretório raiz, e os registros de diretório. Todo
//! campo vindo da imagem é conferido antes de usar; registro torto encerra o
//! setor, nunca entra em pânico.

use crate::sector::{SectorReader, USER_DATA};

/// Um arquivo ou diretório da imagem: setor inicial e tamanho em bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
    pub lba: u32,
    pub size: u32,
    pub is_dir: bool,
}

fn le_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn le_u16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

/// Lê um registro de diretório de 34+ bytes (o do raiz fica no descritor).
fn record(rec: &[u8]) -> Option<(Entry, &[u8])> {
    let lba = le_u32(rec, 2)?;
    let size = le_u32(rec, 10)?;
    let flags = *rec.get(25)?;
    let name_len = usize::from(*rec.get(32)?);
    let name = rec.get(33..33 + name_len)?;
    let entry = Entry {
        lba,
        size,
        is_dir: flags & 0x02 != 0,
    };
    Some((entry, name))
}

/// Diretório raiz, pelo descritor de volume primário (a partir do setor 16).
pub(crate) fn root(reader: &mut SectorReader) -> Option<Entry> {
    /// Os descritores ficam em sequência a partir do 16; o primário vem
    /// primeiro na prática, e uns poucos bastam pra achar.
    const MAX_DESCRIPTORS: u32 = 8;
    for lba in 16..16 + MAX_DESCRIPTORS {
        let s = reader.read_sector(lba)?;
        if &s[1..6] != b"CD001" {
            return None;
        }
        match s[0] {
            1 => {
                if le_u16(&s, 128)? != USER_DATA as u16 {
                    return None;
                }
                let rec = s.get(156..156 + 34)?;
                return record(rec).map(|(e, _)| e).filter(|e| e.is_dir);
            }
            255 => return None,
            _ => {}
        }
    }
    None
}

/// Nome do registro sem a versão (`;1`) nem o ponto final de nome sem extensão.
fn same_name(raw: &[u8], wanted: &str) -> bool {
    let raw = raw.split(|&b| b == b';').next().unwrap_or_default();
    let raw = raw.strip_suffix(b".").unwrap_or(raw);
    raw.eq_ignore_ascii_case(wanted.as_bytes())
}

/// Procura `name` dentro do diretório `dir`.
pub(crate) fn find(reader: &mut SectorReader, dir: Entry, name: &str) -> Option<Entry> {
    if !dir.is_dir {
        return None;
    }
    let sectors = dir.size.div_ceil(USER_DATA as u32);
    for i in 0..sectors {
        let s = reader.read_sector(dir.lba.checked_add(i)?)?;
        let mut pos = 0usize;
        while pos < USER_DATA {
            let len = usize::from(s[pos]);
            // 0: o resto do setor é enchimento; registro que não cabe: torto.
            if len == 0 || pos + len > USER_DATA {
                break;
            }
            if let Some((entry, raw)) = s.get(pos..pos + len).and_then(record) {
                if same_name(raw, name) {
                    return Some(entry);
                }
            }
            pos += len;
        }
    }
    None
}

/// Acha o arquivo pelo caminho (`["PSP_GAME", "PARAM.SFO"]`) a partir da raiz.
pub(crate) fn lookup(reader: &mut SectorReader, path: &[&str]) -> Option<Entry> {
    let mut entry = root(reader)?;
    for part in path {
        entry = find(reader, entry, part)?;
    }
    Some(entry)
}

/// Os bytes do arquivo, até `max` (o resto não é lido).
pub(crate) fn read_file(reader: &mut SectorReader, file: Entry, max: usize) -> Option<Vec<u8>> {
    if file.is_dir {
        return None;
    }
    let len = (file.size as usize).min(max);
    let mut out = Vec::with_capacity(len);
    let mut lba = file.lba;
    while out.len() < len {
        let s = reader.read_sector(lba)?;
        let take = (len - out.len()).min(USER_DATA);
        out.extend_from_slice(&s[..take]);
        lba = lba.checked_add(1)?;
    }
    Some(out)
}
