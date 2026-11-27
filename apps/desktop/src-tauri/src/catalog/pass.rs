//! Passada de resolução em lotes contra `POST /api/partner/v1/games/resolve`
//! (D135, D140, D145, D151).

use super::answer::{candidate_from_answer, metadata_from_answer_for_rom, GameAnswer};
use super::identify::identify_unidentified;
use crate::alvanista::client::{partner_error_code, ClientError};
use crate::alvanista::store::now_secs;
use crate::alvanista::Alvanista;
use domain::library::{Rom, RomRepository};
use domain::metadata::{
    GameMetadata, MatchStatus, MetadataRepository, ResolutionState, RomResolution,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

const BATCH_SIZE: usize = 200;
const DEFAULT_PENDING_RETRY_SECS: i64 = 30 * 60;
const MEDIA_PENDING_RETRY_SECS: i64 = 60 * 60;
const LOOKUP_FAILED_RETRY_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassScope {
    All,
    Roms(Vec<String>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PassReport {
    pub sent: usize,
    pub found: usize,
    pub pending: usize,
    pub unknown: usize,
    pub identified_late: usize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CatalogError {
    #[error("o catálogo do Alvanista está desligado no servidor")]
    Disabled,
    #[error("sem sessão ativa com o Alvanista")]
    AuthEnded,
    #[error("catálogo indisponível no momento: {0}")]
    Transient(String),
    #[error("pedido recusado pelo catálogo: {0}")]
    Rejected(String),
    #[error("erro de banco: {0}")]
    Repo(String),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CatalogEvent {
    Pass {
        found: usize,
        pending: usize,
        unknown: usize,
    },
    Status,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "CatalogStatus"))]
pub struct CatalogStatus {
    pub connected: bool,
    pub enabled: bool,
    pub disabled_by_server: bool,
    pub running: bool,
    pub found: usize,
    pub pending: usize,
    pub unknown: usize,
}

#[derive(Serialize)]
struct ResolveBatchReq<'a> {
    langs: Vec<String>,
    items: Vec<ResolveItemReq<'a>>,
}

#[derive(Serialize)]
pub(crate) struct ResolveItemReq<'a> {
    pub id: &'a str,
    pub system: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crc32: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub md5: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
}

#[derive(Deserialize)]
struct ResolveBatchResp {
    #[serde(default)]
    results: Vec<ResolveItemResp>,
}

#[derive(Deserialize)]
struct ResolveItemResp {
    id: String,
    status: String,
    #[serde(default)]
    matched_by: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    retry_after_s: Option<i64>,
    #[serde(default)]
    game: Option<GameAnswer>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "CatalogSearchGame"))]
pub struct SearchGameDto {
    pub game_id: String,
    pub title: String,
    pub platforms: Vec<String>,
    pub release_date: Option<String>,
    pub cover: Option<String>,
}

#[derive(Deserialize)]
struct SearchGameWire {
    game_id: String,
    title: String,
    #[serde(default)]
    platforms: Vec<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    cover: Option<String>,
}

#[derive(Deserialize)]
struct SearchGamesResp {
    #[serde(default)]
    games: Vec<SearchGameWire>,
}

#[derive(Serialize)]
struct ClaimBatchReq<'a> {
    game_id: &'a str,
    langs: Vec<String>,
    items: Vec<ResolveItemReq<'a>>,
}

#[derive(Deserialize)]
struct ClaimBatchResp {
    #[serde(default)]
    game: Option<GameAnswer>,
}

fn map_client_error(err: ClientError) -> CatalogError {
    match err {
        ClientError::AuthEnded => CatalogError::AuthEnded,
        ClientError::Transient(msg) => CatalogError::Transient(msg),
        ClientError::Rejected(msg) => CatalogError::Rejected(msg),
    }
}

/// Ordem de preferência de idioma enviada ao catálogo (`1..=8` caracteres por item).
pub fn langs_for(ui_lang: Option<&str>) -> Vec<String> {
    let primary = ui_lang
        .and_then(|l| l.split(['-', '_']).next())
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .filter(|s| !s.is_empty() && s.len() <= 8);
    let mut out = Vec::with_capacity(3);
    if let Some(p) = primary {
        out.push(p);
    }
    for fallback in ["pt", "en"] {
        if !out.iter().any(|x| x == fallback) {
            out.push(fallback.to_owned());
        }
    }
    out
}

pub(crate) fn item_from_rom(rom: &Rom) -> Option<ResolveItemReq<'_>> {
    let system = rom.system_id.trim();
    if rom.id.trim().is_empty() || system.is_empty() {
        return None;
    }
    let filename = rom
        .ident_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            Path::new(&rom.file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        });
    let hex_opt = |s: &str, len: usize| -> Option<String> {
        let t = s.trim();
        (t.len() == len && t.bytes().all(|b| b.is_ascii_hexdigit())).then(|| t.to_ascii_lowercase())
    };
    let crc32 = hex_opt(&rom.crc32, 8);
    let md5 = hex_opt(&rom.md5, 32);
    let sha1 = rom.sha1.as_deref().and_then(|s| hex_opt(s, 40));
    let serial = rom
        .serial
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_uppercase);
    let size = rom.ident_size.filter(|&n| n >= 0);

    Some(ResolveItemReq {
        id: &rom.id,
        system,
        size,
        filename,
        crc32,
        md5,
        sha1,
        serial,
    })
}

pub type PlayingFn = Arc<dyn Fn() -> bool + Send + Sync>;
pub type EmitFn = Arc<dyn Fn(CatalogEvent) + Send + Sync>;
pub type ClockFn = Arc<dyn Fn() -> i64 + Send + Sync>;

pub struct CatalogEngine {
    db: db::Db,
    covers_dir: PathBuf,
    is_playing: PlayingFn,
    emit: EmitFn,
    clock: ClockFn,
    pass_lock: tokio::sync::Mutex<()>,
    running: AtomicBool,
}

impl CatalogEngine {
    pub fn new(db: db::Db, covers_dir: PathBuf, is_playing: PlayingFn, emit: EmitFn) -> Self {
        Self::with_clock(db, covers_dir, is_playing, emit, Arc::new(now_secs))
    }

    pub fn with_clock(
        db: db::Db,
        covers_dir: PathBuf,
        is_playing: PlayingFn,
        emit: EmitFn,
        clock: ClockFn,
    ) -> Self {
        Self {
            db,
            covers_dir,
            is_playing,
            emit,
            clock,
            pass_lock: tokio::sync::Mutex::new(()),
            running: AtomicBool::new(false),
        }
    }

    pub fn db(&self) -> &db::Db {
        &self.db
    }

    #[allow(dead_code)]
    pub fn covers_dir(&self) -> &Path {
        &self.covers_dir
    }

    pub fn now(&self) -> i64 {
        (self.clock)()
    }

    pub fn is_playing(&self) -> bool {
        (self.is_playing)()
    }

    pub fn emit(&self, ev: CatalogEvent) {
        (self.emit)(ev);
    }

    fn state_repo(&self) -> db::AlvanistaRepo {
        db::AlvanistaRepo::new(self.db.clone())
    }

    pub async fn is_enabled(&self) -> bool {
        self.state_repo().catalog_enabled().await.unwrap_or(true)
    }

    pub async fn is_disabled_by_server(&self) -> bool {
        self.state_repo()
            .catalog_disabled_by_server()
            .await
            .unwrap_or(false)
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<(), CatalogError> {
        self.state_repo()
            .set_catalog_enabled(enabled)
            .await
            .map_err(|e| CatalogError::Repo(e.to_string()))?;
        self.emit(CatalogEvent::Status);
        Ok(())
    }

    pub async fn status(&self, alva: &Alvanista) -> CatalogStatus {
        let connected = alva.has_session().await;
        let enabled = self.is_enabled().await;
        let disabled_by_server = self.is_disabled_by_server().await;
        let (found, pending, unknown) = db::RomResolutionRepo::new(self.db.clone())
            .counts()
            .await
            .unwrap_or((0, 0, 0));
        CatalogStatus {
            connected,
            enabled,
            disabled_by_server,
            running: self.running.load(Ordering::Relaxed),
            found,
            pending,
            unknown,
        }
    }

    pub async fn has_follow_up(&self) -> bool {
        db::RomResolutionRepo::new(self.db.clone())
            .has_follow_up(self.now())
            .await
            .unwrap_or(false)
    }

    async fn call_partner<T: serde::de::DeserializeOwned>(
        &self,
        alva: &Alvanista,
        method: reqwest::Method,
        path: &str,
        payload: Option<&serde_json::Value>,
        fallback_err: &'static str,
        cleared_server_flag: &mut bool,
    ) -> Result<T, CatalogError> {
        let (status, body) = alva
            .partner_json(method, path, payload)
            .await
            .map_err(map_client_error)?;

        let err_code = partner_error_code(&body);
        if status == 503 && err_code == Some("rom_resolve_disabled") {
            let _ = self.state_repo().set_catalog_disabled_by_server(true).await;
            return Err(CatalogError::Disabled);
        }
        if !(200..300).contains(&status) {
            return Err(CatalogError::Rejected(
                err_code.unwrap_or(fallback_err).to_owned(),
            ));
        }
        if !*cleared_server_flag {
            let _ = self
                .state_repo()
                .set_catalog_disabled_by_server(false)
                .await;
            *cleared_server_flag = true;
        }
        serde_json::from_value(body)
            .map_err(|e| CatalogError::Transient(format!("resposta ilegível: {e}")))
    }

    /// Busca jogos no catálogo (`GET /api/partner/v1/games/search?q=&system=`)
    /// para a escolha manual ("Escolher o jogo", D141).
    pub async fn search(
        &self,
        alva: &Alvanista,
        q: &str,
        system: &str,
    ) -> Result<Vec<SearchGameDto>, CatalogError> {
        let trimmed: String = q.trim().chars().take(100).collect();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        if !alva.has_session().await {
            return Err(CatalogError::AuthEnded);
        }
        if !self.is_enabled().await {
            return Err(CatalogError::Disabled);
        }

        let path = {
            let mut ser = url::form_urlencoded::Serializer::new(String::new());
            ser.append_pair("q", &trimmed);
            let sys = system.trim();
            if !sys.is_empty() && sys != "disc" {
                ser.append_pair("system", sys);
            }
            ser.append_pair("limit", "20");
            format!("/api/partner/v1/games/search?{}", ser.finish())
        };

        let mut cleared = false;
        let parsed: SearchGamesResp = self
            .call_partner(
                alva,
                reqwest::Method::GET,
                &path,
                None,
                "search_failed",
                &mut cleared,
            )
            .await?;

        Ok(parsed
            .games
            .into_iter()
            .map(|g| SearchGameDto {
                game_id: g.game_id,
                title: g.title,
                platforms: g.platforms,
                release_date: g.release_date,
                cover: g.cover.as_deref().and_then(net::http::https_url),
            })
            .collect())
    }

    /// Registra a escolha manual da pessoa (`POST /api/partner/v1/games/claims`,
    /// D141) e aplica os metadados na hora com `state = 'manual'`.
    pub async fn pick(
        &self,
        alva: &Alvanista,
        rom_id: &str,
        game_id: &str,
        lang: Option<&str>,
    ) -> Result<GameMetadata, CatalogError> {
        if !alva.has_session().await {
            return Err(CatalogError::AuthEnded);
        }
        if !self.is_enabled().await {
            return Err(CatalogError::Disabled);
        }
        let game_id = game_id.trim();
        if game_id.is_empty() {
            return Err(CatalogError::Rejected("invalid_game_id".into()));
        }

        let roms_repo = db::RomsRepo::new(self.db.clone());
        let mut rom = roms_repo
            .get(rom_id)
            .await
            .map_err(|e| CatalogError::Repo(e.to_string()))?
            .ok_or_else(|| CatalogError::Rejected("rom_not_found".into()))?;

        let now = self.now();
        if rom.identified_at.is_none() && !self.is_playing() {
            let mut only = HashSet::with_capacity(1);
            only.insert(rom_id.to_owned());
            let _ =
                identify_unidentified(&self.db, Some(&only), now, self.is_playing.as_ref()).await;
            if let Ok(Some(refreshed)) = roms_repo.get(rom_id).await {
                rom = refreshed;
            }
        }

        let item =
            item_from_rom(&rom).ok_or_else(|| CatalogError::Rejected("missing_system".into()))?;
        let payload = serde_json::to_value(ClaimBatchReq {
            game_id,
            langs: langs_for(lang),
            items: vec![item],
        })
        .map_err(|e| CatalogError::Rejected(e.to_string()))?;

        let mut cleared = false;
        let parsed: ClaimBatchResp = self
            .call_partner(
                alva,
                reqwest::Method::POST,
                "/api/partner/v1/games/claims",
                Some(&payload),
                "claim_failed",
                &mut cleared,
            )
            .await?;
        let answer = parsed
            .game
            .ok_or_else(|| CatalogError::Transient("resposta sem dados do jogo".into()))?;

        #[cfg(not(test))]
        let answer = {
            let mut enriched = answer;
            enrich_answer_from_public_catalog(&mut enriched).await;
            enriched
        };
        if let Some(data_dir) = self.covers_dir.parent() {
            crate::covers::invalidate_game_media(&data_dir.join("media"), &answer.game_id);
        }

        let meta = self
            .apply_answer_with_region(
                rom_id,
                &answer,
                ResolutionState::Manual,
                None,
                None,
                None,
                None,
                self.now(),
            )
            .await?;

        let (found, pending, unknown) = db::RomResolutionRepo::new(self.db.clone())
            .counts()
            .await
            .unwrap_or((1, 0, 0));
        self.emit(CatalogEvent::Pass {
            found,
            pending,
            unknown,
        });
        self.emit(CatalogEvent::Status);
        Ok(meta)
    }

    /// Grava a resolução (`found` automático ou `manual` escolhida pela pessoa)
    /// e aplica `game_metadata` + `scrape_matches` + invalidação da capa.
    /// No modo `Found`, respeita uma confirmação manual prévia (`user_confirmed`).
    #[allow(dead_code)]
    pub async fn apply_answer(
        &self,
        rom_id: &str,
        answer: &GameAnswer,
        state: ResolutionState,
        matched_by: Option<String>,
        existing: Option<&RomResolution>,
        now: i64,
    ) -> Result<GameMetadata, CatalogError> {
        self.apply_answer_with_region(rom_id, answer, state, matched_by, None, None, existing, now)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn apply_answer_with_region(
        &self,
        rom_id: &str,
        answer: &GameAnswer,
        state: ResolutionState,
        matched_by: Option<String>,
        server_region: Option<&str>,
        server_revision: Option<&str>,
        existing: Option<&RomResolution>,
        now: i64,
    ) -> Result<GameMetadata, CatalogError> {
        let res_repo = db::RomResolutionRepo::new(self.db.clone());
        let meta_repo = db::MetadataRepo::new(self.db.clone());
        let rom_opt = db::RomsRepo::new(self.db.clone())
            .get(rom_id)
            .await
            .ok()
            .flatten();
        let detected_region = rom_opt.as_ref().and_then(|r| {
            scraping::rom_name::detect_rom_region(
                &r.file_path,
                r.ident_name.as_deref(),
                r.serial.as_deref(),
            )
        });
        let effective_region = server_region
            .filter(|s| !s.trim().is_empty())
            .or(detected_region);

        let answer_json = serde_json::to_string(answer).ok();
        let is_manual = state == ResolutionState::Manual;
        let media_pending = !is_manual && answer.media_pending;
        // Na resposta automática, mantém o resolved_at da 1ª descoberta enquanto
        // a mídia ainda estiver pendente, para a janela de 7 dias não reiniciar.
        let resolved_at = existing
            .filter(|r| !is_manual && r.state == ResolutionState::Found && r.media_pending)
            .map(|r| r.resolved_at)
            .unwrap_or(now);
        let retry_after = media_pending.then_some(now + MEDIA_PENDING_RETRY_SECS);

        res_repo
            .put(&RomResolution {
                rom_id: rom_id.to_owned(),
                state,
                game_uuid: Some(answer.game_id.clone()),
                matched_by,
                reason: None,
                retry_after,
                media_pending,
                answer_json,
                resolved_at,
            })
            .await
            .map_err(|e| CatalogError::Repo(e.to_string()))?;

        let meta = metadata_from_answer_for_rom(rom_id, answer, effective_region, server_revision);
        let skip_metadata = !is_manual
            && meta_repo
                .is_user_confirmed(rom_id)
                .await
                .map_err(|e| CatalogError::Repo(e.to_string()))?;

        if !skip_metadata {
            if is_manual {
                let _ = meta_repo.clear_artwork_edits(rom_id).await;
            }
            let candidate = candidate_from_answer(&meta);
            let match_status = if is_manual {
                MatchStatus::UserConfirmed
            } else {
                MatchStatus::AutoMatched
            };
            meta_repo
                .upsert_metadata(&meta)
                .await
                .map_err(|e| CatalogError::Repo(e.to_string()))?;
            meta_repo
                .record_match(rom_id, &candidate, match_status)
                .await
                .map_err(|e| CatalogError::Repo(e.to_string()))?;
            crate::covers::invalidate(&self.covers_dir, rom_id);
        }

        Ok(meta)
    }

    /// Executa uma passada contra `/api/partner/v1/games/resolve`.
    /// Sem sessão marcada ou com a chave desligada em Configurações, volta na
    /// hora sem tocar no chaveiro nem na rede.
    pub async fn run_pass(
        &self,
        alva: &Alvanista,
        scope: PassScope,
        lang: Option<&str>,
    ) -> Result<PassReport, CatalogError> {
        if !alva.has_session().await || !self.is_enabled().await {
            return Ok(PassReport::default());
        }

        let _guard = self.pass_lock.lock().await;
        self.running.store(true, Ordering::Relaxed);
        let result = self.run_pass_locked(alva, scope, lang).await;
        self.running.store(false, Ordering::Relaxed);
        self.emit(CatalogEvent::Status);
        result
    }

    async fn run_pass_locked(
        &self,
        alva: &Alvanista,
        scope: PassScope,
        lang: Option<&str>,
    ) -> Result<PassReport, CatalogError> {
        if !alva.has_session().await || !self.is_enabled().await {
            return Ok(PassReport::default());
        }

        let mut report = PassReport::default();
        let now = self.now();
        let only_set: Option<HashSet<String>> = match &scope {
            PassScope::All => None,
            PassScope::Roms(ids) => Some(ids.iter().cloned().collect()),
        };

        report.identified_late =
            identify_unidentified(&self.db, only_set.as_ref(), now, self.is_playing.as_ref())
                .await
                .map_err(CatalogError::Repo)?;

        let res_repo = db::RomResolutionRepo::new(self.db.clone());
        let roms_repo = db::RomsRepo::new(self.db.clone());

        let target_ids: Vec<String> = match scope {
            PassScope::All => {
                let mut ids = res_repo
                    .unresolved_rom_ids()
                    .await
                    .map_err(|e| CatalogError::Repo(e.to_string()))?;
                let mut seen: HashSet<String> = ids.iter().cloned().collect();
                let due = res_repo
                    .pending_due(now)
                    .await
                    .map_err(|e| CatalogError::Repo(e.to_string()))?;
                for id in due {
                    if seen.insert(id.clone()) {
                        ids.push(id);
                    }
                }
                ids
            }
            PassScope::Roms(ids) => {
                let mut unique = Vec::with_capacity(ids.len());
                let mut seen = HashSet::new();
                for id in ids {
                    if !seen.insert(id.clone()) {
                        continue;
                    }
                    if let Ok(Some(existing)) = res_repo.get(&id).await {
                        if existing.state == ResolutionState::Manual {
                            continue;
                        }
                    }
                    unique.push(id);
                }
                unique
            }
        };

        if target_ids.is_empty() {
            return Ok(report);
        }

        let mut roms_to_send = Vec::with_capacity(target_ids.len());
        for id in &target_ids {
            if let Some(rom) = roms_repo
                .get(id)
                .await
                .map_err(|e| CatalogError::Repo(e.to_string()))?
            {
                if !rom.system_id.trim().is_empty() {
                    roms_to_send.push(rom);
                }
            }
        }
        if roms_to_send.is_empty() {
            return Ok(report);
        }

        let langs = langs_for(lang);
        let mut server_answered_ok = false;

        for chunk in roms_to_send.chunks(BATCH_SIZE) {
            if !alva.has_session().await || !self.is_enabled().await {
                break;
            }
            let items: Vec<ResolveItemReq<'_>> = chunk.iter().filter_map(item_from_rom).collect();
            if items.is_empty() {
                continue;
            }
            let payload = serde_json::to_value(ResolveBatchReq {
                langs: langs.clone(),
                items,
            })
            .map_err(|e| CatalogError::Rejected(e.to_string()))?;

            let parsed: ResolveBatchResp = self
                .call_partner(
                    alva,
                    reqwest::Method::POST,
                    "/api/partner/v1/games/resolve",
                    Some(&payload),
                    "unexpected_status",
                    &mut server_answered_ok,
                )
                .await?;

            report.sent += chunk.len();
            let batch_now = self.now();

            for item in parsed.results {
                let existing = res_repo
                    .get(&item.id)
                    .await
                    .map_err(|e| CatalogError::Repo(e.to_string()))?;
                if existing
                    .as_ref()
                    .is_some_and(|r| r.state == ResolutionState::Manual)
                {
                    continue;
                }

                match item.status.as_str() {
                    "found" => {
                        let Some(answer) = item.game else {
                            continue;
                        };
                        self.apply_answer_with_region(
                            &item.id,
                            &answer,
                            ResolutionState::Found,
                            item.matched_by,
                            item.region.as_deref(),
                            item.revision.as_deref(),
                            existing.as_ref(),
                            batch_now,
                        )
                        .await?;
                        report.found += 1;
                    }
                    "pending" => {
                        let wait = item
                            .retry_after_s
                            .filter(|&s| s > 0)
                            .unwrap_or(DEFAULT_PENDING_RETRY_SECS);
                        res_repo
                            .put(&RomResolution {
                                rom_id: item.id,
                                state: ResolutionState::Pending,
                                game_uuid: None,
                                matched_by: None,
                                reason: item.reason,
                                retry_after: Some(batch_now + wait),
                                media_pending: false,
                                answer_json: None,
                                resolved_at: batch_now,
                            })
                            .await
                            .map_err(|e| CatalogError::Repo(e.to_string()))?;
                        report.pending += 1;
                    }
                    "unknown" => {
                        let retry_after = (item.reason.as_deref() == Some("lookup_failed"))
                            .then_some(batch_now + LOOKUP_FAILED_RETRY_SECS);
                        res_repo
                            .put(&RomResolution {
                                rom_id: item.id,
                                state: ResolutionState::Unknown,
                                game_uuid: None,
                                matched_by: None,
                                reason: item.reason,
                                retry_after,
                                media_pending: false,
                                answer_json: None,
                                resolved_at: batch_now,
                            })
                            .await
                            .map_err(|e| CatalogError::Repo(e.to_string()))?;
                        report.unknown += 1;
                    }
                    _ => {}
                }
            }
        }

        if report.sent > 0 {
            let (total_found, total_pending, total_unknown) =
                res_repo
                    .counts()
                    .await
                    .unwrap_or((report.found, report.pending, report.unknown));
            self.emit(CatalogEvent::Pass {
                found: total_found,
                pending: total_pending,
                unknown: total_unknown,
            });
        }
        Ok(report)
    }
}

fn safe_catalog_asset_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let normalized = if let Some(rest) = trimmed.strip_prefix("//images.igdb.com/") {
        format!("https://images.igdb.com/{rest}")
    } else {
        trimmed.to_owned()
    };
    let upgraded = normalized
        .replace("/t_thumb/", "/t_1080p/")
        .replace("/t_cover_small/", "/t_1080p/");
    let parsed = reqwest::Url::parse(&upgraded).ok()?;
    if parsed.scheme() != "https" || !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    match parsed.host_str()? {
        "cdn.gamekave.net" | "images.igdb.com" => Some(parsed.to_string()),
        _ => None,
    }
}

fn push_unique_asset(list: &mut Vec<String>, candidate: Option<String>, max_len: usize) {
    let Some(url) = candidate else {
        return;
    };
    if list.len() < max_len && !list.contains(&url) {
        list.push(url);
    }
}

pub(crate) fn pick_best_public_catalog_cover(
    platform: Option<&str>,
    current_cover: Option<&str>,
    assets: Option<&serde_json::Value>,
    game: Option<&serde_json::Value>,
) -> Option<String> {
    let mut candidates: Vec<((u8, u8, u8), usize, String)> = Vec::new();
    let mut push = |url: String, is_default: bool, dims: Option<(u32, u32)>| {
        if candidates.iter().any(|(_, _, existing)| existing == &url) {
            return;
        }
        let idx = candidates.len();
        let rank = scraping::rom_name::cover_url_rank(&url, platform, is_default, dims);
        candidates.push((rank, idx, url));
    };

    let default_asset_url = assets
        .and_then(|a| a.pointer("/covers/default/url"))
        .and_then(serde_json::Value::as_str)
        .and_then(safe_catalog_asset_url);

    if let Some(cur) = current_cover.and_then(safe_catalog_asset_url) {
        let is_def = default_asset_url.as_deref() == Some(cur.as_str())
            && !scraping::rom_name::is_non_front_cover_url(&cur);
        push(cur, is_def, None);
    }

    if let Some(def) = default_asset_url.clone() {
        push(def, true, None);
    }

    if let Some(list) = assets
        .and_then(|a| a.pointer("/covers/list"))
        .and_then(serde_json::Value::as_array)
    {
        for it in list {
            if it["hidden"].as_bool() == Some(true) {
                continue;
            }
            let Some(url) = it["url"].as_str().and_then(safe_catalog_asset_url) else {
                continue;
            };
            let is_def = it["is_default"].as_bool() == Some(true)
                || default_asset_url.as_deref() == Some(url.as_str());
            let dims = match (
                it["width"].as_u64().and_then(|v| u32::try_from(v).ok()),
                it["height"].as_u64().and_then(|v| u32::try_from(v).ok()),
            ) {
                (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
                _ => None,
            };
            push(url, is_def, dims);
        }
    }

    if let Some(game) = game {
        let meta = &game["metadata"];
        for raw in [
            game["cover_url"].as_str(),
            meta["cover_url"].as_str(),
            meta.pointer("/cover/url")
                .and_then(serde_json::Value::as_str),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(url) = safe_catalog_asset_url(raw) {
                push(url, true, None);
            }
        }
    }

    candidates
        .into_iter()
        .min_by_key(|(rank, idx, _)| (*rank, *idx))
        .map(|(_, _, url)| url)
}

pub(crate) fn apply_public_catalog_to_answer(
    answer: &mut GameAnswer,
    assets: Option<&serde_json::Value>,
    game: Option<&serde_json::Value>,
) {
    if let Some(best_cover) = pick_best_public_catalog_cover(
        answer.platform.as_deref(),
        answer.media.cover.as_deref(),
        assets,
        game,
    ) {
        let should_replace = match answer.media.cover.as_deref() {
            None => true,
            Some(cur) if scraping::rom_name::is_non_front_cover_url(cur) => true,
            Some(cur) => {
                let cur_rank =
                    scraping::rom_name::cover_url_rank(cur, answer.platform.as_deref(), true, None);
                let new_rank = scraping::rom_name::cover_url_rank(
                    &best_cover,
                    answer.platform.as_deref(),
                    true,
                    None,
                );
                (new_rank.0, new_rank.1) < (cur_rank.0, cur_rank.1)
            }
        };
        if should_replace {
            answer.media.cover = Some(best_cover);
        }
    }

    if let Some(assets) = assets {
        if let Some(reg_obj) = assets["regional_covers"].as_object() {
            for (region, val) in reg_obj {
                let Some(url) = val.as_str().and_then(safe_catalog_asset_url) else {
                    continue;
                };
                if scraping::rom_name::is_non_front_cover_url(&url) {
                    continue;
                }
                let key = scraping::rom_name::normalize_region_code(region)
                    .map(str::to_owned)
                    .unwrap_or_else(|| region.trim().to_ascii_uppercase());
                if !key.is_empty() {
                    answer.media.regional_covers.entry(key).or_insert(url);
                }
            }
        }
        if answer.media.hero.is_none() {
            answer.media.hero = assets
                .pointer("/heroes/default/url")
                .and_then(serde_json::Value::as_str)
                .and_then(safe_catalog_asset_url)
                .or_else(|| {
                    assets
                        .pointer("/heroes/list")
                        .and_then(serde_json::Value::as_array)?
                        .iter()
                        .filter(|it| it["hidden"].as_bool() != Some(true))
                        .find_map(|it| it["url"].as_str().and_then(safe_catalog_asset_url))
                });
        }
        if answer.media.logo.is_none() {
            answer.media.logo = assets
                .pointer("/logos/default/url")
                .and_then(serde_json::Value::as_str)
                .and_then(safe_catalog_asset_url)
                .or_else(|| {
                    assets
                        .pointer("/logos/list")
                        .and_then(serde_json::Value::as_array)?
                        .iter()
                        .filter(|it| it["hidden"].as_bool() != Some(true))
                        .find_map(|it| it["url"].as_str().and_then(safe_catalog_asset_url))
                });
        }
        if let Some(shots) = assets
            .pointer("/screenshots/list")
            .and_then(serde_json::Value::as_array)
        {
            for it in shots {
                if it["hidden"].as_bool() == Some(true) {
                    continue;
                }
                push_unique_asset(
                    &mut answer.media.screenshots,
                    it["url"].as_str().and_then(safe_catalog_asset_url),
                    crate::covers::MAX_SCREENSHOTS,
                );
            }
        }
        if answer.media.title_screen.is_none() {
            answer.media.title_screen = assets
                .pointer("/other/list")
                .and_then(serde_json::Value::as_array)
                .and_then(|arr| {
                    arr.iter().find_map(|it| {
                        let stem = it["stem"].as_str().unwrap_or_default();
                        (stem.eq_ignore_ascii_case("title")
                            || stem.eq_ignore_ascii_case("screentitle"))
                        .then(|| it["url"].as_str().and_then(safe_catalog_asset_url))
                        .flatten()
                    })
                });
        }
        if answer.media.video.is_none() {
            answer.media.video = ["/trailers/list", "/videos/list"]
                .into_iter()
                .find_map(|ptr| {
                    assets
                        .pointer(ptr)
                        .and_then(serde_json::Value::as_array)?
                        .iter()
                        .filter(|it| it["hidden"].as_bool() != Some(true))
                        .find_map(|it| it["url"].as_str().and_then(safe_catalog_asset_url))
                });
        }
    }

    if let Some(game) = game {
        let meta = &game["metadata"];
        if answer.media.cover.is_none() {
            answer.media.cover = game["cover_url"]
                .as_str()
                .and_then(safe_catalog_asset_url)
                .or_else(|| meta["cover_url"].as_str().and_then(safe_catalog_asset_url))
                .or_else(|| {
                    meta.pointer("/cover/url")
                        .and_then(serde_json::Value::as_str)
                        .and_then(safe_catalog_asset_url)
                });
        }
        if answer.media.hero.is_none() {
            answer.media.hero = meta["hero_url"].as_str().and_then(safe_catalog_asset_url);
        }
        if answer.media.title_screen.is_none() {
            answer.media.title_screen = meta
                .pointer("/retroachievements/title_url")
                .and_then(serde_json::Value::as_str)
                .and_then(safe_catalog_asset_url);
        }
        if let Some(shots) = meta["screenshots"].as_array() {
            for item in shots {
                let raw = item.as_str().or_else(|| item["url"].as_str());
                push_unique_asset(
                    &mut answer.media.screenshots,
                    raw.and_then(safe_catalog_asset_url),
                    crate::covers::MAX_SCREENSHOTS,
                );
            }
        }
    }
}

#[cfg(not(test))]
async fn enrich_answer_from_public_catalog(answer: &mut GameAnswer) {
    let game_id = answer.game_id.trim();
    if game_id.is_empty() || !game_id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return;
    }
    let client = net::http::client();
    let assets_url = format!("https://api.gamekave.net/games/{game_id}/assets");
    let game_url = format!("https://api.gamekave.net/games/{game_id}");
    let fetch_json = |url: String| async move {
        client
            .get(&url)
            .timeout(std::time::Duration::from_secs(6))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()
    };
    let assets_task = tauri::async_runtime::spawn(fetch_json(assets_url));
    let game_task = tauri::async_runtime::spawn(fetch_json(game_url));
    let assets = assets_task.await.ok().flatten();
    let game = game_task.await.ok().flatten();
    apply_public_catalog_to_answer(answer, assets.as_ref(), game.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::Client;
    use crate::alvanista::store::test_support::{tokens, MemSecrets};
    use crate::alvanista::store::Store;
    use serde_json::json;
    use std::sync::atomic::AtomicI64;
    use std::sync::Mutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    struct Harness {
        db: db::Db,
        covers: tempfile::TempDir,
        alva: Alvanista,
        engine: CatalogEngine,
        playing: Arc<AtomicBool>,
        clock: Arc<AtomicI64>,
        #[allow(dead_code)]
        events: Arc<Mutex<Vec<CatalogEvent>>>,
    }

    async fn harness(server: &MockServer) -> Harness {
        let db = db::connect_in_memory().await.unwrap();
        let secrets = Arc::new(MemSecrets::default());
        let store = Store::new(db.clone(), secrets);
        store
            .start_session(&tokens("cat"), &["profile:read"])
            .await
            .unwrap();
        let client = Client::with_api(server.uri(), store);
        let alva = Alvanista::new(client);

        let covers = tempfile::tempdir().unwrap();
        let playing = Arc::new(AtomicBool::new(false));
        let clock = Arc::new(AtomicI64::new(1_700_000_000));
        let events = Arc::new(Mutex::new(Vec::new()));

        let p2 = playing.clone();
        let c2 = clock.clone();
        let e2 = events.clone();
        let engine = CatalogEngine::with_clock(
            db.clone(),
            covers.path().to_path_buf(),
            Arc::new(move || p2.load(Ordering::SeqCst)),
            Arc::new(move |ev| e2.lock().unwrap().push(ev)),
            Arc::new(move || c2.load(Ordering::SeqCst)),
        );

        Harness {
            db,
            covers,
            alva,
            engine,
            playing,
            clock,
            events,
        }
    }

    async fn add_rom(db: &db::Db, rom: Rom) {
        db::RomsRepo::new(db.clone()).add(&rom).await.unwrap();
    }

    #[tokio::test]
    async fn pass_sends_identity_fields() {
        let server = MockServer::start().await;
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/resolve"))
            .respond_with(move |req: &Request| {
                let val: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
                seen2.lock().unwrap().push(val);
                ResponseTemplate::new(200).set_body_json(json!({
                    "results": [
                        { "id": "r1", "status": "unknown", "reason": "not_indexed" },
                        { "id": "r2", "status": "unknown", "reason": "not_indexed" }
                    ]
                }))
            })
            .mount(&server)
            .await;

        let h = harness(&server).await;
        add_rom(
            &h.db,
            Rom {
                id: "r1".into(),
                file_path: "/roms/snes/Chrono Trigger (USA).sfc".into(),
                crc32: "2D206BF7".into(),
                md5: "A2BC447961E52FD2227BAED164F729DC".into(),
                sha1: Some("7FE6EE04AC6A46BEFD30E8E2AA0DC9D0E5792AE8".into()),
                system_id: "snes".into(),
                ident_name: Some("Chrono Trigger (USA).sfc".into()),
                ident_size: Some(4_194_304),
                identified_at: Some(100),
                ..Default::default()
            },
        )
        .await;
        add_rom(
            &h.db,
            Rom {
                id: "r2".into(),
                file_path: "/roms/psx/Castlevania.chd".into(),
                system_id: "psx".into(),
                serial: Some("slus-00067".into()),
                ident_name: Some("Castlevania.chd".into()),
                ident_size: Some(412_345_678),
                identified_at: Some(100),
                ..Default::default()
            },
        )
        .await;

        let report = h
            .engine
            .run_pass(&h.alva, PassScope::All, Some("pt-BR"))
            .await
            .unwrap();
        assert_eq!(report.sent, 2);
        assert_eq!(report.unknown, 2);

        let bodies = seen.lock().unwrap();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0]["langs"], json!(["pt", "en"]));
        let items = bodies[0]["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        let by_id = |id: &str| items.iter().find(|i| i["id"] == id).unwrap();
        let i1 = by_id("r1");
        assert_eq!(i1["system"], "snes");
        assert_eq!(i1["size"], 4_194_304);
        assert_eq!(i1["filename"], "Chrono Trigger (USA).sfc");
        assert_eq!(i1["crc32"], "2d206bf7");
        assert_eq!(i1["md5"], "a2bc447961e52fd2227baed164f729dc");
        assert_eq!(i1["sha1"], "7fe6ee04ac6a46befd30e8e2aa0dc9d0e5792ae8");
        assert!(i1.get("serial").is_none());

        let i2 = by_id("r2");
        assert_eq!(i2["system"], "psx");
        assert_eq!(i2["serial"], "SLUS-00067");
        assert_eq!(i2["filename"], "Castlevania.chd");
        assert!(i2.get("crc32").is_none() && i2.get("sha1").is_none());
    }

    #[tokio::test]
    async fn found_writes_metadata_and_match() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/resolve"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [{
                    "id": "r1",
                    "status": "found",
                    "matched_by": "sha1",
                    "game": {
                        "game_id": "01950f10-aaaa-7bbb-8ccc-111122223333",
                        "title": "Super Mario World",
                        "release_date": "1990-11-21",
                        "genres": ["Plataforma"],
                        "developer": "Nintendo EAD",
                        "publisher": "Nintendo",
                        "players": "1-2",
                        "description": "Clássico do SNES.",
                        "description_lang": "pt",
                        "media": {
                            "cover": "https://cdn.alvanista.com/smw/cover.webp",
                            "hero": "https://cdn.alvanista.com/smw/hero.webp"
                        },
                        "media_pending": false
                    }
                }]
            })))
            .mount(&server)
            .await;

        let h = harness(&server).await;
        let stale_cover = h.covers.path().join("r1.png");
        std::fs::write(&stale_cover, b"old").unwrap();

        add_rom(
            &h.db,
            Rom {
                id: "r1".into(),
                file_path: "/roms/snes/smw.sfc".into(),
                crc32: "B19ED489".into(),
                md5: "cdd3c8c37322978ca8669b34bc89c804".into(),
                system_id: "snes".into(),
                identified_at: Some(1),
                ..Default::default()
            },
        )
        .await;

        let report = h
            .engine
            .run_pass(&h.alva, PassScope::All, Some("pt-BR"))
            .await
            .unwrap();
        assert_eq!(report.found, 1);
        assert!(
            !stale_cover.exists(),
            "capa antiga deveria ter sido invalidada"
        );

        let res = db::RomResolutionRepo::new(h.db.clone())
            .get("r1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(res.state, ResolutionState::Found);
        assert_eq!(
            res.game_uuid.as_deref(),
            Some("01950f10-aaaa-7bbb-8ccc-111122223333")
        );
        assert_eq!(res.matched_by.as_deref(), Some("sha1"));
        assert!(!res.media_pending);
        assert_eq!(res.retry_after, None);

        let meta = db::MetadataRepo::new(h.db.clone())
            .get_metadata("r1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(meta.title, "Super Mario World");
        assert_eq!(
            meta.cover_url.as_deref(),
            Some("https://cdn.alvanista.com/smw/cover.webp")
        );
        assert_eq!(
            meta.game_uuid.as_deref(),
            Some("01950f10-aaaa-7bbb-8ccc-111122223333")
        );
        let without_match = db::MetadataRepo::new(h.db.clone())
            .rom_ids_without_match()
            .await
            .unwrap();
        assert!(without_match.is_empty());
    }

    #[tokio::test]
    async fn pending_is_asked_again_only_after_retry_after() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicI64::new(0));
        let calls2 = calls.clone();
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/resolve"))
            .respond_with(move |_: &Request| {
                let n = calls2.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "results": [{ "id": "r1", "status": "pending", "retry_after_s": 600 }]
                    }))
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "results": [{
                            "id": "r1",
                            "status": "found",
                            "matched_by": "crc32",
                            "game": {
                                "game_id": "01950f10-0000-7000-8000-000000000001",
                                "title": "Contra",
                                "media": {},
                                "media_pending": false
                            }
                        }]
                    }))
                }
            })
            .mount(&server)
            .await;

        let h = harness(&server).await;
        add_rom(
            &h.db,
            Rom {
                id: "r1".into(),
                file_path: "/roms/nes/Contra.nes".into(),
                crc32: "12345678".into(),
                md5: "0123456789abcdef0123456789abcdef".into(),
                system_id: "nes".into(),
                identified_at: Some(1),
                ..Default::default()
            },
        )
        .await;

        let r1 = h
            .engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();
        assert_eq!(r1.pending, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        h.clock.fetch_add(599, Ordering::SeqCst);
        let r2 = h
            .engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();
        assert_eq!(r2.sent, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        h.clock.fetch_add(1, Ordering::SeqCst);
        let r3 = h
            .engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();
        assert_eq!(r3.found, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn found_with_media_pending_is_asked_again_until_the_media_arrive() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicI64::new(0));
        let calls2 = calls.clone();
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/resolve"))
            .respond_with(move |_: &Request| {
                let n = calls2.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "results": [{
                            "id": "r1",
                            "status": "found",
                            "matched_by": "sha1",
                            "game": {
                                "game_id": "01950f10-0000-7000-8000-000000000002",
                                "title": "EarthBound",
                                "media": {},
                                "media_pending": true
                            }
                        }]
                    }))
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "results": [{
                            "id": "r1",
                            "status": "found",
                            "matched_by": "sha1",
                            "game": {
                                "game_id": "01950f10-0000-7000-8000-000000000002",
                                "title": "EarthBound",
                                "media": { "cover": "https://cdn.alvanista.com/eb/cover.webp" },
                                "media_pending": false
                            }
                        }]
                    }))
                }
            })
            .mount(&server)
            .await;

        let h = harness(&server).await;
        add_rom(
            &h.db,
            Rom {
                id: "r1".into(),
                file_path: "/roms/snes/EarthBound.sfc".into(),
                system_id: "snes".into(),
                identified_at: Some(1),
                ..Default::default()
            },
        )
        .await;

        h.engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();
        let meta1 = db::MetadataRepo::new(h.db.clone())
            .get_metadata("r1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(meta1.cover_url, None);

        h.clock.fetch_add(3599, Ordering::SeqCst);
        let r_early = h
            .engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();
        assert_eq!(r_early.sent, 0);

        h.clock.fetch_add(1, Ordering::SeqCst);
        let r_second = h
            .engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();
        assert_eq!(r_second.found, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let meta2 = db::MetadataRepo::new(h.db.clone())
            .get_metadata("r1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            meta2.cover_url.as_deref(),
            Some("https://cdn.alvanista.com/eb/cover.webp")
        );
        let res2 = db::RomResolutionRepo::new(h.db.clone())
            .get("r1")
            .await
            .unwrap()
            .unwrap();
        assert!(!res2.media_pending);
        assert_eq!(res2.retry_after, None);

        h.clock.fetch_add(86_400, Ordering::SeqCst);
        let r_third = h
            .engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();
        assert_eq!(r_third.sent, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn disabled_flag_stops_and_keeps_old_answers() {
        let server = MockServer::start().await;
        let disabled = Arc::new(AtomicBool::new(false));
        let d2 = disabled.clone();
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/resolve"))
            .respond_with(move |_: &Request| {
                if d2.load(Ordering::SeqCst) {
                    ResponseTemplate::new(503)
                        .set_body_json(json!({ "error": { "code": "rom_resolve_disabled" } }))
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "results": [{
                            "id": "r1",
                            "status": "found",
                            "matched_by": "sha1",
                            "game": {
                                "game_id": "01950f10-0000-7000-8000-000000000003",
                                "title": "F-Zero",
                                "media": { "cover": "https://cdn.alvanista.com/fz/cover.webp" },
                                "media_pending": false
                            }
                        }]
                    }))
                }
            })
            .mount(&server)
            .await;

        let h = harness(&server).await;
        add_rom(
            &h.db,
            Rom {
                id: "r1".into(),
                file_path: "/roms/snes/F-Zero.sfc".into(),
                system_id: "snes".into(),
                identified_at: Some(1),
                ..Default::default()
            },
        )
        .await;
        h.engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();

        disabled.store(true, Ordering::SeqCst);
        add_rom(
            &h.db,
            Rom {
                id: "r2".into(),
                file_path: "/roms/snes/Pilotwings.sfc".into(),
                system_id: "snes".into(),
                identified_at: Some(1),
                ..Default::default()
            },
        )
        .await;

        let err = h
            .engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap_err();
        assert_eq!(err, CatalogError::Disabled);
        assert!(h.engine.status(&h.alva).await.disabled_by_server);

        let kept = db::MetadataRepo::new(h.db.clone())
            .get_metadata("r1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(kept.title, "F-Zero");
    }

    #[tokio::test]
    async fn user_choice_is_not_overwritten() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/resolve"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [
                    {
                        "id": "r_confirmed",
                        "status": "found",
                        "matched_by": "crc32",
                        "game": {
                            "game_id": "01950f10-0000-7000-8000-999999999999",
                            "title": "Jogo Errado do Servidor",
                            "media": {},
                            "media_pending": false
                        }
                    },
                    {
                        "id": "r_manual",
                        "status": "found",
                        "matched_by": "crc32",
                        "game": {
                            "game_id": "01950f10-0000-7000-8000-888888888888",
                            "title": "Outro Jogo Errado",
                            "media": {},
                            "media_pending": false
                        }
                    }
                ]
            })))
            .mount(&server)
            .await;

        let h = harness(&server).await;
        for id in ["r_confirmed", "r_manual"] {
            add_rom(
                &h.db,
                Rom {
                    id: id.into(),
                    file_path: format!("/roms/snes/{id}.sfc"),
                    system_id: "snes".into(),
                    identified_at: Some(1),
                    ..Default::default()
                },
            )
            .await;
        }

        let meta_repo = db::MetadataRepo::new(h.db.clone());
        let user_cand = domain::metadata::ScrapeCandidate {
            provider: "thegamesdb".into(),
            external_id: "42".into(),
            title: "Escolha Confirmada".into(),
            description: None,
            cover_url: None,
            release_date: None,
            genre: None,
            exact_hash_match: false,
            exact_filename_match: false,
            details: Default::default(),
        };
        meta_repo
            .record_match("r_confirmed", &user_cand, MatchStatus::UserConfirmed)
            .await
            .unwrap();
        meta_repo
            .upsert_metadata(&GameMetadata {
                rom_id: "r_confirmed".into(),
                title: "Escolha Confirmada".into(),
                provider_source: Some("thegamesdb".into()),
                ..Default::default()
            })
            .await
            .unwrap();

        db::RomResolutionRepo::new(h.db.clone())
            .put(&RomResolution {
                rom_id: "r_manual".into(),
                state: ResolutionState::Manual,
                game_uuid: Some("01950f10-0000-7000-8000-111111111111".into()),
                matched_by: None,
                reason: None,
                retry_after: None,
                media_pending: false,
                answer_json: None,
                resolved_at: 100,
            })
            .await
            .unwrap();
        meta_repo
            .upsert_metadata(&GameMetadata {
                rom_id: "r_manual".into(),
                title: "Escolha Manual".into(),
                provider_source: Some("alvanista".into()),
                game_uuid: Some("01950f10-0000-7000-8000-111111111111".into()),
                ..Default::default()
            })
            .await
            .unwrap();

        h.engine
            .run_pass(
                &h.alva,
                PassScope::Roms(vec!["r_confirmed".into(), "r_manual".into()]),
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            meta_repo
                .get_metadata("r_confirmed")
                .await
                .unwrap()
                .unwrap()
                .title,
            "Escolha Confirmada"
        );
        assert_eq!(
            meta_repo
                .get_metadata("r_manual")
                .await
                .unwrap()
                .unwrap()
                .title,
            "Escolha Manual"
        );
        assert_eq!(
            db::RomResolutionRepo::new(h.db.clone())
                .get("r_manual")
                .await
                .unwrap()
                .unwrap()
                .state,
            ResolutionState::Manual
        );
    }

    #[tokio::test]
    async fn late_identification_runs_for_old_rows_and_never_while_playing() {
        let dir = tempfile::tempdir().unwrap();
        let track: Vec<u8> = (0..2352 * 4).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.path().join("Jogo (Track 1).bin"), &track).unwrap();
        let cue_text =
            "FILE \"Jogo (Track 1).bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n";
        let cue_path = dir.path().join("Jogo.cue");
        std::fs::write(&cue_path, cue_text.as_bytes()).unwrap();

        let expected = library_scan::identify(&cue_path, "psx", None).unwrap();
        let expected_hashes = expected.hashes.unwrap();

        let server = MockServer::start().await;
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let seen2 = seen.clone();
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/resolve"))
            .respond_with(move |req: &Request| {
                seen2
                    .lock()
                    .unwrap()
                    .push(serde_json::from_slice(&req.body).unwrap());
                ResponseTemplate::new(200).set_body_json(json!({
                    "results": [{ "id": "old_cue", "status": "unknown", "reason": "not_indexed" }]
                }))
            })
            .mount(&server)
            .await;

        let h = harness(&server).await;
        add_rom(
            &h.db,
            Rom {
                id: "old_cue".into(),
                file_path: cue_path.to_string_lossy().into_owned(),
                crc32: "00000000".into(),
                md5: "00000000000000000000000000000000".into(),
                system_id: "psx".into(),
                identified_at: None,
                ..Default::default()
            },
        )
        .await;

        h.playing.store(true, Ordering::SeqCst);
        let identified_while_playing =
            identify_unidentified(&h.db, None, h.engine.now(), h.engine.is_playing.as_ref())
                .await
                .unwrap();
        assert_eq!(identified_while_playing, 0);
        assert_eq!(
            db::RomsRepo::new(h.db.clone())
                .get("old_cue")
                .await
                .unwrap()
                .unwrap()
                .identified_at,
            None
        );

        h.playing.store(false, Ordering::SeqCst);
        let report = h
            .engine
            .run_pass(&h.alva, PassScope::All, None)
            .await
            .unwrap();
        assert_eq!(report.identified_late, 1);

        let updated = db::RomsRepo::new(h.db.clone())
            .get("old_cue")
            .await
            .unwrap()
            .unwrap();
        assert!(updated.identified_at.is_some());
        assert_eq!(updated.ident_name.as_deref(), Some("Jogo (Track 1).bin"));
        assert_eq!(updated.sha1.as_deref(), Some(expected_hashes.sha1.as_str()));

        let bodies = seen.lock().unwrap();
        let sent_item = &bodies[0]["items"][0];
        assert_eq!(sent_item["filename"], "Jogo (Track 1).bin");
        assert_eq!(sent_item["size"], track.len() as i64);
        assert_eq!(sent_item["sha1"], expected_hashes.sha1);
        assert_eq!(
            sent_item["crc32"],
            expected_hashes.crc32.to_ascii_lowercase()
        );
    }

    #[tokio::test]
    async fn pick_writes_manual_resolution_and_metadata() {
        let server = MockServer::start().await;
        let game_id = "01950f10-aaaa-7bbb-8ccc-ff66ff66ff66";

        Mock::given(method("GET"))
            .and(path("/api/partner/v1/games/search"))
            .respond_with(|req: &Request| {
                let query: std::collections::HashMap<String, String> = req
                    .url
                    .query_pairs()
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect();
                assert_eq!(query.get("q").map(String::as_str), Some("Final Fantasy"));
                assert_eq!(query.get("system").map(String::as_str), Some("snes"));
                ResponseTemplate::new(200).set_body_json(json!({
                    "games": [
                        {
                            "game_id": "01950f10-aaaa-7bbb-8ccc-ff66ff66ff66",
                            "title": "Final Fantasy VI",
                            "platforms": ["Super Nintendo"],
                            "release_date": "1994-04-02",
                            "cover": "https://cdn.alvanista.com/ff6/cover.webp"
                        },
                        {
                            "game_id": "01950f10-aaaa-7bbb-8ccc-ff44ff44ff44",
                            "title": "Final Fantasy IV",
                            "platforms": ["Super Nintendo"],
                            "release_date": "1991-07-19",
                            "cover": "http://insecure.example/cover.png"
                        }
                    ]
                }))
            })
            .mount(&server)
            .await;

        let seen_claims: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_claims2 = seen_claims.clone();
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/claims"))
            .respond_with(move |req: &Request| {
                seen_claims2
                    .lock()
                    .unwrap()
                    .push(serde_json::from_slice(&req.body).unwrap());
                ResponseTemplate::new(200).set_body_json(json!({
                    "accepted": 1,
                    "promoted": false,
                    "game": {
                        "game_id": game_id,
                        "title": "Final Fantasy VI",
                        "synopsis": "A Guerra dos Magi.",
                        "release_date": "1994-04-02",
                        "developers": ["Square"],
                        "publishers": ["Square"],
                        "genres": ["RPG"],
                        "media": {
                            "cover": "https://cdn.alvanista.com/ff6/cover.webp",
                            "hero": "https://cdn.alvanista.com/ff6/hero.webp"
                        },
                        "media_pending": true
                    }
                }))
            })
            .mount(&server)
            .await;

        let h = harness(&server).await;
        add_rom(
            &h.db,
            Rom {
                id: "rom_ff6".into(),
                file_path: "/roms/snes/FF6 T-BR.sfc".into(),
                crc32: "11223344".into(),
                md5: "00112233445566778899aabbccddeeff".into(),
                sha1: Some("0123456789abcdef0123456789abcdef01234567".into()),
                system_id: "snes".into(),
                ident_name: Some("FF6 T-BR.sfc".into()),
                ident_size: Some(3_145_728),
                identified_at: Some(100),
                ..Default::default()
            },
        )
        .await;

        let hits = h
            .engine
            .search(&h.alva, "  Final Fantasy  ", "snes")
            .await
            .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].title, "Final Fantasy VI");
        assert_eq!(
            hits[0].cover.as_deref(),
            Some("https://cdn.alvanista.com/ff6/cover.webp")
        );
        // URL http:// é descartada na fronteira.
        assert_eq!(hits[1].cover, None);

        let meta = h
            .engine
            .pick(&h.alva, "rom_ff6", game_id, Some("pt-BR"))
            .await
            .unwrap();
        assert_eq!(meta.title, "Final Fantasy VI");
        assert_eq!(meta.game_uuid.as_deref(), Some(game_id));
        assert_eq!(meta.provider_source.as_deref(), Some("alvanista"));

        {
            let claims = seen_claims.lock().unwrap();
            assert_eq!(claims.len(), 1);
            assert_eq!(claims[0]["game_id"], game_id);
            assert_eq!(claims[0]["langs"], json!(["pt", "en"]));
            assert_eq!(claims[0]["items"][0]["crc32"], "11223344");
            assert_eq!(claims[0]["items"][0]["filename"], "FF6 T-BR.sfc");
        }

        let res = db::RomResolutionRepo::new(h.db.clone())
            .get("rom_ff6")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(res.state, ResolutionState::Manual);
        assert_eq!(res.game_uuid.as_deref(), Some(game_id));
        assert!(!res.media_pending);

        let meta_repo = db::MetadataRepo::new(h.db.clone());
        let saved = meta_repo.get_metadata("rom_ff6").await.unwrap().unwrap();
        assert_eq!(saved.title, "Final Fantasy VI");
        assert_eq!(saved.game_uuid.as_deref(), Some(game_id));
        assert!(meta_repo.is_user_confirmed("rom_ff6").await.unwrap());
    }

    #[tokio::test]
    async fn manual_resolution_survives_a_later_pass() {
        let server = MockServer::start().await;
        let chosen_id = "01950f10-aaaa-7bbb-8ccc-ff66ff66ff66";
        let other_id = "01950f10-aaaa-7bbb-8ccc-000000000000";

        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/claims"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "accepted": 1,
                "promoted": false,
                "game": {
                    "game_id": chosen_id,
                    "title": "Final Fantasy VI",
                    "media": { "cover": "https://cdn.alvanista.com/ff6/cover.webp" },
                    "media_pending": false
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/partner/v1/games/resolve"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": [
                    {
                        "id": "rom_manual",
                        "status": "found",
                        "matched_by": "sha1",
                        "game": {
                            "game_id": other_id,
                            "title": "Outro Jogo Automático",
                            "media": {},
                            "media_pending": false
                        }
                    },
                    {
                        "id": "rom_other",
                        "status": "unknown",
                        "reason": "not_indexed"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let h = harness(&server).await;
        for id in ["rom_manual", "rom_other"] {
            add_rom(
                &h.db,
                Rom {
                    id: id.into(),
                    file_path: format!("/roms/snes/{id}.sfc"),
                    crc32: "11223344".into(),
                    md5: "00112233445566778899aabbccddeeff".into(),
                    system_id: "snes".into(),
                    identified_at: Some(100),
                    ..Default::default()
                },
            )
            .await;
        }

        h.engine
            .pick(&h.alva, "rom_manual", chosen_id, Some("pt"))
            .await
            .unwrap();

        // Nem uma passada geral nem uma passada explícita da própria ROM podem
        // sobrescrever a escolha manual.
        let r_all = h
            .engine
            .run_pass(&h.alva, PassScope::All, Some("pt"))
            .await
            .unwrap();
        assert_eq!(r_all.sent, 1);
        assert_eq!(r_all.found, 0);

        let r_explicit = h
            .engine
            .run_pass(
                &h.alva,
                PassScope::Roms(vec!["rom_manual".into(), "rom_other".into()]),
                Some("pt"),
            )
            .await
            .unwrap();
        assert_eq!(r_explicit.sent, 1);
        assert_eq!(r_explicit.found, 0);

        let res = db::RomResolutionRepo::new(h.db.clone())
            .get("rom_manual")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(res.state, ResolutionState::Manual);
        assert_eq!(res.game_uuid.as_deref(), Some(chosen_id));

        let saved = db::MetadataRepo::new(h.db.clone())
            .get_metadata("rom_manual")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.title, "Final Fantasy VI");
        assert_eq!(saved.game_uuid.as_deref(), Some(chosen_id));
    }

    #[test]
    fn apply_public_catalog_fills_missing_cover_title_screen_and_screenshots() {
        let mut answer = GameAnswer {
            game_id: "2015da97-ad78-4020-97ff-4caa387cf66d".into(),
            title: "Spawn: Armageddon".into(),
            platform: Some("nintendo-gamecube".into()),
            media: super::super::answer::Media::default(),
            media_pending: false,
            ..Default::default()
        };
        let assets = json!({
            "covers": {
                "default": {
                    "url": "https://cdn.gamekave.net/games/2015da97-ad78-4020-97ff-4caa387cf66d/cover/1080p.jpg"
                },
                "list": []
            },
            "regional_covers": {
                "jp": "https://cdn.gamekave.net/igdb-assets/covers/co8nm4.jpg",
                "us": "https://www.screenscraper.fr/image.php?devid=secret"
            },
            "screenshots": {
                "list": [
                    {"hidden": false, "url": "https://cdn.gamekave.net/games/2015da97-ad78-4020-97ff-4caa387cf66d/screenshot/igdb/screenshot_1.jpg"},
                    {"hidden": true, "url": "https://cdn.gamekave.net/games/2015da97-ad78-4020-97ff-4caa387cf66d/screenshot/igdb/hidden.jpg"}
                ]
            },
            "other": {
                "list": [
                    {"stem": "title", "url": "https://cdn.gamekave.net/games/2015da97-ad78-4020-97ff-4caa387cf66d/retroachievements/title.png"}
                ]
            }
        });
        let game = json!({
            "metadata": {
                "screenshots": [
                    {"url": "//images.igdb.com/igdb/image/upload/t_thumb/scuazf.jpg"}
                ]
            }
        });

        apply_public_catalog_to_answer(&mut answer, Some(&assets), Some(&game));

        assert_eq!(
            answer.media.cover.as_deref(),
            Some("https://cdn.gamekave.net/games/2015da97-ad78-4020-97ff-4caa387cf66d/cover/1080p.jpg")
        );
        assert_eq!(
            answer.media.regional_covers.get("JP").map(String::as_str),
            Some("https://cdn.gamekave.net/igdb-assets/covers/co8nm4.jpg")
        );
        assert!(!answer.media.regional_covers.contains_key("NA"));
        assert_eq!(
            answer.media.title_screen.as_deref(),
            Some("https://cdn.gamekave.net/games/2015da97-ad78-4020-97ff-4caa387cf66d/retroachievements/title.png")
        );
        assert_eq!(
            answer.media.screenshots,
            vec![
                "https://cdn.gamekave.net/games/2015da97-ad78-4020-97ff-4caa387cf66d/screenshot/igdb/screenshot_1.jpg",
                "https://images.igdb.com/igdb/image/upload/t_1080p/scuazf.jpg"
            ]
        );
    }

    #[test]
    fn apply_public_catalog_replaces_box_texture_with_front_and_square_cover() {
        let mut answer = GameAnswer {
            game_id: "27079dab-d849-4d1b-9b77-3b3a81f63cfd".into(),
            title: "Gremlins 2 : The New Batch".into(),
            platform: Some("game-boy".into()),
            media: super::super::answer::Media {
                cover: Some(
                    "https://cdn.gamekave.net/games/27079dab-d849-4d1b-9b77-3b3a81f63cfd/cover/screenscraper/box-texture_0af13c71.png"
                        .into(),
                ),
                ..Default::default()
            },
            media_pending: false,
            ..Default::default()
        };
        let assets = json!({
            "covers": {
                "list": [
                    {
                        "hidden": false,
                        "is_default": false,
                        "url": "https://cdn.gamekave.net/games/27079dab-d849-4d1b-9b77-3b3a81f63cfd/cover/screenscraper/box-texture_0af13c71.png"
                    },
                    {
                        "hidden": false,
                        "is_default": false,
                        "url": "https://cdn.gamekave.net/games/27079dab-d849-4d1b-9b77-3b3a81f63cfd/cover/screenscraper/box-3D_b4c06009.png"
                    },
                    {
                        "hidden": false,
                        "is_default": false,
                        "url": "https://cdn.gamekave.net/games/27079dab-d849-4d1b-9b77-3b3a81f63cfd/cover/screenscraper/box-2D_d75bd986.png"
                    },
                    {
                        "hidden": false,
                        "is_default": true,
                        "url": "https://cdn.gamekave.net/games/27079dab-d849-4d1b-9b77-3b3a81f63cfd/cover/screenscraper/cover_d5a30ce5.png"
                    }
                ]
            }
        });

        apply_public_catalog_to_answer(&mut answer, Some(&assets), None);

        assert_eq!(
            answer.media.cover.as_deref(),
            Some("https://cdn.gamekave.net/games/27079dab-d849-4d1b-9b77-3b3a81f63cfd/cover/screenscraper/cover_d5a30ce5.png")
        );
    }
}
