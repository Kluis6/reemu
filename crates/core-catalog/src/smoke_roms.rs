//! ROMs mínimas pro teste de fumaça do catálogo (fase 2), geradas aqui —
//! escritas por nós, sem arquivo de terceiro nem licença a conferir. Cada
//! uma só monta o cabeçalho do formato, aponta o reset pra um laço e pinta a
//! cor de fundo, pra dar pra conferir que o core carregou o jogo, rodou
//! quadros e mandou imagem (não preta).
//!
//! Sem logos/marcas de terceiros (logo da Nintendo no cabeçalho do GB/GBA,
//! "SEGA" do TMSS): os cores sem BIOS não conferem. Um core que exigir fica
//! como falha visível no relatório, não é mascarado.

/// Sistema de uma ROM de teste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomKind {
    Nes,
    Snes,
    Gb,
    Gba,
    MegaDrive,
    MasterSystem,
    PcEngine,
    Atari2600,
}

impl RomKind {
    pub fn extension(self) -> &'static str {
        match self {
            RomKind::Nes => "nes",
            RomKind::Snes => "sfc",
            RomKind::Gb => "gb",
            RomKind::Gba => "gba",
            RomKind::MegaDrive => "md",
            RomKind::MasterSystem => "sms",
            RomKind::PcEngine => "pce",
            RomKind::Atari2600 => "a26",
        }
    }

    pub fn build(self) -> Vec<u8> {
        match self {
            RomKind::Nes => nes(),
            RomKind::Snes => snes(),
            RomKind::Gb => gb(),
            RomKind::Gba => gba(),
            RomKind::MegaDrive => mega_drive(),
            RomKind::MasterSystem => master_system(),
            RomKind::PcEngine => pc_engine(),
            RomKind::Atari2600 => atari2600(),
        }
    }

    /// Pelo texto `systems` do catálogo (`core_catalog::CATALOG`). A ordem
    /// importa: Mega Drive/Genesis e "SNES" antes de "NES", "Game Boy
    /// Advance" antes de "Game Boy".
    pub fn for_systems(systems: &str) -> Option<RomKind> {
        let s = systems.to_ascii_lowercase();
        let has = |w: &str| s.contains(w);
        // Mega Drive antes de NES: "ge-nes-is" contém "nes".
        if has("mega drive") || has("genesis") || has("md/") || has("/md") {
            Some(RomKind::MegaDrive)
        } else if has("snes") || has("super nintendo") {
            Some(RomKind::Snes)
        } else if has("nes") || has("famicom") {
            Some(RomKind::Nes)
        } else if has("advance") || has("gba") {
            Some(RomKind::Gba)
        } else if has("game boy") {
            Some(RomKind::Gb)
        } else if has("master system") || has("ms/gg") {
            Some(RomKind::MasterSystem)
        } else if has("pc engine") || has("turbografx") || has("supergrafx") {
            Some(RomKind::PcEngine)
        } else if has("atari 2600") {
            Some(RomKind::Atari2600)
        } else {
            None
        }
    }
}

fn put(rom: &mut [u8], at: usize, bytes: &[u8]) {
    rom[at..at + bytes.len()].copy_from_slice(bytes);
}

/// NES, iNES mapeador 0: 16 KiB de PRG em $C000 + 8 KiB de CHR zerado.
/// Pinta o fundo ($3F00) de azul-claro e liga o fundo no PPUMASK.
fn nes() -> Vec<u8> {
    let mut prg = vec![0u8; 16 * 1024];
    #[rustfmt::skip]
    let code: &[u8] = &[
        0x78,             // $C000 SEI
        0xD8,             //       CLD
        0xA9, 0x00,       //       LDA #$00
        0x8D, 0x01, 0x20, //       STA $2001   ; renderização desligada
        0xAD, 0x02, 0x20, //       LDA $2002   ; zera o latch de endereço
        0xA9, 0x3F,       //       LDA #$3F
        0x8D, 0x06, 0x20, //       STA $2006
        0xA9, 0x00,       //       LDA #$00
        0x8D, 0x06, 0x20, //       STA $2006   ; PPUADDR = $3F00
        0xA9, 0x21,       //       LDA #$21    ; azul-claro
        0x8D, 0x07, 0x20, //       STA $2007
        0xA9, 0x08,       //       LDA #$08
        0x8D, 0x01, 0x20, //       STA $2001   ; mostra o fundo
        0x4C, 0x1E, 0xC0, // $C01E JMP $C01E
        0x40,             // $C021 RTI (NMI/IRQ)
    ];
    put(&mut prg, 0, code);
    // vetores: NMI, RESET, IRQ
    put(&mut prg, 0x3FFA, &[0x21, 0xC0, 0x00, 0xC0, 0x21, 0xC0]);
    let mut rom = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0];
    rom.resize(16, 0);
    rom.extend(prg);
    rom.extend(vec![0u8; 8 * 1024]);
    rom
}

/// SNES LoROM, 128 KiB (código no banco $00:8000). Cor 0 do CGRAM =
/// vermelho, tela ligada em brilho máximo. 128 KiB e não 32: o Snes9x 2010
/// recusa ROM menor ("ROM is corrupt or invalid", testado com 32 e 64).
fn snes() -> Vec<u8> {
    let mut rom = vec![0u8; 128 * 1024];
    #[rustfmt::skip]
    let code: &[u8] = &[
        0x78,             // $8000 SEI
        0x18,             //       CLC
        0xFB,             //       XCE          ; modo nativo
        0xE2, 0x20,       //       SEP #$20     ; A de 8 bits
        0xA9, 0x8F,       //       LDA #$8F
        0x8D, 0x00, 0x21, //       STA $2100    ; forced blank
        0x9C, 0x21, 0x21, //       STZ $2121    ; CGADD = 0
        0xA9, 0x1F,       //       LDA #$1F     ; BGR555 vermelho
        0x8D, 0x22, 0x21, //       STA $2122
        0x9C, 0x22, 0x21, //       STZ $2122
        0xA9, 0x0F,       //       LDA #$0F
        0x8D, 0x00, 0x21, //       STA $2100    ; tela ligada, brilho 15
        0x80, 0xFE,       // $801A BRA $801A
        0x40,             // $801C RTI
    ];
    put(&mut rom, 0, code);
    let h = 0x7FC0;
    put(&mut rom, h, b"REEMU SMOKE TEST     "); // título, 21 bytes
    rom[h + 0x15] = 0x20; // LoROM
    rom[h + 0x16] = 0x00; // só ROM
    rom[h + 0x17] = 0x07; // 128 KiB (1 << 7 KiB)
    rom[h + 0x19] = 0x01; // América do Norte
                          // vetores nativos ($FFE4..) e de emulação ($FFF4..) → RTI; reset → $8000
    for v in (0x7FE4..0x7FFC).step_by(2) {
        put(&mut rom, v, &[0x1C, 0x80]);
    }
    put(&mut rom, 0x7FFC, &[0x00, 0x80]);
    put(&mut rom, 0x7FFE, &[0x1C, 0x80]);
    // checksum: soma dos bytes com complemento/checksum = FFFF/0000
    put(&mut rom, h + 0x1C, &[0xFF, 0xFF, 0x00, 0x00]);
    let sum = rom.iter().fold(0u16, |a, &b| a.wrapping_add(u16::from(b)));
    put(&mut rom, h + 0x1C, &(!sum).to_le_bytes());
    put(&mut rom, h + 0x1E, &sum.to_le_bytes());
    rom
}

/// Game Boy, 32 KiB sem mapeador. A tela já liga depois do boot com o mapa
/// de fundo zerado (branco); o código só fica parado.
fn gb() -> Vec<u8> {
    let mut rom = vec![0u8; 32 * 1024];
    put(&mut rom, 0x100, &[0x00, 0xC3, 0x50, 0x01]); // NOP; JP $0150
    put(&mut rom, 0x134, b"REEMUSMOKE");
    rom[0x143] = 0x00; // só DMG
    rom[0x147] = 0x00; // ROM sem mapeador
    rom[0x148] = 0x00; // 32 KiB
    rom[0x149] = 0x00; // sem RAM
    let chk = rom[0x134..=0x14C]
        .iter()
        .fold(0u8, |x, &b| x.wrapping_sub(b).wrapping_sub(1));
    rom[0x14D] = chk;
    put(&mut rom, 0x150, &[0xF3, 0x18, 0xFE]); // DI; JR -2
    rom
}

/// GBA: salto pro código em $C0, que pinta a cor 0 da paleta (fundo) de
/// vermelho e zera o DISPCNT (modo 0, sem camadas, sem forced blank).
fn gba() -> Vec<u8> {
    let mut rom = vec![0u8; 0x200];
    put(&mut rom, 0x00, &0xEA00_002Eu32.to_le_bytes()); // B $C0
    put(&mut rom, 0xA0, b"REEMUSMOKE\0\0"); // título, 12 bytes
    put(&mut rom, 0xAC, b"RSMK00");
    rom[0xB2] = 0x96; // valor fixo
    let chk = rom[0xA0..=0xBC]
        .iter()
        .fold(0u8, |a, &b| a.wrapping_sub(b))
        .wrapping_sub(0x19);
    rom[0xBD] = chk;
    #[rustfmt::skip]
    let code: [u32; 7] = [
        0xE3A0_0405, // MOV  r0, #0x05000000  ; paleta
        0xE3A0_101F, // MOV  r1, #0x1F        ; vermelho
        0xE1C0_10B0, // STRH r1, [r0]
        0xE3A0_0301, // MOV  r0, #0x04000000  ; DISPCNT
        0xE3A0_1000, // MOV  r1, #0
        0xE1C0_10B0, // STRH r1, [r0]
        0xEAFF_FFFE, // B    .
    ];
    for (i, w) in code.iter().enumerate() {
        put(&mut rom, 0xC0 + i * 4, &w.to_le_bytes());
    }
    rom
}

/// Mega Drive (68000, big-endian), 128 KiB. Cor 0 do CRAM = vermelho e
/// display ligado (reg 1 = $44). Sem TMSS (os cores só exigem com BIOS).
fn mega_drive() -> Vec<u8> {
    let mut rom = vec![0u8; 128 * 1024];
    put(&mut rom, 0, &0x00FF_FE00u32.to_be_bytes()); // SP inicial
    put(&mut rom, 4, &0x0000_0200u32.to_be_bytes()); // PC inicial
    for v in (8..0x100).step_by(4) {
        put(&mut rom, v, &0x0000_021Cu32.to_be_bytes()); // exceções → RTE
    }
    put(&mut rom, 0x100, b"SEGA MEGA DRIVE ");
    put(&mut rom, 0x150, b"REEMU SMOKE TEST");
    put(&mut rom, 0x1A0, &0u32.to_be_bytes());
    let end = (rom.len() as u32) - 1;
    put(&mut rom, 0x1A4, &end.to_be_bytes());
    put(&mut rom, 0x1F0, b"JUE");
    #[rustfmt::skip]
    let code: &[u8] = &[
        0x23, 0xFC, 0xC0, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x04, // MOVE.L #$C0000000,$C00004 ; CRAM 0
        0x33, 0xFC, 0x00, 0x0E, 0x00, 0xC0, 0x00, 0x00,             // MOVE.W #$000E,$C00000     ; vermelho
        0x33, 0xFC, 0x81, 0x44, 0x00, 0xC0, 0x00, 0x04,             // MOVE.W #$8144,$C00004     ; display on
        0x60, 0xFE,                                                 // BRA.S *
        0x4E, 0x73,                                                 // $21C RTE
    ];
    put(&mut rom, 0x200, code);
    let sum = rom[0x200..].chunks_exact(2).fold(0u16, |a, w| {
        a.wrapping_add(u16::from_be_bytes([w[0], w[1]]))
    });
    put(&mut rom, 0x18E, &sum.to_be_bytes());
    rom
}

/// Master System (Z80), 32 KiB. Modo 4, cor de fundo = entrada 16 do CRAM
/// (reg 7 = 0) pintada de vermelho, display ligado.
fn master_system() -> Vec<u8> {
    let mut rom = vec![0u8; 32 * 1024];
    #[rustfmt::skip]
    let code: &[u8] = &[
        0xF3,                               // DI
        0x3E, 0x04, 0xD3, 0xBF, 0x3E, 0x80, 0xD3, 0xBF, // reg 0 = $04 (modo 4)
        0x3E, 0x00, 0xD3, 0xBF, 0x3E, 0x87, 0xD3, 0xBF, // reg 7 = 0 (fundo = cor 16)
        0x3E, 0x10, 0xD3, 0xBF, 0x3E, 0xC0, 0xD3, 0xBF, // endereço CRAM 16
        0x3E, 0x03, 0xD3, 0xBE,             // vermelho
        0x3E, 0x40, 0xD3, 0xBF, 0x3E, 0x81, 0xD3, 0xBF, // reg 1 = $40 (display on)
        0x18, 0xFE,                         // JR $
    ];
    put(&mut rom, 0, code);
    put(&mut rom, 0x66, &[0xED, 0x45]); // NMI (botão pause): RETN
    put(&mut rom, 0x7FF0, b"TMR SEGA");
    rom[0x7FFF] = 0x4C; // SMS export, 32 KiB
    rom
}

/// PC Engine (HuC6280), HuCard de 8 KiB mapeado em $E000 no reset. Cor 0
/// do VCE = azul.
fn pc_engine() -> Vec<u8> {
    let mut rom = vec![0u8; 8 * 1024];
    #[rustfmt::skip]
    let code: &[u8] = &[
        0x78,             // $E000 SEI
        0xD8,             //       CLD
        0xA9, 0xFF,       //       LDA #$FF
        0x53, 0x01,       //       TAM #$01   ; MPR0 = página de E/S
        0xA9, 0x00,       //       LDA #$00
        0x8D, 0x02, 0x04, //       STA $0402  ; endereço da cor no VCE
        0x8D, 0x03, 0x04, //       STA $0403
        0xA9, 0x07,       //       LDA #$07   ; azul (GRB 3-3-3)
        0x8D, 0x04, 0x04, //       STA $0404
        0xA9, 0x00,       //       LDA #$00
        0x8D, 0x05, 0x04, //       STA $0405
        0x80, 0xFE,       // $E018 BRA $E018
        0x40,             // $E01A RTI
    ];
    put(&mut rom, 0, code);
    // vetores IRQ2, IRQ1, TIMER, NMI → RTI; RESET → $E000
    for v in (0x1FF6..0x1FFE).step_by(2) {
        put(&mut rom, v, &[0x1A, 0xE0]);
    }
    put(&mut rom, 0x1FFE, &[0x00, 0xE0]);
    rom
}

/// Atari 2600, 4 KiB em $F000. Fundo vermelho (COLUBK) e quadros com VSYNC
/// de verdade (3 linhas) + 255 linhas.
fn atari2600() -> Vec<u8> {
    let mut rom = vec![0u8; 4 * 1024];
    #[rustfmt::skip]
    let code: &[u8] = &[
        0x78,             // $F000 SEI
        0xD8,             //       CLD
        0xA2, 0xFF,       //       LDX #$FF
        0x9A,             //       TXS
        0xA9, 0x44,       //       LDA #$44
        0x85, 0x09,       //       STA COLUBK ; fundo vermelho
        0xA9, 0x02,       // $F009 LDA #$02   ; quadro:
        0x85, 0x00,       //       STA VSYNC
        0x85, 0x02,       //       STA WSYNC
        0x85, 0x02,       //       STA WSYNC
        0x85, 0x02,       //       STA WSYNC
        0xA9, 0x00,       //       LDA #$00
        0x85, 0x00,       //       STA VSYNC
        0xA2, 0xFF,       //       LDX #$FF
        0x85, 0x02,       // $F019 STA WSYNC  ; linha:
        0xCA,             //       DEX
        0xD0, 0xFB,       //       BNE linha
        0x4C, 0x09, 0xF0, //       JMP quadro
    ];
    put(&mut rom, 0, code);
    put(&mut rom, 0xFFC, &[0x00, 0xF0, 0x00, 0xF0]); // RESET, IRQ
    rom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_from_catalog_systems() {
        assert_eq!(RomKind::for_systems("NES / Famicom"), Some(RomKind::Nes));
        assert_eq!(
            RomKind::for_systems("Nintendo SNES / SFC"),
            Some(RomKind::Snes)
        );
        assert_eq!(RomKind::for_systems("Super Nintendo"), Some(RomKind::Snes));
        assert_eq!(
            RomKind::for_systems("Game Boy Advance / GB / GBC"),
            Some(RomKind::Gba)
        );
        assert_eq!(RomKind::for_systems("Game Boy / Color"), Some(RomKind::Gb));
        assert_eq!(RomKind::for_systems("Sega MD/CD"), Some(RomKind::MegaDrive));
        assert_eq!(
            RomKind::for_systems("Mega Drive / Genesis"),
            Some(RomKind::MegaDrive)
        );
        assert_eq!(
            RomKind::for_systems("Sega MS/GG"),
            Some(RomKind::MasterSystem)
        );
        assert_eq!(
            RomKind::for_systems("NEC PC Engine / SuperGrafx / CD"),
            Some(RomKind::PcEngine)
        );
        assert_eq!(RomKind::for_systems("Atari 2600"), Some(RomKind::Atari2600));
        assert_eq!(RomKind::for_systems("PlayStation"), None);
    }

    #[test]
    fn headers_and_checksums() {
        let nes = RomKind::Nes.build();
        assert_eq!(&nes[..4], b"NES\x1A");
        assert_eq!(nes.len(), 16 + 16 * 1024 + 8 * 1024);
        // reset → $C000
        assert_eq!(&nes[16 + 0x3FFC..16 + 0x3FFE], &[0x00, 0xC0]);

        let snes = RomKind::Snes.build();
        let chk = u16::from_le_bytes([snes[0x7FDE], snes[0x7FDF]]);
        let cmp = u16::from_le_bytes([snes[0x7FDC], snes[0x7FDD]]);
        assert_eq!(chk ^ cmp, 0xFFFF);
        let sum = snes.iter().fold(0u16, |a, &b| a.wrapping_add(u16::from(b)));
        assert_eq!(sum, chk, "a soma com o checksum gravado continua igual");

        let gb = RomKind::Gb.build();
        let x = gb[0x134..=0x14C]
            .iter()
            .fold(0u8, |x, &b| x.wrapping_sub(b).wrapping_sub(1));
        assert_eq!(gb[0x14D], x);

        let gba = RomKind::Gba.build();
        let s = gba[0xA0..=0xBD].iter().fold(0u8, |a, &b| a.wrapping_add(b));
        assert_eq!(s.wrapping_add(0x19), 0, "complemento do cabeçalho GBA");

        let md = RomKind::MegaDrive.build();
        assert_eq!(&md[0x100..0x110], b"SEGA MEGA DRIVE ");

        let pce = RomKind::PcEngine.build();
        assert_eq!(&pce[0x1FFE..], &[0x00, 0xE0]);
        let a26 = RomKind::Atari2600.build();
        assert_eq!(a26.len(), 4096);
    }
}
