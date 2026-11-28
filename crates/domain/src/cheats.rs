//! Cheats por código e parser do formato `.cht` (docs/ai-context/17-cheats-rewind.md,
//! fases 1 e 2). Quem entende o código é o core: o frontend entrega o texto pelo
//! `retro_cheat_set(index, enabled, code)` (libretro.h). Game Genie, Action
//! Replay, GameShark, CWCheat, conforme o core.
//!
//! O parser `.cht` abaixo é implementação própria (MIT), escrita do zero a partir
//! apenas do formato textual documentado (`cheats = N`, `cheat<i>_desc`,
//! `cheat<i>_code`, `cheat<i>_enable`).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// O que vai pro core: o código e se está ligado. A posição na lista é o
/// `index` do `retro_cheat_set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheatCode {
    pub code: String,
    pub enabled: bool,
}

/// Cheat salvo de um jogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cheat {
    pub id: i64,
    pub rom_id: String,
    pub description: String,
    pub code: String,
    pub enabled: bool,
    /// Ordem na lista (e o `index` do `retro_cheat_set`).
    pub position: i64,
}

impl Cheat {
    pub fn to_code(&self) -> CheatCode {
        CheatCode {
            code: self.code.clone(),
            enabled: self.enabled,
        }
    }
}

/// Entrada lida de um arquivo `.cht` pronta para importar no banco.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedCheat {
    pub description: String,
    pub code: String,
    pub enabled: bool,
}

#[derive(Default)]
struct RawChtSlot {
    desc: Option<String>,
    code: Option<String>,
    enable: Option<bool>,
}

/// Desfaz aspas externas `"..."` e sequências escapadas (`\"`, `\\`, `\n`, `\t`).
/// Valores sem aspas perdem comentários inline iniciados por `#`.
fn parse_cht_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(after_open) = trimmed.strip_prefix('"') {
        let mut out = String::with_capacity(after_open.len());
        let mut chars = after_open.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '\\' => match chars.next() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('n') | Some('r') | Some('t') => out.push(' '),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                },
                '"' => break,
                other => out.push(other),
            }
        }
        return out;
    }

    let without_comment = trimmed.split('#').next().unwrap_or(trimmed);
    without_comment.trim().to_string()
}

fn parse_cht_bool(raw: &str) -> Option<bool> {
    let v = parse_cht_value(raw);
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

/// Extrai `(índice, campo)` de chaves como `cheat0_desc`, `cheat12_code`, `cheat3_enable`.
fn split_cheat_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("cheat")?;
    let (idx_str, field) = rest.split_once('_')?;
    if idx_str.is_empty() || !idx_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let idx = idx_str.parse::<usize>().ok()?;
    Some((idx, field))
}

/// Lê um arquivo `.cht` em texto (UTF-8, CRLF ou LF) e devolve os cheats por
/// código em ordem de índice.
///
/// - Entradas sem código (`cheatN_code = ""`, comuns em cheats puramente de
///   endereço de memória da fase 4) são ignoradas.
/// - `enabled` assume `false` por padrão quando `cheatN_enable` está ausente ou
///   é `false`, evitando ligar dezenas de cheats de uma só vez ao importar.
/// - Descrições vazias recebem o próprio código como fallback.
pub fn parse_cht(input: &str) -> Vec<ParsedCheat> {
    let text = input.strip_prefix('\u{feff}').unwrap_or(input);
    let mut declared_count: Option<usize> = None;
    let mut slots: BTreeMap<usize, RawChtSlot> = BTreeMap::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((raw_key, raw_val)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        if key == "cheats" {
            let val = parse_cht_value(raw_val);
            if let Ok(n) = val.trim().parse::<usize>() {
                declared_count = Some(n);
            }
            continue;
        }

        let Some((idx, field)) = split_cheat_key(&key) else {
            continue;
        };
        let slot = slots.entry(idx).or_default();
        match field {
            "desc" => slot.desc = Some(parse_cht_value(raw_val)),
            "code" => slot.code = Some(parse_cht_value(raw_val)),
            "enable" => slot.enable = parse_cht_bool(raw_val),
            // Campos de memória (address, value, cheat_type, memory_search_size, etc.)
            // ficam reconhecidos sem efeito até a fase 4.
            _ => {}
        }
    }

    if let Some(limit) = declared_count {
        slots.retain(|&idx, _| idx < limit);
    }

    let mut out = Vec::with_capacity(slots.len());
    for slot in slots.into_values() {
        let code = slot.code.unwrap_or_default();
        let code = code.trim();
        if code.is_empty() || code.contains('\0') {
            continue;
        }
        let desc_raw = slot.desc.unwrap_or_default();
        let desc = desc_raw.trim();
        let description = if desc.is_empty() {
            code.to_string()
        } else {
            desc.to_string()
        };
        out.push(ParsedCheat {
            description,
            code: code.to_string(),
            enabled: slot.enable.unwrap_or(false),
        });
    }
    out
}

/// Filtra `incoming` removendo pares `(description, code)` já presentes em
/// `existing` ou repetidos dentro do próprio `incoming`.
pub fn deduplicate_parsed_cheats(
    existing: &[Cheat],
    incoming: impl IntoIterator<Item = ParsedCheat>,
) -> Vec<ParsedCheat> {
    let mut seen: HashSet<(String, String)> = existing
        .iter()
        .map(|c| (c.description.trim().to_string(), c.code.trim().to_string()))
        .collect();
    let mut unique = Vec::new();
    for item in incoming {
        let desc = item.description.trim().to_string();
        let code = item.code.trim().to_string();
        if code.is_empty() {
            continue;
        }
        let desc = if desc.is_empty() { code.clone() } else { desc };
        if seen.insert((desc.clone(), code.clone())) {
            unique.push(ParsedCheat {
                description: desc,
                code,
                enabled: item.enabled,
            });
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_libretro_cht_with_crlf_comments_and_escapes() {
        let sample = "\u{feff}# Comentário no topo\r\n\
            cheats = 4\r\n\
            \r\n\
            cheat0_desc = \"P1/Team 1 Has \\\"Lots\\\" of Money\"\r\n\
            cheat0_code = \"7F80CAFF+7F80CCFF+7F80CBFF\"\r\n\
            cheat0_enable = false\r\n\
            \r\n\
            ; Entrada só de memória (sem código): deve ser pulada\r\n\
            cheat1_desc = \"Infinite Lives (Memory)\"\r\n\
            cheat1_code = \"\"\r\n\
            cheat1_address = \"2024\"\r\n\
            cheat1_value = \"99\"\r\n\
            cheat1_cheat_type = \"1\"\r\n\
            cheat1_enable = true\r\n\
            \r\n\
            cheat2_desc = \"   \"\r\n\
            cheat2_code = \"  7E0DBF09  \"\r\n\
            cheat2_enable = \"true\"\r\n\
            \r\n\
            cheat3_desc = \"Invencibilidade\"\r\n\
            cheat3_code = \"DD64-6D07\"\r\n\
            \r\n\
            # Índice >= cheats (4) deve ser ignorado\r\n\
            cheat4_desc = \"Fora do limite\"\r\n\
            cheat4_code = \"AAAA-BBBB\"\r\n";

        let parsed = parse_cht(sample);
        assert_eq!(
            parsed,
            vec![
                ParsedCheat {
                    description: "P1/Team 1 Has \"Lots\" of Money".into(),
                    code: "7F80CAFF+7F80CCFF+7F80CBFF".into(),
                    enabled: false,
                },
                ParsedCheat {
                    description: "7E0DBF09".into(),
                    code: "7E0DBF09".into(),
                    enabled: true,
                },
                ParsedCheat {
                    description: "Invencibilidade".into(),
                    code: "DD64-6D07".into(),
                    enabled: false,
                },
            ]
        );
    }

    #[test]
    fn parses_cht_without_declared_count_and_handles_zero_count() {
        let without_count = "cheat2_desc = \"Segundo\"\ncheat2_code = \"BB\"\n\
                             cheat0_desc = \"Primeiro\"\ncheat0_code = \"AA\"\n";
        let parsed = parse_cht(without_count);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].description, "Primeiro");
        assert_eq!(parsed[1].description, "Segundo");
        assert!(!parsed[0].enabled);

        let zero_count = "cheats = 0\ncheat0_desc = \"X\"\ncheat0_code = \"AA\"\n";
        assert!(parse_cht(zero_count).is_empty());
    }

    #[test]
    fn deduplicates_against_existing_and_internal_duplicates() {
        let existing = vec![Cheat {
            id: 1,
            rom_id: "rom-1".into(),
            description: "Infinite Lives".into(),
            code: "7E0DBF09".into(),
            enabled: true,
            position: 0,
        }];
        let incoming = vec![
            ParsedCheat {
                description: "Infinite Lives".into(),
                code: "7E0DBF09".into(),
                enabled: false,
            },
            ParsedCheat {
                description: "Moon Jump".into(),
                code: "7E001901".into(),
                enabled: false,
            },
            ParsedCheat {
                description: "Moon Jump".into(),
                code: "7E001901".into(),
                enabled: true,
            },
            ParsedCheat {
                description: "Infinite Lives".into(),
                code: "7E0DBF63".into(), // código diferente: entra!
                enabled: false,
            },
        ];

        let unique = deduplicate_parsed_cheats(&existing, incoming);
        assert_eq!(
            unique,
            vec![
                ParsedCheat {
                    description: "Moon Jump".into(),
                    code: "7E0DBF09".replace("0DBF09", "001901"),
                    enabled: false,
                },
                ParsedCheat {
                    description: "Infinite Lives".into(),
                    code: "7E0DBF63".into(),
                    enabled: false,
                },
            ]
        );
    }
}
