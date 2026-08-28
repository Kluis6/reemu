//! Estado do RetroPad — o que o callback `retro_input_state_t` lê a cada
//! `retro_run`. Global (como o resto do `ffi_state`), atômico, escrito pela
//! camada de input (`input-desktop` / comandos Tauri) e lido pelo core.

use domain::input::RetroPadButton;
use std::sync::atomic::{AtomicU16, Ordering};

/// `RETRO_DEVICE_ID_JOYPAD_*` — ordem fixa da API libretro.
pub fn libretro_joypad_id(button: RetroPadButton) -> u32 {
    use RetroPadButton::*;
    match button {
        B => 0,
        Y => 1,
        Select => 2,
        Start => 3,
        Up => 4,
        Down => 5,
        Left => 6,
        Right => 7,
        A => 8,
        X => 9,
        L1 => 10,
        R1 => 11,
        L2 => 12,
        R2 => 13,
        L3 => 14,
        R3 => 15,
    }
}

const MAX_PORTS: usize = 4;

pub struct RetroPadState {
    // bit i (por `libretro_joypad_id`) = pressionado, um u16 por porta
    ports: [AtomicU16; MAX_PORTS],
}

impl RetroPadState {
    const fn new() -> Self {
        Self {
            ports: [
                AtomicU16::new(0),
                AtomicU16::new(0),
                AtomicU16::new(0),
                AtomicU16::new(0),
            ],
        }
    }

    pub fn set(&self, port: usize, button: RetroPadButton, pressed: bool) {
        let Some(slot) = self.ports.get(port) else {
            return;
        };
        let bit = 1u16 << libretro_joypad_id(button);
        if pressed {
            slot.fetch_or(bit, Ordering::Relaxed);
        } else {
            slot.fetch_and(!bit, Ordering::Relaxed);
        }
    }

    pub fn is_pressed(&self, port: usize, button: RetroPadButton) -> bool {
        self.ports
            .get(port)
            .map(|s| s.load(Ordering::Relaxed) & (1u16 << libretro_joypad_id(button)) != 0)
            .unwrap_or(false)
    }

    /// Consulta crua pelo id libretro (o que o callback usa).
    pub(crate) fn query_id(&self, port: usize, id: u32) -> bool {
        id < 16
            && self
                .ports
                .get(port)
                .map(|s| s.load(Ordering::Relaxed) & (1u16 << id) != 0)
                .unwrap_or(false)
    }

    pub fn clear(&self) {
        for p in &self.ports {
            p.store(0, Ordering::Relaxed);
        }
    }
}

static PAD: RetroPadState = RetroPadState::new();

/// Estado global do RetroPad. Escreva aqui pra enviar input pro core.
pub fn retropad() -> &'static RetroPadState {
    &PAD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_query() {
        let p = RetroPadState::new();
        assert!(!p.is_pressed(0, RetroPadButton::A));
        p.set(0, RetroPadButton::A, true);
        assert!(p.is_pressed(0, RetroPadButton::A));
        assert!(p.query_id(0, libretro_joypad_id(RetroPadButton::A)));
        assert!(!p.query_id(1, libretro_joypad_id(RetroPadButton::A)));
        p.set(0, RetroPadButton::A, false);
        assert!(!p.is_pressed(0, RetroPadButton::A));
    }

    #[test]
    fn ports_are_independent() {
        let p = RetroPadState::new();
        p.set(1, RetroPadButton::Start, true);
        assert!(p.is_pressed(1, RetroPadButton::Start));
        assert!(!p.is_pressed(0, RetroPadButton::Start));
        p.set(99, RetroPadButton::Start, true); // fora do range: no-op
    }
}
