//! Serial do disco lido da imagem (D138): PS1 e PS2 pelo `SYSTEM.CNF`, PSP
//! pelo `PARAM.SFO`, Saturn, Sega CD e Dreamcast pelo cabeçalho do primeiro
//! setor da trilha de dados, e GameCube pelo cabeçalho de 216 bytes (`.iso`,
//! `.gcm`, `.rvz`, `.wia`). Lê no máximo 2 MiB por imagem (NFR-5, ver
//! [`crate::sector`]); qualquer falha (sistema de arquivos desconhecido,
//! imagem quebrada, homebrew sem `SYSTEM.CNF`, `.cso`) dá `None`.

use crate::iso9660::{self, le_u16, le_u32};
use crate::sector::{open_file, Area, Opener, SectorReader};
use std::path::Path;

/// Sistemas do ReEmu (`systems.rs`) em que o serial vem da imagem.
pub const SERIAL_SYSTEMS: &[&str] = &[
    "psx",
    "ps2",
    "psp",
    "saturn",
    "segacd",
    "dreamcast",
    "gamecube",
];

/// `SYSTEM.CNF` tem algumas dezenas de bytes.
const SYSTEM_CNF_MAX: usize = 16 * 1024;
/// `PARAM.SFO` tem por volta de 1 KiB.
const PARAM_SFO_MAX: usize = 64 * 1024;

/// Serial do disco em `path`, normalizado (`SLUS_005.94` → `SLUS-00594`),
/// pra `system` (id do ReEmu). Lê `.iso`, `.img`, `.bin`, `.cue`, `.gdi` e
/// `.chd`; outro formato, outro sistema ou falha de leitura: `None`.
pub fn read_serial(path: &Path, system: &str) -> Option<String> {
    read_serial_with(path, system, &open_file)
}

/// [`read_serial`] com outro jeito de abrir os arquivos (os testes contam
/// os bytes lidos e servem imagens da memória).
pub(crate) fn read_serial_with(path: &Path, system: &str, open: Opener) -> Option<String> {
    if !SERIAL_SYSTEMS.contains(&system) {
        return None;
    }
    if system == "gamecube" {
        return read_gamecube_serial(path, open);
    }
    let mut reader = SectorReader::open(path, Area::for_system(system), open)?;
    let serial = serial_from(&mut reader, system);
    // Só o sistema e a contagem: nunca o arquivo nem o serial.
    log::debug!(
        "serial: {system}, {} ({} bytes lidos da imagem)",
        if serial.is_some() {
            "lido"
        } else {
            "não lido"
        },
        reader.used()
    );
    serial
}

fn read_gamecube_serial(path: &Path, open: Opener) -> Option<String> {
    let ext = crate::sector::ext_of(path);
    if !matches!(ext.as_str(), "iso" | "gcm" | "rvz" | "wia") {
        return None;
    }
    let mut src = open(path).ok()?;
    let mut buf = [0u8; crate::disc_sniff::RVZ_HEADER_MIN];
    let mut n = 0usize;
    while n < buf.len() {
        match std::io::Read::read(&mut src, &mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(_) => return None,
        }
    }
    let dhead = crate::disc_sniff::gc_disc_header(&buf[..n])?;
    let id_bytes = dhead.get(0..6)?;
    if !id_bytes.iter().all(u8::is_ascii_alphanumeric) {
        return None;
    }
    Some(String::from_utf8_lossy(id_bytes).to_ascii_uppercase())
}

/// O serial de `system` lido pelos setores de `reader`.
pub(crate) fn serial_from(reader: &mut SectorReader, system: &str) -> Option<String> {
    match system {
        "psx" => boot_serial(reader, "BOOT"),
        "ps2" => boot_serial(reader, "BOOT2"),
        "psp" => {
            let sfo = iso9660::lookup(reader, &["PSP_GAME", "PARAM.SFO"])?;
            let sfo = iso9660::read_file(reader, sfo, PARAM_SFO_MAX)?;
            normalize_sony(&sfo_string(&sfo, "DISC_ID")?)
        }
        "saturn" => {
            let s = reader.read_sector(0)?;
            s.starts_with(b"SEGA SEGASATURN ")
                .then(|| header_field(&s, 0x20, 10))?
        }
        "segacd" => {
            let s = reader.read_sector(0)?;
            let field = s
                .starts_with(b"SEGADISCSYSTEM")
                .then(|| header_field(&s, 0x180, 14))??;
            let field = field.strip_prefix("GM ").unwrap_or(&field).trim();
            (!field.is_empty()).then(|| field.to_string())
        }
        "dreamcast" => {
            let s = reader.read_sector(0)?;
            s.starts_with(b"SEGA SEGAKATANA ")
                .then(|| header_field(&s, 0x40, 10))?
        }
        _ => None,
    }
}

/// `SYSTEM.CNF` na raiz, linha `key = cdrom:\SLUS_005.94;1`: o nome do
/// arquivo depois da última `\` (ou do `:`) e antes do `;`, normalizado.
fn boot_serial(reader: &mut SectorReader, key: &str) -> Option<String> {
    let cnf = iso9660::lookup(reader, &["SYSTEM.CNF"])?;
    let cnf = iso9660::read_file(reader, cnf, SYSTEM_CNF_MAX)?;
    let text = String::from_utf8_lossy(&cnf);
    let value = text.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        k.trim().eq_ignore_ascii_case(key).then_some(v)
    })?;
    let file = value.trim().rsplit(['\\', '/', ':']).next()?;
    let file = file.split([';', ' ', '\t']).next()?;
    normalize_sony(file)
}

/// Serial da Sony em maiúsculas, `PREFIXO-NÚMERO`: `SLUS_005.94` →
/// `SLUS-00594`, `ULUS10041` → `ULUS-10041`. Nome que não tem esse formato
/// (`PSX.EXE` de homebrew) não é serial: `None`.
pub(crate) fn normalize_sony(name: &str) -> Option<String> {
    let up = name.trim().to_ascii_uppercase();
    let prefix_len = up.bytes().take_while(u8::is_ascii_alphabetic).count();
    if !(2..=6).contains(&prefix_len) {
        return None;
    }
    let (prefix, rest) = up.split_at(prefix_len);
    let digits: String = rest
        .chars()
        .filter(|c| !matches!(c, '_' | '.' | '-'))
        .collect();
    let ok = (3..=8).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit());
    ok.then(|| format!("{prefix}-{digits}"))
}

/// Campo de texto de cabeçalho Sega (ASCII com espaços), aparado e em maiúsculas.
fn header_field(sector: &[u8], at: usize, len: usize) -> Option<String> {
    let raw = sector.get(at..at + len)?;
    if !raw.iter().all(|&b| b == 0 || (0x20..0x7f).contains(&b)) {
        return None;
    }
    let text = String::from_utf8_lossy(raw);
    let text = text.trim_matches(|c: char| c == '\0' || c.is_ascii_whitespace());
    (!text.is_empty()).then(|| text.to_ascii_uppercase())
}

/// `u32` little-endian em `at`, como índice.
fn le_usize(b: &[u8], at: usize) -> Option<usize> {
    usize::try_from(le_u32(b, at)?).ok()
}

/// Valor de texto da chave `key` num `PARAM.SFO`: cabeçalho `\0PSF`, tabela
/// de chaves e de dados, e uma entrada de 16 bytes por chave a partir de 0x14.
fn sfo_string(sfo: &[u8], key: &str) -> Option<String> {
    /// Um `PARAM.SFO` de jogo tem umas 20 chaves.
    const MAX_ENTRIES: usize = 256;
    if sfo.get(0..4)? != b"\0PSF" {
        return None;
    }
    let keys = le_usize(sfo, 8)?;
    let data = le_usize(sfo, 12)?;
    let count = le_usize(sfo, 16)?.min(MAX_ENTRIES);
    for i in 0..count {
        let e = 0x14 + i * 16;
        let key_at = keys.checked_add(usize::from(le_u16(sfo, e)?))?;
        let format = le_u16(sfo, e + 2)?;
        let len = le_usize(sfo, e + 4)?;
        let data_at = data.checked_add(le_usize(sfo, e + 12)?)?;
        let name = sfo.get(key_at..)?;
        let name = &name[..name.iter().position(|&b| b == 0)?];
        if name != key.as_bytes() {
            continue;
        }
        // 0x0204: texto UTF-8 terminado em NUL; 0x0004: texto sem NUL.
        if format != 0x0204 && format != 0x0004 {
            return None;
        }
        let value = sfo.get(data_at..data_at.checked_add(len)?)?;
        let value = value.split(|&b| b == 0).next()?;
        return std::str::from_utf8(value).ok().map(str::to_string);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sector::{Layout, ReadSeek, READ_BUDGET, USER_DATA};
    use crate::test_io::Counting;
    use std::collections::HashMap;
    use std::io::{self, Cursor};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    // ---------- imagens sintéticas ----------

    const SECTOR: usize = USER_DATA;

    /// Registro de diretório ISO9660.
    fn dir_record(name: &[u8], lba: u32, size: u32, dir: bool) -> Vec<u8> {
        let mut len = 33 + name.len();
        len += len % 2;
        let mut r = vec![0u8; len];
        r[0] = len as u8;
        r[2..6].copy_from_slice(&lba.to_le_bytes());
        r[6..10].copy_from_slice(&lba.to_be_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[14..18].copy_from_slice(&size.to_be_bytes());
        r[25] = if dir { 2 } else { 0 };
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    /// ISO9660 de 64 KiB (32 setores): descritor primário no 16, terminador
    /// no 17, raiz no 18, um subdiretório opcional no 19 e os arquivos do 20
    /// em diante. `files`: caminho com no máximo uma pasta (`PSP_GAME/X`).
    fn iso(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut img = vec![0u8; 32 * SECTOR];
        let mut root = Vec::new();
        root.extend(dir_record(&[0], 18, SECTOR as u32, true));
        root.extend(dir_record(&[1], 18, SECTOR as u32, true));
        let mut sub = Vec::new();
        let mut sub_name: Option<String> = None;
        let mut next = 20u32;
        for (path, bytes) in files {
            let lba = next;
            let at = lba as usize * SECTOR;
            img[at..at + bytes.len()].copy_from_slice(bytes);
            next += (bytes.len().max(1)).div_ceil(SECTOR) as u32;
            let (dir, name) = match path.split_once('/') {
                Some((d, n)) => (Some(d), n),
                None => (None, *path),
            };
            let rec = dir_record(
                format!("{name};1").as_bytes(),
                lba,
                bytes.len() as u32,
                false,
            );
            match dir {
                Some(d) => {
                    sub_name = Some(d.to_string());
                    sub.extend(rec);
                }
                None => root.extend(rec),
            }
        }
        if let Some(d) = &sub_name {
            root.extend(dir_record(d.as_bytes(), 19, SECTOR as u32, true));
            let mut s = dir_record(&[0], 19, SECTOR as u32, true);
            s.extend(dir_record(&[1], 18, SECTOR as u32, true));
            s.extend(sub);
            img[19 * SECTOR..19 * SECTOR + s.len()].copy_from_slice(&s);
        }
        img[18 * SECTOR..18 * SECTOR + root.len()].copy_from_slice(&root);
        let pvd = &mut img[16 * SECTOR..17 * SECTOR];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 1;
        pvd[128..130].copy_from_slice(&(SECTOR as u16).to_le_bytes());
        pvd[156..156 + 34].copy_from_slice(&dir_record(&[0], 18, SECTOR as u32, true));
        let term = &mut img[17 * SECTOR..18 * SECTOR];
        term[0] = 255;
        term[1..6].copy_from_slice(b"CD001");
        img
    }

    /// Setores de 2048 → crus de 2352 (sync, modo, subcabeçalho no MODE2).
    fn to_raw(cooked: &[u8], mode: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(cooked.len() / SECTOR * 2352);
        for (i, s) in cooked.chunks(SECTOR).enumerate() {
            let mut raw = vec![0u8; 2352];
            raw[0] = 0;
            raw[1..11].fill(0xFF);
            raw[12] = (i / 4500) as u8;
            raw[13] = (i / 75 % 60) as u8;
            raw[14] = (i % 75) as u8;
            raw[15] = mode;
            let off = if mode == 2 {
                raw[16..24].copy_from_slice(&[0, 0, 8, 0, 0, 0, 8, 0]);
                24
            } else {
                16
            };
            raw[off..off + s.len()].copy_from_slice(s);
            out.extend(raw);
        }
        out
    }

    fn psx_cnf() -> &'static [u8] {
        b"BOOT = cdrom:\\SLUS_005.94;1\r\nTCB = 4\r\nEVENT = 10\r\nSTACK = 801FFFF0\r\n"
    }

    /// Imagem com cabeçalho Sega no setor 0 (32 KiB).
    fn sega_header(magic: &[u8], at: usize, field: &[u8]) -> Vec<u8> {
        let mut img = vec![0u8; 16 * SECTOR];
        img[..magic.len()].copy_from_slice(magic);
        img[at..at + field.len()].copy_from_slice(field);
        img
    }

    /// Escreve campos de texto num cabeçalho, como o disco de verdade tem:
    /// texto legível também nos offsets que outro sistema leria.
    fn fields(img: &mut [u8], list: &[(usize, &[u8])]) {
        for (at, text) in list {
            img[*at..*at + text.len()].copy_from_slice(text);
        }
    }

    fn saturn_image() -> Vec<u8> {
        let mut img = sega_header(b"SEGA SEGASATURN ", 0x20, b"MK-81022  ");
        fields(
            &mut img,
            &[
                (0x10, b"SEGA TP T-81    "),
                (0x2A, b"V1.000"),
                (0x30, b"19950512CD-1/1  "),
                (0x40, b"JTUE            "),
                (0x60, b"VIRTUA FIGHTER REMIX"),
            ],
        );
        img
    }

    fn segacd_image() -> Vec<u8> {
        let mut img = sega_header(b"SEGADISCSYSTEM  ", 0x180, b"GM MK-4407 -00");
        fields(
            &mut img,
            &[
                (0x10, b"SEGASYSTEM  "),
                (0x100, b"SEGA MEGA DRIVE (C)SEGA 1993.JUL"),
                (0x120, b"SONIC THE HEDGEHOG CD"),
            ],
        );
        img
    }

    fn dreamcast_ip_bin() -> Vec<u8> {
        let mut img = sega_header(b"SEGA SEGAKATANA ", 0x40, b"MK-51000  ");
        fields(
            &mut img,
            &[
                (0x10, b"SEGA ENTERPRISES"),
                (0x20, b"0000 GD-ROM1/1  "),
                (0x30, b"U       0799A10 "),
                (0x4A, b"V1.000"),
                (0x50, b"19990909"),
                (0x60, b"SEGA ENTERPRISES"),
                (0x80, b"SHENMUE"),
            ],
        );
        img
    }

    fn param_sfo(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut keys = Vec::new();
        let mut data = Vec::new();
        let mut index = Vec::new();
        for (k, v) in entries {
            let key_off = keys.len() as u16;
            keys.extend_from_slice(k.as_bytes());
            keys.push(0);
            let data_off = data.len() as u32;
            let mut value = v.as_bytes().to_vec();
            value.push(0);
            let len = value.len() as u32;
            let max = len.next_multiple_of(4);
            value.resize(max as usize, 0);
            data.extend(value);
            index.extend(key_off.to_le_bytes());
            index.extend(0x0204u16.to_le_bytes());
            index.extend(len.to_le_bytes());
            index.extend(max.to_le_bytes());
            index.extend(data_off.to_le_bytes());
        }
        let key_table = 0x14 + index.len();
        while keys.len() % 4 != 0 {
            keys.push(0);
        }
        let data_table = key_table + keys.len();
        let mut sfo = Vec::new();
        sfo.extend(b"\0PSF");
        sfo.extend(0x0101u32.to_le_bytes());
        sfo.extend((key_table as u32).to_le_bytes());
        sfo.extend((data_table as u32).to_le_bytes());
        sfo.extend((entries.len() as u32).to_le_bytes());
        sfo.extend(index);
        sfo.extend(keys);
        sfo.extend(data);
        sfo
    }

    // ---------- arquivos em disco e na memória ----------

    fn scratch() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn put(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    /// Abre do disco, contando cada byte lido.
    fn counting_file(count: Arc<AtomicU64>) -> impl Fn(&Path) -> io::Result<Box<dyn ReadSeek>> {
        move |p| {
            let f = std::fs::File::open(p)?;
            Ok(Box::new(Counting {
                inner: f,
                read: count.clone(),
            }))
        }
    }

    /// Serve arquivos da memória (pro teste de imagens quebradas).
    fn in_memory(
        files: HashMap<PathBuf, Vec<u8>>,
    ) -> impl Fn(&Path) -> io::Result<Box<dyn ReadSeek>> {
        move |p| {
            let bytes = files.get(p).cloned().ok_or(io::ErrorKind::NotFound)?;
            Ok(Box::new(Cursor::new(bytes)))
        }
    }

    // ---------- .chd montado no teste ----------

    fn cd_frame(sector: &[u8]) -> Vec<u8> {
        let mut f = sector.to_vec();
        f.resize(2448, 0);
        f
    }

    /// `.chd` de CD/GD-ROM: cada trilha é (texto de metadado, setores de
    /// 2352 ou 2048), completada até múltiplo de 4 frames como o `chdman`.
    fn cd_chd(path: &Path, tag: chd::metadata::KnownMetadata, tracks: &[(String, Vec<Vec<u8>>)]) {
        use chd::write::{ChdWriteOptions, ChdWriter};
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut w = ChdWriter::new(file, ChdWriteOptions::cd()).unwrap();
        for (text, _) in tracks {
            let text = std::ffi::CString::new(text.as_str()).unwrap();
            w.add_metadata(tag, text.as_c_str(), true).unwrap();
        }
        for (_, sectors) in tracks {
            for s in sectors {
                w.write_all(&cd_frame(s)).unwrap();
            }
            for _ in sectors.len()..sectors.len().next_multiple_of(4) {
                w.write_all(&[0u8; 2448]).unwrap();
            }
        }
        w.finish().unwrap();
    }

    /// `.chd` de DVD (`chdman createdvd`): setores de 2048 seguidos.
    fn dvd_chd(path: &Path, image: &[u8]) {
        use chd::write::{ChdWriteOptions, ChdWriter};
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut w = ChdWriter::new(file, ChdWriteOptions::dvd()).unwrap();
        w.write_all(image).unwrap();
        w.finish().unwrap();
    }

    fn sectors_of(image: &[u8], size: usize) -> Vec<Vec<u8>> {
        image.chunks(size).map(<[u8]>::to_vec).collect()
    }

    // ---------- testes do brief ----------

    #[test]
    fn psx_serial_from_system_cnf() {
        let dir = scratch();
        let img = iso(&[("SYSTEM.CNF", psx_cnf())]);
        let p = put(dir.path(), "Jogo.iso", &img);
        assert_eq!(read_serial(&p, "psx").as_deref(), Some("SLUS-00594"));

        // O mesmo disco em .bin cru MODE2/2352 atrás de um .cue.
        put(dir.path(), "Jogo (Track 1).bin", &to_raw(&img, 2));
        let cue = put(
            dir.path(),
            "Jogo.cue",
            b"FILE \"Jogo (Track 1).bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n",
        );
        assert_eq!(read_serial(&cue, "psx").as_deref(), Some("SLUS-00594"));
    }

    #[test]
    fn ps2_uses_boot2() {
        let dir = scratch();
        let cnf = b"BOOT2 = cdrom0:\\SLUS_200.62;1\nVER = 1.00\nVMODE = NTSC\n";
        let p = put(dir.path(), "Jogo.iso", &iso(&[("SYSTEM.CNF", cnf)]));
        assert_eq!(read_serial(&p, "ps2").as_deref(), Some("SLUS-20062"));
        assert_eq!(read_serial(&p, "psx"), None, "no PS1 a chave é BOOT");

        let both = b"BOOT = cdrom:\\SLPS_000.01;1\nBOOT2 = cdrom0:\\SLPM_650.01;1\n";
        let p = put(dir.path(), "Dois.iso", &iso(&[("SYSTEM.CNF", both)]));
        assert_eq!(read_serial(&p, "ps2").as_deref(), Some("SLPM-65001"));
        assert_eq!(read_serial(&p, "psx").as_deref(), Some("SLPS-00001"));
    }

    #[test]
    fn saturn_product_number() {
        let dir = scratch();
        let img = saturn_image();
        let p = put(dir.path(), "Jogo.iso", &img);
        assert_eq!(read_serial(&p, "saturn").as_deref(), Some("MK-81022"));

        put(dir.path(), "Jogo.bin", &to_raw(&img, 1));
        let cue = put(
            dir.path(),
            "Jogo.cue",
            b"FILE \"Jogo.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n",
        );
        assert_eq!(read_serial(&cue, "saturn").as_deref(), Some("MK-81022"));
        assert_eq!(
            read_serial(&p, "segacd"),
            None,
            "assinatura de outro sistema"
        );
    }

    #[test]
    fn segacd_product_number() {
        let dir = scratch();
        let img = segacd_image();
        let p = put(dir.path(), "Jogo.iso", &img);
        assert_eq!(read_serial(&p, "segacd").as_deref(), Some("MK-4407 -00"));
        // .bin cru aberto direto (sem .cue): o sync diz que é 2352.
        let raw = put(dir.path(), "Jogo.bin", &to_raw(&img, 1));
        assert_eq!(read_serial(&raw, "segacd").as_deref(), Some("MK-4407 -00"));
    }

    #[test]
    fn dreamcast_product_number() {
        let dir = scratch();
        // Trilha 1 (densidade simples) sem IP.BIN, 2 de áudio, 3 na área de
        // alta densidade com o IP.BIN no primeiro setor.
        put(dir.path(), "track01.bin", &to_raw(&iso(&[]), 1));
        put(dir.path(), "track02.raw", &vec![0u8; 2352 * 4]);
        put(dir.path(), "track03.bin", &to_raw(&dreamcast_ip_bin(), 1));
        let gdi = put(
            dir.path(),
            "Jogo.gdi",
            b"3\r\n1 0 4 2352 track01.bin 0\r\n2 450 0 2352 track02.raw 0\r\n3 45000 4 2352 track03.bin 0\r\n",
        );
        assert_eq!(read_serial(&gdi, "dreamcast").as_deref(), Some("MK-51000"));

        // Formato novo do Redump: .cue com as áreas marcadas.
        let cue = put(
            dir.path(),
            "Jogo.cue",
            b"REM SINGLE-DENSITY AREA\nFILE \"track01.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\nFILE \"track02.raw\" BINARY\n  TRACK 02 AUDIO\n    INDEX 01 00:00:00\nREM HIGH-DENSITY AREA\nFILE \"track03.bin\" BINARY\n  TRACK 03 MODE1/2352\n    INDEX 01 00:00:00\n",
        );
        assert_eq!(read_serial(&cue, "dreamcast").as_deref(), Some("MK-51000"));
    }

    #[test]
    fn unreadable_image_is_none() {
        let dir = scratch();
        // Lixo, vazio, truncado.
        let junk = put(dir.path(), "Lixo.iso", &[0xAB; 64 * 1024]);
        let empty = put(dir.path(), "Vazio.iso", &[]);
        let short = put(
            dir.path(),
            "Curto.iso",
            &iso(&[("SYSTEM.CNF", psx_cnf())])[..17 * SECTOR],
        );
        // Homebrew sem SYSTEM.CNF; SYSTEM.CNF sem serial (PSX.EXE).
        let homebrew = put(
            dir.path(),
            "Homebrew.iso",
            &iso(&[("PSX.EXE", b"PS-X EXE")]),
        );
        let exe = put(
            dir.path(),
            "Exe.iso",
            &iso(&[("SYSTEM.CNF", b"BOOT = cdrom:\\PSX.EXE;1\n")]),
        );
        // Formatos sem leitura de serial e sistemas fora da lista.
        let cso = put(dir.path(), "Jogo.cso", b"CISO");
        let pbp = put(dir.path(), "Jogo.pbp", b"\0PBP");
        let good = put(dir.path(), "Bom.iso", &iso(&[("SYSTEM.CNF", psx_cnf())]));
        let missing = dir.path().join("Sumiu.iso");
        // PARAM.SFO torto: magia errada, tabela apontando pra fora.
        let mut bad_sfo = param_sfo(&[("DISC_ID", "ULUS10041")]);
        bad_sfo[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        let psp_bad = put(
            dir.path(),
            "Psp.iso",
            &iso(&[("PSP_GAME/PARAM.SFO", &bad_sfo)]),
        );
        let psp_magic = put(
            dir.path(),
            "PspMagia.iso",
            &iso(&[("PSP_GAME/PARAM.SFO", b"XPSF\x01\x01\0\0")]),
        );
        // .cue apontando pra arquivo que não existe; .chd que não é chd.
        let cue = put(
            dir.path(),
            "Orfao.cue",
            b"FILE \"nada.bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\n",
        );
        let chd = put(dir.path(), "Falso.chd", b"MComprHD\0\0\0\x7c\0\0\0\x05");

        for sys in SERIAL_SYSTEMS {
            for p in [
                &junk, &empty, &short, &homebrew, &exe, &cso, &pbp, &missing, &cue, &chd,
            ] {
                assert_eq!(read_serial(p, sys), None, "{sys} {}", p.display());
            }
        }
        assert_eq!(read_serial(&psp_bad, "psp"), None);
        assert_eq!(read_serial(&psp_magic, "psp"), None);
        for other in ["nes", "disc", "n64", "3do", ""] {
            assert_eq!(read_serial(&good, other), None, "{other}");
        }
        assert_eq!(read_serial(&good, "gamecube"), None);
    }

    #[test]
    fn gamecube_iso_gcm_and_rvz_serials() {
        let dir = scratch();

        let mut gc_iso = vec![0u8; 4096];
        gc_iso[0..6].copy_from_slice(b"GALE01");
        gc_iso[0x1C..0x20].copy_from_slice(&0xC233_9F3Du32.to_be_bytes());
        let iso_path = put(dir.path(), "Smash.iso", &gc_iso);
        let gcm_path = put(dir.path(), "Smash.gcm", &gc_iso);

        let count = Arc::new(AtomicU64::new(0));
        let opener = counting_file(count.clone());
        assert_eq!(
            read_serial_with(&iso_path, "gamecube", &opener).as_deref(),
            Some("GALE01")
        );
        assert!(
            count.load(Ordering::Relaxed) <= crate::disc_sniff::RVZ_HEADER_MIN as u64,
            "leu mais que o cabecalho minimo"
        );
        assert_eq!(
            read_serial(&gcm_path, "gamecube").as_deref(),
            Some("GALE01")
        );

        let mut rvz = vec![0u8; 512];
        rvz[0..4].copy_from_slice(b"RVZ\x01");
        rvz[0x48..0x4C].copy_from_slice(&1u32.to_be_bytes());
        rvz[0x58..0x5E].copy_from_slice(b"gm8e01");
        rvz[0x58 + 0x1C..0x58 + 0x20].copy_from_slice(&0xC233_9F3Du32.to_be_bytes());
        let rvz_path = put(dir.path(), "Metroid.rvz", &rvz);
        assert_eq!(
            read_serial(&rvz_path, "gamecube").as_deref(),
            Some("GM8E01")
        );

        // Container RVZ de Wii (disc_type = 2) nao vira serial de GameCube.
        let mut wii_rvz = vec![0u8; 512];
        wii_rvz[0..4].copy_from_slice(b"RVZ\x01");
        wii_rvz[0x48..0x4C].copy_from_slice(&2u32.to_be_bytes());
        wii_rvz[0x58..0x5E].copy_from_slice(b"RMGE01");
        wii_rvz[0x58 + 0x18..0x58 + 0x1C].copy_from_slice(&0x5D1C_9EA3u32.to_be_bytes());
        let wii_path = put(dir.path(), "Galaxy.rvz", &wii_rvz);
        assert_eq!(read_serial(&wii_path, "gamecube"), None);
    }

    /// Imagem de 4 MiB cujo diretório raiz diz ter 64 MiB e não tem
    /// `SYSTEM.CNF`: sem o limite, a busca leria o arquivo até o fim.
    fn endless_root(size: usize) -> Vec<u8> {
        let mut img = iso(&[]);
        img.resize(size, 0);
        let pvd = &mut img[16 * SECTOR..17 * SECTOR];
        pvd[156 + 10..156 + 14].copy_from_slice(&(64u32 << 20).to_le_bytes());
        img
    }

    #[test]
    fn reads_at_most_two_mib() {
        let dir = scratch();
        let img = endless_root(4 << 20);

        // .iso de 2048: cada byte que sai do arquivo passa pelo contador.
        let iso_path = put(dir.path(), "Grande.iso", &img);
        let count = Arc::new(AtomicU64::new(0));
        let opener = counting_file(count.clone());
        assert_eq!(read_serial_with(&iso_path, "psx", &opener), None);
        let read = count.load(Ordering::Relaxed);
        assert!(read > 1 << 20, "leu de verdade ({read})");
        assert!(read <= READ_BUDGET, "leu {read} bytes de um .iso");

        // .bin cru atrás de .cue: conta os 2352 de cada setor.
        put(dir.path(), "Grande.bin", &to_raw(&img, 2));
        let cue = put(
            dir.path(),
            "Grande.cue",
            b"FILE \"Grande.bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\n",
        );
        let count = Arc::new(AtomicU64::new(0));
        let opener = counting_file(count.clone());
        assert_eq!(read_serial_with(&cue, "ps2", &opener), None);
        let read = count.load(Ordering::Relaxed);
        let cue_len = std::fs::metadata(&cue).unwrap().len();
        assert!(read <= READ_BUDGET + cue_len, "leu {read} bytes de um .bin");

        // .chd: conta o que sai do descompressor, à parte do limite.
        let chd_path = dir.path().join("Grande.chd");
        dvd_chd(&chd_path, &img);
        let mut reader = SectorReader::open(&chd_path, Area::FirstData, &open_file).unwrap();
        assert_eq!(serial_from(&mut reader, "psx"), None);
        let out = reader.decompressed();
        assert!(out > 1 << 20, "leu de verdade ({out})");
        assert!(out <= READ_BUDGET, "descomprimiu {out} bytes de um .chd");
    }

    // ---------- além do brief ----------

    #[test]
    fn psp_disc_id_from_param_sfo() {
        let dir = scratch();
        let sfo = param_sfo(&[
            ("CATEGORY", "UG"),
            ("DISC_ID", "ULUS10041"),
            ("TITLE", "Jogo"),
        ]);
        let p = put(
            dir.path(),
            "Jogo.iso",
            &iso(&[("PSP_GAME/PARAM.SFO", &sfo)]),
        );
        assert_eq!(read_serial(&p, "psp").as_deref(), Some("ULUS-10041"));
        // Sem DISC_ID: nada.
        let sfo = param_sfo(&[("TITLE", "Jogo")]);
        let p = put(
            dir.path(),
            "SemId.iso",
            &iso(&[("PSP_GAME/PARAM.SFO", &sfo)]),
        );
        assert_eq!(read_serial(&p, "psp"), None);
    }

    /// O número de produto só vale com a assinatura do sistema: o mesmo
    /// campo, com a assinatura errada ou de outro sistema, não é serial.
    #[test]
    fn sega_fields_need_their_signature() {
        /// Sistema, assinatura, offset do campo, campo, serial esperado.
        type Case = (
            &'static str,
            &'static [u8],
            usize,
            &'static [u8],
            &'static str,
        );
        let dir = scratch();
        let cases: [Case; 3] = [
            (
                "saturn",
                b"SEGA SEGASATURN ",
                0x20,
                b"MK-81022  ",
                "MK-81022",
            ),
            (
                "segacd",
                b"SEGADISCSYSTEM  ",
                0x180,
                b"GM MK-4407 -00",
                "MK-4407 -00",
            ),
            (
                "dreamcast",
                b"SEGA SEGAKATANA ",
                0x40,
                b"MK-51000  ",
                "MK-51000",
            ),
        ];
        for (sys, magic, at, field, want) in cases {
            let good = put(
                dir.path(),
                &format!("{sys}.iso"),
                &sega_header(magic, at, field),
            );
            assert_eq!(read_serial(&good, sys).as_deref(), Some(want), "{sys}");
            // Uma letra trocada na assinatura.
            let mut typo = magic.to_vec();
            typo[5] ^= 0x20;
            let typo = put(
                dir.path(),
                &format!("{sys}-typo.iso"),
                &sega_header(&typo, at, field),
            );
            assert_eq!(
                read_serial(&typo, sys),
                None,
                "{sys} com a assinatura trocada"
            );
            // Sem assinatura nenhuma.
            let blank = put(
                dir.path(),
                &format!("{sys}-zero.iso"),
                &sega_header(&[0; 16], at, field),
            );
            assert_eq!(read_serial(&blank, sys), None, "{sys} sem assinatura");
        }

        // Disco de um sistema lido como outro: os cabeçalhos têm texto nos
        // offsets do outro, e nada disso pode virar serial.
        let images = [
            ("saturn", saturn_image()),
            ("segacd", segacd_image()),
            ("dreamcast", dreamcast_ip_bin()),
        ];
        for (own, img) in &images {
            let p = put(dir.path(), &format!("{own}-real.iso"), img);
            for sys in ["saturn", "segacd", "dreamcast"] {
                if sys != *own {
                    assert_eq!(read_serial(&p, sys), None, "{own} lido como {sys}");
                }
            }
        }
    }

    /// `FILE` de `.cue`/`.gdi` que é pasta ou não existe: nada é aberto.
    #[test]
    fn cue_and_gdi_naming_a_folder_or_nothing_is_none() {
        let dir = scratch();
        let img = saturn_image();
        put(dir.path(), "Jogo.bin", &to_raw(&img, 1));
        std::fs::create_dir(dir.path().join("Pasta.bin")).unwrap();
        let cue = |name: &str, file: &str| {
            let text =
                format!("FILE \"{file}\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n");
            put(dir.path(), name, text.as_bytes())
        };
        assert_eq!(
            read_serial(&cue("Bom.cue", "Jogo.bin"), "saturn").as_deref(),
            Some("MK-81022")
        );
        assert_eq!(read_serial(&cue("Pasta.cue", "Pasta.bin"), "saturn"), None);
        assert_eq!(read_serial(&cue("Sumiu.cue", "Nada.bin"), "saturn"), None);

        let gdi = |name: &str, file: &str| {
            put(
                dir.path(),
                name,
                format!("1\n1 45000 4 2352 \"{file}\" 0\n").as_bytes(),
            )
        };
        assert_eq!(
            read_serial(&gdi("Pasta.gdi", "Pasta.bin"), "dreamcast"),
            None
        );
        assert_eq!(
            read_serial(&gdi("Sumiu.gdi", "Nada.bin"), "dreamcast"),
            None
        );
        // A própria imagem sendo uma pasta.
        std::fs::create_dir(dir.path().join("Disco.iso")).unwrap();
        assert_eq!(read_serial(&dir.path().join("Disco.iso"), "psx"), None);
    }

    /// `FILE` de `.cue`/`.gdi` absoluto, com unidade, de rede ou com `..`:
    /// nada fora da pasta é pedido. Fora da pasta o "mundo" tem um disco de
    /// Saturn válido, então um caminho que escapasse daria serial.
    #[test]
    fn cue_and_gdi_never_open_outside_their_folder() {
        use crate::disc_sniff::SHEET_ESCAPES;
        use std::cell::RefCell;
        let lib = Path::new("lib");
        let world = to_raw(&saturn_image(), 1);
        for name in SHEET_ESCAPES {
            for (sheet, system, text) in [
                (
                    lib.join("Jogo.cue"),
                    "saturn",
                    format!("FILE \"{name}\" BINARY\n TRACK 01 MODE1/2352\n  INDEX 01 00:00:00\n"),
                ),
                (
                    lib.join("Jogo.gdi"),
                    "dreamcast",
                    format!("1\n1 45000 4 2352 \"{name}\" 0\n"),
                ),
            ] {
                let opened = RefCell::new(Vec::<PathBuf>::new());
                let open = |p: &Path| -> io::Result<Box<dyn ReadSeek>> {
                    opened.borrow_mut().push(p.to_path_buf());
                    if p == sheet {
                        Ok(Box::new(Cursor::new(text.clone().into_bytes())))
                    } else if !p.starts_with(lib) {
                        Ok(Box::new(Cursor::new(world.clone())))
                    } else {
                        Err(io::ErrorKind::NotFound.into())
                    }
                };
                assert_eq!(read_serial_with(&sheet, system, &open), None, "{name}");
                for p in opened.borrow().iter() {
                    assert!(p.starts_with(lib), "{name} abriu {p:?}");
                    assert!(
                        !p.components()
                            .any(|c| matches!(c, std::path::Component::ParentDir)),
                        "{name} abriu {p:?}"
                    );
                }
            }
        }
        // Controle: o "mundo" responde de verdade se o caminho chegar nele.
        let open = |_: &Path| -> io::Result<Box<dyn ReadSeek>> {
            Ok(Box::new(Cursor::new(world.clone())))
        };
        assert_eq!(
            read_serial_with(Path::new("fora/Jogo.bin"), "saturn", &open).as_deref(),
            Some("MK-81022")
        );
    }

    #[test]
    fn serial_normalisation() {
        assert_eq!(normalize_sony("SLUS_005.94").as_deref(), Some("SLUS-00594"));
        assert_eq!(normalize_sony("slus_005.94").as_deref(), Some("SLUS-00594"));
        assert_eq!(normalize_sony("SCES-12345").as_deref(), Some("SCES-12345"));
        assert_eq!(normalize_sony("ULJM05000").as_deref(), Some("ULJM-05000"));
        assert_eq!(normalize_sony("PSX.EXE"), None);
        assert_eq!(normalize_sony("MAIN"), None);
        assert_eq!(normalize_sony("12345"), None);
        assert_eq!(normalize_sony(""), None);
    }

    #[test]
    fn boot_line_variants() {
        let dir = scratch();
        for (cnf, want) in [
            (&b"BOOT=cdrom:SCUS_944.55;1\n"[..], "SCUS-94455"),
            (b"boot = cdrom:\\\\SLES_012.34;1\n", "SLES-01234"),
            (b"BOOT = cdrom:\\DATA\\SLPS_123.45;1\n", "SLPS-12345"),
            (b"BOOT\t=\tcdrom:\\SLUS_009.99\n", "SLUS-00999"),
        ] {
            let p = put(dir.path(), "Jogo.iso", &iso(&[("SYSTEM.CNF", cnf)]));
            assert_eq!(read_serial(&p, "psx").as_deref(), Some(want));
        }
    }

    #[test]
    fn chd_images() {
        use chd::metadata::KnownMetadata;
        let dir = scratch();

        // PS1 em CD: uma trilha MODE2_RAW.
        let img = iso(&[("SYSTEM.CNF", psx_cnf())]);
        let raw = to_raw(&img, 2);
        let frames = raw.len() / 2352;
        let psx = dir.path().join("Psx.chd");
        cd_chd(
            &psx,
            KnownMetadata::CdRomTrack2,
            &[(
                format!("TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE FRAMES:{frames} PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0"),
                sectors_of(&raw, 2352),
            )],
        );
        assert_eq!(read_serial(&psx, "psx").as_deref(), Some("SLUS-00594"));

        // Dreamcast em GD-ROM: trilha 1 de 5 frames (vira 8 no .chd), 2 de
        // áudio, 3 com 150 frames de pregap gravados antes do IP.BIN.
        let mut t3 = vec![vec![0u8; 2352]; 150];
        t3.extend(sectors_of(&to_raw(&dreamcast_ip_bin(), 1), 2352));
        let dc = dir.path().join("Dc.chd");
        cd_chd(
            &dc,
            KnownMetadata::GdRomTrack,
            &[
                (
                    "TRACK:1 TYPE:MODE1_RAW SUBTYPE:NONE FRAMES:5 PAD:0 PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0".into(),
                    sectors_of(&to_raw(&vec![0u8; 5 * SECTOR], 1), 2352),
                ),
                (
                    "TRACK:2 TYPE:AUDIO SUBTYPE:NONE FRAMES:6 PAD:0 PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0".into(),
                    vec![vec![0u8; 2352]; 6],
                ),
                (
                    format!("TRACK:3 TYPE:MODE1_RAW SUBTYPE:NONE FRAMES:{} PAD:0 PREGAP:150 PGTYPE:VMODE1 PGSUB:RW POSTGAP:0", t3.len()),
                    t3,
                ),
            ],
        );
        assert_eq!(read_serial(&dc, "dreamcast").as_deref(), Some("MK-51000"));

        // PS2 em DVD (createdvd): sem metadado de CD, setores de 2048.
        let ps2 = dir.path().join("Ps2.chd");
        dvd_chd(
            &ps2,
            &iso(&[("SYSTEM.CNF", b"BOOT2 = cdrom0:\\SLES_523.72;1\n")]),
        );
        assert_eq!(read_serial(&ps2, "ps2").as_deref(), Some("SLES-52372"));
    }

    /// Xorshift pra "fuzz" determinístico.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Imagens truncadas, com bytes trocados e lixo puro, em todo formato e
    /// todo sistema: nada entra em pânico (e nada lê além do limite).
    #[test]
    fn broken_images_never_panic() {
        let dir = scratch();
        let psx = iso(&[("SYSTEM.CNF", psx_cnf())]);
        let psp = iso(&[(
            "PSP_GAME/PARAM.SFO",
            &param_sfo(&[("DISC_ID", "ULUS10041")])[..],
        )]);
        let chd_path = dir.path().join("x.chd");
        dvd_chd(&chd_path, &psx);
        let chd_bytes = std::fs::read(&chd_path).unwrap();
        let cd_chd_path = dir.path().join("cd.chd");
        let raw = to_raw(&psx, 2);
        cd_chd(
            &cd_chd_path,
            chd::metadata::KnownMetadata::CdRomTrack2,
            &[(
                format!("TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE FRAMES:{} PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0", raw.len() / 2352),
                sectors_of(&raw, 2352),
            )],
        );
        let cd_chd_bytes = std::fs::read(&cd_chd_path).unwrap();
        let seeds: Vec<(&str, Vec<u8>)> = vec![
            ("iso", psx.clone()),
            ("iso", psp),
            ("iso", saturn_image()),
            ("iso", segacd_image()),
            ("iso", dreamcast_ip_bin()),
            ("bin", to_raw(&psx, 2)),
            ("chd", chd_bytes),
            ("chd", cd_chd_bytes),
        ];
        let cue = b"FILE \"t.bin\" BINARY\n TRACK 01 MODE2/2352\n  INDEX 01 00:00:00\n".to_vec();
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

        let check = |ext: &str, bytes: Vec<u8>| {
            let path = PathBuf::from(format!("img.{ext}"));
            let mut files = HashMap::new();
            files.insert(path.clone(), bytes.clone());
            files.insert(PathBuf::from("t.bin"), bytes);
            files.insert(PathBuf::from("img.cue"), cue.clone());
            let open = in_memory(files);
            for sys in SERIAL_SYSTEMS {
                let _ = read_serial_with(&path, sys, &open);
                let _ = read_serial_with(Path::new("img.cue"), sys, &open);
                if let Some(mut r) = SectorReader::open(&path, Area::FirstData, &open) {
                    let _ = serial_from(&mut r, sys);
                    assert!(r.used() <= READ_BUDGET);
                }
            }
        };

        for (ext, seed) in &seeds {
            // Truncada em pontos que cortam estruturas no meio.
            for cut in [
                0,
                1,
                11,
                12,
                15,
                16,
                100,
                2047,
                2048,
                2049,
                16 * SECTOR + 157,
                18 * SECTOR + 40,
            ] {
                if cut < seed.len() {
                    check(ext, seed[..cut].to_vec());
                }
            }
            for _ in 0..12 {
                let cut = rng.below(seed.len());
                check(ext, seed[..cut].to_vec());
            }
            // Bytes trocados ao acaso (perto das estruturas e em qualquer lugar).
            for round in 0..60 {
                let mut b = seed.clone();
                let hot = b.len().min(if *ext == "chd" { 4096 } else { 20 * SECTOR });
                for _ in 0..1 + round % 8 {
                    let at = if round % 2 == 0 {
                        rng.below(hot)
                    } else {
                        rng.below(b.len())
                    };
                    b[at] = rng.next() as u8;
                }
                check(ext, b);
            }
        }
        // Lixo puro, com e sem as assinaturas no começo.
        for (ext, magic) in [
            ("iso", &b""[..]),
            ("iso", b"SEGA SEGASATURN "),
            ("iso", b"SEGADISCSYSTEM  "),
            ("bin", b"\x00\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\x00"),
            ("chd", b"MComprHD\0\0\0\x7c\0\0\0\x05"),
        ] {
            for len in [0usize, 13, 2048, 40 * 1024] {
                let mut b: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
                let m = magic.len().min(b.len());
                b[..m].copy_from_slice(&magic[..m]);
                check(ext, b);
            }
        }
    }

    /// Diagnóstico com imagem de verdade: `REEMU_SERIAL_IMAGE=<caminho>` e,
    /// opcional, `REEMU_SERIAL_SYSTEM=<psx|ps2|psp|saturn|segacd|dreamcast>`
    /// (sem ele, tenta todos). Imprime o serial, os bytes lidos e o tempo.
    #[test]
    #[ignore = "diagnóstico manual (REEMU_SERIAL_IMAGE)"]
    fn real_image_serial() {
        let Ok(image) = std::env::var("REEMU_SERIAL_IMAGE") else {
            return;
        };
        let path = PathBuf::from(image);
        let systems: Vec<String> = match std::env::var("REEMU_SERIAL_SYSTEM") {
            Ok(s) => vec![s],
            Err(_) => SERIAL_SYSTEMS.iter().map(|s| s.to_string()).collect(),
        };
        for sys in systems {
            let count = Arc::new(AtomicU64::new(0));
            let opener = counting_file(count.clone());
            let started = std::time::Instant::now();
            let serial = read_serial_with(&path, &sys, &opener);
            eprintln!(
                "{sys}: {serial:?} ({} bytes do arquivo, {:?})",
                count.load(Ordering::Relaxed),
                started.elapsed()
            );
        }
    }

    #[test]
    fn layout_from_cue_modes() {
        assert_eq!(Layout::from_mode("AUDIO"), None);
        assert_eq!(Layout::from_mode("MODE1/2048"), Some(Layout::Cooked));
    }
}
