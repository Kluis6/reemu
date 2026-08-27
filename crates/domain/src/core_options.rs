//! Opções específicas de core (ex: resolução interna e buffering no PS2,
//! filtragem de textura no Dreamcast). Schema populado em runtime via
//! retro_core_options/retro_core_options_v2 do próprio core — mesma
//! filosofia de "deixar o core se declarar" usada em core_loader.

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

pub trait CoreOptionsStore: Send + Sync {
    fn schema_for(&self, core_id: &str) -> Vec<CoreOptionDefinition>;
    fn get_value(&self, core_id: &str, option_key: &str) -> Option<String>;
    fn set_value(&self, core_id: &str, option_key: &str, value: &str) -> Result<(), String>;
}
