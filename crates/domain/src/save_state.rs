//! Save states (snapshot de emulação via retro_serialize) e save RAM
//! (battery save nativo do jogo via retro_get_memory_data) — entidades
//! distintas, ciclos de vida diferentes.
//!
//! Timing decidido: save imediato entre frames (nunca no meio de um
//! retro_run em andamento), sem pausar o core — Abordagem A.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveStateMetadata {
    pub rom_id: String,
    pub core_id: String,
    pub slot: Option<u32>,
    pub file_path: String,
    pub thumbnail_path: Option<String>,
    pub created_at: i64,
    pub play_time_at_save: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveRamMetadata {
    pub rom_id: String,
    pub core_id: String,
    pub file_path: String,
    pub updated_at: i64,
}

pub trait SaveStateManager: Send + Sync {
    /// Deve ser chamado só entre frames (nunca durante um retro_run),
    /// pela camada que orquestra o loop de emulação.
    fn save(&self, rom_id: &str, core_id: &str, slot: Option<u32>) -> Result<SaveStateMetadata, String>;
    fn load(&self, metadata: &SaveStateMetadata) -> Result<(), String>;
    fn list_for_rom(&self, rom_id: &str) -> Vec<SaveStateMetadata>;
}
