//! Mapa teclado → RetroPad (porta 0). Os `scancode` são valores físicos do
//! SO — o default aqui usa os do padrão `KeyCode` do winit/Tauri (W3C
//! `KeyboardEvent.code` numerados). A UI de binding (etapa 05) sobrescreve.

use domain::input::RetroPadButton;
use std::collections::BTreeMap;

/// Scancodes W3C usados no default (mesmos que winit `KeyCode as u32` expõe
/// via `to_scancode` não — aqui é um id estável nosso; a ponte winit converte).
pub mod sc {
    pub const ARROW_UP: u32 = 1;
    pub const ARROW_DOWN: u32 = 2;
    pub const ARROW_LEFT: u32 = 3;
    pub const ARROW_RIGHT: u32 = 4;
    pub const KEY_Z: u32 = 10;
    pub const KEY_X: u32 = 11;
    pub const KEY_A: u32 = 12;
    pub const KEY_S: u32 = 13;
    pub const KEY_Q: u32 = 14;
    pub const KEY_W: u32 = 15;
    pub const ENTER: u32 = 20;
    pub const SHIFT_RIGHT: u32 = 21;
}

/// `KeyboardEvent.code` (W3C, o que a webview manda) → RetroPad, porta 0.
/// Default do app enquanto não há UI de binding.
pub fn web_code_to_retropad(code: &str) -> Option<(u8, RetroPadButton)> {
    use RetroPadButton::*;
    Some((
        0,
        match code {
            "ArrowUp" => Up,
            "ArrowDown" => Down,
            "ArrowLeft" => Left,
            "ArrowRight" => Right,
            "KeyZ" => B,
            "KeyX" => A,
            "KeyA" => Y,
            "KeyS" => X,
            "KeyQ" => L1,
            "KeyW" => R1,
            "Enter" => Start,
            "ShiftRight" | "ShiftLeft" => Select,
            _ => return None,
        },
    ))
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

#[derive(Debug, Clone)]
pub struct KeyboardMap {
    map: BTreeMap<u32, (u8, RetroPadButton)>,
}

impl KeyboardMap {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    pub fn bind(&mut self, scancode: u32, port: u8, button: RetroPadButton) {
        self.map.insert(scancode, (port, button));
    }

    pub fn resolve(&self, scancode: u32) -> Option<(u8, RetroPadButton)> {
        self.map.get(&scancode).copied()
    }

    pub fn entries(&self) -> impl Iterator<Item = (u32, u8, RetroPadButton)> + '_ {
        self.map.iter().map(|(k, (p, b))| (*k, *p, *b))
    }
}

impl Default for KeyboardMap {
    /// Layout comum: setas = d-pad, Z/X = B/A, A/S = Y/X, Q/W = L1/R1,
    /// Enter = Start, Shift direito = Select.
    fn default() -> Self {
        use RetroPadButton::*;
        let mut m = Self::new();
        for (k, b) in [
            (sc::ARROW_UP, Up),
            (sc::ARROW_DOWN, Down),
            (sc::ARROW_LEFT, Left),
            (sc::ARROW_RIGHT, Right),
            (sc::KEY_Z, B),
            (sc::KEY_X, A),
            (sc::KEY_A, Y),
            (sc::KEY_S, X),
            (sc::KEY_Q, L1),
            (sc::KEY_W, R1),
            (sc::ENTER, Start),
            (sc::SHIFT_RIGHT, Select),
        ] {
            m.bind(k, 0, b);
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_covers_dpad_and_face() {
        let m = KeyboardMap::default();
        assert_eq!(m.resolve(sc::ARROW_UP), Some((0, RetroPadButton::Up)));
        assert_eq!(m.resolve(sc::KEY_Z), Some((0, RetroPadButton::B)));
        assert_eq!(m.resolve(sc::ENTER), Some((0, RetroPadButton::Start)));
        assert_eq!(m.resolve(999), None);
        assert_eq!(m.entries().count(), 12);
    }

    #[test]
    fn rebind_overrides() {
        let mut m = KeyboardMap::default();
        m.bind(sc::KEY_Z, 1, RetroPadButton::A);
        assert_eq!(m.resolve(sc::KEY_Z), Some((1, RetroPadButton::A)));
    }

    #[test]
    fn key_scancode_is_stable_and_distinct() {
        assert_eq!(key_scancode("Escape"), key_scancode("Escape"));
        assert_ne!(key_scancode("Escape"), key_scancode("F1"));
        assert_ne!(key_scancode(""), key_scancode("F1"));
    }

    #[test]
    fn web_codes() {
        assert_eq!(web_code_to_retropad("KeyZ"), Some((0, RetroPadButton::B)));
        assert_eq!(
            web_code_to_retropad("ArrowLeft"),
            Some((0, RetroPadButton::Left))
        );
        assert_eq!(web_code_to_retropad("Space"), None);
    }
}
