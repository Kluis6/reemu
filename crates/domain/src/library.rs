//! Biblioteca de ROMs. Matching por hash (CRC32/MD5), não por nome de
//! arquivo (ver `metadata.rs`). `Rom` é a entidade base — scraping, save
//! states e save RAM referenciam `rom_id`.

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const SUFAMI_TURBO_HEADER_LEN: usize = 30;

/// Identifica um cartucho Sufami Turbo sem confundir `.st` com disco Atari ST.
/// A assinatura na posição 0 também ocorre na BIOS, que tem outro tamanho.
pub fn is_sufami_turbo_cart(size: u64, header: &[u8; SUFAMI_TURBO_HEADER_LEN]) -> bool {
    (0x80000..=0x100000).contains(&size)
        && header.starts_with(b"BANDAI SFC-ADX")
        && &header[16..30] != b"SFC-ADX BACKUP"
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rom {
    pub id: String,
    pub file_path: String,
    /// CRC32 e MD5 do que identifica o arquivo. Vazios (as colunas são NOT
    /// NULL) quando ele não é lido inteiro: aí o nome, o tamanho e o serial
    /// identificam.
    pub crc32: String,
    pub md5: String,
    pub system_id: String,
    /// Unix timestamp (segundos) de quando a ROM entrou na biblioteca.
    pub added_at: i64,
    /// Unix timestamp (segundos) do último load do jogo. `None` = nunca jogado.
    pub last_played_at: Option<i64>,
    /// Aba "Favoritos" da biblioteca.
    pub is_favorite: bool,
    /// Nome definido manualmente pelo usuário (`set_metadata`). `None` = usa o
    /// nome do arquivo. A plataforma editada vai direto em `system_id`.
    pub user_title: Option<String>,
    /// SHA1 (hex minúsculo) do que foi lido pro CRC32/MD5, na mesma passada.
    /// `None` nas linhas de antes da identidade (o passo em segundo plano
    /// preenche; o scan não relê) e quando não há hash (ver `crc32`); quem
    /// separa os dois casos é `identified_at`.
    pub sha1: Option<String>,
    /// Serial do disco (ex.: `SLUS-00594`), lido da imagem. `None` = não lido.
    pub serial: Option<String>,
    /// Nome do arquivo que foi lido pra identidade: a entrada de dentro do
    /// `.zip`/`.7z`, a 1ª trilha de dados de um `.cue`/`.gdi`, ou o próprio
    /// arquivo.
    pub ident_name: Option<String>,
    /// Tamanho em bytes do que foi lido (sem o header iNES).
    pub ident_size: Option<i64>,
    /// Unix segundos de quando a identidade foi calculada.
    pub identified_at: Option<i64>,
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
    /// Troca o sistema de uma ROM já catalogada (a varredura reclassifica
    /// quando a regra de identificação muda — ex.: NAOMI que entrou como
    /// `disc`). Mantém tempo de jogo, favorito e metadados. A identidade fica
    /// pra calcular de novo (`identified_at` nulo): a trilha e o serial
    /// dependem do sistema (a trilha 3 e o serial do Dreamcast, por exemplo).
    async fn set_system(&self, id: &str, system_id: &str) -> Result<(), RepoError>;
    /// Marca a ROM como jogada agora (`last_played_at`).
    async fn mark_played(&self, id: &str, at_unix: i64) -> Result<(), RepoError>;
    /// Soma `secs` ao tempo de jogo acumulado da ROM.
    async fn add_play_time(&self, id: &str, secs: u64) -> Result<(), RepoError>;
    /// Tempo de jogo acumulado, em segundos (0 se nunca jogou ou não existe).
    async fn play_time(&self, id: &str) -> Result<u64, RepoError>;
    /// Liga/desliga o favorito.
    async fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), RepoError>;
    /// Edição manual: `title = Some("")` limpa (volta pro nome do arquivo);
    /// `system_id = Some(...)` troca a plataforma. `None` = não mexe no campo.
    async fn set_metadata(
        &self,
        id: &str,
        title: Option<&str>,
        system_id: Option<&str>,
    ) -> Result<(), RepoError>;
}
