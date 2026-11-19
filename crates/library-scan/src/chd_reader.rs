//! Trilha de dados de dentro de um `.chd` (CD/GD-ROM pelos metadados de
//! trilha, ou DVD), lida hunk a hunk pelo crate `chd` e entregue setor a
//! setor ao [`crate::sector::SectorReader`], que cobra o limite de leitura.
//!
//! O `chd` aloca o que o arquivo pede (mapa, metadados) e indexa campos sem
//! conferir. Antes de entregar o arquivo a ele, todo tamanho que ele usaria
//! pra alocar é conferido aqui contra uma constante e contra o tamanho do
//! arquivo; o que sobra de pânico vira `None` pelo `catch_unwind`.

use crate::sector::{Area, Layout, ReadSeek, USER_DATA};
use chd::map::{CompressionTypeLegacy, CompressionTypeV5, Map, MapEntry};
use chd::metadata::KnownMetadata;
use chd::Chd;
use std::io::{Read, Seek, SeekFrom};
use std::panic::{catch_unwind, AssertUnwindSafe};

// O `catch_unwind` daqui só segura pânico com `panic = "unwind"`. Com
// "abort", todo `.chd` quebrado derrubaria o app em vez de ficar sem serial.
#[cfg(panic = "abort")]
compile_error!(
    "library-scan precisa de panic = \"unwind\": o leitor de chd conta com catch_unwind"
);

/// Maior hunk aceito num `.chd` (o de CD tem 19584 bytes; o de DVD, 4096).
const CHD_MAX_HUNK: u32 = 1024 * 1024;
/// Mais hunks que isto e o mapa não cabe em memória com folga (um DVD de
/// dupla camada em hunks de 4096 tem uns 2,1 milhões).
const CHD_MAX_HUNKS: u32 = 1 << 22;
/// Uma entrada do mapa comprimido do V5 ocupa uns 4 bytes (no máximo uns 7:
/// código de Huffman, tamanho e CRC); 16 por hunk mais uma folga cobre
/// qualquer mapa de verdade e põe teto no que o `chd` aloca pra ler o mapa.
const CHD_MAP_BYTES_PER_HUNK: u64 = 16;
const CHD_MAP_SLACK: u64 = 64 * 1024;
/// Entradas de metadados: uma trilha tem ~100 bytes, um disco rígido ~40.
/// Maior que isto não é de disco de jogo, e o `chd` alocaria o tamanho todo.
const CHD_MAX_METADATA: usize = 64 * 1024;
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

fn be_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn be_u64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
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
        use chd::header::Header;
        let file_len = src.seek(SeekFrom::End(0)).ok()?;
        // O começo do cabeçalho lido aqui, antes do `chd`: no V3/V4 ele
        // percorre os metadados dentro do próprio parse do cabeçalho, então a
        // lista é conferida (e limitada) antes.
        let mut raw = [0u8; 56];
        src.seek(SeekFrom::Start(0)).ok()?;
        src.read_exact(&mut raw).ok()?;
        if raw.get(..8)? != b"MComprHD" {
            return None;
        }
        let meta_offset = match be_u32(&raw, 12)? {
            3 | 4 => be_u64(&raw, 36)?,
            5 => be_u64(&raw, 48)?,
            _ => 0,
        };
        let layout = Self::chd_layout(&mut src, meta_offset, area)?;

        src.seek(SeekFrom::Start(0)).ok()?;
        let header = Header::try_read_header(&mut src).ok()?;
        let hunk_size = header.hunk_size();
        if header.has_parent()
            || hunk_size == 0
            || hunk_size > CHD_MAX_HUNK
            || header.hunk_count() > CHD_MAX_HUNKS
        {
            return None;
        }
        match layout {
            ChdLayout::Dvd if !(hunk_size as usize).is_multiple_of(USER_DATA) => return None,
            ChdLayout::Cd { .. } if !u64::from(hunk_size).is_multiple_of(CD_FRAME) => return None,
            _ => {}
        }
        // O mapa: o `chd` aloca o tamanho dele antes de ler. Tem de caber no
        // arquivo e no que o número de hunks justifica (o comprimido diz o
        // próprio tamanho, e um `mapoffset` torto leria esse número de
        // qualquer lugar do arquivo).
        let hunks = u64::from(header.hunk_count());
        let map_end = match &header {
            Header::V5Header(v5) if header.is_compressed() => {
                let mut len = [0u8; 4];
                src.seek(SeekFrom::Start(v5.map_offset)).ok()?;
                src.read_exact(&mut len).ok()?;
                let len = u64::from(u32::from_be_bytes(len));
                if len > hunks * CHD_MAP_BYTES_PER_HUNK + CHD_MAP_SLACK {
                    return None;
                }
                v5.map_offset.checked_add(16)?.checked_add(len)?
            }
            Header::V5Header(v5) => v5.map_offset.checked_add(hunks * 4)?,
            Header::V3Header(_) | Header::V4Header(_) => {
                u64::from(header.len()).checked_add((hunks + 1) * 16)?
            }
            Header::V1Header(_) | Header::V2Header(_) => {
                u64::from(header.len()).checked_add((hunks + 1) * 8)?
            }
        };
        if map_end > file_len {
            return None;
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
    /// Lido aqui, e não pelo `chd`, pra conferir o tamanho antes de alocar;
    /// lista longa, em ciclo ou com entrada grande demais: `None`.
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
            if len > CHD_MAX_METADATA {
                return None;
            }
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
    use std::io;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    /// Arquivo que diz ter `len` bytes, entrega `data` no começo e zeros no
    /// resto, e conta cada byte que sai dele.
    struct Sparse {
        data: Vec<u8>,
        len: u64,
        pos: u64,
        read: Arc<AtomicU64>,
    }

    impl Read for Sparse {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = (buf.len() as u64).min(self.len.saturating_sub(self.pos)) as usize;
            for (i, b) in buf[..n].iter_mut().enumerate() {
                let at = self.pos as usize + i;
                *b = self.data.get(at).copied().unwrap_or(0);
            }
            self.pos += n as u64;
            self.read.fetch_add(n as u64, Ordering::Relaxed);
            Ok(n)
        }
    }

    impl Seek for Sparse {
        fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
            let base = match to {
                SeekFrom::Start(n) => Some(n),
                SeekFrom::End(d) => self.len.checked_add_signed(d),
                SeekFrom::Current(d) => self.pos.checked_add_signed(d),
            };
            self.pos = base.ok_or(io::ErrorKind::InvalidInput)?;
            Ok(self.pos)
        }
    }

    const GIB: u64 = 1 << 30;

    /// Cabeçalho V5 (124 bytes); `lzma` como primeiro codec torna o mapa
    /// comprimido, sem codec é o mapa cru de 4 bytes por hunk.
    fn v5(logical: u64, map_offset: u64, meta_offset: u64, hunk: u32, compressed: bool) -> Vec<u8> {
        let mut h = vec![0u8; 124];
        h[..8].copy_from_slice(b"MComprHD");
        h[8..12].copy_from_slice(&124u32.to_be_bytes());
        h[12..16].copy_from_slice(&5u32.to_be_bytes());
        if compressed {
            h[16..20].copy_from_slice(b"lzma");
        }
        h[32..40].copy_from_slice(&logical.to_be_bytes());
        h[40..48].copy_from_slice(&map_offset.to_be_bytes());
        h[48..56].copy_from_slice(&meta_offset.to_be_bytes());
        h[56..60].copy_from_slice(&hunk.to_be_bytes());
        h[60..64].copy_from_slice(&2048u32.to_be_bytes());
        h
    }

    /// Cabeçalho V4 (108 bytes), sem compressão.
    fn v4(hunks: u32, meta_offset: u64) -> Vec<u8> {
        let mut h = vec![0u8; 108];
        h[..8].copy_from_slice(b"MComprHD");
        h[8..12].copy_from_slice(&108u32.to_be_bytes());
        h[12..16].copy_from_slice(&4u32.to_be_bytes());
        h[24..28].copy_from_slice(&hunks.to_be_bytes());
        h[28..36].copy_from_slice(&(u64::from(hunks) * 4096).to_be_bytes());
        h[36..44].copy_from_slice(&meta_offset.to_be_bytes());
        h[44..48].copy_from_slice(&4096u32.to_be_bytes());
        h
    }

    /// Entrada de metadados: tag, tamanho em 24 bits, próxima.
    fn meta(data: &mut Vec<u8>, at: usize, tag: &[u8; 4], len: u32, next: u64) {
        if data.len() < at + 16 {
            data.resize(at + 16, 0);
        }
        data[at..at + 4].copy_from_slice(tag);
        data[at + 5..at + 8].copy_from_slice(&len.to_be_bytes()[1..]);
        data[at + 8..at + 16].copy_from_slice(&next.to_be_bytes());
    }

    /// Abre `data` como se fosse um arquivo de `len` bytes; devolve se abriu
    /// e quantos bytes foram lidos dele.
    fn open_crafted(data: Vec<u8>, len: u64) -> (bool, u64) {
        let read = Arc::new(AtomicU64::new(0));
        let src = Sparse {
            data,
            len,
            pos: 0,
            read: read.clone(),
        };
        let opened = ChdTrack::open(Box::new(src), Area::FirstData).is_some();
        (opened, read.load(Ordering::Relaxed))
    }

    /// Cabeçalhos forjados que pedem mapa ou metadados enormes: nada é
    /// alocado nem lido além de uns poucos KiB, e a imagem fica sem serial.
    #[test]
    fn crafted_headers_never_allocate_what_they_ask() {
        // V5 comprimido, 256 hunks, mapa que diz ter 256 MiB num arquivo de
        // 8 GiB (o `mapoffset` apontando pro meio dos dados): cabe no arquivo,
        // não cabe no que 256 hunks justificam.
        let mut f = v5(256 * 4096, 4096, 0, 4096, true);
        f.resize(4096 + 16, 0);
        f[4096..4100].copy_from_slice(&(256u32 << 20).to_be_bytes());
        let (opened, read) = open_crafted(f, 8 * GIB);
        assert!(!opened);
        assert!(read < 64 * 1024, "leu {read} bytes");

        // V5 comprimido dizendo ter 4 GiB de mapa: maior que tudo.
        let mut f = v5(256 * 4096, 4096, 0, 4096, true);
        f.resize(4096 + 16, 0);
        f[4096..4100].copy_from_slice(&u32::MAX.to_be_bytes());
        let (opened, read) = open_crafted(f, 8 * GIB);
        assert!(!opened);
        assert!(read < 64 * 1024, "leu {read} bytes");

        // V5 cru com 4 milhões de hunks (16 MiB de mapa) num arquivo de 64 KiB.
        let f = v5((1 << 22) * 4096, 124, 0, 4096, false);
        let (opened, read) = open_crafted(f, 64 * 1024);
        assert!(!opened);
        assert!(read < 64 * 1024, "leu {read} bytes");

        // Hunks demais, hunk grande demais, tamanho lógico que estoura a soma
        // do `chd` (pânico dele, que vira `None`).
        for f in [
            v5(u64::from(u32::MAX) * 4096, 124, 0, 4096, true),
            v5(64 * GIB, 124, 0, 1 << 30, true),
            v5(u64::MAX, 124, 0, 4096, true),
        ] {
            let (opened, read) = open_crafted(f, 8 * GIB);
            assert!(!opened);
            assert!(read < 64 * 1024, "leu {read} bytes");
        }

        // Metadado de 16 MiB (o `chd` alocaria tudo pra ler).
        let mut f = v5(256 * 4096, 0, 4096, 4096, true);
        meta(&mut f, 4096, b"CHT2", 0x00FF_FFFF, 0);
        let (opened, read) = open_crafted(f.clone(), 8 * GIB);
        assert!(!opened);
        assert!(read < 64 * 1024, "leu {read} bytes");
        meta(&mut f, 4096, b"GDDD", 0x00FF_FFFF, 0);
        assert!(!open_crafted(f, 8 * GIB).0);

        // V4: o `chd` percorre os metadados no parse do cabeçalho. Lista de
        // 1000 entradas e lista em ciclo param antes, na nossa conferência.
        let mut f = v4(16, 4096);
        for i in 0..1000u64 {
            let at = 4096 + i as usize * 16;
            meta(&mut f, at, b"IDNT", 0, 4096 + (i + 1) * 16);
        }
        let (opened, read) = open_crafted(f, 8 * GIB);
        assert!(!opened);
        assert!(read < 64 * 1024, "leu {read} bytes");
        let mut f = v4(16, 4096);
        meta(&mut f, 4096, b"IDNT", 0, 4112);
        meta(&mut f, 4112, b"IDNT", 0, 4096);
        assert!(!open_crafted(f, 8 * GIB).0);
    }

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
