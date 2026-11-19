//! Trilha de dados de dentro de um `.chd` (CD/GD-ROM pelos metadados de
//! trilha, ou DVD), lida hunk a hunk pelo crate `chd` e entregue setor a
//! setor ao [`crate::sector::SectorReader`], que cobra o limite de leitura.

use crate::sector::{Area, Layout, ReadSeek, USER_DATA};
use chd::map::{CompressionTypeLegacy, CompressionTypeV5, Map, MapEntry};
use chd::metadata::KnownMetadata;
use chd::Chd;
use std::io::{Read, Seek, SeekFrom};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Maior hunk aceito num `.chd` (o de CD tem 19584 bytes; o de DVD, 4096).
const CHD_MAX_HUNK: u32 = 1024 * 1024;
/// Mais hunks que isto e o mapa não cabe em memória com folga (um DVD de
/// dupla camada em hunks de 4096 tem uns 2,1 milhões).
const CHD_MAX_HUNKS: u32 = 1 << 22;
/// Um frame de CD no `.chd`: 2352 do setor + 96 de subcódigo.
const CD_FRAME: u64 = 2448;
/// O `chdman` completa cada trilha até um múltiplo de 4 frames.
const CD_TRACK_PADDING: u64 = 4;

/// Como os setores estão dentro do `.chd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChdLayout {
    /// Sem metadados de CD: setores de 2048 seguidos (`chdman createdvd`).
    Dvd,
    /// CD/GD-ROM: frames de 2448; a trilha escolhida começa no frame
    /// `first_frame` e tem setores no formato `layout`.
    Cd { first_frame: u64, layout: Layout },
}

/// Uma trilha de dados de dentro de um `.chd`.
pub(crate) struct ChdTrack {
    chd: Chd<Box<dyn ReadSeek>>,
    file_len: u64,
    hunk_size: u32,
    layout: ChdLayout,
    hunk: Vec<u8>,
    cached: Option<u32>,
    comp: Vec<u8>,
    /// O leitor de `.chd` já entrou em pânico com esta imagem: não tenta mais.
    poisoned: bool,
    /// Bytes que o descompressor foi chamado a produzir, contados à parte do
    /// limite (os testes conferem um com o outro).
    #[cfg(test)]
    pub(crate) decompressed: u64,
}

/// Trilha descrita nos metadados de CD/GD-ROM do `.chd`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ChdCdTrack {
    number: u32,
    kind: String,
    frames: u64,
    pregap: u64,
    pregap_in_data: bool,
}

/// `TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE FRAMES:1234 PREGAP:0 PGTYPE:MODE1 ...`
fn parse_cd_track(text: &str) -> Option<ChdCdTrack> {
    let mut t = ChdCdTrack {
        number: 0,
        kind: String::new(),
        frames: 0,
        pregap: 0,
        pregap_in_data: false,
    };
    for word in text.split(|c: char| c.is_whitespace() || c == '\0') {
        let Some((key, value)) = word.split_once(':') else {
            continue;
        };
        match key {
            "TRACK" => t.number = value.parse().ok()?,
            "TYPE" => t.kind = value.to_ascii_uppercase(),
            "FRAMES" => t.frames = value.parse().ok()?,
            "PREGAP" => t.pregap = value.parse().ok()?,
            // `V` na frente: o pregap está gravado nos frames da trilha.
            "PGTYPE" => t.pregap_in_data = value.starts_with('V'),
            _ => {}
        }
    }
    (t.number > 0 && !t.kind.is_empty()).then_some(t)
}

/// Escolhe a trilha de dados e devolve o frame do `.chd` onde os dados dela
/// começam (as trilhas anteriores, cada uma completada até múltiplo de 4,
/// mais o pregap gravado da própria) e o formato dos setores.
fn chd_cd_start(tracks: &[ChdCdTrack], gd: bool, area: Area) -> Option<(u64, Layout)> {
    let is_data = |t: &&ChdCdTrack| Layout::from_mode(&t.kind).is_some();
    let chosen = match area {
        Area::HighDensity if gd => tracks
            .iter()
            .filter(is_data)
            .find(|t| t.number >= 3)
            .or_else(|| tracks.iter().find(is_data)),
        _ => tracks.iter().find(is_data),
    }?;
    let mut frame = 0u64;
    for t in tracks {
        if t.number == chosen.number {
            let skip = if t.pregap_in_data { t.pregap } else { 0 };
            return Some((frame.checked_add(skip)?, Layout::from_mode(&t.kind)?));
        }
        let padded = t.frames.checked_next_multiple_of(CD_TRACK_PADDING)?;
        frame = frame.checked_add(padded)?;
    }
    None
}

impl ChdTrack {
    /// Abre o `.chd`. O leitor do `chd` indexa e soma campos do arquivo sem
    /// conferir (um pânico com imagem quebrada); aqui o pânico vira `None`.
    pub(crate) fn open(src: Box<dyn ReadSeek>, area: Area) -> Option<ChdTrack> {
        catch_unwind(AssertUnwindSafe(move || Self::open_checked(src, area))).unwrap_or_else(|_| {
            log::warn!("serial: o leitor de chd falhou numa imagem; ela fica sem serial");
            None
        })
    }

    fn open_checked(mut src: Box<dyn ReadSeek>, area: Area) -> Option<ChdTrack> {
        let file_len = src.seek(SeekFrom::End(0)).ok()?;
        src.seek(SeekFrom::Start(0)).ok()?;
        let header = chd::header::Header::try_read_header(&mut src).ok()?;
        let hunk_size = header.hunk_size();
        if header.has_parent()
            || hunk_size == 0
            || hunk_size > CHD_MAX_HUNK
            || header.hunk_count() > CHD_MAX_HUNKS
        {
            return None;
        }
        // O mapa comprimido do V5 diz o próprio tamanho; o leitor aloca isso
        // antes de ler, então um tamanho maior que o arquivo para aqui.
        if let chd::header::Header::V5Header(v5) = &header {
            if header.is_compressed() {
                let mut len = [0u8; 4];
                src.seek(SeekFrom::Start(v5.map_offset)).ok()?;
                src.read_exact(&mut len).ok()?;
                let end = v5
                    .map_offset
                    .checked_add(16)?
                    .checked_add(u64::from(u32::from_be_bytes(len)))?;
                if end > file_len {
                    return None;
                }
            }
        }
        let layout = match header.meta_offset() {
            Some(offset) => Self::chd_layout(&mut src, offset, area)?,
            None => ChdLayout::Dvd,
        };
        match layout {
            ChdLayout::Dvd if !(hunk_size as usize).is_multiple_of(USER_DATA) => return None,
            ChdLayout::Cd { .. } if !u64::from(hunk_size).is_multiple_of(CD_FRAME) => return None,
            _ => {}
        }
        src.seek(SeekFrom::Start(0)).ok()?;
        let chd = Chd::open(src, None).ok()?;
        Some(ChdTrack {
            chd,
            file_len,
            hunk_size,
            layout,
            hunk: vec![0u8; hunk_size as usize],
            cached: None,
            comp: Vec::new(),
            poisoned: false,
            #[cfg(test)]
            decompressed: 0,
        })
    }

    /// Percorre a lista de metadados do `.chd` (cabeçalho de 16 bytes: tag,
    /// flags, tamanho em 24 bits, próximo) atrás das trilhas de CD/GD-ROM.
    /// Sem nenhuma: DVD. É CD mas sem trilha de dados legível: `None`.
    /// Lido aqui, e não pelo `chd`, pra conferir o tamanho antes de alocar.
    fn chd_layout(src: &mut Box<dyn ReadSeek>, first: u64, area: Area) -> Option<ChdLayout> {
        /// Mais entradas que isto não é CD (o máximo é 99 trilhas).
        const MAX_ENTRIES: usize = 128;
        /// O texto de uma trilha tem uns 100 bytes.
        const MAX_TRACK_TEXT: usize = 1024;
        let cd_tags = [
            KnownMetadata::CdRomTrack2 as u32,
            KnownMetadata::CdRomTrack as u32,
            KnownMetadata::GdRomTrack as u32,
            KnownMetadata::GdRomOld as u32,
        ];
        let gd_tags = [
            KnownMetadata::GdRomTrack as u32,
            KnownMetadata::GdRomOld as u32,
        ];
        let mut tracks = Vec::new();
        let (mut is_cd, mut gd) = (false, false);
        let mut visited = Vec::new();
        let mut offset = first;
        while offset != 0 {
            if visited.len() >= MAX_ENTRIES || visited.contains(&offset) {
                return None;
            }
            visited.push(offset);
            let mut head = [0u8; 16];
            src.seek(SeekFrom::Start(offset)).ok()?;
            src.read_exact(&mut head).ok()?;
            let tag = u32::from_be_bytes(head[0..4].try_into().ok()?);
            let len = u32::from_be_bytes([0, head[5], head[6], head[7]]) as usize;
            offset = u64::from_be_bytes(head[8..16].try_into().ok()?);
            is_cd |= KnownMetadata::is_cdrom(tag);
            if !cd_tags.contains(&tag) {
                continue;
            }
            if len > MAX_TRACK_TEXT {
                return None;
            }
            gd |= gd_tags.contains(&tag);
            let mut text = vec![0u8; len];
            src.read_exact(&mut text).ok()?;
            tracks.push(parse_cd_track(&String::from_utf8_lossy(&text))?);
        }
        if !is_cd {
            return Some(ChdLayout::Dvd);
        }
        tracks.sort_by_key(|t| t.number);
        let (first_frame, layout) = chd_cd_start(&tracks, gd, area)?;
        Some(ChdLayout::Cd {
            first_frame,
            layout,
        })
    }

    /// Quanto custa ler o setor `lba`: o hunk inteiro, a não ser que já
    /// esteja descomprimido.
    pub(crate) fn cost_of(&self, lba: u32) -> Option<u64> {
        let (hunk, _) = self.locate(lba)?;
        Some(if self.cached == Some(hunk) {
            0
        } else {
            u64::from(self.hunk_size)
        })
    }

    /// Hunk e posição, dentro dele, do setor `lba`.
    fn locate(&self, lba: u32) -> Option<(u32, usize)> {
        let hunk_size = u64::from(self.hunk_size);
        let byte = match self.layout {
            ChdLayout::Cd { first_frame, .. } => first_frame
                .checked_add(u64::from(lba))?
                .checked_mul(CD_FRAME)?,
            ChdLayout::Dvd => u64::from(lba).checked_mul(USER_DATA as u64)?,
        };
        let hunk = u32::try_from(byte / hunk_size).ok()?;
        (hunk < self.chd.header().hunk_count()).then_some((hunk, (byte % hunk_size) as usize))
    }

    pub(crate) fn read_sector(&mut self, lba: u32) -> Option<[u8; USER_DATA]> {
        let (hunk, at) = self.locate(lba)?;
        if self.cached != Some(hunk) {
            self.cached = None;
            self.load(hunk)?;
            self.cached = Some(hunk);
        }
        let sector = self.hunk.get(at..)?;
        let off = match self.layout {
            ChdLayout::Cd { layout, .. } => layout.user_offset(sector),
            ChdLayout::Dvd => 0,
        };
        sector.get(off..off + USER_DATA)?.try_into().ok()
    }

    /// Descomprime o hunk `n` em `self.hunk`.
    fn load(&mut self, n: u32) -> Option<()> {
        if self.poisoned {
            return None;
        }
        let n = self.resolve(n)?;
        #[cfg(test)]
        {
            self.decompressed += u64::from(self.hunk_size);
        }
        let ok = catch_unwind(AssertUnwindSafe(|| {
            let mut hunk = self.chd.hunk(n).ok()?;
            hunk.read_hunk_in(&mut self.comp, &mut self.hunk).ok()
        }));
        match ok {
            Ok(Some(_)) => Some(()),
            Ok(None) => None,
            Err(_) => {
                self.poisoned = true;
                log::warn!("serial: o leitor de chd falhou numa imagem; ela fica sem serial");
                None
            }
        }
    }

    /// Segue "igual ao hunk tal" até o hunk que tem os bytes, sempre pra
    /// trás (o `chdman` só aponta pra hunk anterior; um ciclo viraria
    /// recursão sem fim no leitor), e confere que o bloco cabe no arquivo.
    fn resolve(&self, mut n: u32) -> Option<u32> {
        /// O `chdman` aponta direto pro hunk original; cadeia longa é imagem quebrada.
        const MAX_HOPS: usize = 16;
        let limit = u64::from(self.hunk_size) * 2;
        let fits = |offset: u64, size: u32| {
            u64::from(size) <= limit
                && offset
                    .checked_add(u64::from(size))
                    .is_some_and(|end| end <= self.file_len)
        };
        for _ in 0..MAX_HOPS {
            let map: &Map = self.chd.map();
            let target = match map.get_entry(n as usize)? {
                MapEntry::V5Compressed(e) => match e.hunk_type().ok()? {
                    CompressionTypeV5::CompressionSelf => e.block_offset(),
                    CompressionTypeV5::CompressionType0
                    | CompressionTypeV5::CompressionType1
                    | CompressionTypeV5::CompressionType2
                    | CompressionTypeV5::CompressionType3
                    | CompressionTypeV5::CompressionNone => {
                        return fits(e.block_offset(), e.block_size()).then_some(n);
                    }
                    _ => return None,
                },
                MapEntry::V5Uncompressed(e) => {
                    let ok = e.block_offset() == 0 || fits(e.block_offset(), e.block_size());
                    return ok.then_some(n);
                }
                MapEntry::LegacyEntry(e) => match e.hunk_type().ok()? {
                    CompressionTypeLegacy::SelfHunk => e.block_offset(),
                    CompressionTypeLegacy::Compressed | CompressionTypeLegacy::Uncompressed => {
                        return fits(e.block_offset(), e.block_size()).then_some(n);
                    }
                    CompressionTypeLegacy::Mini => return Some(n),
                    _ => return None,
                },
            };
            let target = u32::try_from(target).ok()?;
            if target >= n {
                return None;
            }
            n = target;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chd_track_offsets_count_padding_and_recorded_pregap() {
        let t = |number, kind: &str, frames, pregap, v| ChdCdTrack {
            number,
            kind: kind.to_string(),
            frames,
            pregap,
            pregap_in_data: v,
        };
        let tracks = vec![
            t(1, "MODE1_RAW", 302, 0, false),
            t(2, "AUDIO", 75, 0, false),
            t(3, "MODE1_RAW", 1000, 150, true),
        ];
        // 302 → 304, 75 → 76, e o pregap gravado da trilha 3.
        assert_eq!(
            chd_cd_start(&tracks, true, Area::HighDensity),
            Some((304 + 76 + 150, Layout::Raw { mode2: false }))
        );
        assert_eq!(
            chd_cd_start(&tracks, true, Area::FirstData),
            Some((0, Layout::Raw { mode2: false }))
        );
        assert_eq!(
            parse_cd_track(
                "TRACK:3 TYPE:MODE1_RAW SUBTYPE:NONE FRAMES:1000 PAD:0 PREGAP:150 PGTYPE:VMODE1 PGSUB:RW POSTGAP:0\0"
            ),
            Some(t(3, "MODE1_RAW", 1000, 150, true))
        );
        assert_eq!(parse_cd_track("TRACK:x TYPE:AUDIO"), None);
    }
}
