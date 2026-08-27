//! Scraping de metadata de jogos. Matching por hash (CRC32/MD5), não por
//! nome de arquivo. Match automático só com hash exato (Abordagem B) —
//! qualquer busca heurística vai sempre pra revisão manual. Provedor
//! configurável pelo usuário (IGDB, ScreenScraper, TheGamesDB...).

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeCandidate {
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    /// True somente se veio de correspondência exata de hash — é o único
    /// critério que dispara auto-aplicação (decisão: Abordagem B).
    pub exact_hash_match: bool,
}

#[async_trait]
pub trait MetadataProvider: Send + Sync {
    fn provider_name(&self) -> &str;
    async fn search_by_hash(&self, hash: &RomHash) -> Result<Vec<ScrapeCandidate>, String>;
}

pub trait RomHashService: Send + Sync {
    fn compute(&self, file_path: &str) -> Result<RomHash, String>;
}
