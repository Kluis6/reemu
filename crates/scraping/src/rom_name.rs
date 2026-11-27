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
}
