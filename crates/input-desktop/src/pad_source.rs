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
pub use self::gilrs::{init_on_main_thread, set_driver_on_main_thread, Source};
#[cfg(windows)]
pub use self::sdl::{init_on_main_thread, set_driver_on_main_thread, Source};

/// Dá pra escolher o driver ([`Driver`]) neste sistema? Só no Windows — nos
/// outros a fonte é o `gilrs`, sem escolha.
pub const DRIVER_CHOICE_SUPPORTED: bool = cfg!(windows);

/// Driver de controle do SDL no Windows (Configurações › Controles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Driver {
    /// Padrão do SDL: HIDAPI (PlayStation, Switch, Xbox), XInput e
    /// DirectInput.
    #[default]
    Auto,
    /// Só XInput: controles Xbox e os virtuais (DS4Windows, DSX, ViGEm).
    XInput,
    /// Só DirectInput: controles genéricos.
    DirectInput,
    /// Só Windows.Gaming.Input — o caminho de antes do SDL (via `gilrs`).
    Wgi,
}

impl Driver {
    /// Chave guardada no `app-settings.json` e usada pelo frontend.
    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::XInput => "xinput",
            Self::DirectInput => "directinput",
            Self::Wgi => "wgi",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "auto" => Self::Auto,
            "xinput" => Self::XInput,
            "directinput" => Self::DirectInput,
            "wgi" => Self::Wgi,
            _ => return None,
        })
    }
}

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

#[cfg(test)]
mod tests {
    use super::Driver;

    #[test]
    fn driver_keys_round_trip() {
        for d in [
            Driver::Auto,
            Driver::XInput,
            Driver::DirectInput,
            Driver::Wgi,
        ] {
            assert_eq!(Driver::from_key(d.key()), Some(d));
        }
        // chave desconhecida (arquivo antigo/editado à mão) não vira driver
        assert_eq!(Driver::from_key(""), None);
        assert_eq!(Driver::default(), Driver::Auto);
    }
}
