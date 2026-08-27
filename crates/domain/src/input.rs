//! Reconhecimento de dispositivos de input (teclado/gamepad) e tradução
//! pro layout canônico RetroPad. Desktop usa gilrs + SDL_GameControllerDB;
//! Android cobre no MVP apenas Xbox/PlayStation via Bluetooth (decisão:
//! Abordagem A), com fluxo manual de binding para o resto.
//!
//! Captura de binding com combinação (janela hold+press) vale tanto pra
//! hotkeys de sistema (ver `hotkeys.rs`) quanto pra mapeamento de controle
//! — decisão revisada; originalmente só hotkeys suportavam combinação,
//! mapeamento de controle usava tecla única. `ControllerMapping::layout`
//! reflete isso: a chave é uma combinação (`Vec<RawInputEvent>`), não mais
//! um índice único. Combinação em ação de jogo é incomum e deve ser
//! tratada como recurso avançado na UI, não como fluxo padrão sugerido.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RetroPadButton {
    A, B, X, Y,
    L1, L2, L3, R1, R2, R3,
    Up, Down, Left, Right,
    Start, Select,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerMapping {
    pub guid: String,
    pub display_name: String,
    /// Um ou mais eventos brutos (combinação hold+press, igual a
    /// `HotkeyBinding::trigger`) -> botão RetroPad canônico. Mapeamento
    /// comum tem Vec de 1 elemento; combinação é opcional/avançado.
    pub layout: Vec<ControllerLayoutEntry>,
    pub source: MappingSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllerLayoutEntry {
    pub trigger: Vec<RawInputEvent>,
    pub button: RetroPadButton,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MappingSource {
    SdlGameControllerDb,
    BundledAndroid,
    UserOverride,
}

/// Evento bruto de input, usado tanto na resolução normal (gamepad -> RetroPad)
/// quanto na UI de captura de binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RawInputEvent {
    Keyboard { scancode: u32 },
    GamepadButton { device_guid: String, index: u32 },
    GamepadAxis { device_guid: String, index: u32, value: f32 },
}

pub trait ControllerMappingResolver: Send + Sync {
    fn resolve_mapping(&self, device_guid: &str) -> Option<ControllerMapping>;
}

pub trait InputManager: Send + Sync {
    /// Ação de sistema (hotkey de menu) sempre tem prioridade sobre
    /// resolução de RetroPad — checado antes de rotear pro core.
    fn poll(&mut self) -> Vec<RawInputEvent>;
}
