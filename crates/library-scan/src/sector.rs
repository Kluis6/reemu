//! Leitor de setores de imagem de disco: devolve os 2048 bytes de dados de
//! usuário de cada setor, venha a imagem como `.iso` (2048), `.bin` cru
//! (2352, MODE1 ou MODE2), MODE2/2336, ou de dentro de um `.chd`.
//!
//! Lê no máximo [`READ_BUDGET`] bytes por imagem (NFR-5): cada setor cru
//! puxado do arquivo, ou cada hunk descomprimido do `.chd`, conta inteiro
//! contra o limite, e passado dele a leitura falha (`None`). Uma imagem de
//! vários GB nunca é lida inteira, e nada aqui entra em pânico com imagem
//! quebrada: offset e tamanho vindos do arquivo são conferidos antes de usar.

use crate::chd_reader::ChdTrack;
use crate::disc_sniff::{parse_cue, sibling, CUE_MAX_BYTES};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Bytes de dados de usuário num setor.
pub const USER_DATA: usize = 2048;

/// Quanto se lê, no máximo, de uma imagem (NFR-5: 2 MiB).
pub const READ_BUDGET: u64 = 2 * 1024 * 1024;

/// Fonte de bytes de uma imagem (o arquivo, ou outra coisa nos testes).
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

/// Abre um arquivo da imagem (a própria, o `.bin` de um `.cue`...).
pub type Opener<'a> = &'a dyn Fn(&Path) -> io::Result<Box<dyn ReadSeek>>;

/// O jeito normal de abrir: o arquivo do disco. Sem `BufReader`: a leitura
/// é de setor inteiro depois de um `seek`, e o buffer só leria a mais.
///
/// Só arquivo comum: um `FILE "CON"` ou `"COM1"` num `.cue`/`.gdi` viraria
/// dispositivo no Windows (sem `seek`, e a leitura pode travar), e uma pasta
/// não é imagem.
pub fn open_file(path: &Path) -> io::Result<Box<dyn ReadSeek>> {
    if !path.is_file() {
        return Err(io::ErrorKind::NotFound.into());
    }
    Ok(Box::new(std::fs::File::open(path)?))
}

/// Como os setores de uma trilha de dados estão gravados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// 2048 bytes por setor, só os dados (`.iso`, `MODE1/2048`).
    Cooked,
    /// 2352 bytes por setor, cru. Os dados começam em 16 no MODE1 e em 24 no
    /// MODE2 form 1; vale o byte de modo do próprio setor, e `mode2` quando
    /// ele não diz.
    Raw { mode2: bool },
    /// MODE2/2336: sem sync nem cabeçalho, os dados começam em 8.
    Mode2Headerless,
}

impl Layout {
    /// Bytes por setor no arquivo.
    pub fn sector_size(self) -> usize {
        match self {
            Layout::Cooked => 2048,
            Layout::Raw { .. } => 2352,
            Layout::Mode2Headerless => 2336,
        }
    }

    /// Onde começam os dados de usuário dentro de `sector`.
    pub fn user_offset(self, sector: &[u8]) -> usize {
        match self {
            Layout::Cooked => 0,
            Layout::Mode2Headerless => 8,
            Layout::Raw { mode2 } => match sector.get(15) {
                Some(1) => 16,
                Some(2) => 24,
                _ if mode2 => 24,
                _ => 16,
            },
        }
    }

    /// Modo de trilha do `.cue` (`MODE2/2352`) ou do `.chd` (`MODE2_RAW`).
    /// `None` pra áudio ou modo desconhecido.
    pub fn from_mode(mode: &str) -> Option<Layout> {
        Some(match mode {
            "MODE1/2048" | "MODE1" | "MODE2/2048" | "MODE2_FORM1" => Layout::Cooked,
            "MODE1/2352" | "MODE1_RAW" => Layout::Raw { mode2: false },
            "MODE2/2352" | "MODE2_RAW" | "CDI/2352" => Layout::Raw { mode2: true },
            "MODE2/2336" | "MODE2" | "MODE2_FORM_MIX" => Layout::Mode2Headerless,
            _ => return None,
        })
    }
}

/// O início dos 12 bytes de sync de um setor cru.
const SYNC: [u8; 12] = [
    0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00,
];

/// Qual trilha de dados ler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    /// A primeira trilha de dados (PS1, PS2, PSP, Saturn, Sega CD).
    FirstData,
    /// A primeira trilha de dados da área de alta densidade do GD-ROM
    /// (Dreamcast); sem essa informação, a primeira trilha de dados.
    HighDensity,
}

impl Area {
    /// De onde vem a identidade de um disco de `system`: no Dreamcast, a área
    /// de alta densidade (a trilha 3 do GD-ROM); nos outros, a 1ª trilha de
    /// dados. O serial e o hash da trilha usam a mesma.
    pub fn for_system(system: &str) -> Area {
        if system == "dreamcast" {
            Area::HighDensity
        } else {
            Area::FirstData
        }
    }
}

/// A trilha de dados escolhida de um `.cue` ou `.gdi`: o arquivo, como os
/// setores estão gravados e em que byte do arquivo a trilha começa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTrack {
    pub file: PathBuf,
    pub layout: Layout,
    pub start: u64,
}

/// Lê o texto de um `.cue`/`.gdi` (pequeno; passado de [`CUE_MAX_BYTES`] não é).
fn read_sheet(path: &Path, open: Opener) -> Option<String> {
    let mut bytes = Vec::new();
    open(path)
        .ok()?
        .take(CUE_MAX_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// A trilha de dados de um `.cue` ou `.gdi` (pela extensão de `path`).
pub fn data_track(path: &Path, area: Area, open: Opener) -> Option<DataTrack> {
    let text = read_sheet(path, open)?;
    match ext_of(path).as_str() {
        "cue" => cue_data_track(path, &text, area),
        "gdi" => gdi_data_track(path, &text, area),
        _ => None,
    }
}

fn cue_data_track(path: &Path, text: &str, area: Area) -> Option<DataTrack> {
    let tracks = parse_cue(text);
    let data = |t: &&crate::disc_sniff::CueTrack| Layout::from_mode(&t.mode).is_some();
    let track = match area {
        Area::HighDensity => tracks
            .iter()
            .filter(data)
            .find(|t| t.high_density)
            .or_else(|| tracks.iter().find(data)),
        Area::FirstData => tracks.iter().find(data),
    }?;
    let layout = Layout::from_mode(&track.mode)?;
    let size = layout.sector_size() as u64;
    // Byte da trilha no arquivo: o que vem antes do INDEX 00 é de trilhas de
    // áudio anteriores (2352 por setor); do INDEX 00 ao 01 é o pregap desta
    // trilha, no formato dela. Primeira do arquivo sem INDEX 00: tudo dela.
    let index1 = u64::from(track.index1.unwrap_or(0));
    let start = match track.index0 {
        Some(i0) => {
            let i0 = u64::from(i0);
            i0 * 2352 + index1.checked_sub(i0)? * size
        }
        None if track.first_in_file => index1 * size,
        None => index1 * 2352,
    };
    Some(DataTrack {
        file: sibling(path, &track.file)?,
        layout,
        start,
    })
}

/// `.gdi`: 1ª linha é o número de trilhas; depois, por trilha,
/// `número lba tipo tamanho_do_setor arquivo offset` (tipo 4 = dados).
fn gdi_data_track(path: &Path, text: &str, area: Area) -> Option<DataTrack> {
    /// A área de alta densidade do GD-ROM começa no LBA 45000.
    const HIGH_DENSITY_LBA: u64 = 45_000;
    let mut tracks = Vec::new();
    for line in text.lines().skip(1) {
        let mut fields = gdi_fields(line).into_iter();
        let (Some(_n), Some(lba), Some(kind), Some(size), Some(file)) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            continue;
        };
        let (Ok(lba), Ok(kind), Ok(size)) =
            (lba.parse::<u64>(), kind.parse::<u32>(), size.parse::<u32>())
        else {
            continue;
        };
        let layout = match size {
            2048 => Layout::Cooked,
            2352 => Layout::Raw { mode2: false },
            2336 => Layout::Mode2Headerless,
            _ => continue,
        };
        if kind == 4 {
            tracks.push((lba, layout, file));
        }
    }
    let track = match area {
        Area::HighDensity => tracks
            .iter()
            .find(|t| t.0 >= HIGH_DENSITY_LBA)
            .or_else(|| tracks.first()),
        Area::FirstData => tracks.first(),
    }?;
    Some(DataTrack {
        file: sibling(path, &track.2)?,
        layout: track.1,
        start: 0,
    })
}

/// Campos de uma linha de `.gdi`, com nome de arquivo entre aspas se tiver espaço.
fn gdi_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line.trim();
    while !rest.is_empty() {
        if let Some(q) = rest.strip_prefix('"') {
            let Some(end) = q.find('"') else {
                break;
            };
            out.push(q[..end].to_string());
            rest = q[end + 1..].trim_start();
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push(rest[..end].to_string());
            rest = rest[end..].trim_start();
        }
    }
    out
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

/// Lê os dados de usuário dos setores de uma trilha de dados, dentro do
/// limite de [`READ_BUDGET`].
pub struct SectorReader {
    backend: Backend,
    used: u64,
}

enum Backend {
    Plain {
        src: Box<dyn ReadSeek>,
        layout: Layout,
        start: u64,
        buf: Vec<u8>,
    },
    Chd(Box<ChdTrack>),
}

impl SectorReader {
    /// Abre a trilha de dados de `path` (`.iso`, `.img`, `.bin`, `.cue`,
    /// `.gdi`, `.chd`). Outro formato (`.cso`, `.pbp`...) ou falha: `None`.
    pub fn open(path: &Path, area: Area, open: Opener) -> Option<SectorReader> {
        match ext_of(path).as_str() {
            "iso" | "img" | "bin" => {
                let mut src = open(path).ok()?;
                let mut head = [0u8; 12];
                src.read_exact(&mut head).ok()?;
                let layout = if head == SYNC {
                    Layout::Raw { mode2: false }
                } else {
                    Layout::Cooked
                };
                let mut reader = SectorReader::plain(src, layout, 0);
                reader.used = head.len() as u64;
                Some(reader)
            }
            "cue" | "gdi" => {
                let track = data_track(path, area, open)?;
                Some(SectorReader::plain(
                    open(&track.file).ok()?,
                    track.layout,
                    track.start,
                ))
            }
            "chd" => {
                let track = ChdTrack::open(open(path).ok()?, area)?;
                Some(SectorReader {
                    backend: Backend::Chd(Box::new(track)),
                    used: 0,
                })
            }
            _ => None,
        }
    }

    /// Leitor sobre uma fonte já aberta: setores no formato `layout`, a
    /// partir do byte `start`.
    pub fn plain(src: Box<dyn ReadSeek>, layout: Layout, start: u64) -> SectorReader {
        SectorReader {
            backend: Backend::Plain {
                src,
                layout,
                start,
                buf: vec![0u8; layout.sector_size()],
            },
            used: 0,
        }
    }

    /// Bytes já puxados da imagem (crus, ou descomprimidos no `.chd`).
    pub fn used(&self) -> u64 {
        self.used
    }

    /// Bytes que o descompressor do `.chd` produziu, contados à parte.
    #[cfg(test)]
    pub fn decompressed(&self) -> u64 {
        match &self.backend {
            Backend::Chd(track) => track.decompressed,
            Backend::Plain { .. } => 0,
        }
    }

    /// Reserva `n` bytes do limite; `false` se passaria dele.
    fn charge(&mut self, n: u64) -> bool {
        match self.used.checked_add(n) {
            Some(total) if total <= READ_BUDGET => {
                self.used = total;
                true
            }
            _ => {
                self.used = READ_BUDGET;
                false
            }
        }
    }

    /// Os 2048 bytes de dados do setor `lba` da trilha.
    pub fn read_sector(&mut self, lba: u32) -> Option<[u8; USER_DATA]> {
        let cost = match &self.backend {
            Backend::Plain { layout, .. } => layout.sector_size() as u64,
            Backend::Chd(track) => track.cost_of(lba)?,
        };
        if !self.charge(cost) {
            return None;
        }
        match &mut self.backend {
            Backend::Plain {
                src,
                layout,
                start,
                buf,
            } => {
                let pos = (u64::from(lba))
                    .checked_mul(layout.sector_size() as u64)?
                    .checked_add(*start)?;
                src.seek(SeekFrom::Start(pos)).ok()?;
                src.read_exact(buf).ok()?;
                let off = layout.user_offset(buf);
                buf.get(off..off + USER_DATA)?.try_into().ok()
            }
            Backend::Chd(track) => track.read_sector(lba),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn raw_sector(mode: u8, payload: &[u8]) -> Vec<u8> {
        let mut s = vec![0u8; 2352];
        s[..12].copy_from_slice(&SYNC);
        s[15] = mode;
        let off = if mode == 2 { 24 } else { 16 };
        s[off..off + payload.len()].copy_from_slice(payload);
        s
    }

    #[test]
    fn user_data_offsets_by_layout() {
        let mode1 = raw_sector(1, b"M1");
        let mode2 = raw_sector(2, b"M2");
        assert_eq!(Layout::Raw { mode2: false }.user_offset(&mode1), 16);
        assert_eq!(Layout::Raw { mode2: false }.user_offset(&mode2), 24);
        assert_eq!(Layout::Raw { mode2: true }.user_offset(&[0u8; 2352]), 24);
        assert_eq!(Layout::Mode2Headerless.user_offset(&[0u8; 2336]), 8);
        assert_eq!(Layout::Cooked.user_offset(&[0u8; 2048]), 0);
    }

    #[test]
    fn reads_user_data_from_raw_and_headerless_sectors() {
        let mut img = Vec::new();
        img.extend(raw_sector(1, b"zero"));
        img.extend(raw_sector(2, b"um"));
        let mut r =
            SectorReader::plain(Box::new(Cursor::new(img)), Layout::Raw { mode2: false }, 0);
        assert_eq!(&r.read_sector(0).unwrap()[..4], b"zero");
        assert_eq!(&r.read_sector(1).unwrap()[..2], b"um");
        assert!(r.read_sector(2).is_none(), "depois do fim");

        let mut img = vec![0u8; 2336 * 2];
        img[2336 + 8..2336 + 12].copy_from_slice(b"2336");
        let mut r = SectorReader::plain(Box::new(Cursor::new(img)), Layout::Mode2Headerless, 0);
        assert_eq!(&r.read_sector(1).unwrap()[..4], b"2336");
    }

    #[test]
    fn budget_stops_reading() {
        let sectors = (READ_BUDGET / 2048) as u32;
        let img = vec![0u8; (READ_BUDGET + 4096) as usize];
        let mut r = SectorReader::plain(Box::new(Cursor::new(img)), Layout::Cooked, 0);
        for lba in 0..sectors {
            assert!(r.read_sector(lba).is_some(), "setor {lba}");
        }
        assert!(r.read_sector(sectors).is_none(), "passou do limite");
        assert_eq!(r.used(), READ_BUDGET);
    }

    #[test]
    fn cue_track_start_and_layout() {
        let cue = "FILE \"Jogo.bin\" BINARY\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n  TRACK 02 MODE1/2352\n    INDEX 00 00:10:00\n    INDEX 01 00:12:00\n";
        let t = cue_data_track(Path::new("d/Jogo.cue"), cue, Area::FirstData).unwrap();
        assert_eq!(t.file, Path::new("d").join("Jogo.bin"));
        assert_eq!(t.layout, Layout::Raw { mode2: false });
        assert_eq!(t.start, 750 * 2352 + 150 * 2352);

        let cue = "FILE \"a.bin\" BINARY\n TRACK 01 MODE2/2336\n  INDEX 00 00:00:00\n  INDEX 01 00:02:00\n";
        let t = cue_data_track(Path::new("a.cue"), cue, Area::FirstData).unwrap();
        assert_eq!(t.layout, Layout::Mode2Headerless);
        assert_eq!(t.start, 150 * 2336);

        let cue = "REM SINGLE-DENSITY AREA\nFILE \"t1.bin\" BINARY\n TRACK 01 MODE1/2352\n  INDEX 01 00:00:00\nFILE \"t2.bin\" BINARY\n TRACK 02 AUDIO\n  INDEX 01 00:00:00\nREM HIGH-DENSITY AREA\nFILE \"t3.bin\" BINARY\n TRACK 03 MODE1/2352\n  INDEX 01 00:00:00\n";
        let hd = cue_data_track(Path::new("dc.cue"), cue, Area::HighDensity).unwrap();
        assert_eq!(hd.file, Path::new("t3.bin"));
        let first = cue_data_track(Path::new("dc.cue"), cue, Area::FirstData).unwrap();
        assert_eq!(first.file, Path::new("t1.bin"));
    }

    #[test]
    fn cue_and_gdi_never_leave_their_folder() {
        let cue = "FILE \"../fora.bin\" BINARY\n TRACK 01 MODE1/2352\n  INDEX 01 00:00:00\n";
        assert!(cue_data_track(Path::new("d/x.cue"), cue, Area::FirstData).is_none());
        let gdi = "1\n1 0 4 2352 \"/etc/fora.bin\" 0\n";
        assert!(gdi_data_track(Path::new("d/x.gdi"), gdi, Area::FirstData).is_none());
    }

    #[test]
    fn gdi_high_density_track() {
        let gdi = "3\n1 0 4 2352 track01.bin 0\n2 450 0 2352 track02.raw 0\n3 45000 4 2352 \"track 03.bin\" 0\n";
        let hd = gdi_data_track(Path::new("g/j.gdi"), gdi, Area::HighDensity).unwrap();
        assert_eq!(hd.file, Path::new("g").join("track 03.bin"));
        assert_eq!(hd.layout, Layout::Raw { mode2: false });
        let first = gdi_data_track(Path::new("g/j.gdi"), gdi, Area::FirstData).unwrap();
        assert_eq!(first.file, Path::new("g").join("track01.bin"));
    }
}
