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

/// Imagem `.p` de memória BASIC do ZX81 com um retângulo preto no centro.
pub fn zx81_program() -> Vec<u8> {
    const START: usize = 0x4009;
    const DISPLAY: usize = 0x407d;
    const VARIABLES: usize = DISPLAY + 1 + 24 * 33;
    const END: usize = VARIABLES + 1;
    let mut ram = vec![0; END - START];

    for (addr, ptr) in [
        (0x400c, DISPLAY),     // D_FILE
        (0x400e, DISPLAY + 1), // DF_CC
        (0x4010, VARIABLES),   // VARS
        (0x4014, END),         // E_LINE
        (0x401a, END),         // STKBOT
        (0x401c, END),         // STKEND
        (0x401f, 0x405d),      // MEM
        (0x4029, DISPLAY),     // NXTLIN
        (0x4034, 0xffff),      // FRAMES
        (0x4039, 0x1821),      // S_POSN
    ] {
        put(&mut ram, addr - START, &(ptr as u16).to_le_bytes());
    }
    ram[0x4022 - START] = 2; // DF_SZ
    ram[0x4028 - START] = 55; // MARGIN, PAL
    ram[0x403b - START] = 0x40; // CDFLAG, modo lento

    ram[DISPLAY - START] = 0x76;
    for (y, line) in ram[DISPLAY + 1 - START..VARIABLES - START]
        .chunks_exact_mut(33)
        .enumerate()
    {
        if (7..=17).contains(&y) {
            line[6..=25].fill(0x80); // Espaço invertido.
        }
        line[32] = 0x76; // HALT encerra a linha na memória de vídeo.
    }
    ram[VARIABLES - START] = 0x80;
    ram
}

/// Cartucho Mega Duck com tom contínuo e fundo cinza que escurece ao apertar B.
pub fn megaduck_program() -> Vec<u8> {
    let program = [
        0xf3, 0x31, 0xff, 0xdf, // Desliga interrupções e inicializa a pilha.
        0xaf, 0xe0, 0x10, // Desliga o LCD.
        0x21, 0x00, 0x80, 0x01, 0x00, 0x20, 0x16, 0x00, // VRAM e tamanho.
        0x7a, 0x22, 0x0b, 0x78, 0xb1, 0x20, 0xf9, // Zera a VRAM.
        0x3e, 0x55, 0xe0, 0x1b, 0x3e, 0xd0, 0xe0, 0x10, // Fundo cinza.
        0x3e, 0x80, 0xe0, 0x45, 0x3e, 0x77, 0xe0, 0x44, 0x3e, 0x11, 0xe0, 0x46, 0x3e, 0x80, 0xe0,
        0x22, 0x3e, 0xf0, 0xe0, 0x21, 0xaf, 0xe0, 0x23, 0x3e, 0x87, 0xe0,
        0x24, // Aciona o primeiro canal de áudio.
        0x3e, 0x10, 0xe0, 0x00, // Seleciona os botões.
        0xf0, 0x00, 0xf0, 0x00, 0xe6, 0x02, 0x20, 0xf8, // Espera B.
        0x3e, 0xff, 0xe0, 0x1b, 0x18, 0xf2, // Mantém a paleta escura.
    ];
    let mut rom = vec![0; 32 * 1024];
    put(&mut rom, 0, &program);
    rom
}

/// Programa VaporSpec. Liga o vídeo em vermelho e muda para branco ao apertar B.
pub fn vaporspec_program() -> Vec<u8> {
    let words: [u16; 17] = [
        0xb07f, // LRC r0, segmento de vídeo e entrada
        0xb111, // LRC r1, endereço dos controles de vídeo
        0xb201, // LRC r2, habilita o vídeo
        0xa201, // STR r2, r0, r1
        0xb110, // LRC r1, endereço da cor de fundo
        0xb2e0, // LRC r2, vermelho
        0xa201, // STR r2, r0, r1
        0xb300, // LRC r3, endereço do primeiro controle e segmento do salto
        0xb420, // LRC r4, máscara para o botão B no core libretro
        0xb50a, // LRC r5, instrução 10 (laço)
        0x9603, // LDR r6, r0, r3
        0xc664, // AND r6, r6, r4
        0x5663, // CMP r6, r6, r3
        0x8635, // JEQ r6, r3, r5
        0xb2ff, // LRC r2, branco
        0xa201, // STR r2, r0, r1
        0x0535, // JMP r3, r5
    ];
    std::iter::once(words.len() as u16)
        .chain(words)
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Programa AVR próprio em Intel HEX. Acende o OLED por SPI e alterna PC6
/// para produzir um tom. Ao apertar B (PB4), apaga o OLED e silencia o som.
pub const ARDUBOY_HEX: &str = concat!(
    ":1000000007E004B901E105B904ED0AB904E80BB948\n",
    ":1000100000E50CBD0DE813D004E111D007EA0FD0C4\n",
    ":100020000FEA0DD000E407B9469A0FD046980DD0DC\n",
    ":100030001C99FACF00E007B906EA01D0FFCF0EBD48\n",
    ":100040001DB517FFFDCF1EB5089528E030E03A95A5\n",
    ":08005000F1F72A95D9F7089594\n",
    ":00000001FF\n",
);

/// Programa CHIP-8 que desenha um bloco no centro e aciona o temporizador de som.
pub const CHIP8_ROM: &[u8] = &[
    0x00, 0xe0, // Limpa a tela.
    0x60, 0x1c, 0x61, 0x0e, // Posição x=28, y=14 no modo 64×32.
    0xa2, 0x14, 0xd0, 0x15, // Desenha as cinco linhas do sprite em $214.
    0x62, 0x10, 0xf2, 0x18, // Toca por 16 ticks.
    0x12, 0x0e, // Permanece em $20E.
    0x00, 0x00, 0x00, 0x00, // Preenchimento até $214.
    0xff, 0xff, 0xff, 0xff, 0xff,
];

/// Cartucho Z80 de 8 KiB. A BIOS chama $4010, que escreve a cor de fundo no
/// VDP, produz um tom no PSG e estaciona o processador num loop.
pub fn msx_cartridge() -> Vec<u8> {
    let mut rom = vec![0; 8192];
    put(&mut rom, 0, b"AB\x10\x40");
    put(
        &mut rom,
        0x10,
        &[
            0xf3, 0x3e, 0x0d, 0xd3, 0x99, 0x3e, 0x87, 0xd3, 0x99, // VDP
            0xaf, 0xd3, 0xa0, 0x3e, 0x64, 0xd3, 0xa1, 0x3e, 0x07, 0xd3, 0xa0, // PSG
            0x3e, 0x3e, 0xd3, 0xa1, 0x3e, 0x08, 0xd3, 0xa0, 0x3e, 0x08, 0xd3, 0xa1, 0x18,
            0xfe, // JR $
        ],
    );
    rom
}

/// Disquete 2HD com setor de boot próprio. Exibe magenta após o IPL carregar
/// o programa e habilitar a primeira camada gráfica.
pub fn x68000_media() -> (Vec<u8>, Vec<u8>) {
    let mut xdf = vec![0; 77 * 2 * 8 * 1024];
    put(&mut xdf, 0, &[0x60, 0x1e]); // BRA.S para o código no offset 32
    put(&mut xdf, 2, b"REEMUX68");
    let mut pos = 32;
    for (address, value) in [
        (0xe82400u32, 0u16), // modo gráfico de 16 cores
        (0xe82500, 0x02e4),  // ordem das páginas
        (0xe82600, 1),       // habilita a página gráfica 0
        (0xe82002, 0x07fe),  // magenta na entrada 1 da paleta
    ] {
        put(&mut xdf, pos, &[0x33, 0xfc]); // MOVE.W #valor, endereço.l
        put(&mut xdf, pos + 2, &value.to_be_bytes());
        put(&mut xdf, pos + 4, &address.to_be_bytes());
        pos += 8;
    }
    #[rustfmt::skip]
    let fill_page = [
        0x20, 0x7c, 0x00, 0xc0, 0x00, 0x00, // MOVEA.L #$C00000,A0
        0x30, 0xfc, 0x00, 0x01,             // MOVE.W  #1,(A0)+
        0xb1, 0xfc, 0x00, 0xc8, 0x00, 0x00, // CMPA.L  #$C80000,A0
        0x66, 0xf4,                         // BNE.S   para o MOVE.W
        0x60, 0xfe,                         // BRA.S   para si mesmo
    ];
    put(&mut xdf, pos, &fill_page);

    let mut dim = vec![0; 256];
    dim[1..=154].fill(1); // 77 cilindros, duas trilhas por cilindro
    dim.extend_from_slice(&xdf);
    (xdf, dim)
}

/// Disquete Atari ST de 720 KiB. O código do setor de boot pinta a paleta 0
/// de magenta, para distinguir o boot do disquete da tela inicial do firmware.
pub fn atari_st_media() -> Vec<u8> {
    const SECTOR_SIZE: usize = 512;
    let mut disk = vec![0; 720 * 1024];
    put(&mut disk, 0, &[0x60, 0x1c]); // BRA.S para o código no offset 30
    put(&mut disk, 2, b"REEMU ");
    put(
        &mut disk,
        11,
        &[0, 2, 2, 1, 0, 2, 112, 0, 0xa0, 5, 0xf9, 3, 0, 9, 0, 2, 0],
    ); // BPB FAT12, 80 cilindros, 2 lados e 9 setores por trilha
    put(
        &mut disk,
        30,
        &[0x31, 0xfc, 0x07, 0x07, 0x82, 0x40, 0x60, 0xfe],
    );
    for sector in [1, 4] {
        put(&mut disk, sector * SECTOR_SIZE, &[0xf9, 0xff, 0xff]);
    }
    let checksum = disk[..SECTOR_SIZE - 2]
        .chunks_exact(2)
        .map(|word| u16::from_be_bytes([word[0], word[1]]))
        .fold(0u16, u16::wrapping_add);
    put(
        &mut disk,
        SECTOR_SIZE - 2,
        &0x1234u16.wrapping_sub(checksum).to_be_bytes(),
    );
    disk
}

/// NES, iNES mapeador 0: 16 KiB de PRG em $C000 + 8 KiB de CHR zerado.
/// Pinta o fundo ($3F00) de azul-claro e liga o fundo no PPUMASK.
#[rustfmt::skip]
const NES_TEST_PROGRAM: &[u8] = &[
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

fn nes() -> Vec<u8> {
    let mut prg = vec![0u8; 16 * 1024];
    put(&mut prg, 0, NES_TEST_PROGRAM);
    // vetores: NMI, RESET, IRQ
    put(&mut prg, 0x3FFA, &[0x21, 0xC0, 0x00, 0xC0, 0x21, 0xC0]);
    let mut rom = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0];
    rom.resize(16, 0);
    rom.extend(prg);
    rom.extend(vec![0u8; 8 * 1024]);
    rom
}

/// Disco FDS vazio e firmware de teste que pinta o mesmo fundo azul do NES.
pub fn fds_media() -> (Vec<u8>, Vec<u8>) {
    let mut disk = vec![0; 16 + 65_500];
    put(&mut disk, 0, b"FDS\x1A\x01");
    put(&mut disk, 16, b"\x01*NINTENDO-HVC*");
    disk[16 + 56] = 0x02; // marcador do bloco de contagem de arquivos

    let mut firmware = vec![0; 8192];
    put(&mut firmware, 0, NES_TEST_PROGRAM);
    firmware[NES_TEST_PROGRAM.len() - 2] = 0xE0; // JMP no banco $E000
    put(&mut firmware, 0x1FFA, &[0x21, 0xE0, 0x00, 0xE0, 0x21, 0xE0]);
    (disk, firmware)
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
        0x9C, 0x00, 0x42, //       STZ $4200    ; desliga NMI/IRQ
        0x9C, 0x0C, 0x42, //       STZ $420C    ; desliga HDMA
        0x9C, 0x2C, 0x21, //       STZ $212C    ; desliga camadas na tela principal
        0x9C, 0x2D, 0x21, //       STZ $212D    ; desliga camadas na subtela
        0x9C, 0x30, 0x21, //       STZ $2130    ; sem recorte por janela
        0x9C, 0x31, 0x21, //       STZ $2131    ; sem mistura de cores
        0x9C, 0x33, 0x21, //       STZ $2133    ; vídeo sem entrelaçamento
        0x9C, 0x21, 0x21, //       STZ $2121    ; CGADD = 0
        0xA9, 0x1F,       //       LDA #$1F     ; BGR555 vermelho
        0x8D, 0x22, 0x21, //       STA $2122
        0x9C, 0x22, 0x21, //       STZ $2122
        0xA9, 0x0F,       //       LDA #$0F
        0x8D, 0x00, 0x21, //       STA $2100    ; tela ligada, brilho 15
        0x80, 0xFE,       //       BRA *
        0x40,             //       RTI
    ];
    put(&mut rom, 0, code);
    let h = 0x7FC0;
    put(&mut rom, h, b"REEMU SMOKE TEST     "); // título, 21 bytes
    rom[h + 0x15] = 0x20; // LoROM
    rom[h + 0x16] = 0x00; // só ROM
    rom[h + 0x17] = 0x07; // 128 KiB (1 << 7 KiB)
    rom[h + 0x19] = 0x01; // América do Norte

    // Vetores nativos ($FFE4..) e de emulação ($FFF4..) → RTI; reset → $8000.
    let rti = (0x8000 + code.len() as u16 - 1).to_le_bytes();
    for v in (0x7FE4..0x7FFC).step_by(2) {
        put(&mut rom, v, &rti);
    }
    put(&mut rom, 0x7FFC, &[0x00, 0x80]);
    put(&mut rom, 0x7FFE, &rti);
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

/// Cartucho GB com a marcação SGB e um firmware SGB2 sintético que pinta
/// o fundo de vermelho. Assim a fumaça distingue SGB de um fallback para GB.
pub fn sgb2_media() -> (Vec<u8>, Vec<u8>) {
    let mut cart = gb();
    cart[0x146] = 0x03; // compatível com Super Game Boy
    cart[0x14B] = 0x33; // usa o novo código de licença
    cart[0x14D] = cart[0x134..=0x14C]
        .iter()
        .fold(0u8, |x, &b| x.wrapping_sub(b).wrapping_sub(1));

    let mut firmware = snes();
    firmware.resize(512 * 1024, 0);
    let header = 0x7FC0;
    firmware[header + 0x16] = 0xE3; // coprocessador Super Game Boy
    firmware[header + 0x17] = 0x09; // 512 KiB
    put(&mut firmware, header + 0x1C, &[0xFF, 0xFF, 0, 0]);
    let sum = firmware
        .iter()
        .fold(0u16, |sum, &b| sum.wrapping_add(u16::from(b)));
    put(&mut firmware, header + 0x1C, &(!sum).to_le_bytes());
    put(&mut firmware, header + 0x1E, &sum.to_le_bytes());
    (cart, firmware)
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

pub(crate) const DIRKSIMPLE_LUA: &str = r#"
local started = false
local action = false

function DirkSimple.tick(_, _, inputs)
    if not started then
        started = true
        DirkSimple.start_clip(0)
    end
    if inputs.pressed.action then
        action = true
    end
    if action then
        DirkSimple.clear_screen(32, 240, 52)
    end
end

function DirkSimple.serialize()
    return { action }
end

function DirkSimple.unserialize(saved)
    action = saved[1]
    return true
end
"#;

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
