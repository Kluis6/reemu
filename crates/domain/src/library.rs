//! Biblioteca de ROMs. Matching por hash (CRC32/MD5), não por nome de
//! arquivo (ver `metadata.rs`). `Rom` é a entidade base — scraping, save
//! states e save RAM referenciam `rom_id`.

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rom {
    pub id: String,
    pub file_path: String,
    pub crc32: String,
    pub md5: String,
    pub system_id: String,
    /// Unix timestamp (segundos) de quando a ROM entrou na biblioteca.
    pub added_at: i64,
    /// Unix timestamp (segundos) do último load do jogo. `None` = nunca jogado.
    pub last_played_at: Option<i64>,
}

#[async_trait]
pub trait RomRepository: Send + Sync {
    async fn add(&self, rom: &Rom) -> Result<(), RepoError>;
    async fn get(&self, id: &str) -> Result<Option<Rom>, RepoError>;
    async fn find_by_path(&self, file_path: &str) -> Result<Option<Rom>, RepoError>;
    /// CRC32 não é único (colisões, dumps distintos) — retorna todas.
    async fn find_by_crc32(&self, crc32: &str) -> Result<Vec<Rom>, RepoError>;
    async fn list_by_system(&self, system_id: &str) -> Result<Vec<Rom>, RepoError>;
    async fn list(&self) -> Result<Vec<Rom>, RepoError>;
    async fn remove(&self, id: &str) -> Result<(), RepoError>;
    /// Marca a ROM como jogada agora (`last_played_at`).
    async fn mark_played(&self, id: &str, at_unix: i64) -> Result<(), RepoError>;
}
