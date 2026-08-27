//! Resolução de decorações/bezels em cascata (rom -> sistema -> default),
//! inspirada na estrutura do RetroBat. Pulado automaticamente quando o
//! ShaderChain ativo tem `includes_bezel = true`.

use crate::shader_chain::AssignmentScope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DecorationPackSource {
    Bundled,
    UserImported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecorationPack {
    pub id: String,
    pub name: String,
    pub source: DecorationPackSource,
    pub base_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecorationAssignment {
    pub scope: AssignmentScope,
    pub system_id: Option<String>,
    pub rom_id: Option<String>,
    pub pack_id: String,
    pub asset_path: String,
}

pub trait DecorationResolver: Send + Sync {
    fn resolve(&self, system_id: &str, rom_id: Option<&str>) -> Option<DecorationAssignment>;
}

/// Importador de compatibilidade: varre a estrutura de pastas de um pacote
/// no formato RetroBat/The Bezel Project e popula `DecorationAssignment`
/// automaticamente, sem depender de convenção de nome em runtime.
pub trait DecorationPackImporter: Send + Sync {
    fn import(&self, pack: &DecorationPack) -> Result<Vec<DecorationAssignment>, String>;
}
