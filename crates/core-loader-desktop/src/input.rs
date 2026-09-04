//! Estado do RetroPad — o que o callback `retro_input_state_t` lê a cada
//! `retro_run`. Global (como o resto do `ffi_state`), atômico, escrito pela
//! camada de input (`input-desktop` / comandos Tauri) e lido pelo core.

use domain::input::RetroPadButton;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, Ordering};

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

/// Eixos analógicos do RetroPad (`RETRO_DEVICE_ANALOG`). Um par `(x, y)` por
/// stick (0 = esquerdo, 1 = direito) e por porta, no range libretro
/// `[-0x8000, 0x7fff]`. `used` fica `true` assim que o core consulta o
/// analógico ao menos uma vez — a camada de gamepad usa isso pra decidir se o
/// stick esquerdo também vira d-pad (sistemas sem analógico) ou não (N64).
pub struct AnalogState {
    // packed: x nos 16 bits altos, y nos 16 baixos (ambos i16).
    axes: [[AtomicI32; 2]; MAX_PORTS],
    used: AtomicBool,
}

impl AnalogState {
    const fn new() -> Self {
        Self {
            axes: [
                [AtomicI32::new(0), AtomicI32::new(0)],
                [AtomicI32::new(0), AtomicI32::new(0)],
                [AtomicI32::new(0), AtomicI32::new(0)],
                [AtomicI32::new(0), AtomicI32::new(0)],
            ],
            used: AtomicBool::new(false),
        }
    }

    /// Grava a posição de um stick (`stick`: 0 = esquerdo, 1 = direito).
    pub fn set_stick(&self, port: usize, stick: usize, x: i16, y: i16) {
        if let Some(a) = self.axes.get(port).and_then(|p| p.get(stick)) {
            let packed = (i32::from(x) << 16) | i32::from(y as u16);
            a.store(packed, Ordering::Relaxed);
        }
    }

    /// Consulta um eixo (o que o callback usa). `id`: 0 = X, 1 = Y.
    pub(crate) fn axis(&self, port: usize, stick: usize, id: u32) -> i16 {
        let packed = self
            .axes
            .get(port)
            .and_then(|p| p.get(stick))
            .map(|a| a.load(Ordering::Relaxed))
            .unwrap_or(0);
        match id {
            0 => (packed >> 16) as i16,
            1 => packed as i16,
            _ => 0,
        }
    }

    /// O core consultou o analógico — o stick esquerdo deixa de virar d-pad.
    pub fn mark_used(&self) {
        self.used.store(true, Ordering::Relaxed);
    }

    pub fn is_used(&self) -> bool {
        self.used.load(Ordering::Relaxed)
    }

    pub fn clear(&self) {
        for p in &self.axes {
            for a in p {
                a.store(0, Ordering::Relaxed);
            }
        }
        self.used.store(false, Ordering::Relaxed);
    }
}

static ANALOG: AnalogState = AnalogState::new();

/// Estado global dos eixos analógicos. Escreva aqui pra enviar analógico pro core.
pub fn analog() -> &'static AnalogState {
    &ANALOG
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

    #[test]
    fn analog_pack_roundtrip() {
        let a = AnalogState::new();
        assert!(!a.is_used());
        a.set_stick(0, 0, -32768, 32767);
        assert_eq!(a.axis(0, 0, 0), -32768); // X
        assert_eq!(a.axis(0, 0, 1), 32767); // Y
        a.set_stick(2, 1, 1234, -5678);
        assert_eq!(a.axis(2, 1, 0), 1234);
        assert_eq!(a.axis(2, 1, 1), -5678);
        assert_eq!(a.axis(0, 1, 0), 0); // stick não escrito
        assert_eq!(a.axis(9, 0, 0), 0); // porta fora do range
        a.mark_used();
        assert!(a.is_used());
        a.clear();
        assert!(!a.is_used());
        assert_eq!(a.axis(2, 1, 0), 0);
    }
}
