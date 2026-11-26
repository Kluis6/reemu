//! RetroAchievements no processo principal (`reemu-desktop`):
//! - cliente HTTP único (`reqwest`) com `User-Agent: ReEmu/<versão> rcheevos/<versão>`
//!   compartilhado entre login, página do jogo e proxy IPC do `reemu-core-host`;
//! - comandos Tauri `ra_status`, `ra_login`, `ra_logout`, `ra_set_preferences` e
//!   `ra_game_achievements`;
//! - espelho em memória do jogo rodando agora (`LiveGameAchievements`), atualizado
//!   pelos eventos do core para refletir desbloqueios na hora e bloquear cheats /
//!   carregamento de save state durante sessões Hardcore.

use crate::app_error::{AppError, ErrorCode};
use crate::commands::AppState;
use crate::credentials::{self, OsKeyring, RaConfig};
use achievements::{
    console::{RC_CONSOLE_ARCADE, RC_CONSOLE_ARDUBOY, RC_CONSOLE_MS_DOS},
    console_id_for_system, hash_buffer, hash_file, AchievementClient, AchievementEntry,
    AchievementEvent, AchievementKind, AsyncOutcome, GameSummary, OutboundHttpRequest, RaError,
    RaLoadedSnapshot, RaSessionConfig,
};
use domain::library::RomRepository as _;
use emu_session::RaHostMessage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
/// Limite oficial `MAX_BUFFER_SIZE` do `rcheevos` (`rc_hash_internal.h`: 64 MiB).
const RA_MAX_ROM_BUFFER: usize = 64 * 1024 * 1024;

/// Calcula o hash oficial do `rcheevos` para `path`, extraindo em memória a ROM
/// interna quando o arquivo está em `.zip` ou `.7z` (exceto Arcade, MS-DOS e
/// pacotes `.arduboy`, nos quais o `rcheevos` hasheia o próprio contêiner).
fn compute_rom_hash(console_id: u32, system_id: &str, path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    let hash_inner_archive = library_scan::is_supported_archive(ext)
        && !matches!(
            console_id,
            RC_CONSOLE_ARCADE | RC_CONSOLE_MS_DOS | RC_CONSOLE_ARDUBOY
        );
    if hash_inner_archive {
        if let Some(archived) = library_scan::peek_archive(path, Some(system_id)) {
            if let Ok(bytes) =
                library_scan::read_archive_entry_capped(path, &archived.entry, RA_MAX_ROM_BUFFER)
            {
                if let Some(h) = hash_buffer(console_id, &bytes) {
                    return Some(h);
                }
            }
        }
    }
    hash_file(console_id, path)
}

async fn resolve_rom_hash(
    ra_state: &RaState,
    console_id: u32,
    rom_id: &str,
    system_id: &str,
    file_path: &str,
) -> Option<String> {
    if let Some(cached) = ra_state.cached_hash(rom_id, file_path) {
        return Some(cached);
    }
    let sys = system_id.to_owned();
    let path_owned = file_path.to_owned();
    let computed = tauri::async_runtime::spawn_blocking(move || {
        compute_rom_hash(console_id, &sys, Path::new(&path_owned))
    })
    .await
    .ok()
    .flatten()?;
    ra_state.store_hash(rom_id, file_path, computed.clone());
    Some(computed)
}

#[derive(Default)]
pub struct RaState {
    http: Mutex<Option<reqwest::Client>>,
    /// Hash RA por `(rom_id, file_path)` para não reler a ROM inteira ao reabrir a aba.
    rom_hashes: Mutex<HashMap<(String, String), String>>,
    /// `rom_id` da partida atualmente em execução (para anexar ao `LiveGameAchievements`).
    active_rom_id: Mutex<Option<String>>,
    /// `true` enquanto o jogo aberto iniciou uma sessão de conquistas em modo Hardcore.
    hardcore_active: AtomicBool,
    /// Estado vivo da partida aberta agora.
    live: Mutex<Option<LiveGameAchievements>>,
}

#[derive(Debug, Clone)]
pub struct LiveGameAchievements {
    pub rom_id: Option<String>,
    pub game_id: u32,
    pub title: String,
    pub badge_url: Option<String>,
    pub hardcore: bool,
    pub summary: GameSummary,
    pub achievements: Vec<AchievementEntry>,
}

impl RaState {
    pub fn http_client(&self) -> reqwest::Client {
        let mut guard = self.http.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(c) = guard.as_ref() {
            return c.clone();
        }
        let mut tmp = AchievementClient::new();
        let ua = format!(
            "ReEmu/{} {}",
            env!("CARGO_PKG_VERSION"),
            tmp.user_agent_clause()
        );
        let client = reqwest::Client::builder()
            .user_agent(ua)
            .timeout(HTTP_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        *guard = Some(client.clone());
        client
    }

    pub fn is_hardcore_active(&self) -> bool {
        self.hardcore_active.load(Ordering::Acquire)
    }

    pub fn set_hardcore_active(&self, active: bool) {
        self.hardcore_active.store(active, Ordering::Release);
    }

    pub fn set_active_rom(&self, rom_id: Option<String>) {
        *self.active_rom_id.lock().unwrap_or_else(|p| p.into_inner()) = rom_id;
    }

    pub fn active_rom(&self) -> Option<String> {
        self.active_rom_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn set_live(&self, snapshot: Option<LiveGameAchievements>) {
        *self.live.lock().unwrap_or_else(|p| p.into_inner()) = snapshot;
    }

    pub fn mark_live_unlocked(&self, achievement_id: u32, hardcore: bool) {
        let mut guard = self.live.lock().unwrap_or_else(|p| p.into_inner());
        let Some(live) = guard.as_mut() else {
            return;
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .ok();
        if let Some(item) = live
            .achievements
            .iter_mut()
            .find(|a| a.id == achievement_id)
        {
            if !item.unlocked && !item.unofficial {
                live.summary.num_unlocked_achievements =
                    live.summary.num_unlocked_achievements.saturating_add(1);
                live.summary.points_unlocked =
                    live.summary.points_unlocked.saturating_add(item.points);
            }
            item.unlocked = true;
            item.unlocked_hardcore |= hardcore;
            if item.unlock_time.is_none() {
                item.unlock_time = now;
            }
        }
    }

    pub fn live_for(&self, rom_id: &str) -> Option<LiveGameAchievements> {
        self.live
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|l| l.rom_id.as_deref() == Some(rom_id))
            .cloned()
    }

    fn cached_hash(&self, rom_id: &str, file_path: &str) -> Option<String> {
        self.rom_hashes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(rom_id.to_owned(), file_path.to_owned()))
            .cloned()
    }

    fn store_hash(&self, rom_id: &str, file_path: &str, hash: String) {
        self.rom_hashes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert((rom_id.to_owned(), file_path.to_owned()), hash);
    }
}

/// Executa um pedido HTTP construído pelo `rcheevos` (`dorequest.php` ou mídia).
/// Usado tanto pelos comandos deste módulo quanto pelo proxy IPC do core.
pub async fn execute_rcheevos_http(
    http: &reqwest::Client,
    req: &OutboundHttpRequest,
) -> (i32, Vec<u8>) {
    let builder = if let Some(post) = &req.post_data {
        let ct = req
            .content_type
            .as_deref()
            .unwrap_or("application/x-www-form-urlencoded");
        http.post(&req.url)
            .header(reqwest::header::CONTENT_TYPE, ct)
            .body(post.clone())
    } else {
        http.get(&req.url)
    };

    match builder.send().await {
        Ok(resp) => {
            let status = i32::from(resp.status().as_u16());
            match resp.bytes().await {
                Ok(bytes) => (status, bytes.to_vec()),
                Err(_) => (
                    achievements::ffi::RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR,
                    Vec::new(),
                ),
            }
        }
        Err(_) => (
            achievements::ffi::RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR,
            Vec::new(),
        ),
    }
}

async fn drive_client_requests(http: &reqwest::Client, client: &mut AchievementClient) {
    loop {
        let batch = client.take_http_requests();
        if batch.is_empty() {
            break;
        }
        for req in batch {
            let (status, body) = execute_rcheevos_http(http, &req).await;
            client.complete_http_request(req.id, status, &body);
        }
    }
}

/// Liga os pedidos HTTP e eventos do `EmuSession` ao `reqwest` e aos eventos Tauri.
pub fn install_session_bridge(app: &AppHandle) {
    let app_handle = app.clone();
    let session = Arc::clone(&app.state::<AppState>().session);
    session.set_ra_sink(Arc::new(move |msg| match msg {
        RaHostMessage::HttpRequest { session_gen, req } => {
            let app_h = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let http = app_h.state::<RaState>().http_client();
                let (status, body) = execute_rcheevos_http(&http, &req).await;
                app_h.state::<AppState>().session.deliver_ra_http_response(
                    session_gen,
                    req.id,
                    status,
                    body,
                );
            });
        }
        RaHostMessage::GameLoaded { snapshot, .. } => {
            handle_game_loaded(&app_handle, snapshot);
        }
        RaHostMessage::Event { event, .. } => {
            handle_runtime_event(&app_handle, event);
        }
    }));
}

fn handle_game_loaded(app: &AppHandle, snapshot: RaLoadedSnapshot) {
    let ra_state = app.state::<RaState>();
    let rom_id = ra_state.active_rom();
    let event_dto = RaGameLoadedEventDto {
        rom_id: rom_id.clone(),
        game_id: snapshot.game_id,
        title: snapshot.title.clone(),
        badge_url: snapshot.badge_url.clone(),
        hardcore: snapshot.hardcore,
        unlocked_achievements: snapshot.summary.num_unlocked_achievements,
        total_achievements: snapshot.summary.num_core_achievements,
        unlocked_points: snapshot.summary.points_unlocked,
        total_points: snapshot.summary.points_core,
    };
    ra_state.set_live(Some(LiveGameAchievements {
        rom_id,
        game_id: snapshot.game_id,
        title: snapshot.title,
        badge_url: snapshot.badge_url,
        hardcore: snapshot.hardcore,
        summary: snapshot.summary,
        achievements: snapshot.achievements,
    }));
    let _ = app.emit("ra-game-loaded", event_dto);
}

fn handle_runtime_event(app: &AppHandle, event: AchievementEvent) {
    let ra_state = app.state::<RaState>();
    match event {
        AchievementEvent::AchievementTriggered {
            id,
            title,
            description,
            points,
            hardcore,
            badge_url,
        } => {
            ra_state.mark_live_unlocked(id, hardcore);
            let _ = app.emit(
                "ra-achievement-unlocked",
                RaUnlockEventDto {
                    rom_id: ra_state.active_rom(),
                    id,
                    title,
                    description,
                    points,
                    hardcore,
                    badge_url,
                    game_completed: false,
                },
            );
        }
        AchievementEvent::GameCompleted { hardcore } => {
            let live_title = ra_state
                .live
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .map(|l| (l.title.clone(), l.badge_url.clone()));
            if let Some((title, badge_url)) = live_title {
                let _ = app.emit(
                    "ra-achievement-unlocked",
                    RaUnlockEventDto {
                        rom_id: ra_state.active_rom(),
                        id: 0,
                        title,
                        description: String::new(),
                        points: 0,
                        hardcore,
                        badge_url,
                        game_completed: true,
                    },
                );
            }
        }
        AchievementEvent::ServerError { api, message, .. } => {
            log::warn!("ra ({api}): {message}");
        }
        _ => {}
    }
}

/// Chamado em `load_game` logo depois de abrir o core. Devolve `true` quando o
/// modo Hardcore está ativo para esta partida (desabilitando cheats e load state).
pub async fn on_game_launched(app: &AppHandle, rom_id: Option<&str>, rom_path: &str) -> bool {
    let ra_state = app.state::<RaState>();
    ra_state.set_live(None);
    ra_state.set_hardcore_active(false);
    ra_state.set_active_rom(rom_id.map(str::to_owned));

    let state = app.state::<AppState>();
    let Some(pool) = state.db.clone() else {
        return false;
    };
    let Ok((cfg, Some(token))) = credentials::load_ra_session(&pool, &OsKeyring).await else {
        return false;
    };
    if !cfg.connected || !cfg.enabled {
        return false;
    }
    let Some(username) = cfg.username else {
        return false;
    };
    let Some(rid) = rom_id else {
        return false;
    };
    let Ok(Some(rom)) = db::RomsRepo::new(pool).get(rid).await else {
        return false;
    };
    let Some(console_id) = console_id_for_system(&rom.system_id) else {
        return false;
    };

    let rom_hash = resolve_rom_hash(&ra_state, console_id, &rom.id, &rom.system_id, rom_path).await;
    ra_state.set_hardcore_active(cfg.hardcore && rom_hash.is_some());
    state.session.init_achievements(RaSessionConfig {
        console_id,
        rom_path: rom_path.to_owned(),
        rom_hash,
        username,
        token,
        hardcore: cfg.hardcore,
        unofficial: cfg.unofficial,
        encore: cfg.encore,
    });
    cfg.hardcore
}

pub fn on_game_unloaded(app: &AppHandle) {
    let ra_state = app.state::<RaState>();
    ra_state.set_hardcore_active(false);
    ra_state.set_active_rom(None);
    ra_state.set_live(None);
}

// ---------------------------------------------------------------------------
// DTOs exportados ao frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaStatus"))]
pub struct RaStatusDto {
    pub connected: bool,
    pub enabled: bool,
    pub hardcore: bool,
    pub unofficial: bool,
    pub encore: bool,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub score: u32,
    pub score_softcore: u32,
    pub linked_alvanista_username: Option<String>,
}

impl From<RaConfig> for RaStatusDto {
    fn from(c: RaConfig) -> Self {
        Self {
            connected: c.connected,
            enabled: c.enabled,
            hardcore: c.hardcore,
            unofficial: c.unofficial,
            encore: c.encore,
            username: c.username,
            display_name: c.display_name,
            avatar_url: c.avatar_url,
            score: c.score,
            score_softcore: c.score_softcore,
            linked_alvanista_username: c.linked_alvanista_username,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaAchievement"))]
pub struct RaAchievementDto {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub unlocked: bool,
    pub unlocked_hardcore: bool,
    pub unlock_time: Option<i64>,
    pub badge_url: Option<String>,
    pub badge_locked_url: Option<String>,
    pub measured_progress: Option<String>,
    pub measured_percent: f32,
    pub rarity: f32,
    pub rarity_hardcore: f32,
    #[cfg_attr(
        test,
        ts(type = "'standard' | 'missable' | 'progression' | 'win_condition'")
    )]
    pub kind: String,
    pub unofficial: bool,
    pub unsupported: bool,
}

impl From<AchievementEntry> for RaAchievementDto {
    fn from(a: AchievementEntry) -> Self {
        let kind = match a.kind {
            AchievementKind::Standard => "standard",
            AchievementKind::Missable => "missable",
            AchievementKind::Progression => "progression",
            AchievementKind::WinCondition => "win_condition",
        };
        Self {
            id: a.id,
            title: a.title,
            description: a.description,
            points: a.points,
            unlocked: a.unlocked,
            unlocked_hardcore: a.unlocked_hardcore,
            unlock_time: a.unlock_time,
            badge_url: a.badge_url,
            badge_locked_url: a.badge_locked_url,
            measured_progress: a.measured_progress,
            measured_percent: a.measured_percent,
            rarity: a.rarity,
            rarity_hardcore: a.rarity_hardcore,
            kind: kind.into(),
            unofficial: a.unofficial,
            unsupported: a.unsupported,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaGameView"))]
pub struct RaGameViewDto {
    #[cfg_attr(
        test,
        ts(type = "'not_connected' | 'unsupported_system' | 'unidentified' | 'loaded'")
    )]
    pub status: String,
    pub game_id: Option<u32>,
    pub title: Option<String>,
    pub badge_url: Option<String>,
    pub hardcore: bool,
    pub total_achievements: u32,
    pub unlocked_achievements: u32,
    pub total_points: u32,
    pub unlocked_points: u32,
    pub achievements: Vec<RaAchievementDto>,
}

impl RaGameViewDto {
    fn empty(status: &str, hardcore: bool) -> Self {
        Self {
            status: status.into(),
            game_id: None,
            title: None,
            badge_url: None,
            hardcore,
            total_achievements: 0,
            unlocked_achievements: 0,
            total_points: 0,
            unlocked_points: 0,
            achievements: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaGameLoadedEvent"))]
pub struct RaGameLoadedEventDto {
    pub rom_id: Option<String>,
    pub game_id: u32,
    pub title: String,
    pub badge_url: Option<String>,
    pub hardcore: bool,
    pub unlocked_achievements: u32,
    pub total_achievements: u32,
    pub unlocked_points: u32,
    pub total_points: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "RaUnlockEvent"))]
pub struct RaUnlockEventDto {
    pub rom_id: Option<String>,
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub hardcore: bool,
    pub badge_url: Option<String>,
    pub game_completed: bool,
}

fn db_pool(state: &AppState) -> Result<db::Db, AppError> {
    state
        .db
        .clone()
        .ok_or_else(|| AppError::from("banco de dados indisponível"))
}

pub fn emit_ra_status_changed(app: &AppHandle, status: &RaStatusDto) {
    let _ = app.emit("ra-status-changed", status);
}

#[tauri::command]
pub async fn ra_status(state: State<'_, AppState>) -> Result<RaStatusDto, AppError> {
    let pool = db_pool(&state)?;
    let (cfg, _) = credentials::load_ra_session(&pool, &OsKeyring)
        .await
        .map_err(AppError::from)?;
    Ok(cfg.into())
}

#[tauri::command]
pub async fn ra_login(
    app: AppHandle,
    state: State<'_, AppState>,
    ra_state: State<'_, RaState>,
    username: String,
    password: String,
) -> Result<RaStatusDto, AppError> {
    let username = username.trim().to_owned();
    if username.is_empty() || password.is_empty() {
        return Err(AppError::from(
            "Informe usuário e senha do RetroAchievements.",
        ));
    }
    let pool = db_pool(&state)?;
    let http = ra_state.http_client();

    let user = {
        let mut client = AchievementClient::new();
        client
            .begin_login_with_password(&username, &password)
            .map_err(|_| AppError::from("Usuário ou senha inválidos no RetroAchievements."))?;
        // A senha já foi copiada para o payload temporário do `r=login2`; daqui
        // em diante só o `connect_token` devolvido pelo servidor é mantido.
        drop(password);
        drive_client_requests(&http, &mut client).await;

        let mut outcome = None;
        for o in client.take_outcomes() {
            if let AsyncOutcome::Login(res) = o {
                outcome = Some(res);
            }
        }
        match outcome {
            Some(Ok(u)) => u,
            Some(Err(RaError::InvalidCredentials)) => {
                return Err(AppError::from(
                    "Usuário ou senha inválidos no RetroAchievements.",
                ))
            }
            Some(Err(e)) => return Err(AppError::new(ErrorCode::Offline, e.to_string())),
            None => {
                return Err(AppError::new(
                    ErrorCode::Offline,
                    "Sem resposta do RetroAchievements.",
                ))
            }
        }
    };

    let cfg = credentials::save_ra_login(
        &pool,
        &OsKeyring,
        &user.username,
        Some(user.display_name),
        user.avatar_url,
        user.score,
        user.score_softcore,
        &user.token,
    )
    .await
    .map_err(AppError::from)?;

    let dto: RaStatusDto = cfg.into();
    emit_ra_status_changed(&app, &dto);
    Ok(dto)
}

#[tauri::command]
pub async fn ra_logout(
    app: AppHandle,
    state: State<'_, AppState>,
    ra_state: State<'_, RaState>,
) -> Result<RaStatusDto, AppError> {
    let pool = db_pool(&state)?;
    let cfg = credentials::clear_ra_login(&pool, &OsKeyring)
        .await
        .map_err(AppError::from)?;
    ra_state.set_hardcore_active(false);
    ra_state.set_live(None);
    let dto: RaStatusDto = cfg.into();
    emit_ra_status_changed(&app, &dto);
    Ok(dto)
}

#[tauri::command]
pub async fn ra_set_preferences(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: Option<bool>,
    hardcore: Option<bool>,
    unofficial: Option<bool>,
    encore: Option<bool>,
) -> Result<RaStatusDto, AppError> {
    let pool = db_pool(&state)?;
    let cfg = credentials::update_ra_preferences(&pool, enabled, hardcore, unofficial, encore)
        .await
        .map_err(AppError::from)?;
    let dto: RaStatusDto = cfg.into();
    emit_ra_status_changed(&app, &dto);
    Ok(dto)
}

#[derive(Deserialize)]
struct UnlocksReply {
    #[serde(rename = "Success", default)]
    success: bool,
    #[serde(rename = "UserUnlocks", default)]
    user_unlocks: Vec<u32>,
}

async fn fetch_unlocks_set(
    http: &reqwest::Client,
    username: &str,
    token: &str,
    game_id: u32,
    hardcore: bool,
) -> HashSet<u32> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("r", "unlocks")
        .append_pair("u", username)
        .append_pair("t", token)
        .append_pair("g", &game_id.to_string())
        .append_pair("h", if hardcore { "1" } else { "0" })
        .finish();
    let req = OutboundHttpRequest {
        id: 0,
        url: "https://retroachievements.org/dorequest.php".into(),
        post_data: Some(body),
        content_type: Some("application/x-www-form-urlencoded".into()),
    };
    let (status, bytes) = execute_rcheevos_http(http, &req).await;
    if status != 200 {
        return HashSet::new();
    }
    serde_json::from_slice::<UnlocksReply>(&bytes)
        .ok()
        .filter(|r| r.success)
        .map(|r| r.user_unlocks.into_iter().collect())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn ra_game_achievements(
    state: State<'_, AppState>,
    ra_state: State<'_, RaState>,
    rom_id: String,
) -> Result<RaGameViewDto, AppError> {
    let pool = db_pool(&state)?;
    let (cfg, Some(token)) = credentials::load_ra_session(&pool, &OsKeyring)
        .await
        .map_err(AppError::from)?
    else {
        return Ok(RaGameViewDto::empty("not_connected", false));
    };
    let Some(username) = cfg.username.clone() else {
        return Ok(RaGameViewDto::empty("not_connected", cfg.hardcore));
    };

    // Se este mesmo jogo está rodando agora, o snapshot vivo já traz os
    // desbloqueios da sessão atual sem nova ida à rede.
    if let Some(live) = ra_state.live_for(&rom_id) {
        return Ok(RaGameViewDto {
            status: "loaded".into(),
            game_id: Some(live.game_id),
            title: Some(live.title),
            badge_url: live.badge_url,
            hardcore: live.hardcore,
            total_achievements: live.summary.num_core_achievements,
            unlocked_achievements: live.summary.num_unlocked_achievements,
            total_points: live.summary.points_core,
            unlocked_points: live.summary.points_unlocked,
            achievements: live.achievements.into_iter().map(Into::into).collect(),
        });
    }

    let Some(rom) = db::RomsRepo::new(pool)
        .get(&rom_id)
        .await
        .map_err(AppError::from)?
    else {
        return Ok(RaGameViewDto::empty("unidentified", cfg.hardcore));
    };
    let Some(console_id) = console_id_for_system(&rom.system_id) else {
        return Ok(RaGameViewDto::empty("unsupported_system", cfg.hardcore));
    };

    let Some(hash) = resolve_rom_hash(
        &ra_state,
        console_id,
        &rom.id,
        &rom.system_id,
        &rom.file_path,
    )
    .await
    else {
        return Ok(RaGameViewDto::empty("unidentified", cfg.hardcore));
    };

    let http = ra_state.http_client();
    let (loaded_game, mut entries) = {
        let mut client = AchievementClient::new();
        // `spectator = true` carrega definições (`r=achievementsets`) sem enviar
        // `r=startsession`, evitando marcar "Jogando" só por abrir a página do jogo.
        client.set_spectator(true);
        client.set_hardcore(cfg.hardcore);
        client.set_unofficial(cfg.unofficial);
        client
            .begin_login_with_token(&username, &token)
            .map_err(|e| AppError::from(e.to_string()))?;
        drive_client_requests(&http, &mut client).await;

        let logged_in = client
            .take_outcomes()
            .into_iter()
            .any(|o| matches!(o, AsyncOutcome::Login(Ok(_))));
        if !logged_in {
            return Err(AppError::from(
                "Sessão expirada no RetroAchievements. Entre novamente em Configurações.",
            ));
        }

        client
            .begin_load_game_by_hash(&hash)
            .map_err(|e| AppError::from(e.to_string()))?;
        drive_client_requests(&http, &mut client).await;

        let Some(game) = client.game_info() else {
            return Ok(RaGameViewDto::empty("unidentified", cfg.hardcore));
        };
        let list = client.achievements(cfg.unofficial);
        (game, list)
    };

    let soft_task = {
        let (http, u, t, gid) = (
            http.clone(),
            username.clone(),
            token.clone(),
            loaded_game.id,
        );
        tauri::async_runtime::spawn(
            async move { fetch_unlocks_set(&http, &u, &t, gid, false).await },
        )
    };
    let hard_unlocks = fetch_unlocks_set(&http, &username, &token, loaded_game.id, true).await;
    let soft_unlocks = soft_task.await.unwrap_or_default();

    let mut unlocked_count = 0u32;
    let mut unlocked_points = 0u32;
    let mut total_count = 0u32;
    let mut total_points = 0u32;
    for item in &mut entries {
        let is_hard = hard_unlocks.contains(&item.id);
        let is_soft = soft_unlocks.contains(&item.id) || is_hard;
        item.unlocked = is_soft;
        item.unlocked_hardcore = is_hard;
        if !item.unofficial {
            total_count += 1;
            total_points += item.points;
            if is_soft {
                unlocked_count += 1;
                unlocked_points += item.points;
            }
        }
    }

    Ok(RaGameViewDto {
        status: "loaded".into(),
        game_id: Some(loaded_game.id),
        title: Some(loaded_game.title),
        badge_url: loaded_game.badge_url,
        hardcore: cfg.hardcore,
        total_achievements: total_count,
        unlocked_achievements: unlocked_count,
        total_points,
        unlocked_points,
        achievements: entries.into_iter().map(Into::into).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn compute_rom_hash_extracts_inner_rom_from_zip_and_preserves_arcade_zip() {
        let dir = tempfile::tempdir().unwrap();
        let payload = vec![0xA5u8; 4096];
        let raw_path = dir.path().join("River Raid (USA).a26");
        std::fs::write(&raw_path, &payload).unwrap();

        let zip_path = dir.path().join("River Raid (USA).zip");
        {
            let file = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zw.start_file("readme.html", opts).unwrap();
            zw.write_all(b"<html>bundle note</html>").unwrap();
            zw.start_file("River Raid (USA).A26", opts).unwrap();
            zw.write_all(&payload).unwrap();
            zw.finish().unwrap();
        }

        let a26_console = console_id_for_system("atari2600").unwrap();
        let raw_hash = compute_rom_hash(a26_console, "atari2600", &raw_path).unwrap();
        let zip_hash = compute_rom_hash(a26_console, "atari2600", &zip_path).unwrap();
        assert_eq!(zip_hash, raw_hash);

        let arcade_zip = dir.path().join("mslug.zip");
        {
            let file = std::fs::File::create(&arcade_zip).unwrap();
            let mut zw = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("201-p1.p1", opts).unwrap();
            zw.write_all(b"chip").unwrap();
            zw.finish().unwrap();
        }
        let arcade_hash = compute_rom_hash(RC_CONSOLE_ARCADE, "arcade", &arcade_zip).unwrap();
        let expected_arcade = hash_file(RC_CONSOLE_ARCADE, &arcade_zip).unwrap();
        assert_eq!(arcade_hash, expected_arcade);
    }
}
