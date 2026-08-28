//! Parser do formato SDL_GameControllerDB.
//!
//! Uma linha é `GUID,Nome,<binding>,<binding>,...,platform:<Plataforma>`,
//! onde cada binding é `<sdlkey>:<source>` — `source` sendo `bN` (botão),
//! `hN.M` (hat/dpad, M = bitmask 1/2/4/8) ou `aN`/`-aN`/`+aN`/`aN~` (eixo).
//!
//! Convenção Nintendo↔Xbox (libretro): SDL `a`→RetroPad B, `b`→A, `x`→Y,
//! `y`→X.

use domain::input::RetroPadButton;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SdlDbError {
    #[error("linha malformada (falta guid/nome)")]
    Malformed,
    #[error("source inválido: {0:?}")]
    BadSource(String),
}

/// De onde vem um botão RetroPad no gamepad físico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadSource {
    Button(u32),
    /// hat `index`, direção = bitmask (1 up, 2 right, 4 down, 8 left).
    Hat {
        index: u32,
        mask: u32,
    },
    /// eixo `index`; `positive` = metade positiva (ex: gatilho).
    Axis {
        index: u32,
        positive: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMapping {
    pub guid: String,
    pub name: String,
    pub platform: Option<String>,
    /// RetroPad → onde está no gamepad. Só as chaves que traduzem pra RetroPad.
    pub bindings: Vec<(RetroPadButton, GamepadSource)>,
}

impl ParsedMapping {
    pub fn source_for(&self, button: RetroPadButton) -> Option<GamepadSource> {
        self.bindings
            .iter()
            .find(|(b, _)| *b == button)
            .map(|(_, s)| *s)
    }
}

fn sdl_key_to_retropad(key: &str) -> Option<RetroPadButton> {
    use RetroPadButton::*;
    Some(match key {
        "a" => B,
        "b" => A,
        "x" => Y,
        "y" => X,
        "back" => Select,
        "start" => Start,
        "leftshoulder" => L1,
        "rightshoulder" => R1,
        "lefttrigger" => L2,
        "righttrigger" => R2,
        "leftstick" => L3,
        "rightstick" => R3,
        "dpup" => Up,
        "dpdown" => Down,
        "dpleft" => Left,
        "dpright" => Right,
        _ => return None, // guide, misc, paddles, touchpad, sticks analógicos...
    })
}

fn parse_source(raw: &str) -> Result<GamepadSource, SdlDbError> {
    let err = || SdlDbError::BadSource(raw.to_string());
    let s = raw.trim_end_matches('~'); // sufixo de inversão de eixo
    if let Some(rest) = s.strip_prefix('b') {
        Ok(GamepadSource::Button(rest.parse().map_err(|_| err())?))
    } else if let Some(rest) = s.strip_prefix('h') {
        let (idx, mask) = rest.split_once('.').ok_or_else(err)?;
        Ok(GamepadSource::Hat {
            index: idx.parse().map_err(|_| err())?,
            mask: mask.parse().map_err(|_| err())?,
        })
    } else {
        let (positive, rest) = match s.strip_prefix('+') {
            Some(r) => (true, r),
            None => match s.strip_prefix('-') {
                Some(r) => (false, r),
                None => (true, s),
            },
        };
        let rest = rest.strip_prefix('a').ok_or_else(err)?;
        Ok(GamepadSource::Axis {
            index: rest.parse().map_err(|_| err())?,
            positive,
        })
    }
}

pub fn parse_mapping(line: &str) -> Result<ParsedMapping, SdlDbError> {
    let line = line.trim();
    let mut parts = line.split(',');
    let guid = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or(SdlDbError::Malformed)?;
    let name = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or(SdlDbError::Malformed)?;

    let mut platform = None;
    let mut bindings = Vec::new();
    for field in parts {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }
        let Some((key, src)) = field.split_once(':') else {
            continue;
        };
        if key == "platform" {
            platform = Some(src.to_string());
            continue;
        }
        if let Some(rp) = sdl_key_to_retropad(key) {
            match parse_source(src) {
                Ok(source) => bindings.push((rp, source)),
                Err(e) => log::warn!("{name}: {key}:{src} — {e}"),
            }
        }
    }

    Ok(ParsedMapping {
        guid: guid.to_string(),
        name: name.to_string(),
        platform,
        bindings,
    })
}

/// Varre um `gamecontrollerdb.txt` inteiro (ignora comentários/linhas vazias),
/// mantendo, por GUID, a última entrada da plataforma dada (ou qualquer uma).
pub fn parse_db<'a>(text: &'a str, platform: &str) -> impl Iterator<Item = ParsedMapping> + 'a {
    let platform = platform.to_string();
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| parse_mapping(l).ok())
        .filter(move |m| m.platform.as_deref() == Some(platform.as_str()) || m.platform.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use RetroPadButton::*;

    const XBOX: &str = "030000005e0400008e02000010010000,Xbox 360 Controller,\
a:b0,b:b1,x:b2,y:b3,back:b6,start:b7,leftshoulder:b4,rightshoulder:b5,\
leftstick:b9,rightstick:b10,dpup:h0.1,dpright:h0.2,dpdown:h0.4,dpleft:h0.8,\
lefttrigger:a2,righttrigger:a5,leftx:a0,lefty:a1,guide:b8,platform:Linux";

    #[test]
    fn parses_buttons_hats_axes_nintendo_swap() {
        let m = parse_mapping(XBOX).unwrap();
        assert_eq!(m.name, "Xbox 360 Controller");
        assert_eq!(m.platform.as_deref(), Some("Linux"));
        // swap Nintendo: SDL a -> RetroPad B
        assert_eq!(m.source_for(B), Some(GamepadSource::Button(0)));
        assert_eq!(m.source_for(A), Some(GamepadSource::Button(1)));
        assert_eq!(m.source_for(Y), Some(GamepadSource::Button(2)));
        assert_eq!(m.source_for(X), Some(GamepadSource::Button(3)));
        assert_eq!(m.source_for(Select), Some(GamepadSource::Button(6)));
        assert_eq!(
            m.source_for(Up),
            Some(GamepadSource::Hat { index: 0, mask: 1 })
        );
        assert_eq!(
            m.source_for(Left),
            Some(GamepadSource::Hat { index: 0, mask: 8 })
        );
        assert_eq!(
            m.source_for(L2),
            Some(GamepadSource::Axis {
                index: 2,
                positive: true
            })
        );
        // 16 chaves RetroPad (a/b/x/y, back, start, 2 shoulders, 2 triggers,
        // 2 sticks, 4 dpad); guide/leftx/lefty/platform não contam.
        assert_eq!(m.bindings.len(), 16);
        assert_eq!(
            m.source_for(RetroPadButton::L3),
            Some(GamepadSource::Button(9))
        );
    }

    #[test]
    fn rejects_malformed() {
        assert_eq!(parse_mapping("").unwrap_err(), SdlDbError::Malformed);
        assert_eq!(
            parse_mapping("onlyguid").unwrap_err(),
            SdlDbError::Malformed
        );
    }

    #[test]
    fn axis_sign_prefixes() {
        assert_eq!(
            parse_source("+a3"),
            Ok(GamepadSource::Axis {
                index: 3,
                positive: true
            })
        );
        assert_eq!(
            parse_source("-a3"),
            Ok(GamepadSource::Axis {
                index: 3,
                positive: false
            })
        );
        assert_eq!(
            parse_source("a3~"),
            Ok(GamepadSource::Axis {
                index: 3,
                positive: true
            })
        );
    }

    #[test]
    fn db_filters_by_platform() {
        let db = format!("# comment\n\n{XBOX}\nAAA,Win Pad,a:b0,platform:Windows\n");
        let linux: Vec<_> = parse_db(&db, "Linux").collect();
        assert_eq!(linux.len(), 1);
        assert_eq!(linux[0].name, "Xbox 360 Controller");
    }
}
