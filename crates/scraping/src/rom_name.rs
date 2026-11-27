//! Limpeza via expressões regulares e comparação canônica de nomes de ROMs e
//! ISOs (No-Intro, Redump, TOSEC, GoodTools e releases de cena).
//!
//! Separa duas tarefas que antes ficavam misturadas numa única passada simples:
//! 1. Extrair do nome do arquivo (ou da pasta pai, quando o arquivo é genérico
//!    como `disc1.cue` ou `eboot.pbp`) um título limpo para consulta HTTP nas
//!    fontes de metadados (`clean_rom_title`, `search_title`, `search_queries`).
//! 2. Comparar o título da ROM com o título devolvido pelo catálogo tolerando
//!    diferenças de pontuação (` - ` vs `: `), artigos invertidos (`, The`),
//!    acentos (`Pokemon` vs `Pokémon`), `&` vs `and` e algarismos romanos
//!    (`III` vs `3`), sem abrir mão de correspondência determinística.

use std::path::Path;
use std::sync::LazyLock;

use domain::library::Rom;
use regex::Regex;

/// Extensões internas que sobram após `Path::file_stem()` em arquivos com
/// extensão composta (ex.: `Jogo (USA).nkit.iso` -> stem `Jogo (USA).nkit`,
/// `Jogo.bin.ecm` -> `Jogo.bin`, `Jogo.sfc.7z` -> `Jogo.sfc`).
static INNER_EXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\.(?:nkit|wbfs|iso|ciso|cso|chd|rvz|gcz|wia|gcm|mdf|mds|img|bin|cue|ccd|sub|ecm|pbp|xiso|nsp|xci|cia|3ds|nds|dsi|gba|gbc|gb|sfc|smc|nes|fds|unf|z64|n64|v64|md|gen|smd|32x|sms|gg|sg|pce|ngp|ngc|ws|wsc|lnx|j64|a26|a52|a78|col|int|vec|rom|dsk|adf|tzx|tap|z80|sna|prg|d64|t64|crt)$",
    )
    .expect("INNER_EXT_RE válido")
});

/// Seriais de disco Sony (PS1, PS2, PSP, PS3) no início do arquivo:
/// `SLUS-20062 - GTA III`, `SLUS_211.15.God.of.War`, `ULUS10041_Lumines`.
static SONY_SERIAL_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?i)(?:S[CL][A-Z]{2}|U[CL][A-Z]{2}|NP[A-Z]{2}|P[AB]PX)[\s._-]?\d{3}[._-]?\d{2}(?:[\s._:-]+|$)",
    )
    .expect("SONY_SERIAL_PREFIX_RE válido")
});

/// Seriais Sega (Saturn / Dreamcast / Mega-CD) no início do arquivo:
/// `MK-51000 - Sonic Adventure`, `HDR-0001 - Sonic Adventure`, `T-12345G - Jogo`.
static SEGA_SERIAL_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?i)(?:MK-\d{4,5}|HDR-\d{4}|T-\d{4,6}[A-Z]?)\s*[-_:]\s*")
        .expect("SEGA_SERIAL_PREFIX_RE válido")
});

/// IDs de 6 caracteres de GameCube / Wii no início do arquivo (caixa alta para
/// nunca confundir palavras comuns): `GALE01 - Super Smash Bros. Melee`.
static NINTENDO_DISC_ID_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[GDRPUW][A-Z0-9]{2}[EFJPKXDUW][0-9A-Z]{2}(?:\s*[-_:]\s*|\s+|\.)")
        .expect("NINTENDO_DISC_ID_PREFIX_RE válido")
});

/// Captura um serial Sony em qualquer posição do nome (para cruzar com `psn_serials`).
static SONY_SERIAL_FIND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[\s._(\[-])(S[CL][A-Z]{2}|U[CL][A-Z]{2}|NP[A-Z]{2}|P[AB]PX)[\s._-]?(\d{3})[._-]?(\d{2})(?:[\s._)\]:-]|$)",
    )
    .expect("SONY_SERIAL_FIND_RE válido")
});

/// Numeração explícita com cerquilha no início (`#0042 - River Raid`).
static HASH_NUM_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^#\d{1,5}\s*(?:[-._:]\s*|\s+)").expect("HASH_NUM_PREFIX_RE válido")
});

/// Numeração de release com zero à esquerda (`0001 - Mario Kart DS`, `042. River Raid`).
/// O código preserva `007` (`007 - GoldenEye`).
static ZERO_PAD_NUM_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(0\d{1,4})\s*(?:[-._:]\s*|\s+)").expect("ZERO_PAD_NUM_PREFIX_RE válido")
});

/// Numeração de 4 dígitos de sets GBA/NDS/3DS (`2145 - New Super Mario Bros.`).
static FOUR_DIGIT_SCENE_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([1-9]\d{3})\s*-\s+(.+)$").expect("FOUR_DIGIT_SCENE_PREFIX_RE válido")
});

/// Sufixos de mídia/grupo de releases de cena (`PS2DVD-LIGHTFORCE`, `PSPUMD`, `NTSC-U`, `MULTI5`).
static SCENE_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\s+[-_:]?\s*)(?:(?:PS2DVD|PS2CD|PSXPSP|PSPUMD|XBOXDVD|GCISO|WIIISO)(?:-[A-Z0-9_]+)?|NTSC-?[UJ]|MULTI\d+|PROPER|READNFO|REPACK|INTERNAL)(?:-[A-Z0-9_]+)?$",
    )
    .expect("SCENE_TAIL_RE válido")
});

/// Sufixos de região/grupo quando o arquivo veio separado por pontos no padrão
/// de cena (`God.of.War.II.USA.PS2DVD-LIGHTFORCE`, `Burnout.3.Takedown.EUR`).
static DOT_SCENE_REGION_TAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s+(?:USA|EUR|PAL|JPN|JAP|KOR|ntsc|pal)(?:-[A-Z0-9_]+)?$")
        .expect("DOT_SCENE_REGION_TAIL_RE válido")
});

/// Marcadores soltos de disco, trilha, lado, chip ou revisão fora de parênteses:
/// `- Disc 1`, `CD1`, `CD 2`, `DVD 1 of 2`, `GD1`, `Side A`, `Track 01`, `v1.02`,
/// `Ver. 1.1`, `Rev 1`, `Rev A`. Não casa com `Version` sem dígitos (`Emerald Version`).
static BARE_MEDIA_VER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\s+[-:]\s*|\s+)(?:(?:disc|disk|cd|dvd|gd|umd)\s*\d+(?:\s*of\s*\d+)?|(?:side|track|chip)\s*(?:\d+|[a-d])\b|v\d+(?:\.\d+)*[a-z]?|ver\.?\s*\d+(?:\.\d+)*|rev\.?\s*(?:\d+|[a-c])\b)$",
    )
    .expect("BARE_MEDIA_VER_RE válido")
});

/// Artigo posposto no padrão No-Intro / Redump / TOSEC (`Legend of Zelda, The`).
static INVERTED_ARTICLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(.{2,}?),\s*(the|a|an|el|la|los|las|le|les|der|die|das|o|os|as)$")
        .expect("INVERTED_ARTICLE_RE válido")
});

/// Nomes de arquivo genéricos de imagem/trilha onde o título real costuma estar
/// em `ident_name` ou na pasta pai (`disc1.cue`, `track01.bin`, `eboot.pbp`, `game.iso`).
static GENERIC_STEM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:disc|disk|cd|dvd|gd|umd|track|game|rom|boot|eboot|default|start|main|image)[\s._-]?\d*$")
        .expect("GENERIC_STEM_RE válido")
});

/// Pastas de sistema ou organização que nunca devem virar nome de jogo.
const GENERIC_FOLDER_NAMES: &[&str] = &[
    "roms",
    "rom",
    "isos",
    "iso",
    "games",
    "jogos",
    "biblioteca",
    "library",
    "chd",
    "cso",
    "usa",
    "eur",
    "europe",
    "japan",
    "jpn",
    "pal",
    "ntsc",
    "nes",
    "snes",
    "sfc",
    "n64",
    "gb",
    "gbc",
    "gba",
    "vb",
    "nds",
    "gamecube",
    "gc",
    "ngc",
    "wii",
    "megadrive",
    "genesis",
    "mastersystem",
    "sms",
    "gamegear",
    "gg",
    "sega32x",
    "32x",
    "segacd",
    "megacd",
    "saturn",
    "dreamcast",
    "dc",
    "sg1000",
    "naomi",
    "atomiswave",
    "psx",
    "ps1",
    "psone",
    "playstation",
    "ps2",
    "psp",
    "pcengine",
    "pce",
    "pcenginecd",
    "supergrafx",
    "pcfx",
    "atari2600",
    "atari5200",
    "atari7800",
    "atari8bit",
    "atarist",
    "jaguar",
    "lynx",
    "wonderswan",
    "ngp",
    "neogeocd",
    "coleco",
    "intellivision",
    "vectrex",
    "odyssey2",
    "supervision",
    "3do",
    "cdi",
    "msx",
    "c64",
    "amiga",
    "zxspectrum",
    "zx81",
    "x68000",
    "amstradcpc",
    "arcade",
    "mame",
    "fbneo",
    "dos",
    "scummvm",
    "disc",
    "cd1",
    "cd2",
    "cd3",
    "cd4",
    "disc1",
    "disc2",
    "disc3",
    "disc4",
    "disc 1",
    "disc 2",
    "disc 3",
    "disc 4",
];

/// Remove blocos entre `(...)`, `[...]` e `{...}` (inclusive aninhados ou
/// truncados no fim do nome).
fn strip_bracketed_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut paren = 0i32;
    let mut bracket = 0i32;
    let mut brace = 0i32;
    for ch in input.chars() {
        match ch {
            '(' => paren += 1,
            ')' => paren = (paren - 1).max(0),
            '[' => bracket += 1,
            ']' => bracket = (bracket - 1).max(0),
            '{' => brace += 1,
            '}' => brace = (brace - 1).max(0),
            _ if paren == 0 && bracket == 0 && brace == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

fn collapse_spaces(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn restore_inverted_article_segment(segment: &str) -> String {
    let trimmed = segment.trim();
    if let Some(caps) = INVERTED_ARTICLE_RE.captures(trimmed) {
        let body = caps[1].trim();
        let article = caps[2].trim();
        return format!("{article} {body}");
    }
    trimmed.to_string()
}

/// Restaura artigos invertidos tanto no título principal quanto após ` - `
/// (`Legend of Zelda, The - A Link to the Past` -> `The Legend of Zelda - A Link to the Past`).
fn restore_inverted_articles(title: &str) -> String {
    if title.contains(" - ") {
        title
            .split(" - ")
            .map(restore_inverted_article_segment)
            .collect::<Vec<_>>()
            .join(" - ")
    } else {
        restore_inverted_article_segment(title)
    }
}

fn strip_leading_serials(mut s: String) -> String {
    loop {
        let next = if let Some(m) = SONY_SERIAL_PREFIX_RE.find(&s) {
            s[m.end()..].to_string()
        } else if let Some(m) = SEGA_SERIAL_PREFIX_RE.find(&s) {
            s[m.end()..].to_string()
        } else if let Some(m) = NINTENDO_DISC_ID_PREFIX_RE.find(&s) {
            let rest = s[m.end()..].trim();
            if rest.chars().filter(|c| c.is_alphanumeric()).count() >= 2 {
                rest.to_string()
            } else {
                break;
            }
        } else {
            break;
        };
        if next == s {
            break;
        }
        s = next;
    }
    s
}

fn strip_scene_number_prefix(s: &str) -> String {
    if let Some(m) = HASH_NUM_PREFIX_RE.find(s) {
        let rest = s[m.end()..].trim();
        if !rest.is_empty() {
            return rest.to_string();
        }
    }
    if let Some(caps) = ZERO_PAD_NUM_PREFIX_RE.captures(s) {
        let digits = &caps[1];
        // Preserva "007" (ex.: "007 - GoldenEye", "007 - Everything or Nothing").
        if digits != "007" {
            let rest = s[caps.get(0).unwrap().end()..].trim();
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
    }
    if let Some(caps) = FOUR_DIGIT_SCENE_PREFIX_RE.captures(s) {
        if let Ok(num) = caps[1].parse::<u32>() {
            // Preserva anos/títulos reais como "1942", "1943 - The Battle of Midway",
            // "1080" e anos 1900..=2099; `search_queries` oferece fallback sem prefixo.
            let looks_like_year_or_title = (1900..=2099).contains(&num) || num == 1080;
            let rest = caps[2].trim();
            if !looks_like_year_or_title && rest.chars().next().is_some_and(|c| c.is_alphabetic()) {
                return rest.to_string();
            }
        }
    }
    s.to_string()
}

/// Extrai um título limpo e legível a partir do nome-base de uma ROM ou ISO.
pub fn clean_rom_title(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if s.is_empty() {
        return String::new();
    }

    // 1. Extensões compostas remanescentes (`.nkit`, `.bin`, `.wbfs`, `.sfc`...).
    while let Some(m) = INNER_EXT_RE.find(&s) {
        if m.start() == 0 {
            break;
        }
        s.truncate(m.start());
    }

    // 2. Tags entre parênteses, colchetes e chaves (`(USA)`, `[!]`, `{cr}`, etc.).
    s = strip_bracketed_tags(&s);
    if s.trim().is_empty() {
        return String::new();
    }

    // 3. Seriais de disco no início do arquivo (`SLUS-20062 - ...`, `GALE01 - ...`).
    s = strip_leading_serials(s.trim().to_string());

    // 4. Underscores viram espaço; nomes de cena separados apenas por pontos
    //    (`God.of.War.II.USA.PS2DVD-LIGHTFORCE`) convertem pontos em espaços.
    s = s.replace('_', " ");
    let was_dot_scene = !s.contains(' ') && s.chars().filter(|&c| c == '.').count() >= 2;
    if was_dot_scene {
        s = s.replace('.', " ");
    }
    s = collapse_spaces(&s);

    // 5. Numeração inicial de sets/releases (`0001 - Mario Kart DS`, `#042 - ...`).
    s = strip_scene_number_prefix(&s);

    // 6. Caudas de release de cena e marcadores soltos de mídia/versão/disco.
    loop {
        let prev = s.clone();
        s = SCENE_TAIL_RE.replace(&s, "").trim().to_string();
        if was_dot_scene {
            s = DOT_SCENE_REGION_TAIL_RE.replace(&s, "").trim().to_string();
        }
        s = BARE_MEDIA_VER_RE.replace(&s, "").trim().to_string();
        if s == prev {
            break;
        }
    }

    // Remove traços/dois-pontos soltos nas pontas que tenham sobrado após cortar sufixos.
    s = s
        .trim_matches(|c: char| matches!(c, '-' | ':' | ';' | ',' | ' '))
        .to_string();
    s = collapse_spaces(&s);
    if s.is_empty() {
        return String::new();
    }

    // 7. Artigos pospostos (`Legend of Zelda, The` -> `The Legend of Zelda`).
    s = restore_inverted_articles(&s);
    collapse_spaces(&s)
}

/// Nome do arquivo -> título pronto para consulta por texto nas APIs de busca.
/// Mantém palavras e conectores significativos (`Sonic 3 & Knuckles`) e remove
/// separadores de subtítulo ` - ` para casar tanto com `: ` quanto com espaço.
pub fn search_title(file_stem: &str) -> String {
    let cleaned = clean_rom_title(file_stem);
    if cleaned.is_empty() {
        return String::new();
    }
    cleaned
        .replace(" - ", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Gera a lista ordenada e sem duplicatas de consultas para tentar contra uma
/// API de busca por título. A primeira é sempre [`search_title`]; variantes
/// adicionais cobrem casos ambíguos como `1985 - Jogo` (ano vs número de cena)
/// ou `&` vs `and`.
pub fn search_queries(file_stem: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |q: String| {
        let q = collapse_spaces(&q);
        if !q.is_empty() && !out.iter().any(|existing| existing.eq_ignore_ascii_case(&q)) {
            out.push(q);
        }
    };

    let cleaned = clean_rom_title(file_stem);
    let primary = search_title(file_stem);
    push(primary.clone());

    // Se o título limpo começava com `19xx - ` ou `20xx - ` (preservado por
    // segurança para jogos como `1943 - The Battle of Midway`), adiciona a
    // cauda sem o número como segunda tentativa caso seja release numerado.
    if let Some(caps) = FOUR_DIGIT_SCENE_PREFIX_RE.captures(&cleaned) {
        let tail = restore_inverted_articles(caps[2].trim()).replace(" - ", " ");
        push(tail);
    }

    if primary.contains(" & ") {
        push(primary.replace(" & ", " and "));
    }

    out
}

/// Extrai e normaliza um serial Sony (`SLUS-20062`) presente no nome do arquivo,
/// quando a varredura não preencheu `rom.serial`.
pub fn extract_disc_serial(text: &str) -> Option<String> {
    let caps = SONY_SERIAL_FIND_RE.captures(text)?;
    Some(format!(
        "{}-{}{}",
        caps[1].to_ascii_uppercase(),
        &caps[2],
        &caps[3]
    ))
}

/// Normaliza um serial de disco (`slus_200.62` -> `SLUS-20062`).
pub fn normalize_serial(raw: &str) -> Option<String> {
    if let Some(sony) = extract_disc_serial(raw) {
        return Some(sony);
    }
    let out: String = raw
        .trim()
        .to_ascii_uppercase()
        .chars()
        .filter(|c| *c != '.')
        .map(|c| if c == '_' || c == ' ' { '-' } else { c })
        .collect();
    let trimmed = out.trim_matches('-');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn is_generic_or_empty_stem(stem: &str) -> bool {
    let trimmed = stem.trim();
    if trimmed.is_empty() || GENERIC_STEM_RE.is_match(trimmed) {
        return true;
    }
    clean_rom_title(trimmed).is_empty()
}

fn parent_folder_candidate(file_path: &str) -> Option<String> {
    let path = Path::new(file_path);
    let mut dir = path.parent()?;
    for _ in 0..2 {
        let name = dir.file_name().and_then(|n| n.to_str())?.trim();
        let lower = name.to_ascii_lowercase();
        if GENERIC_FOLDER_NAMES.contains(&lower.as_str()) || GENERIC_STEM_RE.is_match(name) {
            dir = dir.parent()?;
            continue;
        }
        if !clean_rom_title(name).is_empty() {
            return Some(name.to_string());
        }
        break;
    }
    None
}

/// Escolhe o melhor nome-base para busca textual de uma ROM: usa `raw_stem`
/// normalmente, mas cai para `rom.ident_name` ou para a pasta do jogo quando o
/// arquivo se chama apenas `disc1.cue`, `track01.bin`, `eboot.pbp` ou `SLUS_200.62.iso`.
pub fn effective_rom_stem(rom: &Rom, raw_stem: &str) -> String {
    if !is_generic_or_empty_stem(raw_stem) {
        return raw_stem.to_string();
    }
    if let Some(ident) = rom.ident_name.as_deref() {
        let ident_stem = Path::new(ident)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(ident);
        if !is_generic_or_empty_stem(ident_stem) {
            return ident_stem.to_string();
        }
    }
    if let Some(folder) = parent_folder_candidate(&rom.file_path) {
        return folder;
    }
    raw_stem.to_string()
}

/// Remove acentos e diacríticos latinos comuns para comparação ASCII (`Pokémon` -> `Pokemon`).
pub fn fold_diacritics(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => out.push('a'),
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' => out.push('A'),
            'æ' => out.push_str("ae"),
            'Æ' => out.push_str("AE"),
            'ç' | 'ć' | 'č' => out.push('c'),
            'Ç' | 'Ć' | 'Č' => out.push('C'),
            'ď' | 'đ' => out.push('d'),
            'Ď' | 'Đ' => out.push('D'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' => out.push('e'),
            'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ė' | 'Ę' => out.push('E'),
            'ì' | 'í' | 'î' | 'ï' | 'ī' => out.push('i'),
            'Ì' | 'Í' | 'Î' | 'Ï' | 'Ī' => out.push('I'),
            'ł' => out.push('l'),
            'Ł' => out.push('L'),
            'ñ' | 'ń' => out.push('n'),
            'Ñ' | 'Ń' => out.push('N'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => out.push('o'),
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' => out.push('O'),
            'œ' => out.push_str("oe"),
            'Œ' => out.push_str("OE"),
            'ř' => out.push('r'),
            'Ř' => out.push('R'),
            'ś' | 'š' | 'ş' => out.push('s'),
            'Ś' | 'Š' | 'Ş' => out.push('S'),
            'ß' => out.push_str("ss"),
            'ť' => out.push('t'),
            'Ť' => out.push('T'),
            'ù' | 'ú' | 'û' | 'ü' | 'ū' => out.push('u'),
            'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ū' => out.push('U'),
            'ý' | 'ÿ' => out.push('y'),
            'Ý' | 'Ÿ' => out.push('Y'),
            'ź' | 'ż' | 'ž' => out.push('z'),
            'Ź' | 'Ż' | 'Ž' => out.push('Z'),
            // Marcas combinantes Unicode (NFD): descartadas após a letra base.
            '\u{0300}'..='\u{036f}' => {}
            _ => out.push(ch),
        }
    }
    out
}

fn roman_token_to_arabic(tok: &str) -> Option<&'static str> {
    match tok {
        "ii" => Some("2"),
        "iii" => Some("3"),
        "iv" => Some("4"),
        "v" => Some("5"),
        "vi" => Some("6"),
        "vii" => Some("7"),
        "viii" => Some("8"),
        "ix" => Some("9"),
        "x" => Some("10"),
        "xi" => Some("11"),
        "xii" => Some("12"),
        "xiii" => Some("13"),
        "xiv" => Some("14"),
        "xv" => Some("15"),
        "xvi" => Some("16"),
        "xvii" => Some("17"),
        "xviii" => Some("18"),
        "xix" => Some("19"),
        "xx" => Some("20"),
        _ => None,
    }
}

/// Gera tokens canônicos de um título para comparação entre ROM e catálogo:
/// - restaura artigos invertidos (`, The`)
/// - remove diacríticos (`Pokémon` -> `pokemon`)
/// - unifica `&` e `+` isolados em `and`, `versus`/`vs.` em `vs`, `bros.` em `bros`
/// - remove apóstrofos possessivos/tipográficos (`Luigi's` -> `luigis`)
/// - converte algarismos romanos `II`..`XX` em `2`..`20` quando `fold_roman = true`
fn canonical_tokens(input: &str, fold_roman: bool) -> Vec<String> {
    let restored = restore_inverted_articles(input.trim());
    let folded = fold_diacritics(&restored).to_lowercase();
    let mut prepared = String::with_capacity(folded.len() + 8);
    for ch in folded.chars() {
        match ch {
            '\'' | '\u{2018}' | '\u{2019}' | '`' => {}
            '&' => prepared.push_str(" and "),
            ':' | ';' | '/' | '\\' | '-' | '\u{2013}' | '.' | ',' | '!' | '?' | '"' | '(' | ')'
            | '[' | ']' | '{' | '}' => prepared.push(' '),
            _ => prepared.push(ch),
        }
    }

    prepared
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|tok| {
            if tok == "versus" {
                return "vs".to_string();
            }
            if fold_roman {
                if let Some(num) = roman_token_to_arabic(tok) {
                    return num.to_string();
                }
            }
            tok.to_string()
        })
        .collect()
}

/// Chave canônica estrita de um título.
pub fn canonical_key(title: &str) -> String {
    canonical_tokens(title, true).join(" ")
}

/// Compara o título limpo da ROM com o título de um candidato do catálogo:
/// - `Some(3)`: equivalência canônica completa (ex.: `Castlevania - Symphony of the Night`
///   == `Castlevania: Symphony of the Night`, `Pokemon - Emerald Version` == `Pokémon Emerald Version`,
///   `Legend of Zelda, The - A Link to the Past` == `The Legend of Zelda: A Link to the Past`).
/// - `Some(2)`: equivalência canônica ignorando `the` inicial em títulos com 2+ palavras
///   (ex.: `Legend of Zelda` == `The Legend of Zelda`).
/// - `None`: títulos diferentes (ex.: `Super Mario` != `Super Mario World`).
pub fn title_match_tier(rom_title: &str, candidate_title: &str) -> Option<u8> {
    let rom_clean = clean_rom_title(rom_title);
    let rom_ref = if rom_clean.is_empty() {
        rom_title
    } else {
        &rom_clean
    };
    let a = canonical_tokens(rom_ref, true);
    let b = canonical_tokens(candidate_title, true);
    if a.is_empty() || b.is_empty() {
        return None;
    }
    if a == b {
        return Some(3);
    }
    fn strip_leading_the(tokens: &[String]) -> &[String] {
        if tokens.len() >= 2 && tokens[0] == "the" {
            &tokens[1..]
        } else {
            tokens
        }
    }
    let a_relaxed = strip_leading_the(&a);
    let b_relaxed = strip_leading_the(&b);
    if !a_relaxed.is_empty() && a_relaxed == b_relaxed {
        return Some(2);
    }
    None
}

static PAREN_GROUP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(([^()]+)\)|\[([^\[\]]+)\]").expect("PAREN_GROUP_RE válido"));

static NINTENDO_DISC_REGION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[\s._(\[-])[GDRPUW][A-Z0-9]{2}([EFJPKXDUWSI])[0-9A-Z]{2}(?:[\s._)\]:-]|$)")
        .expect("NINTENDO_DISC_REGION_RE válido")
});

static QUALIFIER_DISC_CHIP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:(?:disc|disk|cd|dvd|gd|umd|side|chip)\s*(?:\d+|[a-d])(?:\s*of\s*\d+)?)$")
        .expect("QUALIFIER_DISC_CHIP_RE válido")
});

static QUALIFIER_REV_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:rev\.?\s*(?:\d+|[a-c])|v\d+(?:\.\d+)+[a-z]?|ver\.?\s*\d+(?:\.\d+)*)$")
        .expect("QUALIFIER_REV_RE válido")
});

static QUALIFIER_EDITION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:beta(?:\s*\d+)?|proto(?:\s*\d+)?|prototype|demo|sample|promo|kiosk|arcade|virtual console|unl|pirate|t[+-][a-z]{2,5}.*)$",
    )
    .expect("QUALIFIER_EDITION_RE válido")
});

/// Normaliza um código ou nome de região (No-Intro, Redump, ScreenScraper,
/// LaunchBox/IGDB ou GoodTools) para o vocabulário canônico:
/// `"NA"`, `"EU"`, `"JP"`, `"BR"`, `"KR"`, `"AU"`, `"CN"` ou `"WORLD"`.
pub fn normalize_region_code(raw: &str) -> Option<&'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Listas No-Intro separadas por vírgula: ex. "USA, Europe", "USA, Canada", "Europe, Australia".
    if trimmed.contains(',') {
        let mut codes: Vec<&'static str> = Vec::new();
        for part in trimmed.split(',') {
            if let Some(code) = normalize_single_region(part.trim(), false) {
                if !codes.contains(&code) {
                    codes.push(code);
                }
            }
        }
        return match codes.as_slice() {
            [] => None,
            [single] => Some(*single),
            _ if codes.iter().all(|c| matches!(*c, "EU" | "AU")) => Some("EU"),
            _ if codes.iter().all(|c| *c == "NA") => Some("NA"),
            _ if (codes.contains(&"NA") && codes.contains(&"EU"))
                || (codes.contains(&"NA") && codes.contains(&"JP"))
                || (codes.contains(&"EU") && codes.contains(&"JP")) =>
            {
                Some("WORLD")
            }
            [first, ..] => Some(*first),
        };
    }
    normalize_single_region(trimmed, true)
}

fn normalize_single_region(token: &str, allow_single_letter: bool) -> Option<&'static str> {
    let lower = token.trim().to_ascii_lowercase();
    match lower.as_str() {
        "na" | "usa" | "us" | "u.s.a." | "north america" | "canada" | "ca" | "cus" | "ntsc-u"
        | "ntsc" | "americas" => Some("NA"),
        "eu" | "eur" | "europe" | "pal" | "pal-e" | "pal-m" | "uk" | "united kingdom"
        | "england" | "great britain" | "germany" | "ger" | "de" | "deu" | "france" | "fra"
        | "fr" | "spain" | "esp" | "es" | "italy" | "ita" | "it" | "netherlands" | "nl"
        | "holland" | "sweden" | "swe" | "se" | "scandinavia" | "portugal" | "pt" | "belgium"
        | "austria" | "switzerland" => Some("EU"),
        "jp" | "jpn" | "jap" | "japan" | "ja" | "ntsc-j" => Some("JP"),
        "br" | "bra" | "brazil" | "brasil" => Some("BR"),
        "kr" | "kor" | "korea" | "south korea" | "ko" => Some("KR"),
        "au" | "aus" | "australia" => Some("AU"),
        "cn" | "chn" | "china" | "asia" | "taiwan" | "tw" | "hong kong" | "hk" => Some("CN"),
        "world" | "wor" | "jue" | "ue" | "ju" | "uj" | "eu-us" => Some("WORLD"),
        "u" if allow_single_letter => Some("NA"),
        "e" | "f" | "g" | "s" | "i" if allow_single_letter => Some("EU"),
        "j" if allow_single_letter => Some("JP"),
        "b" if allow_single_letter => Some("BR"),
        "k" if allow_single_letter => Some("KR"),
        "a" if allow_single_letter => Some("AU"),
        "w" if allow_single_letter => Some("WORLD"),
        _ => None,
    }
}

fn region_from_serial(serial_text: &str) -> Option<&'static str> {
    if let Some(sony) = extract_disc_serial(serial_text) {
        let prefix = sony.split('-').next().unwrap_or("");
        return match prefix {
            "SLUS" | "SCUS" | "ULUS" | "UCUS" | "NPUI" | "NPUA" | "NPUF" | "NPUG" | "NPUH"
            | "NPUJ" | "NPUX" | "NPUZ" => Some("NA"),
            "SLES" | "SCES" | "ULES" | "UCES" | "NPEF" | "NPEG" | "NPEA" | "NPEH" | "NPEX"
            | "NPEZ" => Some("EU"),
            "SLPS" | "SLPM" | "SCPS" | "SCPM" | "ULJM" | "ULJS" | "UCJS" | "NPJJ" | "NPJA"
            | "NPJG" | "NPJH" | "PAPX" | "PBPX" => Some("JP"),
            "SLKA" | "SCKA" | "ULKS" | "UCKS" => Some("KR"),
            "SLAJ" | "SCAJ" | "ULAS" | "UCAS" => Some("CN"),
            _ => None,
        };
    }
    if let Some(caps) = NINTENDO_DISC_REGION_RE.captures(serial_text) {
        return match &caps[1] {
            "E" | "N" => Some("NA"),
            "J" => Some("JP"),
            "K" => Some("KR"),
            "P" | "D" | "F" | "S" | "I" | "U" | "X" | "Y" => Some("EU"),
            _ => None,
        };
    }
    None
}

fn region_from_name(name: &str) -> Option<&'static str> {
    for caps in PAREN_GROUP_RE.captures_iter(name) {
        let inner = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str().trim())
            .unwrap_or("");
        if inner.is_empty() {
            continue;
        }
        // Em colchetes `[...]`, letras soltas do GoodTools como `[b]`, `[f]`, `[p]`
        // são flags de dump (bad/fixed/pirate), não regiões; só aceita 2+ letras
        // ou `[U]`/`[E]`/`[J]` maiúsculos.
        if caps.get(2).is_some() && inner.len() == 1 {
            match inner {
                "U" => return Some("NA"),
                "E" => return Some("EU"),
                "J" => return Some("JP"),
                _ => continue,
            }
        }
        if let Some(code) = normalize_region_code(inner) {
            return Some(code);
        }
    }
    if let Some(from_serial) = region_from_serial(name) {
        return Some(from_serial);
    }
    // Releases de cena separados por pontos ou com sufixo NTSC/PAL explícito.
    let upper = name.to_ascii_uppercase();
    if upper.contains("NTSC-U") || upper.contains(".USA.") || upper.ends_with(".USA") {
        return Some("NA");
    }
    if upper.contains("NTSC-J")
        || upper.contains(".JPN.")
        || upper.contains(".JAP.")
        || upper.ends_with(".JPN")
        || upper.ends_with(".JAP")
    {
        return Some("JP");
    }
    if upper.contains(".EUR.")
        || upper.contains(".PAL.")
        || upper.ends_with(".EUR")
        || upper.ends_with(".PAL")
    {
        return Some("EU");
    }
    None
}

/// Detecta a região canônica (`NA`, `EU`, `JP`, `BR`, `KR`, `AU`, `CN`, `WORLD`)
/// de uma ROM combinando `ident_name`, nome do arquivo, pasta pai e `serial`.
pub fn detect_rom_region(
    file_path: &str,
    ident_name: Option<&str>,
    serial: Option<&str>,
) -> Option<&'static str> {
    if let Some(ident) = ident_name {
        if let Some(code) = region_from_name(ident) {
            return Some(code);
        }
    }
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path);
    if let Some(code) = region_from_name(file_name) {
        return Some(code);
    }
    if let Some(s) = serial {
        if let Some(code) = region_from_serial(s) {
            return Some(code);
        }
    }
    if let Some(folder) = parent_folder_candidate(file_path) {
        if let Some(code) = region_from_name(&folder) {
            return Some(code);
        }
    }
    None
}

/// Extrai qualificadores de edição/revisão/mídia (ex.: `Disc 1`, `Chip 2`,
/// `Rev 1`, `v1.02`, `Beta`, `T-PtBr`) para diferenciar múltiplas ROMs do mesmo
/// jogo no seletor de versões.
pub fn extract_rom_qualifier(
    file_path: &str,
    ident_name: Option<&str>,
    revision_override: Option<&str>,
) -> Option<String> {
    let source = ident_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path)
        });

    fn push_unique(parts: &mut Vec<String>, label: &str) {
        let trimmed = label.trim();
        if !trimmed.is_empty()
            && !parts
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(trimmed))
        {
            parts.push(trimmed.to_string());
        }
    }

    let mut parts: Vec<String> = Vec::new();

    for caps in PAREN_GROUP_RE.captures_iter(source) {
        let inner = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str().trim())
            .unwrap_or("");
        if inner.is_empty() || inner == "!" {
            continue;
        }
        if QUALIFIER_DISC_CHIP_RE.is_match(inner)
            || QUALIFIER_REV_RE.is_match(inner)
            || QUALIFIER_EDITION_RE.is_match(inner)
        {
            push_unique(&mut parts, inner);
        }
    }

    // Caso o marcador de disco/versão esteja solto fora de parênteses (`Jogo - Disc 1`).
    let stem = Path::new(source)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(source);
    let without_brackets = collapse_spaces(&strip_bracketed_tags(stem).replace('_', " "));
    if let Some(m) = BARE_MEDIA_VER_RE.find(&without_brackets) {
        let raw_tail = m
            .as_str()
            .trim_matches(|c: char| matches!(c, '-' | ':' | ' '));
        push_unique(&mut parts, raw_tail);
    }

    if let Some(rev) = revision_override
        .map(str::trim)
        .filter(|r| !r.is_empty() && !parts.iter().any(|p| QUALIFIER_REV_RE.is_match(p)))
    {
        push_unique(&mut parts, rev);
    }

    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// Extrai o radical normalizado do arquivo de capa em uma URL ou caminho,
/// removendo extensão e sufixo `_{8 hex}` do espelho (`box-texture_0af13c71.png` -> `box-texture`).
pub fn cover_stem(url_or_path: &str) -> String {
    let clean = url_or_path.split(['?', '#']).next().unwrap_or(url_or_path);
    let file_name = clean.rsplit(['/', '\\']).next().unwrap_or(clean);
    let no_ext = match file_name.rsplit_once('.') {
        Some((base, ext)) if !base.is_empty() && !ext.is_empty() => base,
        _ => file_name,
    };
    let stem = match no_ext.rsplit_once('_') {
        Some((base, hash))
            if !base.is_empty()
                && hash.len() == 8
                && hash.chars().all(|c| c.is_ascii_hexdigit()) =>
        {
            base
        }
        _ => no_ext,
    };
    stem.to_ascii_lowercase()
}

/// Indica se a URL aponta para uma arte não-frontal de caixa: textura aberta
/// com contracapa + lombada + frente (`box-texture`), textura de mídia
/// (`support-texture`), verso/lateral ou render 3D inclinado (`box-3D`).
pub fn is_non_front_cover_url(url: &str) -> bool {
    let stem = cover_stem(url);
    stem.contains("texture")
        || stem.contains("3d")
        || stem == "back"
        || stem.ends_with("-back")
        || stem.ends_with("_back")
        || stem == "side"
        || stem.ends_with("-side")
        || stem.ends_with("_side")
        || stem.contains("spine")
}

/// Sistemas cujas caixas físicas originais são quadradas (~1:1, ex.: 700x700
/// nas capas `cover` / `box-2D` do catálogo).
pub fn is_square_box_system(system_id: &str) -> bool {
    matches!(
        system_id.trim().to_ascii_lowercase().as_str(),
        "gb" | "gbc"
            | "gba"
            | "vb"
            | "nds"
            | "pokemini"
            | "psx"
            | "dreamcast"
            | "pcengine"
            | "pcenginecd"
            | "supergrafx"
            | "pcfx"
            | "ngp"
            | "neogeocd"
            | "3do"
            | "cdi"
            | "lynx"
            | "gamegear"
            | "wonderswan"
            | "supervision"
            | "megaduck"
            | "game-boy"
            | "game-boy-color"
            | "game-boy-advance"
            | "virtual-boy"
            | "pokemon-mini"
            | "nintendo-ds"
            | "nintendo-dsi"
            | "playstation"
            | "turbografx-16-pc-engine"
            | "turbografx-16-pc-engine-cd"
            | "pc-engine-cd-rom"
            | "pc-engine-supergrafx"
            | "neo-geo-pocket"
            | "neo-geo-pocket-color"
            | "neo-geo-cd"
            | "3do-interactive-multiplayer"
            | "philips-cd-i"
            | "atari-lynx"
            | "sega-game-gear"
            | "wonderswan-color"
            | "watara-supervision"
            | "watara-quickshot-supervision"
    )
}

/// Pontuação de prioridade para escolha automática de capa (menor = melhor):
/// `(kind_tier, square_pref, default_pref)`.
/// - `kind_tier = 0`: capa frontal 2D completa (`cover`, `box-2D`, `1080p`, `coXXXX`...)
/// - `kind_tier = 10`: resolução intermediária (`cover_big`)
/// - `kind_tier = 20`: miniatura pequena (`thumb`, `cover_small`)
/// - `kind_tier = 30`: caixa 3D inclinada (`box-3D`)
/// - `kind_tier = 40`: caixa aberta com lombada/verso (`box-texture`, `back`, `side`)
pub fn cover_url_rank(
    url: &str,
    system_id: Option<&str>,
    is_default: bool,
    dims: Option<(u32, u32)>,
) -> (u8, u8, u8) {
    let lower = url.to_ascii_lowercase();
    let stem = cover_stem(url);

    let is_wrap_or_back = stem.contains("texture")
        || stem == "back"
        || stem.ends_with("-back")
        || stem.ends_with("_back")
        || stem == "side"
        || stem.ends_with("-side")
        || stem.ends_with("_side")
        || stem.contains("spine");
    let is_3d = stem.contains("3d");
    let is_tiny = matches!(stem.as_str(), "thumb" | "cover_small")
        || lower.contains("/t_thumb/")
        || lower.contains("/t_cover_small/");
    let is_medium = stem == "cover_big";

    let kind_tier = if is_wrap_or_back {
        40
    } else if is_3d {
        30
    } else if is_tiny {
        20
    } else if is_medium {
        10
    } else {
        0
    };

    let known_square = match dims {
        Some((w, h)) if w > 0 && h > 0 => {
            let ratio = f64::from(w) / f64::from(h);
            (0.85..=1.18).contains(&ratio)
        }
        _ => {
            system_id.is_some_and(is_square_box_system)
                && matches!(stem.as_str(), "cover" | "box-2d")
        }
    };
    let square_pref = if kind_tier == 0 && known_square { 0 } else { 1 };

    let default_pref = if kind_tier == 0 && is_default {
        0
    } else if matches!(
        stem.as_str(),
        "1080p" | "cover" | "box-2d" | "cover_big_2x" | "720p"
    ) {
        1
    } else {
        2
    };

    (kind_tier, square_pref, default_pref)
}

/// Escolhe a melhor capa frontal/quadrada dentre uma sequência de URLs candidatas.
pub fn pick_best_cover_url<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
    system_id: Option<&str>,
    default_url: Option<&str>,
) -> Option<&'a str> {
    candidates
        .into_iter()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .enumerate()
        .min_by_key(|(idx, u)| {
            let is_def = default_url.is_some_and(|d| d.trim() == *u);
            (cover_url_rank(u, system_id, is_def, None), *idx)
        })
        .map(|(_, u)| u)
}

/// Escolhe a capa regional mais adequada para a região da ROM (`JP`, `EU`, `NA`,
/// `BR`, `KR`, `AU`, `WORLD`), caindo para `fallback` quando não houver variante
/// específica para aquela região (e evitando cair em `box-texture`/`box-3D` se
/// houver capa frontal disponível).
pub fn pick_regional_cover<'a>(
    regional_covers: &'a std::collections::BTreeMap<String, String>,
    region: Option<&str>,
    fallback: Option<&'a str>,
) -> Option<&'a str> {
    let fallback_clean = fallback.map(str::trim).filter(|u| !u.is_empty());
    let fallback_is_front = fallback_clean.is_some_and(|u| !is_non_front_cover_url(u));

    if !regional_covers.is_empty() {
        if let Some(raw_reg) = region {
            let norm = normalize_region_code(raw_reg);
            let candidates: &[&str] = match norm {
                Some("JP") => &["JP"],
                Some("EU") => &["EU"],
                Some("AU") => &["AU", "EU"],
                Some("BR") => &["BR", "NA", "WORLD"],
                Some("KR") => &["KR"],
                Some("CN") => &["CN"],
                Some("NA") => &["NA", "WORLD"],
                Some("WORLD") => &["WORLD", "NA", "EU"],
                _ => &[],
            };
            for key in candidates {
                if let Some(url) = regional_covers
                    .get(*key)
                    .map(String::as_str)
                    .map(str::trim)
                    .filter(|u| !u.is_empty() && (!fallback_is_front || !is_non_front_cover_url(u)))
                {
                    return Some(url);
                }
            }
            let direct = raw_reg.trim().to_ascii_uppercase();
            if let Some(url) = regional_covers
                .get(&direct)
                .map(String::as_str)
                .map(str::trim)
                .filter(|u| !u.is_empty() && (!fallback_is_front || !is_non_front_cover_url(u)))
            {
                return Some(url);
            }
        }

        if !fallback_is_front {
            for key in ["WORLD", "NA", "EU", "BR", "JP", "AU", "KR", "CN"] {
                if let Some(url) = regional_covers
                    .get(key)
                    .map(String::as_str)
                    .map(str::trim)
                    .filter(|u| !u.is_empty() && !is_non_front_cover_url(u))
                {
                    return Some(url);
                }
            }
            if let Some(url) = regional_covers
                .values()
                .map(String::as_str)
                .map(str::trim)
                .find(|u| !u.is_empty() && !is_non_front_cover_url(u))
            {
                return Some(url);
            }
        }
    }
    fallback_clean
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_nointro_redump_tosec_and_goodtools_tags() {
        assert_eq!(
            clean_rom_title("Super Mario World (USA)"),
            "Super Mario World"
        );
        assert_eq!(
            clean_rom_title("Chrono_Trigger (USA) (Rev 1) [!]"),
            "Chrono Trigger"
        );
        assert_eq!(
            clean_rom_title("128-in-1 Junior Console (Chip 1) (PAL) [!]"),
            "128-in-1 Junior Console"
        );
        assert_eq!(
            clean_rom_title("11 Sprite Demo (Piero Cavina) (PD)"),
            "11 Sprite Demo"
        );
        assert_eq!(
            clean_rom_title("AAAHH!!! Real Monsters (U) [!]"),
            "AAAHH!!! Real Monsters"
        );
        assert_eq!(
            clean_rom_title("Turrican II (1991)(Rainbow Arts)[cr FLT]{a2}"),
            "Turrican II"
        );
        assert_eq!(clean_rom_title("(Beta)"), "");
        assert_eq!(
            clean_rom_title("Super Mario World (USA, Europe"),
            "Super Mario World"
        );
    }

    #[test]
    fn strips_compound_inner_extensions_from_file_stem() {
        assert_eq!(
            clean_rom_title("Luigi's Mansion (USA).nkit"),
            "Luigi's Mansion"
        );
        assert_eq!(
            clean_rom_title("Final Fantasy VII (USA) (Disc 1).bin"),
            "Final Fantasy VII"
        );
        assert_eq!(
            clean_rom_title("Super Smash Bros. Brawl (USA).wbfs"),
            "Super Smash Bros. Brawl"
        );
    }

    #[test]
    fn strips_disc_serial_prefixes_and_extracts_sony_serial() {
        assert_eq!(
            clean_rom_title("SLUS-20062 - Grand Theft Auto III (USA)"),
            "Grand Theft Auto III"
        );
        assert_eq!(clean_rom_title("SLUS_211.15.God.of.War"), "God of War");
        assert_eq!(clean_rom_title("ULUS10041_Lumines"), "Lumines");
        assert_eq!(
            clean_rom_title("GALE01 - Super Smash Bros. Melee"),
            "Super Smash Bros. Melee"
        );
        assert_eq!(
            clean_rom_title("MK-51000 - Sonic Adventure"),
            "Sonic Adventure"
        );
        assert_eq!(
            extract_disc_serial("SLUS_211.15.God.of.War"),
            Some("SLUS-21115".into())
        );
        assert_eq!(
            extract_disc_serial("ULUS10041_Lumines"),
            Some("ULUS-10041".into())
        );
        assert_eq!(normalize_serial("scus_949.00"), Some("SCUS-94900".into()));
    }

    #[test]
    fn strips_scene_release_numbers_without_breaking_007_or_1943() {
        assert_eq!(
            clean_rom_title("0001 - Mario Kart DS (USA)"),
            "Mario Kart DS"
        );
        assert_eq!(
            clean_rom_title("0425 - Pokemon - Emerald Version (USA, Europe)"),
            "Pokemon - Emerald Version"
        );
        assert_eq!(clean_rom_title("#0042 - River Raid"), "River Raid");
        assert_eq!(clean_rom_title("042. River Raid"), "River Raid");
        assert_eq!(
            clean_rom_title("2145 - New Super Mario Bros. (USA)"),
            "New Super Mario Bros."
        );
        // "007" e "1943" não são descartados na limpeza primária.
        assert_eq!(clean_rom_title("007 - GoldenEye (USA)"), "007 - GoldenEye");
        assert_eq!(
            clean_rom_title("1943 - The Battle of Midway (USA)"),
            "1943 - The Battle of Midway"
        );
        // Mas `search_queries` inclui a cauda sem o ano caso `1985 - ...` seja numeração de cena.
        let qs = search_queries("1985 - Kirby Super Star Ultra (USA)");
        assert_eq!(
            qs,
            vec!["1985 Kirby Super Star Ultra", "Kirby Super Star Ultra"]
        );
    }

    #[test]
    fn cleans_dot_separated_scene_isos_and_bare_disc_or_version_markers() {
        assert_eq!(
            clean_rom_title("God.of.War.II.USA.PS2DVD-LIGHTFORCE"),
            "God of War II"
        );
        assert_eq!(
            clean_rom_title("Burnout.3.Takedown.EUR"),
            "Burnout 3 Takedown"
        );
        // "Cruis'n USA" tem espaços normais: "USA" faz parte do nome!
        assert_eq!(clean_rom_title("Cruis'n USA (USA)"), "Cruis'n USA");
        assert_eq!(
            clean_rom_title("Final Fantasy VII - Disc 1"),
            "Final Fantasy VII"
        );
        assert_eq!(clean_rom_title("Metal Gear Solid CD2"), "Metal Gear Solid");
        assert_eq!(
            clean_rom_title("Super Smash Bros. Melee v1.02"),
            "Super Smash Bros. Melee"
        );
        // "Version" sem número faz parte do título oficial de Pokémon.
        assert_eq!(
            search_title("Pokemon - Emerald Version (USA, Europe)"),
            "Pokemon Emerald Version"
        );
    }

    #[test]
    fn restores_inverted_articles_on_main_title_and_subtitle() {
        assert_eq!(
            clean_rom_title("Legend of Zelda, The (USA)"),
            "The Legend of Zelda"
        );
        assert_eq!(
            clean_rom_title("Legend of Zelda, The - A Link to the Past (USA)"),
            "The Legend of Zelda - A Link to the Past"
        );
        assert_eq!(
            clean_rom_title("Simpsons, The - Bart's Nightmare (USA)"),
            "The Simpsons - Bart's Nightmare"
        );
        // Nomes reais com vírgula que não são artigos continuam intactos.
        assert_eq!(clean_rom_title("Monsters, Inc. (USA)"), "Monsters, Inc.");
        assert_eq!(clean_rom_title("I, Robot (USA)"), "I, Robot");
    }

    #[test]
    fn effective_rom_stem_falls_back_to_parent_folder_for_generic_disc_names() {
        let cue = Rom {
            file_path: "/jogos/psx/Castlevania - Symphony of the Night (USA)/disc1.cue".into(),
            system_id: "psx".into(),
            ..Default::default()
        };
        assert_eq!(
            effective_rom_stem(&cue, "disc1"),
            "Castlevania - Symphony of the Night (USA)"
        );

        let multi_disc_subfolder = Rom {
            file_path: "/jogos/psx/Final Fantasy VII (USA)/Disc 1/track01.bin".into(),
            system_id: "psx".into(),
            ..Default::default()
        };
        assert_eq!(
            effective_rom_stem(&multi_disc_subfolder, "track01"),
            "Final Fantasy VII (USA)"
        );

        let serial_only = Rom {
            file_path: "/jogos/ps2/God of War (USA)/SLUS_211.15.iso".into(),
            system_id: "ps2".into(),
            ..Default::default()
        };
        assert_eq!(
            effective_rom_stem(&serial_only, "SLUS_211.15"),
            "God of War (USA)"
        );
    }

    #[test]
    fn title_match_tier_handles_punctuation_diacritics_articles_and_numerals() {
        assert_eq!(
            title_match_tier(
                "Castlevania - Symphony of the Night (USA)",
                "Castlevania: Symphony of the Night"
            ),
            Some(3)
        );
        assert_eq!(
            title_match_tier(
                "Legend of Zelda, The - A Link to the Past (USA)",
                "The Legend of Zelda: A Link to the Past"
            ),
            Some(3)
        );
        assert_eq!(
            title_match_tier(
                "Pokemon - Emerald Version (USA, Europe)",
                "Pokémon Emerald Version"
            ),
            Some(3)
        );
        assert_eq!(
            title_match_tier("Sonic 3 & Knuckles (World)", "Sonic 3 and Knuckles"),
            Some(3)
        );
        assert_eq!(
            title_match_tier("Final Fantasy III (USA)", "Final Fantasy 3"),
            Some(3)
        );
        assert_eq!(
            title_match_tier("Legend of Zelda (USA)", "The Legend of Zelda"),
            Some(2)
        );
        assert_eq!(title_match_tier("Super Mario", "Super Mario World"), None);
    }

    #[test]
    fn detects_rom_regions_qualifiers_and_picks_regional_covers() {
        assert_eq!(
            detect_rom_region("/roms/snes/Chrono Trigger (USA).sfc", None, None),
            Some("NA")
        );
        assert_eq!(
            detect_rom_region("/roms/snes/Chrono Trigger (Japan).sfc", None, None),
            Some("JP")
        );
        assert_eq!(
            detect_rom_region("/roms/snes/Terranigma (Europe).sfc", None, None),
            Some("EU")
        );
        assert_eq!(
            detect_rom_region("/roms/sms/Turma da Monica (Brazil).sms", None, None),
            Some("BR")
        );
        assert_eq!(
            detect_rom_region(
                "/roms/gba/Pokemon - Emerald Version (USA, Europe).gba",
                None,
                None
            ),
            Some("WORLD")
        );
        // Tags de revisão/lado não devem ser confundidas com códigos de 1 letra como "(A)" = Austrália.
        assert_eq!(
            detect_rom_region("/roms/nes/Metroid (Rev A).nes", None, None),
            None
        );
        assert_eq!(
            detect_rom_region("/roms/ps2/game.iso", None, Some("SLPS-25088")),
            Some("JP")
        );
        assert_eq!(
            detect_rom_region("/roms/gc/GALE01 - Melee.iso", None, None),
            Some("NA")
        );
        assert_eq!(
            extract_rom_qualifier(
                "/roms/atari2600/128-in-1 Junior Console (Chip 2) (PAL) [!].a26",
                None,
                None,
            )
            .as_deref(),
            Some("Chip 2")
        );

        let mut covers = std::collections::BTreeMap::new();
        covers.insert("JP".into(), "https://cdn.gamekave.net/jp.webp".into());
        covers.insert("EU".into(), "https://cdn.gamekave.net/eu.webp".into());
        assert_eq!(
            pick_regional_cover(
                &covers,
                Some("JP"),
                Some("https://cdn.gamekave.net/na.webp")
            ),
            Some("https://cdn.gamekave.net/jp.webp")
        );
        assert_eq!(
            pick_regional_cover(
                &covers,
                Some("AU"),
                Some("https://cdn.gamekave.net/na.webp")
            ),
            Some("https://cdn.gamekave.net/eu.webp")
        );
        assert_eq!(
            pick_regional_cover(
                &covers,
                Some("NA"),
                Some("https://cdn.gamekave.net/na.webp")
            ),
            Some("https://cdn.gamekave.net/na.webp")
        );

        // Nunca mantém `box-texture` como fallback quando há capa frontal disponível.
        assert_eq!(
            pick_regional_cover(
                &covers,
                None,
                Some(
                    "https://cdn.gamekave.net/games/g/cover/screenscraper/box-texture_0af13c71.png"
                )
            ),
            Some("https://cdn.gamekave.net/eu.webp")
        );
    }

    #[test]
    fn ranks_front_and_square_covers_ahead_of_box_texture_and_box_3d() {
        let wrap = "https://cdn.gamekave.net/games/g/cover/screenscraper/box-texture_0af13c71.png";
        let box3d = "https://cdn.gamekave.net/games/g/cover/screenscraper/box-3D_b4c06009.png";
        let box2d = "https://cdn.gamekave.net/games/g/cover/screenscraper/box-2D_d75bd986.png";
        let cover = "https://cdn.gamekave.net/games/g/cover/screenscraper/cover_d5a30ce5.png";
        let igdb = "https://cdn.gamekave.net/games/g/covers/igdb/co8lo8.jpg";

        assert!(is_non_front_cover_url(wrap));
        assert!(is_non_front_cover_url(box3d));
        assert!(!is_non_front_cover_url(box2d));
        assert!(!is_non_front_cover_url(cover));
        assert!(!is_non_front_cover_url(igdb));

        assert_eq!(
            pick_best_cover_url([wrap, box3d, box2d, cover], Some("atari2600"), Some(cover)),
            Some(cover)
        );
        // Em plataformas de caixa quadrada (GB/GBA/PSX/Dreamcast/NDS), prefere `cover`/`box-2D` (1:1).
        assert_eq!(
            pick_best_cover_url([wrap, igdb, cover], Some("gba"), Some(igdb)),
            Some(cover)
        );
    }
}
