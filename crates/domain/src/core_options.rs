//! Opções específicas de core (ex: resolução interna e buffering no PS2,
//! filtragem de textura no Dreamcast). Schema populado em runtime via
//! retro_core_options/retro_core_options_v2 do próprio core — mesma
//! filosofia de "deixar o core se declarar" usada em core_loader.

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoreOptionType {
    Combo { choices: Vec<String> },
    Bool,
    Range { min: f64, max: f64, step: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreOptionDefinition {
    pub option_key: String,
    pub display_name: String,
    pub option_type: CoreOptionType,
    pub default_value: String,
}

#[async_trait]
pub trait CoreOptionsStore: Send + Sync {
    /// Schema declarado pelo core no último load. Vazio se o core nunca
    /// declarou opções (não é erro).
    async fn schema_for(&self, core_id: &str) -> Result<Vec<CoreOptionDefinition>, RepoError>;

    /// Valor escolhido pelo usuário. `Ok(None)` = usa o `default_value` do schema.
    async fn get_value(&self, core_id: &str, option_key: &str)
        -> Result<Option<String>, RepoError>;

    /// Todos os valores escolhidos pra um core (`option_key -> value`). Usado
    /// no load pra alimentar o core antes de ele pedir via `GET_VARIABLE`.
    async fn values_for(
        &self,
        core_id: &str,
    ) -> Result<std::collections::HashMap<String, String>, RepoError>;

    async fn set_value(
        &self,
        core_id: &str,
        option_key: &str,
        value: &str,
    ) -> Result<(), RepoError>;

    /// Overrides por jogo (`option_key -> value`) — vencem os valores do core
    /// só quando aquela ROM está carregada.
    async fn overrides_for_rom(
        &self,
        rom_id: &str,
        core_id: &str,
    ) -> Result<std::collections::HashMap<String, String>, RepoError>;

    /// Grava (ou limpa, com `value = None`) um valor num escopo:
    /// `rom_id = None` → valor por core; `Some(rid)` → override do jogo.
    async fn set_scoped_value(
        &self,
        core_id: &str,
        rom_id: Option<&str>,
        option_key: &str,
        value: Option<&str>,
    ) -> Result<(), RepoError>;

    /// Limpa TODOS os valores de um escopo (volta pro default do schema / pro
    /// valor do core). `rom_id = None` → por core; `Some(rid)` → do jogo.
    async fn reset_scope(&self, core_id: &str, rom_id: Option<&str>)
        -> Result<(), RepoError>;

    /// Substitui o schema inteiro de um core (chamado no load, quando o core
    /// declara `retro_core_options`). Idempotente.
    async fn replace_schema(
        &self,
        core_id: &str,
        defs: &[CoreOptionDefinition],
    ) -> Result<(), RepoError>;
}
