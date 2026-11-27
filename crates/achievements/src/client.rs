//! Wrapper seguro e sem duplicação sobre `rc_client_t` (`include/rc_client.h`).
//!
//! As chamadas HTTP do `rcheevos` não tocam rede diretamente aqui: cada pedido
//! de `server_call` vira um [`OutboundHttpRequest`] numa fila interna, e quem
//! dirige o cliente (`reemu-core-host` via `core-ipc`, `reemu-desktop` via
//! `reqwest` ou testes em memória) entrega a resposta em
//! [`AchievementClient::complete_http_request`].
//!
//! Nota de FFI/aliasing: como o C chama callbacks de volta de dentro de
//! `rc_client_*` e do `server_callback`, todo o estado compartilhado com C mora
//! dentro de `RefCell` (`UnsafeCell`), e nenhum `Ref`/`RefMut` fica vivo
//! enquanto o controle está no C.

use crate::ffi;
use crate::memory::LibretroMemory;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum RaError {
    #[error("credenciais inválidas no RetroAchievements")]
    InvalidCredentials,
    #[error("sessão expirada no RetroAchievements")]
    ExpiredToken,
    #[error("{message} (código {code})")]
    Api { code: i32, message: String },
}

impl RaError {
    fn from_rc(code: c_int, message: *const c_char) -> Self {
        match code {
            ffi::RC_INVALID_CREDENTIALS => Self::InvalidCredentials,
            ffi::RC_EXPIRED_TOKEN => Self::ExpiredToken,
            other => {
                let msg = cstr_or_empty(message);
                let fallback = if msg.is_empty() {
                    cstr_or_empty(unsafe { ffi::rc_error_str(other) })
                } else {
                    msg
                };
                Self::Api {
                    code: other,
                    message: fallback,
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundHttpRequest {
    pub id: u64,
    pub url: String,
    pub post_data: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggedInUser {
    pub username: String,
    pub display_name: String,
    pub token: String,
    pub score: u32,
    pub score_softcore: u32,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedGame {
    pub id: u32,
    pub console_id: u32,
    pub title: String,
    pub hash: String,
    pub badge_url: Option<String>,
    pub summary: GameSummary,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSummary {
    pub num_core_achievements: u32,
    pub num_unofficial_achievements: u32,
    pub num_unlocked_achievements: u32,
    pub num_unsupported_achievements: u32,
    pub points_core: u32,
    pub points_unlocked: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AchievementKind {
    Standard,
    Missable,
    Progression,
    WinCondition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AchievementEntry {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub points: u32,
    pub unlocked: bool,
    pub unlocked_hardcore: bool,
    pub unlock_time: Option<i64>,
    pub badge_name: String,
    pub badge_url: Option<String>,
    pub badge_locked_url: Option<String>,
    pub measured_progress: Option<String>,
    pub measured_percent: f32,
    pub rarity: f32,
    pub rarity_hardcore: f32,
    pub kind: AchievementKind,
    pub unofficial: bool,
    pub unsupported: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AchievementEvent {
    AchievementTriggered {
        id: u32,
        title: String,
        description: String,
        points: u32,
        hardcore: bool,
        badge_url: Option<String>,
    },
    ProgressUpdated {
        id: u32,
        title: String,
        progress_text: String,
        percent: f32,
        badge_url: Option<String>,
    },
    ProgressHidden,
    ChallengeShown {
        id: u32,
        title: String,
        badge_url: Option<String>,
    },
    ChallengeHidden {
        id: u32,
    },
    GameCompleted {
        hardcore: bool,
    },
    ResetRequested,
    ServerError {
        api: String,
        message: String,
        code: i32,
    },
    Disconnected,
    Reconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncOutcome {
    Login(Result<LoggedInUser, RaError>),
    GameLoad(Result<LoadedGame, RaError>),
}

struct PendingHttpCallback {
    callback: unsafe extern "C" fn(*const ffi::rc_api_server_response_t, *mut c_void),
    callback_data: *mut c_void,
}

enum MemoryBackend {
    None,
    Libretro(Box<LibretroMemory>),
    Buffer(Vec<u8>),
}

struct ClientQueues {
    next_request_id: u64,
    pending_http: HashMap<u64, PendingHttpCallback>,
    outbound_http: Vec<OutboundHttpRequest>,
    events: Vec<AchievementEvent>,
    outcomes: Vec<AsyncOutcome>,
}

impl Default for ClientQueues {
    fn default() -> Self {
        Self {
            next_request_id: 1,
            pending_http: HashMap::new(),
            outbound_http: Vec::new(),
            events: Vec::new(),
            outcomes: Vec::new(),
        }
    }
}

struct CallbackContext {
    memory: RefCell<MemoryBackend>,
    queues: RefCell<ClientQueues>,
}

impl Default for CallbackContext {
    fn default() -> Self {
        Self {
            memory: RefCell::new(MemoryBackend::None),
            queues: RefCell::new(ClientQueues::default()),
        }
    }
}

/// Cliente `rcheevos` para uma sessão (login, carga de jogo ou loop por quadro).
///
/// Todas as chamadas ocorrem na mesma thread dona da instância.
pub struct AchievementClient {
    raw: *mut ffi::rc_client_t,
    ctx: *const CallbackContext,
}

// SAFETY: `AchievementClient` é dono exclusivo de `raw` e `ctx` (alocados no
// heap e destruídos em `Drop`), não usa estado thread-local no `rcheevos`, e
// não implementa `Sync` (logo duas threads nunca acessam a instância ao mesmo
// tempo sem sincronização externa).
unsafe impl Send for AchievementClient {}

impl AchievementClient {
    pub fn new() -> Self {
        let ctx = Box::into_raw(Box::new(CallbackContext::default())) as *const CallbackContext;
        let raw = unsafe {
            ffi::rc_client_create(Some(read_memory_trampoline), Some(server_call_trampoline))
        };
        assert!(!raw.is_null(), "rc_client_create retornou nulo");
        unsafe {
            ffi::rc_client_set_userdata(raw, ctx as *mut c_void);
            ffi::rc_client_set_event_handler(raw, Some(event_handler_trampoline));
            // `complete_http_request` roda sempre na própria thread dona do
            // `AchievementClient` (entre quadros no `reemu-core-host`, ou sem
            // memória anexada no `reemu-desktop`), nunca numa thread HTTP
            // concorrente ao `retro_run`. Manter o padrão do `rcheevos` ativa
            // o jogo assim que `startsession` responde.
            ffi::rc_client_enable_logging(
                raw,
                ffi::RC_CLIENT_LOG_LEVEL_WARN,
                Some(log_message_trampoline),
            );
        }
        Self { raw, ctx }
    }

    fn context(&self) -> &CallbackContext {
        unsafe { &*self.ctx }
    }

    /// Cláusula oficial de User-Agent do `rcheevos` (ex.: `rcheevos/12.5.0`),
    /// para anexar ao `ReEmu/<versão>`.
    pub fn user_agent_clause(&mut self) -> String {
        let mut buf = [0 as c_char; 64];
        let n =
            unsafe { ffi::rc_client_get_user_agent_clause(self.raw, buf.as_mut_ptr(), buf.len()) };
        if n == 0 {
            return "rcheevos/12.5.0".to_string();
        }
        cstr_or_empty(buf.as_ptr())
    }

    pub fn set_hardcore(&mut self, enabled: bool) {
        unsafe { ffi::rc_client_set_hardcore_enabled(self.raw, c_int::from(enabled)) };
    }

    pub fn hardcore(&self) -> bool {
        unsafe { ffi::rc_client_get_hardcore_enabled(self.raw) != 0 }
    }

    pub fn set_unofficial(&mut self, enabled: bool) {
        unsafe { ffi::rc_client_set_unofficial_enabled(self.raw, c_int::from(enabled)) };
    }

    pub fn set_encore(&mut self, enabled: bool) {
        unsafe { ffi::rc_client_set_encore_mode_enabled(self.raw, c_int::from(enabled)) };
    }

    pub fn set_spectator(&mut self, enabled: bool) {
        unsafe { ffi::rc_client_set_spectator_mode_enabled(self.raw, c_int::from(enabled)) };
    }

    /// Associa regiões de memória de um core libretro carregado.
    pub fn attach_libretro_memory(&mut self, memory: LibretroMemory) {
        *self.context().memory.borrow_mut() = MemoryBackend::Libretro(Box::new(memory));
    }

    /// Associa um buffer em memória para testes unitários de triggers.
    pub fn attach_test_memory(&mut self, data: Vec<u8>) {
        *self.context().memory.borrow_mut() = MemoryBackend::Buffer(data);
    }

    /// Atualiza bytes no buffer de memória de teste.
    pub fn write_test_memory(&mut self, offset: usize, bytes: &[u8]) {
        if let MemoryBackend::Buffer(buf) = &mut *self.context().memory.borrow_mut() {
            if offset + bytes.len() <= buf.len() {
                buf[offset..offset + bytes.len()].copy_from_slice(bytes);
            }
        }
    }

    pub fn detach_memory(&mut self) {
        *self.context().memory.borrow_mut() = MemoryBackend::None;
    }

    /// Inicia login trocando `username` + `password` por `connect_token`.
    /// A senha só vive no `CString` temporário desta chamada.
    pub fn begin_login_with_password(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<(), RaError> {
        let u = CString::new(username).map_err(|_| RaError::InvalidCredentials)?;
        let p = CString::new(password).map_err(|_| RaError::InvalidCredentials)?;
        unsafe {
            let _ = ffi::rc_client_begin_login_with_password(
                self.raw,
                u.as_ptr(),
                p.as_ptr(),
                Some(login_callback_trampoline),
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    /// Inicia login silencioso usando o `connect_token` guardado no chaveiro.
    pub fn begin_login_with_token(&mut self, username: &str, token: &str) -> Result<(), RaError> {
        let u = CString::new(username).map_err(|_| RaError::InvalidCredentials)?;
        let t = CString::new(token).map_err(|_| RaError::InvalidCredentials)?;
        unsafe {
            let _ = ffi::rc_client_begin_login_with_token(
                self.raw,
                u.as_ptr(),
                t.as_ptr(),
                Some(login_callback_trampoline),
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    pub fn logout(&mut self) {
        unsafe { ffi::rc_client_logout(self.raw) };
    }

    pub fn user_info(&self) -> Option<LoggedInUser> {
        unsafe { read_user_info(self.raw) }
    }

    /// Identifica (por hash oficial do `rcheevos`) e carrega o conjunto do jogo.
    pub fn begin_identify_and_load_game(
        &mut self,
        console_id: u32,
        file_path: &Path,
        data: Option<&[u8]>,
    ) -> Result<(), RaError> {
        let path_c =
            CString::new(file_path.to_string_lossy().into_owned().into_bytes()).map_err(|_| {
                RaError::Api {
                    code: -1,
                    message: "caminho de arquivo inválido".into(),
                }
            })?;
        let (ptr, len) = data.map_or((std::ptr::null(), 0), |b| (b.as_ptr(), b.len()));
        unsafe {
            let _ = ffi::rc_client_begin_identify_and_load_game(
                self.raw,
                console_id,
                path_c.as_ptr(),
                ptr,
                len,
                Some(load_game_callback_trampoline),
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    /// Carrega o conjunto do jogo quando o hash MD5/RA já é conhecido.
    pub fn begin_load_game_by_hash(&mut self, hash: &str) -> Result<(), RaError> {
        let h = CString::new(hash).map_err(|_| RaError::Api {
            code: -1,
            message: "hash inválido".into(),
        })?;
        unsafe {
            let _ = ffi::rc_client_begin_load_game(
                self.raw,
                h.as_ptr(),
                Some(load_game_callback_trampoline),
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    pub fn is_game_loaded(&self) -> bool {
        unsafe { ffi::rc_client_is_game_loaded(self.raw) != 0 }
    }

    pub fn unload_game(&mut self) {
        unsafe { ffi::rc_client_unload_game(self.raw) };
    }

    pub fn game_info(&self) -> Option<LoadedGame> {
        unsafe { read_game_info(self.raw) }
    }

    pub fn game_summary(&self) -> GameSummary {
        unsafe { read_game_summary(self.raw) }
    }

    pub fn achievements(&mut self, include_unofficial: bool) -> Vec<AchievementEntry> {
        let category = if include_unofficial {
            ffi::RC_CLIENT_ACHIEVEMENT_CATEGORY_CORE_AND_UNOFFICIAL
        } else {
            ffi::RC_CLIENT_ACHIEVEMENT_CATEGORY_CORE
        };
        let list_ptr = unsafe {
            ffi::rc_client_create_achievement_list(
                self.raw,
                category,
                ffi::RC_CLIENT_ACHIEVEMENT_LIST_GROUPING_LOCK_STATE,
            )
        };
        if list_ptr.is_null() {
            return Vec::new();
        }
        let mut out = Vec::new();
        unsafe {
            let list = &*list_ptr;
            if !list.buckets.is_null() {
                let buckets = std::slice::from_raw_parts(list.buckets, list.num_buckets as usize);
                for b in buckets {
                    if b.achievements.is_null() {
                        continue;
                    }
                    let items =
                        std::slice::from_raw_parts(b.achievements, b.num_achievements as usize);
                    for &ach_ptr in items {
                        if !ach_ptr.is_null() {
                            out.push(read_achievement(&*ach_ptr));
                        }
                    }
                }
            }
            ffi::rc_client_destroy_achievement_list(list_ptr);
        }
        out
    }

    pub fn is_processing_required(&mut self) -> bool {
        unsafe { ffi::rc_client_is_processing_required(self.raw) != 0 }
    }

    pub fn do_frame(&mut self) {
        unsafe { ffi::rc_client_do_frame(self.raw) };
    }

    pub fn idle(&mut self) {
        unsafe { ffi::rc_client_idle(self.raw) };
    }

    pub fn reset(&mut self) {
        unsafe { ffi::rc_client_reset(self.raw) };
    }

    pub fn serialize_progress(&self) -> Option<Vec<u8>> {
        let size = unsafe { ffi::rc_client_progress_size(self.raw) };
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size];
        let rc =
            unsafe { ffi::rc_client_serialize_progress_sized(self.raw, buf.as_mut_ptr(), size) };
        (rc == ffi::RC_OK).then_some(buf)
    }

    pub fn deserialize_progress(&mut self, data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }
        let rc = unsafe {
            ffi::rc_client_deserialize_progress_sized(self.raw, data.as_ptr(), data.len())
        };
        rc == ffi::RC_OK
    }

    /// Drena os pedidos HTTP pendentes gerados pelo `rcheevos`.
    pub fn take_http_requests(&mut self) -> Vec<OutboundHttpRequest> {
        let mut q = self.context().queues.borrow_mut();
        std::mem::take(&mut q.outbound_http)
    }

    /// Entrega a resposta HTTP de um pedido drenado por [`Self::take_http_requests`].
    /// `status_code < 0` sinaliza falha de rede (`RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR`).
    pub fn complete_http_request(&mut self, request_id: u64, status_code: i32, body: &[u8]) {
        let pending = self
            .context()
            .queues
            .borrow_mut()
            .pending_http
            .remove(&request_id);
        let Some(pending) = pending else {
            return;
        };
        let resp = ffi::rc_api_server_response_t {
            body: if body.is_empty() {
                std::ptr::null()
            } else {
                body.as_ptr().cast::<c_char>()
            },
            body_length: body.len(),
            http_status_code: status_code,
        };
        // Nenhum empréstimo de `RefCell` fica vivo durante a chamada ao C: o
        // próprio callback pode enfileirar o próximo pedido (`startsession`).
        unsafe {
            (pending.callback)(&resp, pending.callback_data);
        }
    }

    /// Drena eventos assíncronos de runtime (conquista desbloqueada, jogo
    /// concluído, pedido de reset, progresso).
    pub fn take_events(&mut self) -> Vec<AchievementEvent> {
        let mut q = self.context().queues.borrow_mut();
        std::mem::take(&mut q.events)
    }

    /// Drena conclusões de operações assíncronas (`login`, `load_game`).
    pub fn take_outcomes(&mut self) -> Vec<AsyncOutcome> {
        let mut q = self.context().queues.borrow_mut();
        std::mem::take(&mut q.outcomes)
    }
}

impl Default for AchievementClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AchievementClient {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                ffi::rc_client_destroy(self.raw);
            }
            self.raw = std::ptr::null_mut();
        }
        if !self.ctx.is_null() {
            unsafe {
                drop(Box::from_raw(self.ctx.cast_mut()));
            }
            self.ctx = std::ptr::null();
        }
    }
}

// ---------------------------------------------------------------------------
// Callbacks C internos
// ---------------------------------------------------------------------------

unsafe fn callback_ctx<'a>(client: *mut ffi::rc_client_t) -> Option<&'a CallbackContext> {
    if client.is_null() {
        return None;
    }
    let ptr = ffi::rc_client_get_userdata(client).cast::<CallbackContext>();
    ptr.as_ref()
}

unsafe extern "C" fn read_memory_trampoline(
    address: u32,
    buffer: *mut u8,
    num_bytes: u32,
    client: *mut ffi::rc_client_t,
) -> u32 {
    if buffer.is_null() || num_bytes == 0 {
        return 0;
    }
    let Some(ctx) = callback_ctx(client) else {
        return 0;
    };
    let out = std::slice::from_raw_parts_mut(buffer, num_bytes as usize);
    let mem = ctx.memory.borrow();
    match &*mem {
        MemoryBackend::None => 0,
        MemoryBackend::Libretro(mem) => mem.read(address, out),
        MemoryBackend::Buffer(buf) => {
            let start = address as usize;
            if start >= buf.len() {
                return 0;
            }
            let avail = (buf.len() - start).min(out.len());
            out[..avail].copy_from_slice(&buf[start..start + avail]);
            avail as u32
        }
    }
}

unsafe extern "C" fn server_call_trampoline(
    request: *const ffi::rc_api_request_t,
    callback: ffi::rc_client_server_callback_t,
    callback_data: *mut c_void,
    client: *mut ffi::rc_client_t,
) {
    let (Some(req), Some(cb), Some(ctx)) = (request.as_ref(), callback, callback_ctx(client))
    else {
        return;
    };
    let mut q = ctx.queues.borrow_mut();
    let id = q.next_request_id;
    q.next_request_id += 1;
    q.pending_http.insert(
        id,
        PendingHttpCallback {
            callback: cb,
            callback_data,
        },
    );
    q.outbound_http.push(OutboundHttpRequest {
        id,
        url: cstr_or_empty(req.url),
        post_data: cstr_opt(req.post_data),
        content_type: cstr_opt(req.content_type),
    });
}

unsafe extern "C" fn login_callback_trampoline(
    result: c_int,
    error_message: *const c_char,
    client: *mut ffi::rc_client_t,
    _userdata: *mut c_void,
) {
    let outcome = if result == ffi::RC_OK {
        read_user_info(client).ok_or_else(|| RaError::Api {
            code: -1,
            message: "usuário ausente após login".into(),
        })
    } else {
        Err(RaError::from_rc(result, error_message))
    };
    if let Some(ctx) = callback_ctx(client) {
        ctx.queues
            .borrow_mut()
            .outcomes
            .push(AsyncOutcome::Login(outcome));
    }
}

unsafe extern "C" fn load_game_callback_trampoline(
    result: c_int,
    error_message: *const c_char,
    client: *mut ffi::rc_client_t,
    _userdata: *mut c_void,
) {
    let outcome = if result == ffi::RC_OK {
        read_game_info(client).ok_or_else(|| RaError::Api {
            code: -1,
            message: "jogo não identificado no RetroAchievements".into(),
        })
    } else {
        Err(RaError::from_rc(result, error_message))
    };
    if let Some(ctx) = callback_ctx(client) {
        ctx.queues
            .borrow_mut()
            .outcomes
            .push(AsyncOutcome::GameLoad(outcome));
    }
}

unsafe extern "C" fn event_handler_trampoline(
    event: *const ffi::rc_client_event_t,
    client: *mut ffi::rc_client_t,
) {
    let Some(ev) = event.as_ref() else {
        return;
    };
    let hardcore = ffi::rc_client_get_hardcore_enabled(client) != 0;
    let Some(ctx) = callback_ctx(client) else {
        return;
    };
    let mut q = ctx.queues.borrow_mut();
    match ev.r#type {
        ffi::RC_CLIENT_EVENT_ACHIEVEMENT_TRIGGERED => {
            if let Some(ach) = ev.achievement.as_ref() {
                q.events.push(AchievementEvent::AchievementTriggered {
                    id: ach.id,
                    title: cstr_or_empty(ach.title),
                    description: cstr_or_empty(ach.description),
                    points: ach.points,
                    hardcore,
                    badge_url: cstr_opt(ach.badge_url),
                });
            }
        }
        ffi::RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_SHOW
        | ffi::RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_UPDATE => {
            if let Some(ach) = ev.achievement.as_ref() {
                q.events.push(AchievementEvent::ProgressUpdated {
                    id: ach.id,
                    title: cstr_or_empty(ach.title),
                    progress_text: cstr_or_empty(ach.measured_progress.as_ptr()),
                    percent: ach.measured_percent,
                    badge_url: cstr_opt(ach.badge_url),
                });
            }
        }
        ffi::RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_HIDE => {
            q.events.push(AchievementEvent::ProgressHidden);
        }
        ffi::RC_CLIENT_EVENT_ACHIEVEMENT_CHALLENGE_INDICATOR_SHOW => {
            if let Some(ach) = ev.achievement.as_ref() {
                q.events.push(AchievementEvent::ChallengeShown {
                    id: ach.id,
                    title: cstr_or_empty(ach.title),
                    badge_url: cstr_opt(ach.badge_url),
                });
            }
        }
        ffi::RC_CLIENT_EVENT_ACHIEVEMENT_CHALLENGE_INDICATOR_HIDE => {
            if let Some(ach) = ev.achievement.as_ref() {
                q.events
                    .push(AchievementEvent::ChallengeHidden { id: ach.id });
            }
        }
        ffi::RC_CLIENT_EVENT_GAME_COMPLETED => {
            q.events.push(AchievementEvent::GameCompleted { hardcore });
        }
        ffi::RC_CLIENT_EVENT_RESET => {
            q.events.push(AchievementEvent::ResetRequested);
        }
        ffi::RC_CLIENT_EVENT_SERVER_ERROR => {
            if let Some(err) = ev.server_error.as_ref() {
                q.events.push(AchievementEvent::ServerError {
                    api: cstr_or_empty(err.api),
                    message: cstr_or_empty(err.error_message),
                    code: err.result,
                });
            }
        }
        ffi::RC_CLIENT_EVENT_DISCONNECTED => {
            q.events.push(AchievementEvent::Disconnected);
        }
        ffi::RC_CLIENT_EVENT_RECONNECTED => {
            q.events.push(AchievementEvent::Reconnected);
        }
        _ => {}
    }
}

unsafe extern "C" fn log_message_trampoline(
    message: *const c_char,
    _client: *const ffi::rc_client_t,
) {
    // Regra de privacidade (doc 16): nunca registrar tokens, senhas ou hashes.
    let msg = cstr_or_empty(message);
    if !msg.is_empty() && !msg.contains("t=") && !msg.contains("p=") {
        log::debug!("rcheevos: {msg}");
    }
}

unsafe fn read_user_info(client: *const ffi::rc_client_t) -> Option<LoggedInUser> {
    let u = ffi::rc_client_get_user_info(client).as_ref()?;
    Some(LoggedInUser {
        username: cstr_or_empty(u.username),
        display_name: cstr_or_empty(u.display_name),
        token: cstr_or_empty(u.token),
        score: u.score,
        score_softcore: u.score_softcore,
        avatar_url: cstr_opt(u.avatar_url),
    })
}

unsafe fn read_game_summary(client: *const ffi::rc_client_t) -> GameSummary {
    let mut s = ffi::rc_client_user_game_summary_t::default();
    ffi::rc_client_get_user_game_summary(client, &mut s);
    GameSummary {
        num_core_achievements: s.num_core_achievements,
        num_unofficial_achievements: s.num_unofficial_achievements,
        num_unlocked_achievements: s.num_unlocked_achievements,
        num_unsupported_achievements: s.num_unsupported_achievements,
        points_core: s.points_core,
        points_unlocked: s.points_unlocked,
    }
}

unsafe fn read_game_info(client: *const ffi::rc_client_t) -> Option<LoadedGame> {
    let g = ffi::rc_client_get_game_info(client).as_ref()?;
    if g.id == 0 {
        return None;
    }
    Some(LoadedGame {
        id: g.id,
        console_id: g.console_id,
        title: cstr_or_empty(g.title),
        hash: cstr_or_empty(g.hash),
        badge_url: cstr_opt(g.badge_url),
        summary: read_game_summary(client),
    })
}

unsafe fn read_achievement(ach: &ffi::rc_client_achievement_t) -> AchievementEntry {
    let kind = match ach.r#type {
        ffi::RC_CLIENT_ACHIEVEMENT_TYPE_MISSABLE => AchievementKind::Missable,
        ffi::RC_CLIENT_ACHIEVEMENT_TYPE_PROGRESSION => AchievementKind::Progression,
        ffi::RC_CLIENT_ACHIEVEMENT_TYPE_WIN => AchievementKind::WinCondition,
        _ => AchievementKind::Standard,
    };
    let progress = cstr_or_empty(ach.measured_progress.as_ptr());
    AchievementEntry {
        id: ach.id,
        title: cstr_or_empty(ach.title),
        description: cstr_or_empty(ach.description),
        points: ach.points,
        unlocked: ach.unlocked != ffi::RC_CLIENT_ACHIEVEMENT_UNLOCKED_NONE
            || ach.state == ffi::RC_CLIENT_ACHIEVEMENT_STATE_UNLOCKED,
        unlocked_hardcore: (ach.unlocked & ffi::RC_CLIENT_ACHIEVEMENT_UNLOCKED_HARDCORE) != 0,
        unlock_time: (ach.unlock_time > 0).then_some(ach.unlock_time),
        badge_name: cstr_or_empty(ach.badge_name.as_ptr()),
        badge_url: cstr_opt(ach.badge_url),
        badge_locked_url: cstr_opt(ach.badge_locked_url),
        measured_progress: (!progress.is_empty()).then_some(progress),
        measured_percent: ach.measured_percent,
        rarity: ach.rarity,
        rarity_hardcore: ach.rarity_hardcore,
        kind,
        unofficial: (ach.category as c_int & ffi::RC_CLIENT_ACHIEVEMENT_CATEGORY_CORE) == 0,
        unsupported: ach.state == ffi::RC_CLIENT_ACHIEVEMENT_STATE_DISABLED,
    }
}

fn cstr_or_empty(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn cstr_opt(ptr: *const c_char) -> Option<String> {
    let s = cstr_or_empty(ptr);
    (!s.is_empty()).then_some(s)
}
