//! Coordenador central de atividade (primeiro plano vs segundo plano) e
//! emissao adaptativa de presenca para o Alvanista (`POST /api/partner/v1/presence`).
//!
//! Objetivos:
//! - Quando a interface do ReEmu esta aberta e focada (ou ha partida rodando sem
//!   pausa), envia presenca `"active"` numa cadencia responsiva (`120 s` por
//!   padrao, respeitando `next_after_s` do servidor em `60..=600 s`), com
//!   antiflap (`MIN_ACTIVE_GAP = 20 s` e `MIN_TRANSITION_GAP = 10 s`);
//! - Quando a janela vai para segundo plano sem partida ativa, recua de forma
//!   gradual (`4m -> 8m -> 16m -> 30m -> 60m`) e, se nao houver jogo aberto
//!   apos o passo de `30m`, suspende completamente os envios de presenca ate a
//!   pessoa voltar ao primeiro plano ou abrir um jogo;
//! - Ao desconectar a conta ou encerrar o app, envia `"offline"` em modo
//!   best-effort (`<= 1500 ms`);
//! - Fornece primitivos reativos (`Notify` + `wait_next_cycle`) para os demais
//!   lacos periodicos (Cloud Save, Catalogo e GAS) recuarem gradualmente em
//!   segundo plano e acordarem na hora ao voltar ao primeiro plano.

use crate::alvanista::Alvanista;
use crate::commands::AppState;
use crate::gas::{clean_game_uuid, clean_system_id};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Notify;
use tokio::time::Instant;

pub const PRESENCE_PATH: &str = "/api/partner/v1/presence";

pub const PRESENCE_ACTIVE_DEFAULT: Duration = Duration::from_secs(120);
pub const PRESENCE_SERVER_MIN_SECS: u64 = 60;
pub const PRESENCE_SERVER_MAX_SECS: u64 = 600;
pub const PRESENCE_BG_SERVER_MAX_SECS: u64 = 86_400;

pub const MIN_ACTIVE_GAP: Duration = Duration::from_secs(20);
pub const MIN_TRANSITION_GAP: Duration = Duration::from_secs(10);
pub const OFFLINE_EXIT_TIMEOUT: Duration = Duration::from_millis(1500);

pub const PRESENCE_BG_STEPS_SECS: [u64; 5] = [240, 480, 960, 1800, 3600];

/// Cloud Save sem jogo aberto: `5 min` em primeiro plano; recuo gradual ate `2 h`.
pub const SYNC_FG_EVERY: Duration = Duration::from_secs(5 * 60);
pub const SYNC_BG_STEPS: [Duration; 5] = [
    Duration::from_secs(10 * 60),
    Duration::from_secs(20 * 60),
    Duration::from_secs(40 * 60),
    Duration::from_secs(60 * 60),
    Duration::from_secs(120 * 60),
];

/// Catalogo com pendencias e sem jogo aberto: `15 min` em primeiro plano; recuo ate `4 h`.
pub const CATALOG_FG_EVERY: Duration = Duration::from_secs(15 * 60);
pub const CATALOG_BG_STEPS: [Duration; 4] = [
    Duration::from_secs(30 * 60),
    Duration::from_secs(60 * 60),
    Duration::from_secs(120 * 60),
    Duration::from_secs(240 * 60),
];

/// Batimento GAS durante partida aberta: `15 min` em primeiro plano; `30m -> 60m` em segundo plano.
pub const GAS_FG_EVERY: Duration = Duration::from_secs(15 * 60);
pub const GAS_BG_STEPS: [Duration; 2] =
    [Duration::from_secs(30 * 60), Duration::from_secs(60 * 60)];

const UNAUTHENTICATED_IDLE_SLEEP: Duration = Duration::from_secs(60);
const MAX_GAME_TITLE_LEN: usize = 200;

type PlayingUnpausedProbe = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceState {
    Active,
    Background,
    Offline,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresenceGame {
    pub rom_id: Option<String>,
    pub game_id: Option<String>,
    pub game_title: Option<String>,
    pub platform: Option<String>,
}

impl PresenceGame {
    pub fn has_identity(&self) -> bool {
        self.game_id.is_some() || self.game_title.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresencePayload {
    pub state: PresenceState,
    pub background_step: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PresenceResponse {
    #[serde(default)]
    next_after_s: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ActivitySnapshot {
    pub is_active: bool,
    pub background_step: u32,
    pub returned_from_background: bool,
    pub game: Option<PresenceGame>,
}

struct ActivityState {
    window_open: bool,
    ui_visible: bool,
    ui_focused: bool,
    playing_unpaused: bool,
    background_step: u32,
    returned_from_background: bool,
    session_reset: bool,
    shutting_down: bool,
    game: Option<PresenceGame>,
    probe: Option<PlayingUnpausedProbe>,
}

impl Default for ActivityState {
    fn default() -> Self {
        Self {
            window_open: true,
            ui_visible: true,
            ui_focused: true,
            playing_unpaused: false,
            background_step: 0,
            returned_from_background: false,
            session_reset: false,
            shutting_down: false,
            game: None,
            probe: None,
        }
    }
}

impl ActivityState {
    fn is_foreground(&self) -> bool {
        self.window_open && self.ui_visible && self.ui_focused
    }

    fn is_playing_unpaused(&self) -> bool {
        match &self.probe {
            Some(p) => p(),
            None => self.playing_unpaused,
        }
    }

    fn is_active(&self) -> bool {
        self.is_foreground() || self.is_playing_unpaused()
    }
}

struct ActivityInner {
    state: Mutex<ActivityState>,
    notify: Notify,
}

#[derive(Clone)]
pub struct ActivityCoordinator {
    inner: Arc<ActivityInner>,
}

impl Default for ActivityCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivityCoordinator {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ActivityInner {
                state: Mutex::new(ActivityState::default()),
                notify: Notify::new(),
            }),
        }
    }

    pub fn install_playing_probe(&self, probe: PlayingUnpausedProbe) {
        let mut s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        s.probe = Some(probe);
    }

    /// `true` quando a janela do ReEmu esta visivel, nao minimizada e com foco.
    pub fn is_foreground(&self) -> bool {
        let s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        s.is_foreground()
    }

    /// `true` quando um core de jogo esta carregado e rodando sem pausa.
    pub fn is_playing_unpaused(&self) -> bool {
        let s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        s.is_playing_unpaused()
    }

    /// `true` para o canal de presenca: interface em primeiro plano OU partida rodando sem pausa.
    pub fn is_active(&self) -> bool {
        let s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        s.is_active()
    }

    pub fn background_step(&self) -> u32 {
        let s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        s.background_step
    }

    pub fn current_game(&self) -> Option<PresenceGame> {
        let s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        s.game.clone()
    }

    pub fn snapshot(&self) -> ActivitySnapshot {
        let s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        ActivitySnapshot {
            is_active: s.is_active(),
            background_step: s.background_step,
            returned_from_background: s.returned_from_background,
            game: s.game.clone(),
        }
    }

    fn apply_mutation(&self, f: impl FnOnce(&mut ActivityState) -> bool) {
        let should_wake = {
            let mut s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
            let prev_fg = s.is_foreground();
            let prev_active = s.is_active();
            let force = f(&mut s);
            let now_fg = s.is_foreground();
            let now_active = s.is_active();

            if !prev_active && now_active {
                s.background_step = 0;
                s.returned_from_background = true;
            } else if prev_active && !now_active {
                s.background_step = 0;
                s.returned_from_background = false;
            } else if !prev_fg && now_fg {
                s.background_step = 0;
            }

            force || prev_fg != now_fg || prev_active != now_active
        };
        if should_wake {
            self.inner.notify.notify_waiters();
        }
    }

    /// Atualiza visibilidade/minimizado da janela nativa sem sobrescrever `ui_focused`
    /// quando a janela segue aberta.
    pub fn set_window_open(&self, open: bool) {
        self.apply_mutation(|s| {
            s.window_open = open;
            if !open {
                s.ui_focused = false;
            }
            false
        });
    }

    /// Atualiza estado completo da janela nativa (aberta e focada).
    pub fn set_window_state(&self, open: bool, focused: bool) {
        self.apply_mutation(|s| {
            s.window_open = open;
            s.ui_focused = open && focused;
            false
        });
    }

    /// Sincroniza visibilidade e foco vindos do WebView (`document.visibilityState` e `hasFocus()`).
    pub fn set_ui_active(&self, visible: bool, focused: bool) {
        self.apply_mutation(|s| {
            s.ui_visible = visible;
            s.ui_focused = visible && focused;
            false
        });
    }

    /// Usado em testes e notificacoes manuais da sessao do emulador.
    pub fn set_playing_unpaused(&self, playing_unpaused: bool) {
        self.apply_mutation(|s| {
            s.playing_unpaused = playing_unpaused;
            false
        });
    }

    /// Registra ou limpa o jogo atualmente aberto no emulador.
    pub fn set_game(&self, game: Option<PresenceGame>) {
        let cleaned = game.map(|g| PresenceGame {
            rom_id: g.rom_id,
            game_id: clean_game_uuid(g.game_id.as_deref()),
            game_title: clean_game_title(g.game_title.as_deref()),
            platform: clean_system_id(g.platform.as_deref()),
        });
        self.apply_mutation(|s| {
            let changed = s.game != cleaned;
            s.game = cleaned;
            changed
        });
    }

    pub fn update_game_title_if_missing(&self, fallback_title: &str) {
        let Some(clean) = clean_game_title(Some(fallback_title)) else {
            return;
        };
        self.apply_mutation(|s| {
            if let Some(g) = s.game.as_mut() {
                if g.game_title.is_none() {
                    g.game_title = Some(clean);
                    return true;
                }
            }
            false
        });
    }

    pub fn mark_reconnected(&self) {
        {
            let mut s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
            s.background_step = 0;
            s.returned_from_background = false;
            s.session_reset = true;
        }
        self.inner.notify.notify_waiters();
    }

    pub fn mark_shutting_down(&self) {
        {
            let mut s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
            s.shutting_down = true;
        }
        self.inner.notify.notify_waiters();
    }

    pub fn is_shutting_down(&self) -> bool {
        let s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        s.shutting_down
    }

    fn advance_background_step(&self) {
        let mut s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        if !s.is_active() {
            s.background_step = s.background_step.saturating_add(1);
        }
    }

    fn clear_returned_from_background(&self) {
        let mut s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        s.returned_from_background = false;
    }

    fn take_session_reset(&self) -> bool {
        let mut s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::replace(&mut s.session_reset, false)
    }

    /// Aguarda o proximo ciclo adaptativo usando uma tabela explicita de recuo em segundo plano.
    ///
    /// - Em primeiro plano: usa `fg_interval` e zera `bg_step = 0`;
    /// - Em segundo plano: usa `bg_steps[*bg_step]` (fixando no teto final) e incrementa `bg_step`;
    /// - Ao retornar do segundo plano para o primeiro plano: acorda via `Notify` na hora; se ja se
    ///   passaram `>= fg_interval` desde `last_run_at`, retorna imediatamente com `bg_step = 0`,
    ///   caso contrario aguarda apenas o tempo restante para completar `fg_interval`.
    pub async fn wait_next_cycle(
        &self,
        fg_interval: Duration,
        bg_steps: &[Duration],
        bg_step: &mut u32,
        last_run_at: Instant,
    ) {
        loop {
            let notified = self.inner.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            if self.is_shutting_down() {
                return;
            }

            let is_fg = self.is_foreground();
            let target = if is_fg {
                *bg_step = 0;
                fg_interval
            } else {
                schedule_step(bg_steps, *bg_step)
            };

            let elapsed = Instant::now().saturating_duration_since(last_run_at);
            if elapsed >= target {
                if !is_fg {
                    *bg_step = bg_step.saturating_add(1);
                }
                return;
            }

            let remaining = target - elapsed;
            tokio::select! {
                () = tokio::time::sleep(remaining) => {
                    if self.is_foreground() {
                        *bg_step = 0;
                    } else {
                        *bg_step = bg_step.saturating_add(1);
                    }
                    return;
                }
                () = &mut notified => {}
            }
        }
    }

    /// Variante exponencial (`bg_base * 2^step` limitada por `bg_max`) com o mesmo despertar imediato.
    pub async fn wait_backoff_cycle(
        &self,
        fg_interval: Duration,
        bg_base: Duration,
        bg_max: Duration,
        bg_step: &mut u32,
        last_run_at: Instant,
    ) {
        loop {
            let notified = self.inner.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            if self.is_shutting_down() {
                return;
            }

            let is_fg = self.is_foreground();
            let target = if is_fg {
                *bg_step = 0;
                fg_interval
            } else {
                exponential_backoff(bg_base, bg_max, *bg_step)
            };

            let elapsed = Instant::now().saturating_duration_since(last_run_at);
            if elapsed >= target {
                if !is_fg {
                    *bg_step = bg_step.saturating_add(1);
                }
                return;
            }

            let remaining = target - elapsed;
            tokio::select! {
                () = tokio::time::sleep(remaining) => {
                    if self.is_foreground() {
                        *bg_step = 0;
                    } else {
                        *bg_step = bg_step.saturating_add(1);
                    }
                    return;
                }
                () = &mut notified => {}
            }
        }
    }

    /// Chamado antes de revogar os tokens em `Alvanista::disconnect` para limpar
    /// imediatamente o status online / jogando agora no servidor.
    pub(crate) async fn on_disconnect(&self, alva: &Alvanista) {
        {
            let mut s = self.inner.state.lock().unwrap_or_else(|p| p.into_inner());
            s.background_step = 0;
            s.returned_from_background = false;
            s.session_reset = true;
        }
        self.inner.notify.notify_waiters();
        send_offline_best_effort(alva).await;
    }
}

fn has_control_chars(s: &str) -> bool {
    s.bytes().any(|b| b < 0x20 || b == 0x7f)
}

pub fn clean_game_title(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() || has_control_chars(s) {
        return None;
    }
    if s.len() <= MAX_GAME_TITLE_LEN {
        return Some(s.to_owned());
    }
    let mut end = MAX_GAME_TITLE_LEN;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    (end > 0).then(|| s[..end].to_owned())
}

pub fn build_presence_payload(
    state: PresenceState,
    background_step: u32,
    game: Option<&PresenceGame>,
) -> PresencePayload {
    match state {
        PresenceState::Offline => PresencePayload {
            state: PresenceState::Offline,
            background_step: 0,
            game_id: None,
            game_title: None,
            platform: None,
        },
        PresenceState::Active | PresenceState::Background => {
            let step = if state == PresenceState::Active {
                0
            } else {
                background_step
            };
            let (game_id, game_title, platform) = match game {
                Some(g) => (
                    clean_game_uuid(g.game_id.as_deref()),
                    clean_game_title(g.game_title.as_deref()),
                    clean_system_id(g.platform.as_deref()),
                ),
                None => (None, None, None),
            };
            PresencePayload {
                state,
                background_step: step,
                game_id,
                game_title,
                platform,
            }
        }
    }
}

/// Cadencia em estado ativo: `120 s` por padrao, respeitando `next_after_s` do servidor (`60..=600 s`).
pub fn presence_active_interval(server_next_after_s: Option<u64>) -> Duration {
    match server_next_after_s.filter(|&s| s > 0) {
        Some(secs) => {
            Duration::from_secs(secs.clamp(PRESENCE_SERVER_MIN_SECS, PRESENCE_SERVER_MAX_SECS))
        }
        None => PRESENCE_ACTIVE_DEFAULT,
    }
}

/// Atraso do proximo envio em segundo plano (`4m -> 8m -> 16m -> 30m -> 60m`).
/// Sem jogo aberto, apos o passo `3` (`step >= 4`), devolve `None` para entrar em repouso profundo.
pub fn presence_background_delay(
    step: u32,
    has_game: bool,
    server_next_after_s: Option<u64>,
) -> Option<Duration> {
    if !has_game && step >= 4 {
        return None;
    }
    let idx = (step as usize).min(PRESENCE_BG_STEPS_SECS.len() - 1);
    let base_secs = PRESENCE_BG_STEPS_SECS[idx];
    let secs = match server_next_after_s.filter(|&s| s > 0) {
        Some(srv) => base_secs.max(srv.min(PRESENCE_BG_SERVER_MAX_SECS)),
        None => base_secs,
    };
    Some(Duration::from_secs(secs))
}

/// Tempo de espera antes do proximo ping `"active"`, aplicando antiflap na volta do segundo plano.
pub fn presence_active_wait(
    now: Instant,
    last_active_at: Option<Instant>,
    last_sent: Option<(PresenceState, Instant, Option<u64>)>,
    returned_from_background: bool,
    game_changed: bool,
) -> Duration {
    let Some((last_state, last_at, server_next_after_s)) = last_sent else {
        return Duration::ZERO;
    };

    if game_changed {
        return Duration::ZERO;
    }

    if returned_from_background || last_state != PresenceState::Active {
        let active_gap = last_active_at.map_or(Duration::ZERO, |t| {
            MIN_ACTIVE_GAP.saturating_sub(now.saturating_duration_since(t))
        });
        let transition_gap = if last_state != PresenceState::Active {
            MIN_TRANSITION_GAP.saturating_sub(now.saturating_duration_since(last_at))
        } else {
            Duration::ZERO
        };
        return active_gap.max(transition_gap);
    }

    let interval = presence_active_interval(server_next_after_s);
    interval.saturating_sub(now.saturating_duration_since(last_at))
}

pub fn schedule_step(steps: &[Duration], step: u32) -> Duration {
    if steps.is_empty() {
        return Duration::from_secs(60);
    }
    let idx = (step as usize).min(steps.len() - 1);
    steps[idx]
}

pub fn exponential_backoff(bg_base: Duration, bg_max: Duration, step: u32) -> Duration {
    let shift = step.min(16);
    let mult = 1u32 << shift;
    bg_base.saturating_mul(mult).min(bg_max)
}

pub fn sync_idle_interval(is_foreground: bool, bg_step: u32) -> Duration {
    if is_foreground {
        SYNC_FG_EVERY
    } else {
        schedule_step(&SYNC_BG_STEPS, bg_step)
    }
}

pub fn catalog_idle_interval(is_foreground: bool, bg_step: u32) -> Duration {
    if is_foreground {
        CATALOG_FG_EVERY
    } else {
        schedule_step(&CATALOG_BG_STEPS, bg_step)
    }
}

pub fn gas_heartbeat_interval(is_foreground: bool, bg_step: u32) -> Duration {
    if is_foreground {
        GAS_FG_EVERY
    } else {
        schedule_step(&GAS_BG_STEPS, bg_step)
    }
}

pub(crate) async fn post_presence(
    alva: &Alvanista,
    payload: &PresencePayload,
    timeout: Option<Duration>,
) -> Option<u64> {
    if !alva.has_session().await {
        return None;
    }
    let Ok(body) = serde_json::to_value(payload) else {
        return None;
    };
    let Ok((status, val)) = alva
        .partner_json_with_timeout(reqwest::Method::POST, PRESENCE_PATH, Some(&body), timeout)
        .await
    else {
        return None;
    };
    if !(200..300).contains(&status) {
        return None;
    }
    serde_json::from_value::<PresenceResponse>(val)
        .ok()
        .and_then(|r| r.next_after_s)
}

pub(crate) async fn send_offline_best_effort(alva: &Alvanista) {
    if !alva.has_session().await {
        return;
    }
    let payload = build_presence_payload(PresenceState::Offline, 0, None);
    let _ = tokio::time::timeout(
        OFFLINE_EXIT_TIMEOUT,
        post_presence(alva, &payload, Some(OFFLINE_EXIT_TIMEOUT)),
    )
    .await;
}

pub(crate) trait PresenceBackend: Send + Sync + 'static {
    fn has_session(&self) -> impl Future<Output = bool> + Send;
    fn send_presence(&self, payload: PresencePayload) -> impl Future<Output = Option<u64>> + Send;
}

impl PresenceBackend for AppHandle {
    async fn has_session(&self) -> bool {
        let Some(alva) = self.try_state::<Alvanista>() else {
            return false;
        };
        alva.has_session().await
    }

    async fn send_presence(&self, payload: PresencePayload) -> Option<u64> {
        let alva = self.try_state::<Alvanista>()?;
        post_presence(&alva, &payload, None).await
    }
}

pub(crate) async fn run_presence_loop<B: PresenceBackend>(
    coordinator: ActivityCoordinator,
    backend: B,
) {
    let mut last_sent: Option<(PresenceState, Instant, Option<u64>)> = None;
    let mut last_active_at: Option<Instant> = None;
    let mut last_sent_game: Option<PresenceGame> = None;
    let mut bg_wait_started_at: Option<Instant> = None;

    loop {
        if coordinator.is_shutting_down() {
            break;
        }

        let notified = coordinator.inner.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();

        if coordinator.is_shutting_down() {
            break;
        }

        if !backend.has_session().await {
            let _ = coordinator.take_session_reset();
            last_sent = None;
            last_active_at = None;
            last_sent_game = None;
            bg_wait_started_at = None;
            tokio::select! {
                () = &mut notified => {}
                () = tokio::time::sleep(UNAUTHENTICATED_IDLE_SLEEP) => {}
            }
            continue;
        }

        if coordinator.take_session_reset() {
            last_sent = None;
            last_active_at = None;
            last_sent_game = None;
            bg_wait_started_at = None;
        }

        let snap = coordinator.snapshot();
        let now = Instant::now();

        if snap.is_active {
            bg_wait_started_at = None;
            let game_changed = last_sent.is_some() && snap.game != last_sent_game;
            let wait = presence_active_wait(
                now,
                last_active_at,
                last_sent,
                snap.returned_from_background,
                game_changed,
            );

            if !wait.is_zero() {
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    () = &mut notified => {
                        continue;
                    }
                }
                if coordinator.is_shutting_down() || !coordinator.is_active() {
                    continue;
                }
            }

            if !backend.has_session().await || coordinator.is_shutting_down() {
                continue;
            }

            let current_game = coordinator.current_game();
            let payload = build_presence_payload(PresenceState::Active, 0, current_game.as_ref());
            coordinator.clear_returned_from_background();
            let next_after_s = backend.send_presence(payload).await;
            let sent_at = Instant::now();
            last_active_at = Some(sent_at);
            last_sent = Some((PresenceState::Active, sent_at, next_after_s));
            last_sent_game = current_game;
        } else {
            let has_game = snap.game.as_ref().is_some_and(PresenceGame::has_identity);
            let server_hint = last_sent.and_then(|(_, _, hint)| hint);
            let Some(target_delay) =
                presence_background_delay(snap.background_step, has_game, server_hint)
            else {
                bg_wait_started_at = None;
                notified.await;
                continue;
            };

            let start = *bg_wait_started_at.get_or_insert(now);
            let elapsed = now.saturating_duration_since(start);
            if elapsed < target_delay {
                let remaining = target_delay - elapsed;
                tokio::select! {
                    () = tokio::time::sleep(remaining) => {}
                    () = &mut notified => {
                        continue;
                    }
                }
                if coordinator.is_shutting_down() || coordinator.is_active() {
                    bg_wait_started_at = None;
                    continue;
                }
            }

            bg_wait_started_at = None;
            if !backend.has_session().await || coordinator.is_shutting_down() {
                continue;
            }

            let step = coordinator.background_step();
            let current_game = coordinator.current_game();
            let payload =
                build_presence_payload(PresenceState::Background, step, current_game.as_ref());
            let next_after_s = backend.send_presence(payload).await;
            coordinator.advance_background_step();
            let sent_at = Instant::now();
            last_sent = Some((PresenceState::Background, sent_at, next_after_s));
            last_sent_game = current_game;
        }
    }
}

// --- Integracao com o ciclo de vida Tauri ----------------------------------

pub fn install_session_probe(app: &AppHandle) {
    let Some(coord) = app.try_state::<ActivityCoordinator>() else {
        return;
    };
    let probe_app = app.clone();
    coord.install_playing_probe(Arc::new(move || {
        let Some(st) = probe_app.try_state::<AppState>() else {
            return false;
        };
        matches!(st.session.state(), emu_session::SessionState::Running)
    }));
}

pub fn spawn_on_start(app: AppHandle) {
    let Some(coord) = app
        .try_state::<ActivityCoordinator>()
        .map(|c| c.inner().clone())
    else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        run_presence_loop(coord, app).await;
    });
}

fn main_window_is_open<R: Runtime>(app: &AppHandle<R>) -> bool {
    let Some(win) = app.get_webview_window("main") else {
        return true;
    };
    !win.is_minimized().unwrap_or(false) && win.is_visible().unwrap_or(true)
}

pub fn sync_from_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(coord) = app.try_state::<ActivityCoordinator>() else {
        return;
    };
    coord.set_window_open(main_window_is_open(app));
}

pub fn on_main_window_restored<R: Runtime>(app: &AppHandle<R>) {
    let Some(coord) = app.try_state::<ActivityCoordinator>() else {
        return;
    };
    coord.set_window_state(true, true);
}

pub fn on_main_window_focused<R: Runtime>(app: &AppHandle<R>, focused: bool) {
    let Some(coord) = app.try_state::<ActivityCoordinator>() else {
        return;
    };
    let open = main_window_is_open(app);
    coord.set_window_state(open, focused);
}

pub fn on_main_window_destroyed<R: Runtime>(app: &AppHandle<R>) {
    let Some(coord) = app.try_state::<ActivityCoordinator>() else {
        return;
    };
    coord.set_window_state(false, false);
}

pub fn notify_session_changed<R: Runtime>(app: &AppHandle<R>) {
    if let Some(coord) = app.try_state::<ActivityCoordinator>() {
        coord.apply_mutation(|_| false);
    }
}

pub fn notify_connected<R: Runtime>(app: &AppHandle<R>) {
    if let Some(coord) = app.try_state::<ActivityCoordinator>() {
        coord.mark_reconnected();
    }
}

pub fn on_game_launched(
    app: &AppHandle,
    rom_id: Option<&str>,
    game_id: Option<String>,
    game_title: Option<String>,
    platform: Option<String>,
) {
    let Some(coord) = app.try_state::<ActivityCoordinator>() else {
        return;
    };
    coord.set_game(Some(PresenceGame {
        rom_id: rom_id.map(str::to_owned),
        game_id,
        game_title,
        platform,
    }));
}

pub fn on_game_title_fallback(app: &AppHandle, title: &str) {
    if let Some(coord) = app.try_state::<ActivityCoordinator>() {
        coord.update_game_title_if_missing(title);
    }
}

pub fn on_game_unloaded(app: &AppHandle) {
    if let Some(coord) = app.try_state::<ActivityCoordinator>() {
        coord.set_game(None);
    }
}

pub async fn on_app_exit(app: &AppHandle) {
    if let Some(coord) = app.try_state::<ActivityCoordinator>() {
        coord.mark_shutting_down();
    }
    if let Some(alva) = app.try_state::<Alvanista>() {
        send_offline_best_effort(&alva).await;
    }
}

#[tauri::command]
pub fn alvanista_set_ui_active(app: AppHandle, visible: bool, focused: bool) {
    let Some(coord) = app.try_state::<ActivityCoordinator>() else {
        return;
    };
    let open = main_window_is_open(&app);
    coord.set_window_open(open);
    coord.set_ui_active(open && visible, open && focused);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::Client;
    use crate::alvanista::store::test_support::{tokens, MemSecrets, SERIAL};
    use crate::alvanista::store::Store;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const VALID_GAME: &str = "550E8400-E29B-41D4-A716-446655440000";
    const LOWER_GAME: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[derive(Clone)]
    struct FakeBackend {
        connected: Arc<AtomicBool>,
        next_after_s: Arc<AtomicU64>,
        sent: Arc<Mutex<Vec<(Instant, PresencePayload)>>>,
    }

    impl FakeBackend {
        fn new(next_after_s: u64) -> Self {
            Self {
                connected: Arc::new(AtomicBool::new(true)),
                next_after_s: Arc::new(AtomicU64::new(next_after_s)),
                sent: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn payloads(&self) -> Vec<PresencePayload> {
            self.sent
                .lock()
                .unwrap()
                .iter()
                .map(|(_, p)| p.clone())
                .collect()
        }
    }

    impl PresenceBackend for FakeBackend {
        async fn has_session(&self) -> bool {
            self.connected.load(Ordering::SeqCst)
        }

        async fn send_presence(&self, payload: PresencePayload) -> Option<u64> {
            self.sent.lock().unwrap().push((Instant::now(), payload));
            let hint = self.next_after_s.load(Ordering::SeqCst);
            (hint > 0).then_some(hint)
        }
    }

    impl PresenceBackend for Arc<Alvanista> {
        async fn has_session(&self) -> bool {
            Alvanista::has_session(self).await
        }

        async fn send_presence(&self, payload: PresencePayload) -> Option<u64> {
            post_presence(self, &payload, None).await
        }
    }

    async fn alva_connected(server: &MockServer) -> Arc<Alvanista> {
        let secrets = Arc::new(MemSecrets::default());
        let db = db::connect_in_memory().await.unwrap();
        let store = Store::new(db, secrets);
        store
            .start_session(&tokens("pres"), &["profile:read"])
            .await
            .unwrap();
        Arc::new(Alvanista::new(Client::with_api(server.uri(), store)))
    }

    async fn settle() {
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }

    #[test]
    fn schedule_functions_match_exact_cadences_and_ceilings() {
        // Active cadence: default 120s, server hint clamped to 60..=600.
        assert_eq!(presence_active_interval(None), Duration::from_secs(120));
        assert_eq!(presence_active_interval(Some(0)), Duration::from_secs(120));
        assert_eq!(presence_active_interval(Some(15)), Duration::from_secs(60));
        assert_eq!(
            presence_active_interval(Some(180)),
            Duration::from_secs(180)
        );
        assert_eq!(
            presence_active_interval(Some(9999)),
            Duration::from_secs(600)
        );

        // Background presence without open game: 4m -> 8m -> 16m -> 30m -> deep sleep (None).
        assert_eq!(
            presence_background_delay(0, false, None),
            Some(Duration::from_secs(240))
        );
        assert_eq!(
            presence_background_delay(1, false, None),
            Some(Duration::from_secs(480))
        );
        assert_eq!(
            presence_background_delay(2, false, None),
            Some(Duration::from_secs(960))
        );
        assert_eq!(
            presence_background_delay(3, false, None),
            Some(Duration::from_secs(1800))
        );
        assert_eq!(presence_background_delay(4, false, None), None);
        assert_eq!(presence_background_delay(9, false, None), None);

        // Background presence with open game (paused): 60m ceiling instead of deep sleep.
        assert_eq!(
            presence_background_delay(4, true, None),
            Some(Duration::from_secs(3600))
        );
        assert_eq!(
            presence_background_delay(8, true, None),
            Some(Duration::from_secs(3600))
        );

        // Honors server next_after_s in background when larger than computed backoff.
        assert_eq!(
            presence_background_delay(0, false, Some(600)),
            Some(Duration::from_secs(600))
        );
        assert_eq!(
            presence_background_delay(2, false, Some(300)),
            Some(Duration::from_secs(960))
        );

        // Cloud Save: 5m fg vs 10m -> 20m -> 40m -> 60m -> 120m bg.
        assert_eq!(sync_idle_interval(true, 0), Duration::from_secs(5 * 60));
        let sync_bg: Vec<u64> = (0..6)
            .map(|step| sync_idle_interval(false, step).as_secs() / 60)
            .collect();
        assert_eq!(sync_bg, vec![10, 20, 40, 60, 120, 120]);

        // Catalog: 15m fg vs 30m -> 60m -> 120m -> 240m bg.
        assert_eq!(catalog_idle_interval(true, 0), Duration::from_secs(15 * 60));
        let cat_bg: Vec<u64> = (0..5)
            .map(|step| catalog_idle_interval(false, step).as_secs() / 60)
            .collect();
        assert_eq!(cat_bg, vec![30, 60, 120, 240, 240]);

        // GAS: 15m fg vs 30m -> 60m bg.
        assert_eq!(
            gas_heartbeat_interval(true, 0),
            Duration::from_secs(15 * 60)
        );
        let gas_bg: Vec<u64> = (0..4)
            .map(|step| gas_heartbeat_interval(false, step).as_secs() / 60)
            .collect();
        assert_eq!(gas_bg, vec![30, 60, 60, 60]);

        // Exponential backoff helper.
        assert_eq!(
            exponential_backoff(Duration::from_secs(30), Duration::from_secs(100), 2),
            Duration::from_secs(100)
        );
    }

    #[test]
    fn build_presence_payload_sanitizes_fields_and_clears_on_offline() {
        let game = PresenceGame {
            rom_id: Some("r1".into()),
            game_id: Some(VALID_GAME.into()),
            game_title: Some("  Chrono Trigger  ".into()),
            platform: Some(" snes ".into()),
        };

        let active = build_presence_payload(PresenceState::Active, 5, Some(&game));
        let wire = serde_json::to_value(&active).unwrap();
        assert_eq!(
            wire,
            json!({
                "state": "active",
                "background_step": 0,
                "game_id": LOWER_GAME,
                "game_title": "Chrono Trigger",
                "platform": "snes"
            })
        );

        let bg = build_presence_payload(PresenceState::Background, 2, Some(&game));
        assert_eq!(bg.background_step, 2);
        assert_eq!(bg.state, PresenceState::Background);

        let offline = build_presence_payload(PresenceState::Offline, 3, Some(&game));
        let off_wire = serde_json::to_value(&offline).unwrap();
        assert_eq!(
            off_wire,
            json!({
                "state": "offline",
                "background_step": 0
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn foreground_cadence_and_gradual_background_backoff_to_deep_sleep_and_wake() {
        let backend = FakeBackend::new(120);
        let coord = ActivityCoordinator::new();
        let task = tokio::spawn(run_presence_loop(coord.clone(), backend.clone()));

        // 1) Ping inicial imediato em primeiro plano (t = 0).
        settle().await;
        assert_eq!(backend.payloads().len(), 1);
        assert_eq!(backend.payloads()[0].state, PresenceState::Active);
        assert_eq!(backend.payloads()[0].background_step, 0);

        // 2) Cadencia ativa de 120s em primeiro plano.
        tokio::time::advance(Duration::from_secs(119)).await;
        settle().await;
        assert_eq!(backend.payloads().len(), 1);

        tokio::time::advance(Duration::from_secs(1)).await;
        settle().await;
        assert_eq!(backend.payloads().len(), 2);

        // 3) Vai para segundo plano sem jogo aberto: 4m -> 8m -> 16m -> 30m -> deep sleep.
        coord.set_ui_active(false, false);
        settle().await;
        assert_eq!(backend.payloads().len(), 2);

        for (expected_step, secs) in [(0u32, 240u64), (1, 480), (2, 960), (3, 1800)] {
            tokio::time::advance(Duration::from_secs(secs - 1)).await;
            settle().await;
            assert_eq!(
                backend.payloads().len(),
                2 + expected_step as usize,
                "nao devia disparar antes de {secs}s (step {expected_step})"
            );

            tokio::time::advance(Duration::from_secs(1)).await;
            settle().await;
            let sent = backend.payloads();
            assert_eq!(sent.len(), 3 + expected_step as usize);
            let p = sent.last().unwrap();
            assert_eq!(p.state, PresenceState::Background);
            assert_eq!(p.background_step, expected_step);
        }

        // 4) Apos step 3 sem jogo aberto: entra em repouso profundo (zero requisicoes mesmo apos 6 horas).
        assert_eq!(coord.background_step(), 4);
        tokio::time::advance(Duration::from_secs(6 * 3600)).await;
        settle().await;
        assert_eq!(backend.payloads().len(), 6);

        // 5) Volta ao primeiro plano: acorda na hora, zera background_step e envia "active" imediatamente.
        coord.set_ui_active(true, true);
        settle().await;
        assert_eq!(coord.background_step(), 0);
        let sent = backend.payloads();
        assert_eq!(sent.len(), 7);
        assert_eq!(sent.last().unwrap().state, PresenceState::Active);
        assert_eq!(sent.last().unwrap().background_step, 0);

        coord.mark_shutting_down();
        let _ = task.await;
    }

    #[tokio::test(start_paused = true)]
    async fn active_gameplay_stays_active_when_unfocused_and_paused_game_caps_at_60m_in_bg() {
        let backend = FakeBackend::new(120);
        let coord = ActivityCoordinator::new();
        coord.set_game(Some(PresenceGame {
            rom_id: Some("r1".into()),
            game_id: Some(VALID_GAME.into()),
            game_title: Some("Super Metroid".into()),
            platform: Some("snes".into()),
        }));
        coord.set_playing_unpaused(true);
        assert!(coord.is_playing_unpaused());

        let task = tokio::spawn(run_presence_loop(coord.clone(), backend.clone()));
        settle().await;
        assert_eq!(backend.payloads().len(), 1);
        assert_eq!(backend.payloads()[0].state, PresenceState::Active);

        // Janela sem foco, mas jogo rodando sem pausa (ex.: segundo monitor + controle): continua Active!
        coord.set_ui_active(true, false);
        settle().await;
        assert!(coord.is_active());
        tokio::time::advance(Duration::from_secs(120)).await;
        settle().await;
        assert_eq!(backend.payloads().len(), 2);
        assert_eq!(backend.payloads()[1].state, PresenceState::Active);

        // Pausou o jogo com a janela sem foco: transita para Background ate o teto de 60m.
        coord.set_playing_unpaused(false);
        settle().await;
        assert!(!coord.is_active());
        for secs in [240u64, 480, 960, 1800, 3600] {
            tokio::time::advance(Duration::from_secs(secs)).await;
            settle().await;
        }
        let sent = backend.payloads();
        assert_eq!(sent.len(), 7);
        assert_eq!(sent.last().unwrap().state, PresenceState::Background);
        assert_eq!(sent.last().unwrap().background_step, 4);
        assert_eq!(
            sent.last().unwrap().game_title.as_deref(),
            Some("Super Metroid")
        );

        coord.mark_shutting_down();
        let _ = task.await;
    }

    #[tokio::test(start_paused = true)]
    async fn anti_flap_debounce_and_server_next_after_s_are_respected() {
        let backend = FakeBackend::new(300);
        let coord = ActivityCoordinator::new();
        let task = tokio::spawn(run_presence_loop(coord.clone(), backend.clone()));

        settle().await;
        assert_eq!(backend.payloads().len(), 1);

        // Varias trocas rapidas Background <-> Foreground em menos de 20s nao geram spam.
        for _ in 0..5 {
            tokio::time::advance(Duration::from_secs(2)).await;
            coord.set_ui_active(true, false);
            settle().await;
            tokio::time::advance(Duration::from_secs(1)).await;
            coord.set_ui_active(true, true);
            settle().await;
        }
        assert_eq!(backend.payloads().len(), 1);

        // Ao completar 20s (MIN_ACTIVE_GAP), envia apenas 1 ping "active" consolidado.
        tokio::time::advance(Duration::from_secs(5)).await;
        settle().await;
        assert_eq!(backend.payloads().len(), 2);

        // Servidor pediu next_after_s = 300s (em vez do default 120s).
        tokio::time::advance(Duration::from_secs(120)).await;
        settle().await;
        assert_eq!(backend.payloads().len(), 2);

        tokio::time::advance(Duration::from_secs(180)).await;
        settle().await;
        assert_eq!(backend.payloads().len(), 3);

        coord.mark_shutting_down();
        let _ = task.await;
    }

    #[tokio::test]
    async fn wiremock_posts_active_presence_and_sends_offline_before_revoke_on_disconnect() {
        let _serial = SERIAL.lock().await;
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/presence"))
            .and(header("authorization", "Bearer alp_pres"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "ok": true, "state": "active", "next_after_s": 240 })),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/revoke"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let alva = alva_connected(&server).await;
        let coord = ActivityCoordinator::new();
        coord.set_game(Some(PresenceGame {
            rom_id: Some("rom_1".into()),
            game_id: Some(VALID_GAME.into()),
            game_title: Some("Chrono Trigger".into()),
            platform: Some("snes".into()),
        }));
        alva.attach_presence(coord.clone());

        let active_payload =
            build_presence_payload(PresenceState::Active, 0, coord.current_game().as_ref());
        let next_after = post_presence(&alva, &active_payload, None).await;
        assert_eq!(next_after, Some(240));

        // Desconectar envia "offline" antes de revogar o par OAuth.
        assert_eq!(alva.disconnect().await, Ok(()));
        let reqs = server.received_requests().await.unwrap();
        let paths: Vec<String> = reqs
            .iter()
            .map(|r| format!("{} {}", r.method, r.url.path()))
            .collect();
        assert_eq!(
            paths,
            vec![
                "POST /api/partner/v1/presence",
                "POST /api/partner/v1/presence",
                "POST /api/partner/oauth/revoke",
            ]
        );
        let first_body: serde_json::Value = serde_json::from_slice(&reqs[0].body).unwrap();
        assert_eq!(
            first_body,
            json!({
                "state": "active",
                "background_step": 0,
                "game_id": LOWER_GAME,
                "game_title": "Chrono Trigger",
                "platform": "snes"
            })
        );
        let offline_body: serde_json::Value = serde_json::from_slice(&reqs[1].body).unwrap();
        assert_eq!(
            offline_body,
            json!({
                "state": "offline",
                "background_step": 0
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn wait_next_cycle_backs_off_in_background_and_wakes_immediately_on_foreground() {
        let coord = ActivityCoordinator::new();
        coord.set_ui_active(false, false);

        let mut bg_step = 0u32;
        let mut t_last = Instant::now();
        let mut observed_mins = Vec::new();

        for _ in 0..5 {
            let start = Instant::now();
            let c = coord.clone();
            let handle = tokio::spawn(async move {
                let mut step = bg_step;
                c.wait_next_cycle(SYNC_FG_EVERY, &SYNC_BG_STEPS, &mut step, t_last)
                    .await;
                (step, Instant::now().duration_since(start))
            });
            settle().await;
            while !handle.is_finished() {
                tokio::time::advance(Duration::from_secs(60)).await;
                settle().await;
            }
            let (next_step, waited) = handle.await.unwrap();
            bg_step = next_step;
            observed_mins.push(waited.as_secs() / 60);
            t_last = Instant::now();
        }
        assert_eq!(observed_mins, vec![10, 20, 40, 60, 120]);
        assert_eq!(bg_step, 5);

        // Agora em segundo plano dormindo 120m; apos 7m (>= 5m de foreground), o usuario foca a janela.
        t_last = Instant::now();
        let c = coord.clone();
        let wake_handle = tokio::spawn(async move {
            let mut step = bg_step;
            c.wait_next_cycle(SYNC_FG_EVERY, &SYNC_BG_STEPS, &mut step, t_last)
                .await;
            (step, Instant::now().duration_since(t_last))
        });
        settle().await;
        tokio::time::advance(Duration::from_secs(7 * 60)).await;
        settle().await;
        assert!(!wake_handle.is_finished());

        coord.set_ui_active(true, true);
        let (new_step, waited) = wake_handle.await.unwrap();
        assert_eq!(new_step, 0);
        assert_eq!(waited, Duration::from_secs(7 * 60));

        // Exercita tambem wait_backoff_cycle exponencial.
        coord.set_ui_active(false, false);
        let t0 = Instant::now();
        let c2 = coord.clone();
        let exp_handle = tokio::spawn(async move {
            let mut step = 1u32;
            c2.wait_backoff_cycle(
                Duration::from_secs(60),
                Duration::from_secs(120),
                Duration::from_secs(600),
                &mut step,
                t0,
            )
            .await;
            (step, Instant::now().duration_since(t0))
        });
        settle().await;
        tokio::time::advance(Duration::from_secs(240)).await;
        let (exp_step, exp_waited) = exp_handle.await.unwrap();
        assert_eq!(exp_step, 2);
        assert_eq!(exp_waited, Duration::from_secs(240));
    }
}
