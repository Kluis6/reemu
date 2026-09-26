//! Mapa teclado → RetroPad (porta 1). A webview manda o `KeyboardEvent.code`
//! (W3C, posição física da tecla); cada tecla pode acionar um ou mais alvos:
//! um botão do RetroPad ou uma direção de um dos analógicos.
//!
//! O padrão vem de [`DEFAULTS`]; o usuário sobrescreve por alvo em
//! Configurações › Controles (tabela `keyboard_bindings`), e o mapa efetivo
//! é trocado inteiro por [`set_overrides`].

use domain::input::RetroPadButton;
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

/// Direção de um analógico acionada por tecla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StickDir {
    Up,
    Down,
    Left,
    Right,
}

/// O que uma tecla aciona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyTarget {
    Button(RetroPadButton),
    /// `stick`: 0 = esquerdo, 1 = direito.
    Stick {
        stick: u8,
        dir: StickDir,
    },
}

/// Todos os alvos, na ordem da tela, com o nome estável usado no banco e no
/// frontend.
pub const TARGETS: &[(&str, KeyTarget)] = {
    use KeyTarget::{Button as B, Stick as S};
    use RetroPadButton::*;
    use StickDir as D;
    &[
        ("Up", B(Up)),
        ("Down", B(Down)),
        ("Left", B(Left)),
        ("Right", B(Right)),
        ("B", B(RetroPadButton::B)),
        ("A", B(RetroPadButton::A)),
        ("Y", B(RetroPadButton::Y)),
        ("X", B(RetroPadButton::X)),
        ("L1", B(L1)),
        ("R1", B(R1)),
        ("L2", B(L2)),
        ("R2", B(R2)),
        ("L3", B(L3)),
        ("R3", B(R3)),
        ("Start", B(Start)),
        ("Select", B(Select)),
        (
            "LStickUp",
            S {
                stick: 0,
                dir: D::Up,
            },
        ),
        (
            "LStickDown",
            S {
                stick: 0,
                dir: D::Down,
            },
        ),
        (
            "LStickLeft",
            S {
                stick: 0,
                dir: D::Left,
            },
        ),
        (
            "LStickRight",
            S {
                stick: 0,
                dir: D::Right,
            },
        ),
        (
            "RStickUp",
            S {
                stick: 1,
                dir: D::Up,
            },
        ),
        (
            "RStickDown",
            S {
                stick: 1,
                dir: D::Down,
            },
        ),
        (
            "RStickLeft",
            S {
                stick: 1,
                dir: D::Left,
            },
        ),
        (
            "RStickRight",
            S {
                stick: 1,
                dir: D::Right,
            },
        ),
    ]
};

/// Tecla padrão de cada alvo (`""` = sem tecla). Setas = d-pad E analógico
/// esquerdo: jogos que só leem o analógico (Dreamcast, N64, PS2) andam no
/// teclado sem configurar nada; os que só leem o d-pad ignoram o stick.
/// Gatilhos em E/R (no Dreamcast e no PS2 são o acelerador/freio).
pub const DEFAULTS: &[(&str, &str)] = &[
    ("Up", "ArrowUp"),
    ("Down", "ArrowDown"),
    ("Left", "ArrowLeft"),
    ("Right", "ArrowRight"),
    ("B", "KeyZ"),
    ("A", "KeyX"),
    ("Y", "KeyA"),
    ("X", "KeyS"),
    ("L1", "KeyQ"),
    ("R1", "KeyW"),
    ("L2", "KeyE"),
    ("R2", "KeyR"),
    ("L3", ""),
    ("R3", ""),
    ("Start", "Enter"),
    ("Select", "ShiftRight"),
    ("LStickUp", "ArrowUp"),
    ("LStickDown", "ArrowDown"),
    ("LStickLeft", "ArrowLeft"),
    ("LStickRight", "ArrowRight"),
    ("RStickUp", ""),
    ("RStickDown", ""),
    ("RStickLeft", ""),
    ("RStickRight", ""),
];

/// Alvo pelo nome estável.
pub fn target_by_name(name: &str) -> Option<KeyTarget> {
    TARGETS.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

fn default_code(name: &str) -> &'static str {
    DEFAULTS
        .iter()
        .find(|(n, _)| *n == name)
        .map_or("", |(_, c)| c)
}

/// Um alvo no mapa efetivo, pra tela de configuração.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub target: &'static str,
    /// `KeyboardEvent.code`; `""` = sem tecla.
    pub code: String,
    pub is_default: bool,
}

struct Effective {
    by_target: Vec<Binding>,
    by_code: HashMap<String, Vec<KeyTarget>>,
}

fn build(overrides: &[(String, String)]) -> Effective {
    let by_target: Vec<Binding> = TARGETS
        .iter()
        .map(|(name, _)| {
            let over = overrides.iter().find(|(t, _)| t == name).map(|(_, c)| c);
            Binding {
                target: name,
                code: over
                    .cloned()
                    .unwrap_or_else(|| default_code(name).to_string()),
                is_default: over.is_none(),
            }
        })
        .collect();
    let mut by_code: HashMap<String, Vec<KeyTarget>> = HashMap::new();
    for b in &by_target {
        if let (false, Some(t)) = (b.code.is_empty(), target_by_name(b.target)) {
            by_code.entry(b.code.clone()).or_default().push(t);
        }
    }
    Effective { by_target, by_code }
}

static EFFECTIVE: RwLock<Option<Effective>> = RwLock::new(None);

/// Troca as escolhas do usuário (`(alvo, code)`); o resto fica no padrão.
pub fn set_overrides(overrides: &[(String, String)]) {
    *EFFECTIVE.write().unwrap_or_else(|p| p.into_inner()) = Some(build(overrides));
}

fn with_effective<R>(f: impl FnOnce(&Effective) -> R) -> R {
    let guard = EFFECTIVE.read().unwrap_or_else(|p| p.into_inner());
    match guard.as_ref() {
        Some(e) => f(e),
        None => f(&build(&[])),
    }
}

/// Alvos acionados pela tecla `code` (vazio se nenhum).
pub fn resolve(code: &str) -> Vec<KeyTarget> {
    with_effective(|e| e.by_code.get(code).cloned().unwrap_or_default())
}

/// Mapa efetivo inteiro, na ordem de [`TARGETS`].
pub fn bindings() -> Vec<Binding> {
    with_effective(|e| e.by_target.clone())
}

/// Direções seguradas por stick (`[esquerdo, direito]`), pra montar a
/// posição do analógico a partir das teclas.
static HELD_DIRS: Mutex<[[bool; 4]; 2]> = Mutex::new([[false; 4]; 2]);

/// Registra uma direção de stick apertada/solta e devolve a posição do stick
/// no range libretro (X+ = direita, Y+ = baixo — libretro.h). Teclas opostas
/// juntas se anulam; diagonal vai ao máximo nos dois eixos.
pub fn stick_key(stick: u8, dir: StickDir, pressed: bool) -> (i16, i16) {
    let mut held = HELD_DIRS.lock().unwrap_or_else(|p| p.into_inner());
    let s = &mut held[usize::from(stick.min(1))];
    s[dir as usize] = pressed;
    let axis = |neg: bool, pos: bool| match (neg, pos) {
        (true, false) => -0x7fff,
        (false, true) => 0x7fff,
        _ => 0,
    };
    (
        axis(s[StickDir::Left as usize], s[StickDir::Right as usize]),
        axis(s[StickDir::Up as usize], s[StickDir::Down as usize]),
    )
}

/// Solta todas as direções (menu aberto, captura de binding).
pub fn release_sticks() {
    *HELD_DIRS.lock().unwrap_or_else(|p| p.into_inner()) = [[false; 4]; 2];
}

/// FNV-1a 32 bits do `KeyboardEvent.code`. A web não expõe scancode físico;
/// só precisamos de um id estável e consistente entre a captura de binding
/// (etapa 05) e a resolução de hotkey em runtime.
pub fn key_scancode(code: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in code.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_target_has_a_default_entry() {
        assert_eq!(TARGETS.len(), DEFAULTS.len());
        for (name, _) in TARGETS {
            assert!(DEFAULTS.iter().any(|(n, _)| n == name), "{name}");
        }
    }

    #[test]
    fn default_arrows_drive_dpad_and_left_stick() {
        let e = build(&[]);
        let up = e.by_code.get("ArrowUp").unwrap();
        assert!(up.contains(&KeyTarget::Button(RetroPadButton::Up)));
        assert!(up.contains(&KeyTarget::Stick {
            stick: 0,
            dir: StickDir::Up
        }));
        assert_eq!(
            e.by_code.get("KeyR").unwrap(),
            &vec![KeyTarget::Button(RetroPadButton::R2)]
        );
        assert!(!e.by_code.contains_key(""));
    }

    #[test]
    fn override_moves_a_target_to_another_key() {
        let e = build(&[("R2".into(), "Space".into()), ("L3".into(), "KeyC".into())]);
        assert!(!e.by_code.contains_key("KeyR"));
        assert_eq!(
            e.by_code.get("Space").unwrap(),
            &vec![KeyTarget::Button(RetroPadButton::R2)]
        );
        let r2 = e.by_target.iter().find(|b| b.target == "R2").unwrap();
        assert!(!r2.is_default);
        // desligar um alvo: code vazio
        let e = build(&[("LStickUp".into(), String::new())]);
        assert_eq!(
            e.by_code.get("ArrowUp").unwrap(),
            &vec![KeyTarget::Button(RetroPadButton::Up)]
        );
    }

    #[test]
    fn stick_keys_combine_and_cancel() {
        release_sticks();
        assert_eq!(stick_key(0, StickDir::Up, true), (0, -0x7fff));
        assert_eq!(stick_key(0, StickDir::Right, true), (0x7fff, -0x7fff));
        assert_eq!(stick_key(0, StickDir::Down, true), (0x7fff, 0));
        assert_eq!(stick_key(0, StickDir::Up, false), (0x7fff, 0x7fff));
        release_sticks();
        assert_eq!(stick_key(1, StickDir::Left, true), (-0x7fff, 0));
        release_sticks();
    }

    #[test]
    fn key_scancode_is_stable_and_distinct() {
        assert_eq!(key_scancode("Escape"), key_scancode("Escape"));
        assert_ne!(key_scancode("Escape"), key_scancode("F1"));
        assert_ne!(key_scancode(""), key_scancode("F1"));
    }
}
