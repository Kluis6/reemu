//! Scraping de metadata de jogos. Matching primário por hash (CRC32/MD5).
//! Match automático com hash exato OU com o nome do arquivo batendo, byte a
//! byte (sem extensão, case-insensitive), o `romfilename` que o PRÓPRIO
//! provedor devolveu pro candidato (Abordagem B revisada, 2026-09-13 —
//! decisão explícita do usuário, ver docs/ai-context/09) — não é heurística
//! nem fuzzy match, é igualdade exata contra o nome canônico que o
//! provedor já reconhece (arquivos No-Intro/Redump costumam bater aqui).
//! Qualquer OUTRA coisa (nome parecido, busca por texto livre, score de
//! confiança da API) continua indo sempre pra revisão manual. Provedor
//! configurável pelo usuário (IGDB, ScreenScraper, TheGamesDB...).

use crate::error::RepoError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// CRC32 e MD5 de uma ROM. Vazios quando a varredura não leu o arquivo
/// inteiro (CSO, CHD, ou arquivo acima de 1,5 GiB, como um ISO de DVD): aí a
/// busca vai pelo serial, pelo nome e pelo tamanho de [`ScrapeQuery`].
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

/// Ficha técnica do jogo além do básico (título, descrição, data, gênero),
/// como o ScreenScraper devolve no `jeuInfos.php` (doc da API v2:
/// `developpeur`, `editeur`, `joueurs`, `note`, `classifications`, `modes`).
/// `serde(default)`: correspondências gravadas antes não têm o campo.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameDetails {
    pub developer: Option<String>,
    pub publisher: Option<String>,
    /// Como o provedor escreve: "1", "1-2", "1-4"…
    pub players: Option<String>,
    /// Nota de 0 a 100 (o ScreenScraper dá de 0 a 20).
    pub rating: Option<u8>,
    /// Classificação etária, já com o órgão: "PEGI 12", "ESRB T"…
    pub age_rating: Option<String>,
    /// Modos de jogo separados por ", " (ex.: "1 jogador, Cooperativo").
    pub modes: Option<String>,
    /// Mídias do jogo (URLs do provedor), pra galeria da página do jogo:
    /// captura de tela, tela de título, arte de fundo, logo e vídeo.
    pub screenshot_url: Option<String>,
    pub title_screen_url: Option<String>,
    pub fanart_url: Option<String>,
    pub logo_url: Option<String>,
    pub video_url: Option<String>,
    pub gamekave: Option<GameKaveData>,
}

/// Dados adicionais preservados com a ficha para uso offline.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct GameKaveData {
    pub descriptions: std::collections::BTreeMap<String, String>,
    pub release_dates: std::collections::BTreeMap<String, String>,
    pub regional_covers: std::collections::BTreeMap<String, String>,
    pub rom_region: Option<String>,
    pub rom_revision: Option<String>,
    pub time_to_beat_minutes: [Option<u32>; 3],
    pub covers: Vec<String>,
    pub heroes: Vec<String>,
    pub screenshots: Vec<String>,
    pub trailers: Vec<String>,
    pub achievement_count: Option<u32>,
    pub soundtrack_count: Option<u32>,
}

/// Campos sobrepostos ? ficha importada. Uma string vazia limpa o campo.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GameMetadataEdits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub players: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_rating: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fanart_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
}

/// Um resultado de provedor pra uma ROM: metadata + os dois critérios que
/// podem disparar auto-aplicação.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeCandidate {
    pub provider: String,
    pub external_id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    /// True se veio de correspondência exata de hash.
    pub exact_hash_match: bool,
    /// True se o `file_stem` da consulta bate, exato e case-insensitive, com
    /// o nome de arquivo que o PRÓPRIO provedor devolveu pro candidato (ex.:
    /// `rom.romfilename` do ScreenScraper, sem extensão). `#[serde(default)]`:
    /// linhas gravadas antes desta mudança não têm este campo no JSON.
    #[serde(default)]
    pub exact_filename_match: bool,
    #[serde(default)]
    pub details: GameDetails,
}

impl ScrapeCandidate {
    /// Os dois critérios que disparam auto-aplicação (decisão: Abordagem B
    /// revisada) — nenhum outro sinal (score de confiança, nome parecido)
    /// conta. Ver comentário do módulo.
    pub fn auto_matches(&self) -> bool {
        self.exact_hash_match || self.exact_filename_match
    }
}

/// O que um provedor recebe pra procurar uma ROM.
#[derive(Debug, Clone)]
pub struct ScrapeQuery<'a> {
    pub hash: &'a RomHash,
    pub system_id: &'a str,
    /// Nome do arquivo (sem extensão). Usado tanto pra busca heurística
    /// quanto, se bater exato com o nome que o provedor reconhece pro
    /// candidato encontrado, pra auto-match (`exact_filename_match`).
    pub file_stem: &'a str,
    /// Serial do disco lido da imagem (`roms.serial`).
    pub serial: Option<&'a str>,
    /// Nome e tamanho do que identifica o arquivo (`roms.ident_name` e
    /// `ident_size`): a 1ª trilha de dados de um cue, o próprio `.iso`...
    pub ident_name: Option<&'a str>,
    pub ident_size: Option<i64>,
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GameMetadata {
    pub rom_id: String,
    pub title: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub release_date: Option<String>,
    pub genre: Option<String>,
    pub provider_source: Option<String>,
    #[serde(default)]
    pub game_uuid: Option<String>,
    #[serde(default)]
    pub details: GameDetails,
}

/// Estado da consulta ao catálogo do Alvanista para uma ROM (`rom_resolution.state`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionState {
    Found,
    Pending,
    Unknown,
    Manual,
}

impl ResolutionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Found => "found",
            Self::Pending => "pending",
            Self::Unknown => "unknown",
            Self::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "found" => Some(Self::Found),
            "pending" => Some(Self::Pending),
            "unknown" => Some(Self::Unknown),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Linha de `rom_resolution`: resposta guardada do catálogo para uma ROM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RomResolution {
    pub rom_id: String,
    pub state: ResolutionState,
    pub game_uuid: Option<String>,
    pub matched_by: Option<String>,
    pub reason: Option<String>,
    pub retry_after: Option<i64>,
    pub media_pending: bool,
    pub answer_json: Option<String>,
    pub resolved_at: i64,
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
    /// Chave de API do TheGamesDB — provedor de reserva, consultado só
    /// quando o ScreenScraper não acha o jogo. `None` = não usar.
    #[serde(default)]
    pub thegamesdb_api_key: Option<String>,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            provider: "screenscraper".into(),
            screenscraper_user: None,
            screenscraper_password: None,
            thegamesdb_api_key: None,
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
