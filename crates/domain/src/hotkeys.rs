//! Hotkeys de sistema (ex: ToggleMenuOverlay), configuráveis pelo usuário.
//! Suportam combinação (janela hold+press) — decisão tomada especificamente
//! para hotkeys, diferente do mapeamento de controle (tecla única).
//! Camada de prioridade acima da resolução normal de RetroPad.

use crate::input::RawInputEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SystemAction {
    ToggleMenuOverlay,
    QuickSave,
    QuickLoad,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyBinding {
    pub action: SystemAction,
    /// Um ou dois eventos (combinação hold+press). Tecla única = Vec de 1.
    pub trigger: Vec<RawInputEvent>,
    pub device_guid: Option<String>,
}

pub trait HotkeyResolver: Send + Sync {
    fn resolve(&self, events: &[RawInputEvent]) -> Option<SystemAction>;
    fn set_binding(&mut self, binding: HotkeyBinding) -> Result<(), String>;
}
