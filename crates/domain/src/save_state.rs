//! Save states (snapshot de emulação via retro_serialize) e save RAM
//! (battery save nativo do jogo via retro_get_memory_data) — entidades
//! distintas, ciclos de vida diferentes.
//!
//! Timing decidido: save imediato entre frames (nunca no meio de um
//! retro_run em andamento), sem pausar o core — Abordagem A.
//!
//! Duas portas aqui:
//! - `SaveStateManager`: alto nível, dispara `retro_serialize`/`retro_unserialize`
//!   e escreve o arquivo em disco. Implementado no core-loader (etapa 08).
//! - `SaveStateRepository`: só a metadata no SQLite (o binário grande fica em
//!   disco, o banco guarda `file_path`). Implementado no crate `db`.

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveStateMetadata {
    pub id: String,
    pub rom_id: String,
    /// `core_id` é obrigatório: states não são portáveis entre cores.
    pub core_id: String,
    pub slot: Option<u32>,
    pub file_path: String,
    pub thumbnail_path: Option<String>,
    pub created_at: i64,
    pub play_time_at_save: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveRamMetadata {
    pub id: String,
    pub rom_id: String,
    pub core_id: String,
    pub file_path: String,
    pub updated_at: i64,
}

/// Alto nível — orquestra o core. NÃO é implementado pelo crate `db`.
pub trait SaveStateManager: Send + Sync {
    /// Deve ser chamado só entre frames (nunca durante um retro_run),
    /// pela camada que orquestra o loop de emulação.
    fn save(
        &self,
        rom_id: &str,
        core_id: &str,
        slot: Option<u32>,
    ) -> Result<SaveStateMetadata, String>;
    fn load(&self, metadata: &SaveStateMetadata) -> Result<(), String>;
    fn list_for_rom(&self, rom_id: &str) -> Vec<SaveStateMetadata>;
}

/// Persistência da metadata (tabelas `save_states` e `save_ram`). O arquivo
/// de state em si é escrito pelo `SaveStateManager`, não aqui.
#[async_trait]
pub trait SaveStateRepository: Send + Sync {
    async fn record_state(&self, meta: &SaveStateMetadata) -> Result<(), RepoError>;
    async fn get_state(&self, id: &str) -> Result<Option<SaveStateMetadata>, RepoError>;
    /// Mais recentes primeiro.
    async fn list_states_for_rom(&self, rom_id: &str) -> Result<Vec<SaveStateMetadata>, RepoError>;
    /// State atualmente ocupando um slot (quem quiser "sobrescrever slot"
    /// combina isto com `delete_state`).
    async fn find_state_in_slot(
        &self,
        rom_id: &str,
        core_id: &str,
        slot: u32,
    ) -> Result<Option<SaveStateMetadata>, RepoError>;
    async fn delete_state(&self, id: &str) -> Result<(), RepoError>;

    /// Save RAM é 1 por (rom, core) — `UNIQUE(rom_id, core_id)` no schema.
    async fn get_save_ram(
        &self,
        rom_id: &str,
        core_id: &str,
    ) -> Result<Option<SaveRamMetadata>, RepoError>;
    async fn upsert_save_ram(&self, meta: &SaveRamMetadata) -> Result<(), RepoError>;
}
