//! Cadeia de passes de shader (filtros de imagem, CRT, Mega Bezel).
//! Formato suportado no MVP: slang. ReShade FX fica em backlog
//! (compilador standalone existe e é BSD-3-clause, mas baixo valor
//! diferencial pro caso de uso — a maioria dos efeitos dependentes de
//! depth buffer não se aplica a cores 2D).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShaderFormat {
    Slang,
    // ReShadeFx, // backlog
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderPreset {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub format: ShaderFormat,
    pub is_builtin: bool,
    /// Se true, o preset já desenha sua própria moldura (ex: Mega Bezel
    /// completo) — o DecorationResolver deve ser pulado quando este preset
    /// está ativo (exclusão mútua por padrão, decisão já tomada).
    pub includes_bezel: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AssignmentScope {
    Default,
    System,
    Rom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderChainAssignment {
    pub scope: AssignmentScope,
    pub system_id: Option<String>,
    pub rom_id: Option<String>,
    pub preset_id: String,
    pub parameter_overrides: HashMap<String, String>,
}

pub trait ShaderChainResolver: Send + Sync {
    /// Resolve em cascata: rom -> sistema -> default.
    fn resolve(&self, system_id: &str, rom_id: Option<&str>) -> Option<ShaderChainAssignment>;
}
