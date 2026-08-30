//! Importador de pacotes de decoração (bezels/molduras) no formato do
//! **The Bezel Project** / RetroBat.
//!
//! O scan é **profundo** e tolerante a estrutura: varre `.png` em qualquer
//! profundidade e classifica pelo caminho.
//!
//! - `.png` com stem `default` → `default`.
//! - `.png` com `.cfg` irmão, ou sob pasta de jogos (`games`, `GameBezels`…)
//!   → bezel de **jogo** (`Rom`).
//! - `.png` cujo stem == pasta-ancestral que parece um sistema → **sistema**.
//! - `.png` sob uma pasta que parece um sistema → **jogo** daquele sistema.
//!
//! O `system` do bezel de jogo pode ser `None` (o importador casa o stem
//! contra qualquer ROM). Lê o `.cfg` irmão pro `custom_viewport_*`.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Retângulo do jogo dentro da moldura, em pixels da imagem (do `.cfg`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecoScope {
    Default,
    System(String),
    /// `system` = `None` quando não deu pra inferir do caminho.
    Rom {
        system: Option<String>,
        stem: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedDecoration {
    pub scope: DecoScope,
    pub asset_path: PathBuf,
}

/// Nomes de pasta comuns em packs de bezel → `system_id` canônico do ReEmu.
pub fn system_from_folder_name(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    let n = n.trim();
    Some(match n {
        "nes"
        | "famicom"
        | "fc"
        | "nintendo entertainment system"
        | "nintendo - nintendo entertainment system" => "nes",
        "snes"
        | "sfc"
        | "super famicom"
        | "super nintendo"
        | "supernintendo"
        | "super nintendo entertainment system"
        | "nintendo - super nintendo entertainment system" => "snes",
        "gb" | "gameboy" | "game boy" | "nintendo - game boy" => "gb",
        "gbc" | "game boy color" | "gameboycolor" | "nintendo - game boy color" => "gbc",
        "gba" | "game boy advance" | "gameboyadvance" | "nintendo - game boy advance" => "gba",
        "n64" | "nintendo 64" | "nintendo64" | "nintendo - nintendo 64" => "n64",
        "genesis"
        | "megadrive"
        | "mega drive"
        | "sega genesis"
        | "sega mega drive"
        | "sega - mega drive - genesis"
        | "md" => "megadrive",
        "sms"
        | "mastersystem"
        | "master system"
        | "sega master system"
        | "sega - master system - mark iii" => "mastersystem",
        "gg" | "gamegear" | "game gear" | "sega game gear" | "sega - game gear" => "gamegear",
        "32x" | "sega32x" | "sega 32x" | "sega - 32x" => "sega32x",
        "pce"
        | "pcengine"
        | "pc engine"
        | "turbografx"
        | "turbografx-16"
        | "turbografx 16"
        | "tg16"
        | "nec - pc engine - turbografx 16" => "pcengine",
        "atari2600" | "atari 2600" | "2600" | "atari - 2600" => "atari2600",
        "lynx" | "atari lynx" | "atari - lynx" => "lynx",
        "wonderswan" | "ws" | "bandai - wonderswan" => "wonderswan",
        "ngp" | "neo geo pocket" | "neogeopocket" | "snk - neo geo pocket" => "ngp",
        _ => return None,
    })
}

const GAME_DIRS: &[&str] = &[
    "games",
    "gamebezels",
    "game_bezels",
    "named_bezels",
    "named_boxarts",
    "roms",
];

/// Varre a pasta de um pacote (qualquer profundidade).
pub fn scan_decoration_pack(base: &Path) -> Vec<ScannedDecoration> {
    let mut out = Vec::new();
    for entry in WalkDir::new(base)
        .max_depth(12)
        .into_iter()
        .filter_map(Result::ok)
    {
        let p = entry.path();
        if !p.is_file()
            || p.extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                != Some("png".into())
        {
            continue;
        }
        let Ok(rel) = p.strip_prefix(base) else {
            continue;
        };
        let Some(scope) = classify(rel, p) else {
            continue;
        };
        // O `.cfg` (viewport) é lido depois, só pra imagem que casar com um
        // jogo — evita ~60k stats num pack completo do Bezel Project.
        out.push(ScannedDecoration {
            scope,
            asset_path: p.to_path_buf(),
        });
    }
    out.sort_by(|a, b| a.asset_path.cmp(&b.asset_path));
    out
}

fn classify(rel: &Path, abs: &Path) -> Option<DecoScope> {
    let stem = abs.file_stem()?.to_str()?.to_string();
    // pastas ancestrais (da base até o arquivo, sem o arquivo)
    let dirs: Vec<String> = rel
        .parent()
        .map(|pp| {
            pp.iter()
                .filter_map(|c| c.to_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let dirs_lower: Vec<String> = dirs.iter().map(|d| d.to_ascii_lowercase()).collect();

    if stem.eq_ignore_ascii_case("default") {
        return Some(DecoScope::Default);
    }

    let in_game_dir = dirs_lower.iter().any(|d| GAME_DIRS.contains(&d.as_str()));
    // sistema mais "próximo" do arquivo entre as pastas ancestrais
    let sys_from_dirs = dirs
        .iter()
        .rev()
        .find_map(|d| system_from_folder_name(d).map(str::to_string));

    // bezel de sistema: stem == nome de pasta-ancestral que parece sistema
    if let Some(sid) = system_from_folder_name(&stem) {
        if dirs_lower.iter().any(|d| d == &stem.to_ascii_lowercase()) || dirs.is_empty() {
            return Some(DecoScope::System(sid.to_string()));
        }
    }

    // bezel de jogo: está numa pasta de jogos, sob um sistema, ou tem `.cfg`
    // irmão (só checa o disco nesse último caso).
    if in_game_dir || sys_from_dirs.is_some() || abs.with_extension("cfg").is_file() {
        return Some(DecoScope::Rom {
            system: sys_from_dirs,
            stem,
        });
    }

    // png solto na raiz do pack sem contexto → assume bezel de jogo
    if dirs.is_empty() {
        return Some(DecoScope::Rom { system: None, stem });
    }
    None
}

/// Viewport do `.cfg` irmão de uma imagem de bezel.
pub fn viewport_for_image(image_path: &Path) -> Option<Viewport> {
    read_viewport(&image_path.with_extension("cfg"))
}

fn read_viewport(cfg: &Path) -> Option<Viewport> {
    let text = std::fs::read_to_string(cfg).ok()?;
    let get = |key: &str| -> Option<i64> {
        text.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key)
                .then(|| {
                    v.trim()
                        .trim_matches('"')
                        .parse::<f64>()
                        .ok()
                        .map(|n| n as i64)
                })
                .flatten()
        })
    };
    Some(Viewport {
        x: get("custom_viewport_x")?,
        y: get("custom_viewport_y")?,
        w: get("custom_viewport_width")?,
        h: get("custom_viewport_height")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn deep_bezel_project_layout() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path();
        touch(&b.join("default.png"));
        // The Bezel Project: .../GameBezels/SNES/<Game>.png (+ .cfg)
        touch(&b.join("retroarch/overlay/GameBezels/SNES/Super Mario World.png"));
        touch(&b.join("retroarch/overlay/GameBezels/SNES/Super Mario World.cfg"));
        // system bezel bem fundo
        touch(&b.join("retroarch/overlay/borders/Nintendo - Super Nintendo Entertainment System/Nintendo - Super Nintendo Entertainment System.png"));

        let f = scan_decoration_pack(b);
        assert!(f.iter().any(|d| d.scope == DecoScope::Default));
        assert!(f.iter().any(|d| d.scope
            == DecoScope::Rom {
                system: Some("snes".into()),
                stem: "Super Mario World".into()
            }));
        assert!(f
            .iter()
            .any(|d| d.scope == DecoScope::System("snes".into())));
    }

    #[test]
    fn game_png_under_system_folder_without_cfg() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path();
        touch(&b.join("Sega Genesis/Sonic the Hedgehog.png"));
        let f = scan_decoration_pack(b);
        assert!(f.iter().any(|d| d.scope
            == DecoScope::Rom {
                system: Some("megadrive".into()),
                stem: "Sonic the Hedgehog".into()
            }));
    }

    #[test]
    fn reads_sibling_cfg_viewport() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path();
        touch(&b.join("snes/Zelda.png"));
        std::fs::write(
            b.join("snes/Zelda.cfg"),
            "custom_viewport_x = \"240\"\ncustom_viewport_y = \"0\"\ncustom_viewport_width = \"1440\"\ncustom_viewport_height = \"1080\"\n",
        )
        .unwrap();
        assert_eq!(
            viewport_for_image(&b.join("snes/Zelda.png")),
            Some(Viewport {
                x: 240,
                y: 0,
                w: 1440,
                h: 1080
            })
        );
    }
}
