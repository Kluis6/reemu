//! De onde vêm os eventos de controle físico. O [`crate::GamepadPoller`] só
//! enxerga o vocabulário daqui (botões e eixos já normalizados, convenção do
//! `gilrs`: eixo Y+ = cima, gatilho `0..=1`); a fonte muda por sistema:
//!
//! - **Windows: SDL3** ([`sdl`]). O `gilrs` usa o Windows.Gaming.Input, que
//!   numa máquina de teste (Windows 11 26H2, build 26300) deixou de listar o
//!   DualSense — a API antiga (winmm) e o driver enxergavam o controle, o WGI
//!   não (2026-10-24). O SDL lê PlayStation/Switch/Xbox direto pelo HIDAPI e
//!   o resto por XInput/RawInput/DirectInput, sem o WGI ("WGI is not used
//!   (default)", `SDL_HINT_JOYSTICK_WGI`, SDL_hints.h 3.4.18).
//! - **Linux/macOS: `gilrs`** ([`gilrs`]), como sempre foi.

#[cfg(not(windows))]
mod gilrs;
#[cfg(windows)]
mod sdl;

#[cfg(not(windows))]
pub use self::gilrs::{init_on_main_thread, Source};
#[cfg(windows)]
pub use self::sdl::{init_on_main_thread, Source};

/// Botão normalizado. Os nomes de face são POSIÇÃO (South = de baixo: A no
/// Xbox, ✕ no PlayStation), como no `gilrs` e no SDL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

/// Eixo normalizado, `-1.0..=1.0`, Y+ = cima. `DPadX`/`DPadY`: d-pad que chega
/// como eixo (hat) em vez de botões.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    LeftStickX,
    LeftStickY,
    RightStickX,
    RightStickY,
    DPadX,
    DPadY,
}

/// Uma conexão física (não o modelo: duas unidades iguais têm ids distintos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PadId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PadEvent {
    Connected,
    Disconnected,
    AxisChanged(Axis, f32),
    /// Valor analógico de um botão — só L2/R2 interessam ao poller, `0..=1`.
    ButtonChanged(Button, f32),
    ButtonPressed(Button),
    ButtonReleased(Button),
}
