//! Scraping de metadata de jogos. Matching por hash (CRC32/MD5), não por
//! nome de arquivo. Match automático só com hash exato (Abordagem B) —
//! qualquer busca heurística vai sempre pra revisão manual. Provedor
//! configurável pelo usuário (IGDB, ScreenScraper, TheGamesDB...).

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RomHash {
    pub crc32: String,
    pub md5: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchStatus {
    AutoMatched,
    PendingReview,
    UserConfirmed,
    NoMatch,
}

/// Um resultado de provedor pra uma ROM: metadata + se veio de hash exato.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeCandidate {
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    /// True somente se veio de correspondência exata de hash — é o único
    /// critério que dispara auto-aplicação (decisão: Abordagem B).
    pub exact_hash_match: bool,
}

/// O que um provedor recebe pra procurar uma ROM.
#[derive(Debug, Clone)]
pub struct ScrapeQuery<'a> {
    pub hash: &'a RomHash,
    pub system_id: &'a str,
    /// Nome do arquivo (sem extensão) — só usado pra busca heurística quando o
    /// hash não bate; nunca dispara auto-match.
    pub file_stem: &'a str,
}

#[async_trait]
pub trait MetadataProvider: Send + Sync {
    fn provider_name(&self) -> &str;
    async fn search(&self, query: &ScrapeQuery<'_>) -> Result<Vec<ScrapeCandidate>, String>;
}

pub trait RomHashService: Send + Sync {
    fn compute(&self, file_path: &str) -> Result<RomHash, String>;
}

/// Metadata catalogada de um jogo (linha de `game_metadata`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameMetadata {
    pub rom_id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    pub provider_source: Option<String>,
}

/// Uma correspondência aguardando revisão do usuário (`scrape_matches` com
/// `status = 'pending_review'`), já com a metadata proposta pra preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMatch {
    pub rom_id: String,
    pub file_stem: String,
    pub provider: String,
    pub external_id: String,
    pub candidate: ScrapeCandidate,
}

/// Config do scraper (linha singleton de `metadata_config`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetadataConfig {
    pub provider: String,
    pub screenscraper_user: Option<String>,
    pub screenscraper_password: Option<String>,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            provider: "screenscraper".into(),
            screenscraper_user: None,
            screenscraper_password: None,
        }
    }
}

/// Persistência de metadata/matches (`game_metadata` + `scrape_matches`).
#[async_trait]
pub trait MetadataRepository: Send + Sync {
    async fn get_config(&self) -> Result<MetadataConfig, RepoError>;
    async fn set_config(&self, cfg: &MetadataConfig) -> Result<(), RepoError>;
    async fn get_metadata(&self, rom_id: &str) -> Result<Option<GameMetadata>, RepoError>;
    async fn upsert_metadata(&self, meta: &GameMetadata) -> Result<(), RepoError>;
    /// Registra o resultado de um scrape (troca o match anterior da ROM).
    async fn record_match(
        &self,
        rom_id: &str,
        candidate: &ScrapeCandidate,
        status: MatchStatus,
    ) -> Result<(), RepoError>;
    /// ROMs sem nenhum match ainda (pra fila de scraping).
    async fn rom_ids_without_match(&self) -> Result<Vec<String>, RepoError>;
    async fn list_pending(&self) -> Result<Vec<PendingMatch>, RepoError>;
    /// Aceita (aplica a metadata + `user_confirmed`) ou rejeita (`no_match`)
    /// uma pendência.
    async fn resolve_pending(&self, rom_id: &str, accept: bool) -> Result<(), RepoError>;
}
