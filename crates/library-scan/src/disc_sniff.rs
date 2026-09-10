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
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().find(|l| l.trim_start().to_ascii_uppercase().starts_with("FILE"))?;
    // FILE "trilha 1.bin" BINARY  → pega o que está entre aspas
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    let track = path.parent()?.join(&line[start..end]);
    sniff_disc_system(&track)
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
    if has(b, b"PSP_GAME") || has(b, b"UMD_DATA.BIN") {
        return Some("psp");
    }
    if has(b, b"BOOT2") || has(b, b"cdrom0:") {
        return Some("ps2");
    }
    if has(b, b"PLAYSTATION")
        || has(b, b"Sony Computer Entertainment")
        || has(b, b"BOOT = cdrom:")
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
}
