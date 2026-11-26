//! Identifica o sistema de uma imagem de disco (`.iso`, `.img`, `.cue`…) pelo
//! **conteúdo** — usado quando o nome da pasta não desambigua (biblioteca não
//! organizada por sistema). Lê só o cabeçalho e procura assinaturas conhecidas.
//! `None` = não deu → o scan cai no balde genérico `"disc"`.

use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const HEADER: usize = 256 * 1024;

/// `system_id` do ReEmu pela assinatura no cabeçalho de `path` (ou `None`).
pub fn sniff_disc_system(path: &Path) -> Option<&'static str> {
    let mut buf = vec![0u8; HEADER];
    let n = File::open(path).ok()?.read(&mut buf).ok()?;
    if n == 0 {
        return None;
    }
    sniff_bytes(&buf[..n])
}

fn has(hay: &[u8], needle: &[u8]) -> bool {
    needle.len() <= hay.len() && hay.windows(needle.len()).any(|w| w == needle)
}

/// `.cue` aponta pro `.bin`/`.img` da trilha 1 — abre esse e fareja nele.
fn sniff_cue(path: &Path) -> Option<&'static str> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(CUE_MAX_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    let first = parse_cue(&decode_sheet(&bytes)).into_iter().next()?;
    let track = sibling(path, &first.file)?;
    // Só arquivo comum: `FILE "CON"` viraria dispositivo no Windows.
    if !track.is_file() {
        return None;
    }
    sniff_disc_system(&track)
}

/// Arquivo citado num `.cue`/`.gdi`, na pasta dele. Só vale nome relativo
/// que não sai da pasta: caminho absoluto, com unidade (`C:\x`, `D:x`), de
/// rede (`\\servidor\x`, que no Windows abre conexão SMB) ou com `..` dá
/// `None`, e nada fora da pasta é tocado.
pub(crate) fn sibling(sheet: &Path, name: &str) -> Option<PathBuf> {
    let rel = Path::new(name);
    if rel
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    Some(sheet.parent()?.join(rel))
}

/// Nomes de `FILE` que sairiam da pasta do cue: absoluto, com unidade, de
/// rede, `..`. No Windows o [`sibling`] recusa todos; fora dele, `\` e `C:`
/// não são separador nem unidade, e o nome vira um arquivo de nome esquisito
/// dentro da pasta. Em nenhum caso o caminho sai dela.
#[cfg(test)]
pub(crate) const SHEET_ESCAPES: &[&str] = &[
    "/fora/Jogo.bin",
    r"C:\Fora\Jogo.bin",
    "D:Jogo.bin",
    r"\\servidor\share\Jogo.bin",
    r"\\?\C:\Jogo.bin",
    "../Jogo.bin",
    r"..\Jogo.bin",
    "sub/../../Jogo.bin",
];

/// Um `.cue` de verdade tem poucos KiB; acima disto não é cue.
pub(crate) const CUE_MAX_BYTES: u64 = 256 * 1024;

/// Texto de um `.cue`/`.gdi` como veio do disco. Sem o BOM do UTF-8 (o Bloco
/// de Notas antigo grava um, e com ele a 1ª linha não é `FILE`), e em
/// Windows-1252 quando não é UTF-8 válido: ferramentas antigas gravam o nome
/// da trilha na página de código do Windows (`Pokémon.bin` com o `é` em
/// 0xE9), e decodificar com perda trocaria a letra e a trilha não se acharia.
pub(crate) fn decode_sheet(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

/// Um byte em Windows-1252: o mesmo do Latin-1, menos 0x80 a 0x9F. As cinco
/// posições que não têm caractere ficam com o código de controle de mesmo
/// número, como no WHATWG Encoding Standard.
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}',
        '\u{017D}', '\u{008F}', '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
    ];
    match b {
        0x80..=0x9F => HIGH[usize::from(b - 0x80)],
        _ => char::from(b),
    }
}

/// Uma trilha de um `.cue`, na ordem em que aparece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CueTrack {
    /// Nome do arquivo da linha `FILE` que vale pra esta trilha, como está no cue.
    pub file: String,
    /// Modo da linha `TRACK` (`MODE1/2352`, `MODE2/2352`, `AUDIO`...), em maiúsculas.
    pub mode: String,
    /// `INDEX 00` e `INDEX 01`, em frames de 1/75 s desde o início do arquivo.
    pub index0: Option<u32>,
    pub index1: Option<u32>,
    /// É a primeira trilha do seu `FILE` (nada de outra trilha antes dela no arquivo).
    pub first_in_file: bool,
    /// Veio depois de `REM HIGH-DENSITY AREA` (Dreamcast no formato do Redump).
    pub high_density: bool,
}

/// Lê as trilhas de um `.cue`. Linhas que não entende ficam de fora; um cue
/// sem `FILE` antes da `TRACK` não dá trilha.
pub(crate) fn parse_cue(text: &str) -> Vec<CueTrack> {
    let mut tracks: Vec<CueTrack> = Vec::new();
    let mut file: Option<String> = None;
    let mut new_file = false;
    let mut high_density = false;
    for line in text.lines() {
        let line = line.trim();
        let mut words = line.split_whitespace();
        let Some(cmd) = words.next() else {
            continue;
        };
        match cmd.to_ascii_uppercase().as_str() {
            "FILE" => {
                file = cue_file_name(&line[cmd.len()..]);
                new_file = true;
            }
            "TRACK" => {
                let Some(name) = file.clone() else {
                    continue;
                };
                let mode = words.nth(1).unwrap_or_default().to_ascii_uppercase();
                tracks.push(CueTrack {
                    file: name,
                    mode,
                    index0: None,
                    index1: None,
                    first_in_file: new_file,
                    high_density,
                });
                new_file = false;
            }
            "INDEX" => {
                let (Some(n), Some(at)) = (words.next(), words.next()) else {
                    continue;
                };
                let (Some(track), Some(frames)) = (tracks.last_mut(), msf_frames(at)) else {
                    continue;
                };
                match n.parse::<u32>() {
                    Ok(0) => track.index0 = Some(frames),
                    Ok(1) => track.index1 = Some(frames),
                    _ => {}
                }
            }
            "REM" => {
                let rest = line[cmd.len()..].trim().to_ascii_uppercase();
                if rest.starts_with("HIGH-DENSITY AREA") {
                    high_density = true;
                } else if rest.starts_with("SINGLE-DENSITY AREA") {
                    high_density = false;
                }
            }
            _ => {}
        }
    }
    tracks
}

/// `FILE "trilha 1.bin" BINARY` → `trilha 1.bin`; sem aspas, tudo até o
/// último espaço (o tipo do arquivo).
fn cue_file_name(rest: &str) -> Option<String> {
    let rest = rest.trim();
    let name = if let Some(quoted) = rest.strip_prefix('"') {
        &quoted[..quoted.find('"')?]
    } else {
        rest.rsplit_once(char::is_whitespace)
            .map_or(rest, |(name, _)| name)
            .trim()
    };
    (!name.is_empty()).then(|| name.to_string())
}

/// `mm:ss:ff` → frames (75 por segundo).
fn msf_frames(at: &str) -> Option<u32> {
    let mut parts = at.split(':').map(|p| p.parse::<u32>().ok());
    let (m, s, f) = (parts.next()??, parts.next()??, parts.next()??);
    if parts.next().is_some() || s >= 60 || f >= 75 {
        return None;
    }
    m.checked_mul(60)?
        .checked_add(s)?
        .checked_mul(75)?
        .checked_add(f)
}

/// Assinatura de DVD do GameCube (`0xC2339F3D` em `0x1C..0x20`) e do Wii
/// (`0x5D1C9EA3` em `0x18..0x1C`). No formato `.rvz`/`.wia` (`WiaAndRvz.md`),
/// `wia_disc_t` começa em `0x48` com `u32 disc_type` big-endian (`1` = GameCube,
/// `2` = Wii) e traz os primeiros `0x80` bytes descomprimidos do disco em
/// `0x58..0xD8`.
const GC_MAGIC: [u8; 4] = [0xC2, 0x33, 0x9F, 0x3D];
const WII_MAGIC: [u8; 4] = [0x5D, 0x1C, 0x9E, 0xA3];
pub(crate) const RVZ_HEADER_MIN: usize = 0xD8;

pub(crate) fn is_rvz_or_wia(b: &[u8]) -> bool {
    b.starts_with(b"RVZ\x01") || b.starts_with(b"WIA\x01")
}

/// Devolve o cabeçalho do disco de GameCube (até `0x80` bytes a partir do Game
/// ID em `0x00..0x06`), tanto em `.iso`/`.gcm` quanto em `.rvz`/`.wia`.
pub(crate) fn gc_disc_header(b: &[u8]) -> Option<&[u8]> {
    if is_rvz_or_wia(b) {
        if b.len() < RVZ_HEADER_MIN {
            return None;
        }
        let disc_type = u32::from_be_bytes([b[0x48], b[0x49], b[0x4A], b[0x4B]]);
        let dhead = &b[0x58..RVZ_HEADER_MIN];
        if disc_type == 1 && dhead[0x18..0x1C] != WII_MAGIC && dhead[0x1C..0x20] == GC_MAGIC {
            return Some(dhead);
        }
        return None;
    }
    if b.len() >= 0x20 && b[0x18..0x1C] != WII_MAGIC && b[0x1C..0x20] == GC_MAGIC {
        return Some(&b[..b.len().min(0x80)]);
    }
    None
}

fn sniff_bytes(b: &[u8]) -> Option<&'static str> {
    // 3DO: 0x01 'ZZZZZ' no byte 0.
    if b.len() >= 7 && b[0] == 0x01 && &b[1..6] == b"ZZZZZ" {
        return Some("3do");
    }
    // RVZ / WIA: o cabeçalho já traz `disc_type` (1 = GameCube, 2 = Wii).
    // Um disco de Wii fica no balde genérico até a etapa do Wii entrar.
    if is_rvz_or_wia(b) {
        return gc_disc_header(b).map(|_| "gamecube");
    }
    if b.len() >= 0x1C && b[0x18..0x1C] == WII_MAGIC {
        return None;
    }
    if gc_disc_header(b).is_some() {
        return Some("gamecube");
    }
    // Sega — string no começo da trilha de dados (offset 0 pra 2048/ISO, +16
    // pra .bin 2352). Procuro nos primeiros 4 KiB pra cobrir os dois.
    let head = &b[..b.len().min(4096)];
    if has(head, b"SEGADISCSYSTEM") {
        return Some("segacd");
    }
    if has(head, b"SEGA SEGASATURN") {
        return Some("saturn");
    }
    if has(head, b"SEGA SEGAKATANA") {
        return Some("dreamcast");
    }
    if has(head, b"PC-FX:Hu_CD-ROM") || has(head, b"PC-FX:") {
        return Some("pcfx");
    }
    if has(head, b"PC Engine CD-ROM SYSTEM") {
        return Some("pcenginecd");
    }
    // Sony — a área de licença/`SYSTEM.CNF` costuma cair nesses 256 KiB.
    // PS3 primeiro: o ReEmu não tem core de PS3, mas o disco carrega marcas
    // "PLAYSTATION" que caíam no PS1 abaixo — e o jogo abria no core errado.
    // Sem sistema, fica no balde genérico em vez de mentir.
    if [
        &b"PS3_GAME"[..],
        b"PS3_DISC.SFB",
        b"PLAYSTATION3",
        b"PlayStation3",
        b"PS3VOLUME",
    ]
    .iter()
    .any(|sig| has(b, sig))
    {
        return None;
    }
    if has(b, b"PSP_GAME") || has(b, b"UMD_DATA.BIN") {
        return Some("psp");
    }
    if has(b, b"BOOT2") || has(b, b"cdrom0:") {
        return Some("ps2");
    }
    if has(b, b"PLAYSTATION") || has(b, b"Sony Computer Entertainment") || has(b, b"BOOT = cdrom:")
    {
        return Some("psx");
    }
    None
}

/// Ponto de entrada do scan: fareja, tratando `.cue` como ponteiro pro binário
/// e caindo na inspeção de setores/ISO9660 para `.chd`, `.gdi` e DVDs de PS2
/// cujo `SYSTEM.CNF` mora depois dos primeiros 256 KiB.
pub fn identify(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let initial = if ext == "cue" {
        sniff_cue(path)
    } else {
        sniff_disc_system(path)
    };
    initial.or_else(|| sniff_via_sectors(path))
}

pub(crate) fn sniff_header_bytes(b: &[u8]) -> Option<&'static str> {
    sniff_bytes(b)
}

fn sniff_via_sectors(path: &Path) -> Option<&'static str> {
    use crate::iso9660;
    use crate::sector::{open_file, Area, SectorReader};

    if let Some(mut hd) = SectorReader::open(path, Area::HighDensity, &open_file) {
        if let Some(s0) = hd.read_sector(0) {
            if s0.starts_with(b"SEGA SEGAKATANA ") {
                return Some("dreamcast");
            }
        }
    }

    let mut reader = SectorReader::open(path, Area::FirstData, &open_file)?;
    if let Some(s0) = reader.read_sector(0) {
        if s0.len() >= 7 && s0[0] == 0x01 && &s0[1..6] == b"ZZZZZ" {
            return Some("3do");
        }
        if s0.starts_with(b"SEGADISCSYSTEM") {
            return Some("segacd");
        }
        if s0.starts_with(b"SEGA SEGASATURN") {
            return Some("saturn");
        }
        if s0.starts_with(b"SEGA SEGAKATANA") {
            return Some("dreamcast");
        }
        if s0.starts_with(b"PC-FX:Hu_CD-ROM") || s0.starts_with(b"PC-FX:") {
            return Some("pcfx");
        }
    }
    if let Some(s1) = reader.read_sector(1) {
        if s1.get(32..55) == Some(b"PC Engine CD-ROM SYSTEM") {
            return Some("pcenginecd");
        }
    }
    if iso9660::lookup(&mut reader, &["PS3_GAME"]).is_some() {
        return None;
    }
    if iso9660::lookup(&mut reader, &["PSP_GAME", "PARAM.SFO"]).is_some() {
        return Some("psp");
    }
    if let Some(cnf) = iso9660::lookup(&mut reader, &["SYSTEM.CNF"]) {
        if let Some(bytes) = iso9660::read_file(&mut reader, cnf, 16 * 1024) {
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines() {
                if let Some((k, _)) = line.split_once('=') {
                    let key = k.trim();
                    if key.eq_ignore_ascii_case("BOOT2") {
                        return Some("ps2");
                    }
                    if key.eq_ignore_ascii_case("BOOT") {
                        return Some("psx");
                    }
                }
            }
        }
    }
    if iso9660::lookup(&mut reader, &["PSX.EXE"]).is_some() {
        return Some("psx");
    }
    if iso9660::lookup(&mut reader, &["IPL.TXT"]).is_some() {
        return Some("neogeocd");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_sega_signatures() {
        let mut b = vec![0u8; 2048];
        b[0..16].copy_from_slice(b"SEGA SEGASATURN ");
        assert_eq!(sniff_bytes(&b), Some("saturn"));
        b[0..16].copy_from_slice(b"SEGA SEGAKATANA ");
        assert_eq!(sniff_bytes(&b), Some("dreamcast"));
    }

    #[test]
    fn ps3_disc_is_not_ps1() {
        let mut b = vec![0u8; 4096];
        b[100..111].copy_from_slice(b"PLAYSTATION");
        assert_eq!(sniff_bytes(&b), Some("psx"));
        b[2000..2008].copy_from_slice(b"PS3_GAME");
        assert_eq!(sniff_bytes(&b), None);
    }

    #[test]
    fn detects_3do() {
        let mut b = vec![0u8; 64];
        b[0] = 0x01;
        b[1..6].copy_from_slice(b"ZZZZZ");
        assert_eq!(sniff_bytes(&b), Some("3do"));
    }

    #[test]
    fn detects_gamecube_iso_and_rvz_without_claiming_wii() {
        let mut gc_iso = vec![0u8; 0x80];
        gc_iso[0..6].copy_from_slice(b"GALE01");
        gc_iso[0x1C..0x20].copy_from_slice(&[0xC2, 0x33, 0x9F, 0x3D]);
        assert_eq!(sniff_bytes(&gc_iso), Some("gamecube"));

        let mut wii_iso = vec![0u8; 0x80];
        wii_iso[0..6].copy_from_slice(b"RSBE01");
        wii_iso[0x18..0x1C].copy_from_slice(&[0x5D, 0x1C, 0x9E, 0xA3]);
        assert_eq!(sniff_bytes(&wii_iso), None);

        let mut gc_rvz = vec![0u8; 0x100];
        gc_rvz[0..4].copy_from_slice(b"RVZ\x01");
        gc_rvz[0x48..0x4C].copy_from_slice(&1u32.to_be_bytes());
        gc_rvz[0x58..0x5E].copy_from_slice(b"GM8E01");
        gc_rvz[0x58 + 0x1C..0x58 + 0x20].copy_from_slice(&[0xC2, 0x33, 0x9F, 0x3D]);
        assert_eq!(sniff_bytes(&gc_rvz), Some("gamecube"));

        let mut wii_rvz = gc_rvz.clone();
        wii_rvz[0x48..0x4C].copy_from_slice(&2u32.to_be_bytes());
        assert_eq!(sniff_bytes(&wii_rvz), None);
    }

    #[test]
    fn ps2_before_ps1() {
        let mut b = vec![0u8; 4096];
        b[100..111].copy_from_slice(b"PLAYSTATION");
        b[500..505].copy_from_slice(b"BOOT2");
        assert_eq!(sniff_bytes(&b), Some("ps2"));
    }

    #[test]
    fn nothing_on_garbage() {
        assert_eq!(sniff_bytes(&[0xAB; 4096]), None);
    }

    #[test]
    fn cue_tracks_files_and_indexes() {
        let cue = "REM GENRE Game\r\nFILE \"Jogo (Track 1).bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\nFILE \"Jogo (Track 2).bin\" BINARY\r\n  TRACK 02 AUDIO\r\n    INDEX 00 00:00:00\r\n    INDEX 01 00:02:00\r\n";
        let t = parse_cue(cue);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].file, "Jogo (Track 1).bin");
        assert_eq!(t[0].mode, "MODE2/2352");
        assert_eq!((t[0].index0, t[0].index1), (None, Some(0)));
        assert!(t[0].first_in_file);
        assert_eq!(t[1].file, "Jogo (Track 2).bin");
        assert_eq!((t[1].index0, t[1].index1), (Some(0), Some(150)));

        // Arquivo único, nome sem aspas, trilha de áudio antes da de dados.
        let t = parse_cue("FILE jogo.bin BINARY\n TRACK 01 AUDIO\n  INDEX 01 00:00:00\n TRACK 02 MODE1/2352\n  INDEX 01 01:00:05\n");
        assert_eq!(t[1].file, "jogo.bin");
        assert_eq!(t[1].index1, Some(60 * 75 + 5));
        assert!(!t[1].first_in_file);

        // Lixo não vira trilha nem entra em pânico.
        assert!(parse_cue("TRACK 01 MODE1/2352\nINDEX 01 99:99:99\nFILE \"\nFILE\n").is_empty());
    }

    #[test]
    fn cue_naming_a_folder_or_nothing_is_not_sniffed() {
        let dir = tempfile::tempdir().unwrap();
        let mut saturn = vec![0u8; 4096];
        saturn[..16].copy_from_slice(b"SEGA SEGASATURN ");
        std::fs::write(dir.path().join("Jogo.bin"), &saturn).unwrap();
        std::fs::create_dir(dir.path().join("Pasta.bin")).unwrap();
        let cue = |name: &str, file: &str| {
            let p = dir.path().join(name);
            let text =
                format!("FILE \"{file}\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n");
            std::fs::write(&p, text).unwrap();
            p
        };
        assert_eq!(identify(&cue("Bom.cue", "Jogo.bin")), Some("saturn"));
        assert_eq!(identify(&cue("Pasta.cue", "Pasta.bin")), None);
        assert_eq!(identify(&cue("Sumiu.cue", "Nada.bin")), None);
    }

    #[test]
    fn sheet_text_without_bom_and_in_cp1252_when_not_utf8() {
        assert_eq!(decode_sheet(b"\xEF\xBB\xBFFILE x"), "FILE x");
        assert_eq!(decode_sheet("Pokémon".as_bytes()), "Pokémon", "UTF-8 fica");
        assert_eq!(
            decode_sheet(b"Pok\xE9mon \x80\x97\x81"),
            "Pok\u{E9}mon \u{20AC}\u{2014}\u{81}"
        );
    }

    /// Cue com BOM e cue em Windows-1252: o sistema sai do disco do mesmo jeito.
    #[test]
    fn cue_with_bom_or_cp1252_is_sniffed() {
        let dir = tempfile::tempdir().unwrap();
        let mut saturn = vec![0u8; 4096];
        saturn[..16].copy_from_slice(b"SEGA SEGASATURN ");
        std::fs::write(dir.path().join("Pokémon.bin"), &saturn).unwrap();
        let body =
            b"FILE \"Pok\xE9mon.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n";
        let cp1252 = dir.path().join("Antigo.cue");
        std::fs::write(&cp1252, body).unwrap();
        assert_eq!(identify(&cp1252), Some("saturn"));
        let mut bom = b"\xEF\xBB\xBF".to_vec();
        bom.extend_from_slice(
            "FILE \"Pokémon.bin\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n"
                .as_bytes(),
        );
        let with_bom = dir.path().join("Bom.cue");
        std::fs::write(&with_bom, bom).unwrap();
        assert_eq!(identify(&with_bom), Some("saturn"));
    }

    #[test]
    fn sibling_never_leaves_the_folder() {
        let sheet = Path::new("lib").join("Jogo.cue");
        assert_eq!(
            sibling(&sheet, "Jogo.bin"),
            Some(Path::new("lib").join("Jogo.bin"))
        );
        assert_eq!(
            sibling(&sheet, "sub/Jogo.bin"),
            Some(Path::new("lib").join("sub/Jogo.bin"))
        );
        let absolute = std::env::temp_dir().join("Jogo.bin");
        assert_eq!(sibling(&sheet, absolute.to_str().unwrap()), None);
        for name in SHEET_ESCAPES {
            match sibling(&sheet, name) {
                None => {}
                Some(p) => {
                    if cfg!(windows) {
                        panic!("{name} não é recusado no Windows: {p:?}");
                    }
                    assert!(p.starts_with("lib"), "{name} -> {p:?}");
                    assert!(
                        !p.components().any(|c| matches!(c, Component::ParentDir)),
                        "{name} -> {p:?}"
                    );
                }
            }
        }
    }

    /// Cue que aponta pra um disco de Saturn de verdade fora da pasta (por
    /// caminho absoluto ou com `..`): não é farejado, e o disco não é lido.
    #[test]
    fn cue_pointing_outside_its_folder_is_not_sniffed() {
        let root = tempfile::tempdir().unwrap();
        let lib = root.path().join("lib");
        let fora = root.path().join("fora");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::create_dir_all(&fora).unwrap();
        let mut saturn = vec![0u8; 4096];
        saturn[..16].copy_from_slice(b"SEGA SEGASATURN ");
        std::fs::write(fora.join("Jogo.bin"), &saturn).unwrap();
        std::fs::write(lib.join("Jogo.bin"), &saturn).unwrap();
        let cue = |name: &str, file: &str| {
            let p = lib.join(name);
            let text =
                format!("FILE \"{file}\" BINARY\n  TRACK 01 MODE1/2352\n    INDEX 01 00:00:00\n");
            std::fs::write(&p, text).unwrap();
            p
        };
        // Controle: o mesmo disco dentro da pasta é farejado.
        assert_eq!(identify(&cue("Dentro.cue", "Jogo.bin")), Some("saturn"));
        let absolute = fora.join("Jogo.bin");
        assert_eq!(
            identify(&cue("Absoluto.cue", absolute.to_str().unwrap())),
            None
        );
        assert_eq!(identify(&cue("Sobe.cue", "../fora/Jogo.bin")), None);
        assert_eq!(identify(&cue("SobeWin.cue", r"..\fora\Jogo.bin")), None);
    }

    fn minimal_iso9660_with_file_at(name: &str, file_lba: u32, payload: &[u8]) -> Vec<u8> {
        const SECTOR: usize = 2048;
        let total_sectors = (file_lba as usize) + payload.len().max(1).div_ceil(SECTOR) + 2;
        let mut img = vec![0u8; total_sectors * SECTOR];

        fn dir_rec(ident: &[u8], lba: u32, size: u32, is_dir: bool) -> Vec<u8> {
            let len = (33 + ident.len()) + ((33 + ident.len()) % 2);
            let mut rec = vec![0u8; len];
            rec[0] = len as u8;
            rec[2..6].copy_from_slice(&lba.to_le_bytes());
            rec[6..10].copy_from_slice(&lba.to_be_bytes());
            rec[10..14].copy_from_slice(&size.to_le_bytes());
            rec[14..18].copy_from_slice(&size.to_be_bytes());
            rec[25] = if is_dir { 0x02 } else { 0x00 };
            rec[28..30].copy_from_slice(&1u16.to_le_bytes());
            rec[30..32].copy_from_slice(&1u16.to_be_bytes());
            rec[32] = ident.len() as u8;
            rec[33..33 + ident.len()].copy_from_slice(ident);
            rec
        }

        let file_off = (file_lba as usize) * SECTOR;
        img[file_off..file_off + payload.len()].copy_from_slice(payload);

        let mut root = dir_rec(&[0], 18, SECTOR as u32, true);
        root.extend(dir_rec(&[1], 18, SECTOR as u32, true));
        root.extend(dir_rec(
            format!("{name};1").as_bytes(),
            file_lba,
            payload.len() as u32,
            false,
        ));
        img[18 * SECTOR..18 * SECTOR + root.len()].copy_from_slice(&root);

        let pvd = &mut img[16 * SECTOR..17 * SECTOR];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[6] = 1;
        pvd[128..130].copy_from_slice(&(SECTOR as u16).to_le_bytes());
        pvd[156..156 + 34].copy_from_slice(&dir_rec(&[0], 18, SECTOR as u32, true));

        let term = &mut img[17 * SECTOR..18 * SECTOR];
        term[0] = 255;
        term[1..6].copy_from_slice(b"CD001");
        img
    }

    #[test]
    fn identifies_ps2_dvd_whose_system_cnf_lives_past_256_kib_and_in_chd() {
        let dir = tempfile::tempdir().unwrap();
        let cnf = b"BOOT2 = cdrom0:\\SLUS_202.28;1\r\nVER = 1.10\r\nVMODE = NTSC\r\n";
        // LBA 200 = 409_600 bytes (> 256 KiB lidos por sniff_disc_system).
        let iso_bytes = minimal_iso9660_with_file_at("SYSTEM.CNF", 200, cnf);
        let iso_path = dir.path().join("Silent Hill 2.iso");
        std::fs::write(&iso_path, &iso_bytes).unwrap();
        assert_eq!(sniff_disc_system(&iso_path), None);
        assert_eq!(identify(&iso_path), Some("ps2"));

        // O mesmo volume empacotado num CHD v5 nao comprimido (DVD, unidade = 2048).
        let hunk_size = 16_384u32;
        let logical_bytes =
            (iso_bytes.len() as u64).div_ceil(u64::from(hunk_size)) * u64::from(hunk_size);
        let hunk_count = (logical_bytes / u64::from(hunk_size)) as usize;
        let header_size = 124usize;
        let map_offset = header_size as u64;
        let data_offset = (map_offset + (hunk_count * 4) as u64).div_ceil(u64::from(hunk_size))
            * u64::from(hunk_size);
        let mut chd = vec![0u8; (data_offset + logical_bytes) as usize];
        chd[0..8].copy_from_slice(b"MComprHD");
        chd[8..12].copy_from_slice(&(header_size as u32).to_be_bytes());
        chd[12..16].copy_from_slice(&5u32.to_be_bytes());
        // compressors[0..4] = 0 -> CHD v5 descomprimido
        chd[32..40].copy_from_slice(&logical_bytes.to_be_bytes());
        chd[40..48].copy_from_slice(&map_offset.to_be_bytes());
        chd[48..56].copy_from_slice(&0u64.to_be_bytes()); // meta_offset = 0 (DVD)
        chd[56..60].copy_from_slice(&hunk_size.to_be_bytes());
        chd[60..64].copy_from_slice(&2048u32.to_be_bytes());
        for i in 0..hunk_count {
            let block_index = (data_offset / u64::from(hunk_size)) as u32 + i as u32;
            let at = header_size + i * 4;
            chd[at..at + 4].copy_from_slice(&block_index.to_be_bytes());
        }
        let d_off = data_offset as usize;
        chd[d_off..d_off + iso_bytes.len()].copy_from_slice(&iso_bytes);
        let chd_path = dir.path().join("Silent Hill 2.chd");
        std::fs::write(&chd_path, &chd).unwrap();
        assert_eq!(identify(&chd_path), Some("ps2"));
    }
}
