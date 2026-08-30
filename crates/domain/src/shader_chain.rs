//! Cadeia de passes de shader (filtros de imagem, CRT, Mega Bezel).
//! Formato suportado no MVP: slang. ReShade FX fica em backlog
//! (compilador standalone existe e é BSD-3-clause, mas baixo valor
//! diferencial pro caso de uso — a maioria dos efeitos dependentes de
//! depth buffer não se aplica a cores 2D).

use crate::error::RepoError;
use async_trait::async_trait;
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

#[async_trait]
pub trait ShaderChainResolver: Send + Sync {
    /// Resolve em cascata: rom -> sistema -> default.
    /// `Ok(None)` = nenhuma atribuição em nenhum escopo (não é erro).
    async fn resolve(
        &self,
        system_id: &str,
        rom_id: Option<&str>,
    ) -> Result<Option<ShaderChainAssignment>, RepoError>;
}

/// Escrita da tabela de presets/atribuições (a UI de Vídeo usa).
#[async_trait]
pub trait ShaderChainStore: Send + Sync {
    /// Registra/atualiza um preset (por `id`).
    async fn upsert_preset(&self, preset: &ShaderPreset) -> Result<(), RepoError>;
    async fn list_presets(&self) -> Result<Vec<ShaderPreset>, RepoError>;
    /// Atribui `preset_id` a um escopo, trocando o que já existir nele.
    async fn set_assignment(
        &self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
        preset_id: &str,
    ) -> Result<(), RepoError>;
    /// Remove a atribuição de um escopo (volta pra cascata).
    async fn clear_assignment(
        &self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
    ) -> Result<(), RepoError>;

    /// Define (ou atualiza) o valor de um parâmetro de shader no escopo dado.
    /// O escopo precisa já ter uma atribuição (a UI cria uma antes de mexer nos
    /// parâmetros). `value` é serializado como string (o schema guarda TEXT).
    async fn set_parameter_override(
        &self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
        key: &str,
        value: &str,
    ) -> Result<(), RepoError>;

    /// Zera todos os overrides de parâmetro do escopo (volta pros defaults do
    /// preset).
    async fn clear_parameter_overrides(
        &self,
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
    ) -> Result<(), RepoError>;
}
