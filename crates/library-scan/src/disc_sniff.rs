//! Identifica o sistema de uma imagem de disco (`.iso`, `.img`, `.cue`…) pelo
//! **conteúdo** — usado quando o nome da pasta não desambigua (biblioteca não
//! organizada por sistema). Lê só o cabeçalho e procura assinaturas conhecidas.
//! `None` = não deu → o scan cai no balde genérico `"disc"`.

use std::fs::File;
use std::io::Read;
use std::path::Path;

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
    let mut text = String::new();
    File::open(path)
        .ok()?
        .take(CUE_MAX_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    let first = parse_cue(&text).into_iter().next()?;
    let track = path.parent()?.join(first.file);
    sniff_disc_system(&track)
}

/// Um `.cue` de verdade tem poucos KiB; acima disto não é cue.
pub(crate) const CUE_MAX_BYTES: u64 = 256 * 1024;

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

fn sniff_bytes(b: &[u8]) -> Option<&'static str> {
    // 3DO: 0x01 'ZZZZZ' no byte 0.
    if b.len() >= 7 && b[0] == 0x01 && &b[1..6] == b"ZZZZZ" {
        return Some("3do");
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

/// Ponto de entrada do scan: fareja, tratando `.cue` como ponteiro pro binário.
pub fn identify(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext == "cue" {
        sniff_cue(path)
    } else {
        sniff_disc_system(path)
    }
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
}
