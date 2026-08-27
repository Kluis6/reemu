//! Resolução de decorações/bezels em cascata (rom -> sistema -> default),
//! inspirada na estrutura do RetroBat. Pulado automaticamente quando o
//! ShaderChain ativo tem `includes_bezel = true`.

use crate::error::RepoError;
use crate::shader_chain::AssignmentScope;
use async_trait::async_trait;
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

#[async_trait]
pub trait DecorationResolver: Send + Sync {
    /// Cascata rom -> sistema -> default. `Ok(None)` = sem decoração.
    async fn resolve(
        &self,
        system_id: &str,
        rom_id: Option<&str>,
    ) -> Result<Option<DecorationAssignment>, RepoError>;
}

/// Importador de compatibilidade: varre a estrutura de pastas de um pacote
/// no formato RetroBat/The Bezel Project e popula `DecorationAssignment`
/// automaticamente, sem depender de convenção de nome em runtime.
/// (Implementação prevista para a etapa 04 — scan de filesystem.)
pub trait DecorationPackImporter: Send + Sync {
    fn import(&self, pack: &DecorationPack) -> Result<Vec<DecorationAssignment>, String>;
}
