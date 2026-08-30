//! Hotkeys de sistema (ex: ToggleMenuOverlay), configuráveis pelo usuário.
//! Suportam combinação (janela hold+press) — decisão tomada especificamente
//! para hotkeys, diferente do mapeamento de controle (tecla única).
//! Camada de prioridade acima da resolução normal de RetroPad.

use crate::error::RepoError;
use crate::input::RawInputEvent;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SystemAction {
    ToggleMenuOverlay,
    QuickSave,
    QuickLoad,
}

impl SystemAction {
    /// Identificador estável usado como chave nas tabelas (`system_hotkeys`)
    /// e nos comandos do shell. Não muda entre versões.
    pub fn as_wire(&self) -> &'static str {
        match self {
            SystemAction::ToggleMenuOverlay => "toggle_menu_overlay",
            SystemAction::QuickSave => "quick_save",
            SystemAction::QuickLoad => "quick_load",
        }
    }

    pub fn from_wire(s: &str) -> Option<Self> {
        Some(match s {
            "toggle_menu_overlay" => SystemAction::ToggleMenuOverlay,
            "quick_save" => SystemAction::QuickSave,
            "quick_load" => SystemAction::QuickLoad,
            _ => return None,
        })
    }

    pub const ALL: [SystemAction; 3] = [
        SystemAction::ToggleMenuOverlay,
        SystemAction::QuickSave,
        SystemAction::QuickLoad,
    ];
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

/// Persistência das hotkeys de sistema (tabela `system_hotkeys`). Uma linha
/// por `SystemAction` — `set` substitui o binding anterior da mesma ação.
/// Implementado pelo crate `db`.
#[async_trait]
pub trait SystemHotkeyRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<HotkeyBinding>, RepoError>;
    async fn set(&self, binding: &HotkeyBinding) -> Result<(), RepoError>;
    async fn delete(&self, action: SystemAction) -> Result<(), RepoError>;
}
