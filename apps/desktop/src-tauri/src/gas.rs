//! Emissão assíncrona de sinais de atividade GAS para o Alvanista
//! (`POST /api/partner/v1/gas/signals`).
//!
//! Usa apenas a sessão OAuth do parceiro (`alp_` com `profile:read`), funcionando
//! mesmo quando a pessoa não habilitou o escopo opcional `gamelog`.
//! - Nunca bloqueia a abertura ou o fechamento do jogo;
//! - Nunca registra nomes ou caminhos de ROM em log;
//! - Falha silenciosamente quando offline ou diante de 404/5xx.

use crate::alvanista::Alvanista;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

pub const GAS_SIGNALS_PATH: &str = "/api/partner/v1/gas/signals";

pub const SIGNAL_SESSION_STARTED: &str = "emulator.session_started";
pub const SIGNAL_SESSION_HEARTBEAT: &str = "emulator.session_heartbeat";
pub const SIGNAL_SESSION_COMPLETED: &str = "emulator.session_completed";
pub const SIGNAL_ACHIEVEMENT_UNLOCKED: &str = "emulator.achievement_unlocked";

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15 * 60);
const HEARTBEAT_DELTA_MIN: i64 = 15;
const MIN_COMPLETED_PLAYTIME: Duration = Duration::from_secs(60);

const MAX_PLAYTIME_MIN: i32 = 720;
const MAX_HEARTBEAT_MIN: i32 = 60;
const MAX_IDEMPOTENCY_KEY_LEN: usize = 128;
const MAX_SYSTEM_ID_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GasSignalPayload {
    pub idempotency_key: String,
    pub signal_type: &'static str,
    pub value: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_id: Option<String>,
    pub playtime_min: i32,
    pub ra_game_id: u32,
    pub ra_achievement_id: u32,
    pub hardcore: bool,
}

#[derive(Debug, Clone)]
struct ActiveGasSession {
    session_uuid: String,
    rom_id: Option<String>,
    game_id: Option<String>,
    system_id: Option<String>,
    ra_game_id: u32,
    hardcore: bool,
    started_at: Instant,
}

#[derive(Default)]
struct GasInner {
    active: Option<ActiveGasSession>,
    pending_ra: Option<(Option<String>, u32, bool)>,
    heartbeat_task: Option<tauri::async_runtime::JoinHandle<()>>,
}

#[derive(Default)]
pub struct GasState {
    inner: Mutex<GasInner>,
}

fn has_control_chars(s: &str) -> bool {
    s.bytes().any(|b| b < 0x20 || b == 0x7f)
}

pub fn clean_game_uuid(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.len() != 36 {
        return None;
    }
    for (idx, b) in s.bytes().enumerate() {
        if matches!(idx, 8 | 13 | 18 | 23) {
            if b != b'-' {
                return None;
            }
        } else if !b.is_ascii_hexdigit() {
            return None;
        }
    }
    Some(s.to_ascii_lowercase())
}

pub fn clean_system_id(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() || has_control_chars(s) {
        return None;
    }
    if s.len() <= MAX_SYSTEM_ID_LEN {
        return Some(s.to_owned());
    }
    let mut end = MAX_SYSTEM_ID_LEN;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    (end > 0).then(|| s[..end].to_owned())
}

fn clean_idempotency_key(raw: String) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() || s.len() > MAX_IDEMPOTENCY_KEY_LEN || has_control_chars(s) {
        return None;
    }
    Some(s.to_owned())
}

pub fn clamp_playtime_min(mins: i64, min_bound: i32) -> i32 {
    let lo = min_bound.clamp(0, MAX_PLAYTIME_MIN);
    mins.clamp(i64::from(lo), i64::from(MAX_PLAYTIME_MIN)) as i32
}

pub fn clamp_heartbeat_value(mins: i64) -> i32 {
    mins.clamp(1, i64::from(MAX_HEARTBEAT_MIN)) as i32
}

pub fn build_session_started(
    session_uuid: &str,
    game_id: Option<&str>,
    system_id: Option<&str>,
    ra_game_id: u32,
    hardcore: bool,
) -> Option<GasSignalPayload> {
    let uid = session_uuid.trim();
    if uid.is_empty() {
        return None;
    }
    let idempotency_key = clean_idempotency_key(format!("sess-start:{uid}"))?;
    Some(GasSignalPayload {
        idempotency_key,
        signal_type: SIGNAL_SESSION_STARTED,
        value: 1,
        game_id: clean_game_uuid(game_id),
        system_id: clean_system_id(system_id),
        playtime_min: 0,
        ra_game_id,
        ra_achievement_id: 0,
        hardcore,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn build_session_heartbeat(
    session_uuid: &str,
    tick: u32,
    delta_min: i64,
    playtime_min: i64,
    game_id: Option<&str>,
    system_id: Option<&str>,
    ra_game_id: u32,
    hardcore: bool,
) -> Option<GasSignalPayload> {
    let uid = session_uuid.trim();
    if uid.is_empty() || tick == 0 {
        return None;
    }
    let idempotency_key = clean_idempotency_key(format!("sess-hb:{uid}:{tick}"))?;
    Some(GasSignalPayload {
        idempotency_key,
        signal_type: SIGNAL_SESSION_HEARTBEAT,
        value: clamp_heartbeat_value(delta_min),
        game_id: clean_game_uuid(game_id),
        system_id: clean_system_id(system_id),
        playtime_min: clamp_playtime_min(playtime_min, 1),
        ra_game_id,
        ra_achievement_id: 0,
        hardcore,
    })
}

pub fn build_achievement_unlocked(
    session_uuid: &str,
    ach_id: u32,
    playtime_min: i64,
    game_id: Option<&str>,
    system_id: Option<&str>,
    ra_game_id: u32,
    hardcore: bool,
) -> Option<GasSignalPayload> {
    let uid = session_uuid.trim();
    if uid.is_empty() || ach_id == 0 {
        return None;
    }
    let idempotency_key = clean_idempotency_key(format!("ra-unlock:{uid}:{ach_id}"))?;
    Some(GasSignalPayload {
        idempotency_key,
        signal_type: SIGNAL_ACHIEVEMENT_UNLOCKED,
        value: 1,
        game_id: clean_game_uuid(game_id),
        system_id: clean_system_id(system_id),
        playtime_min: clamp_playtime_min(playtime_min, 0),
        ra_game_id,
        ra_achievement_id: ach_id,
        hardcore,
    })
}

pub fn build_session_completed(
    session_uuid: &str,
    elapsed: Duration,
    game_id: Option<&str>,
    system_id: Option<&str>,
    ra_game_id: u32,
    hardcore: bool,
) -> Option<GasSignalPayload> {
    if elapsed < MIN_COMPLETED_PLAYTIME {
        return None;
    }
    let uid = session_uuid.trim();
    if uid.is_empty() {
        return None;
    }
    let idempotency_key = clean_idempotency_key(format!("sess-end:{uid}"))?;
    let mins = clamp_playtime_min((elapsed.as_secs() / 60) as i64, 1);
    Some(GasSignalPayload {
        idempotency_key,
        signal_type: SIGNAL_SESSION_COMPLETED,
        value: mins,
        game_id: clean_game_uuid(game_id),
        system_id: clean_system_id(system_id),
        playtime_min: mins,
        ra_game_id,
        ra_achievement_id: 0,
        hardcore,
    })
}

pub(crate) async fn post_gas_signals(alva: &Alvanista, signals: &[GasSignalPayload]) -> bool {
    if signals.is_empty() || !alva.has_session().await {
        return false;
    }
    let body = serde_json::json!({ "signals": signals });
    matches!(
        alva.partner_json(reqwest::Method::POST, GAS_SIGNALS_PATH, Some(&body))
            .await,
        Ok((status, _)) if (200..300).contains(&status)
    )
}

fn spawn_signal(app: &AppHandle, signal: GasSignalPayload) {
    let app_bg = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(alva) = app_bg.try_state::<Alvanista>() {
            let _ = post_gas_signals(&alva, &[signal]).await;
        }
    });
}

/// Inicia a sessão GAS da partida atual e agenda o sinal inicial mais o
/// batimento de 15 em 15 minutos, sem bloquear o carregamento do jogo.
pub fn on_game_launched(
    app: &AppHandle,
    rom_id: Option<&str>,
    game_id: Option<String>,
    system_id: Option<String>,
    hardcore: bool,
) {
    let Some(gas) = app.try_state::<GasState>() else {
        return;
    };
    let session_uuid = Uuid::new_v4().to_string();
    let start_signal = {
        let mut g = gas.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(prev_hb) = g.heartbeat_task.take() {
            prev_hb.abort();
        }
        let (ra_game_id, pending_hc) = match g.pending_ra.take() {
            Some((ref prid, rgid, hc)) if prid.as_deref() == rom_id => (rgid, hc),
            _ => (0, false),
        };
        let eff_hc = hardcore || pending_hc;
        let sig = build_session_started(
            &session_uuid,
            game_id.as_deref(),
            system_id.as_deref(),
            ra_game_id,
            eff_hc,
        );
        g.active = Some(ActiveGasSession {
            session_uuid: session_uuid.clone(),
            rom_id: rom_id.map(str::to_owned),
            game_id,
            system_id,
            ra_game_id,
            hardcore: eff_hc,
            started_at: Instant::now(),
        });
        sig
    };

    if let Some(sig) = start_signal {
        spawn_signal(app, sig);
    }

    let app_hb = app.clone();
    let hb_uuid = session_uuid.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let mut tick = 0u32;
        loop {
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;
            tick = tick.saturating_add(1);
            let sig = {
                let Some(gas) = app_hb.try_state::<GasState>() else {
                    break;
                };
                let g = gas.inner.lock().unwrap_or_else(|p| p.into_inner());
                let Some(sess) = g.active.as_ref().filter(|s| s.session_uuid == hb_uuid) else {
                    break;
                };
                let elapsed_min = (sess.started_at.elapsed().as_secs() / 60) as i64;
                build_session_heartbeat(
                    &sess.session_uuid,
                    tick,
                    HEARTBEAT_DELTA_MIN,
                    elapsed_min,
                    sess.game_id.as_deref(),
                    sess.system_id.as_deref(),
                    sess.ra_game_id,
                    sess.hardcore,
                )
            };
            let Some(sig) = sig else {
                break;
            };
            if let Some(alva) = app_hb.try_state::<Alvanista>() {
                let _ = post_gas_signals(&alva, &[sig]).await;
            }
        }
    });

    let mut g = gas.inner.lock().unwrap_or_else(|p| p.into_inner());
    if g.active
        .as_ref()
        .is_some_and(|s| s.session_uuid == session_uuid)
    {
        g.heartbeat_task = Some(handle);
    } else {
        handle.abort();
    }
}

/// Atualiza o `ra_game_id` e `hardcore` da sessão GAS quando o RetroAchievements
/// identifica o jogo carregado.
pub fn on_ra_game_loaded(app: &AppHandle, rom_id: Option<String>, ra_game_id: u32, hardcore: bool) {
    let Some(gas) = app.try_state::<GasState>() else {
        return;
    };
    let mut g = gas.inner.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(sess) = g.active.as_mut() {
        if sess.rom_id.is_none() || rom_id.is_none() || sess.rom_id == rom_id {
            sess.ra_game_id = ra_game_id;
            sess.hardcore |= hardcore;
            return;
        }
    }
    g.pending_ra = Some((rom_id, ra_game_id, hardcore));
}

/// Dispara `emulator.achievement_unlocked` em segundo plano a cada conquista RA.
pub fn on_ra_achievement_unlocked(app: &AppHandle, ach_id: u32, hardcore: bool) {
    if ach_id == 0 {
        return;
    }
    let Some(gas) = app.try_state::<GasState>() else {
        return;
    };
    let sig = {
        let g = gas.inner.lock().unwrap_or_else(|p| p.into_inner());
        let Some(sess) = g.active.as_ref() else {
            return;
        };
        let elapsed_min = (sess.started_at.elapsed().as_secs() / 60) as i64;
        build_achievement_unlocked(
            &sess.session_uuid,
            ach_id,
            elapsed_min,
            sess.game_id.as_deref(),
            sess.system_id.as_deref(),
            sess.ra_game_id,
            hardcore || sess.hardcore,
        )
    };
    if let Some(sig) = sig {
        spawn_signal(app, sig);
    }
}

/// Encerra a sessão GAS ao fechar o jogo (ou trocar de jogo) e emite
/// `emulator.session_completed` em segundo plano quando jogado >= 1 minuto.
pub fn on_game_unloaded(app: &AppHandle) {
    let Some(gas) = app.try_state::<GasState>() else {
        return;
    };
    let prev = {
        let mut g = gas.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(hb) = g.heartbeat_task.take() {
            hb.abort();
        }
        g.pending_ra = None;
        g.active.take()
    };
    let Some(sess) = prev else {
        return;
    };
    let elapsed = sess.started_at.elapsed();
    if let Some(sig) = build_session_completed(
        &sess.session_uuid,
        elapsed,
        sess.game_id.as_deref(),
        sess.system_id.as_deref(),
        sess.ra_game_id,
        sess.hardcore,
    ) {
        spawn_signal(app, sig);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::Client;
    use crate::alvanista::store::test_support::{tokens, MemSecrets, SERIAL};
    use crate::alvanista::store::Store;
    use serde_json::json;
    use std::sync::Arc;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const VALID_GAME: &str = "550E8400-E29B-41D4-A716-446655440000";
    const LOWER_GAME: &str = "550e8400-e29b-41d4-a716-446655440000";
    const SESS_UUID: &str = "9b1deb4d-3b7d-4bad-9bdd-2b0d7b3dcb6d";

    #[test]
    fn session_started_matches_pinned_wire_contract() {
        let sig =
            build_session_started(SESS_UUID, Some(VALID_GAME), Some("snes"), 319, true).unwrap();
        let wire = serde_json::to_value(json!({ "signals": [sig] })).unwrap();
        assert_eq!(
            wire,
            json!({
                "signals": [
                    {
                        "idempotency_key": format!("sess-start:{SESS_UUID}"),
                        "signal_type": "emulator.session_started",
                        "value": 1,
                        "game_id": LOWER_GAME,
                        "system_id": "snes",
                        "playtime_min": 0,
                        "ra_game_id": 319,
                        "ra_achievement_id": 0,
                        "hardcore": true
                    }
                ]
            })
        );

        let unmapped =
            build_session_started(SESS_UUID, Some("not-a-uuid"), Some("  "), 0, false).unwrap();
        let unmapped_json = serde_json::to_value(&unmapped).unwrap();
        assert!(unmapped_json.get("game_id").is_none());
        assert!(unmapped_json.get("system_id").is_none());
        assert_eq!(unmapped_json["playtime_min"], 0);
        assert_eq!(unmapped_json["ra_game_id"], 0);
        assert_eq!(unmapped_json["ra_achievement_id"], 0);
        assert_eq!(unmapped_json["hardcore"], false);
    }

    #[test]
    fn achievement_and_heartbeat_and_completed_clamp_and_validate_bounds() {
        // Menos de 1 minuto jogado nao emite session_completed.
        assert!(build_session_completed(
            SESS_UUID,
            Duration::from_secs(59),
            Some(VALID_GAME),
            Some("snes"),
            0,
            false,
        )
        .is_none());

        // Exatamente 1 minuto emite value=1 e playtime_min=1.
        let one_min = build_session_completed(
            SESS_UUID,
            Duration::from_secs(60),
            Some(VALID_GAME),
            Some("snes"),
            42,
            false,
        )
        .unwrap();
        assert_eq!(one_min.idempotency_key, format!("sess-end:{SESS_UUID}"));
        assert_eq!(one_min.signal_type, "emulator.session_completed");
        assert_eq!(one_min.value, 1);
        assert_eq!(one_min.playtime_min, 1);

        // Sessao longa faz clamp em 720 minutos (12 horas), igual ao backend.
        let marathon = build_session_completed(
            SESS_UUID,
            Duration::from_secs(20 * 3600),
            None,
            Some(&"x".repeat(100)),
            0,
            true,
        )
        .unwrap();
        assert_eq!(marathon.value, 720);
        assert_eq!(marathon.playtime_min, 720);
        assert_eq!(marathon.system_id.as_deref().unwrap().len(), 64);

        // Heartbeat faz clamp de value em 1..=60 e playtime_min em 1..=720.
        let hb = build_session_heartbeat(
            SESS_UUID,
            2,
            999,
            5000,
            Some(VALID_GAME),
            Some("gba"),
            10,
            false,
        )
        .unwrap();
        assert_eq!(hb.idempotency_key, format!("sess-hb:{SESS_UUID}:2"));
        assert_eq!(hb.signal_type, "emulator.session_heartbeat");
        assert_eq!(hb.value, 60);
        assert_eq!(hb.playtime_min, 720);

        // Conquista RA gera chave idempotente por sessao + id da conquista.
        assert!(build_achievement_unlocked(SESS_UUID, 0, 5, None, None, 0, false).is_none());
        let ach = build_achievement_unlocked(
            SESS_UUID,
            7788,
            -5,
            Some(VALID_GAME),
            Some("snes\ncontrol"),
            319,
            true,
        )
        .unwrap();
        assert_eq!(ach.idempotency_key, format!("ra-unlock:{SESS_UUID}:7788"));
        assert_eq!(ach.signal_type, "emulator.achievement_unlocked");
        assert_eq!(ach.value, 1);
        assert_eq!(ach.playtime_min, 0);
        assert_eq!(ach.ra_game_id, 319);
        assert_eq!(ach.ra_achievement_id, 7788);
        assert!(ach.hardcore);
        assert!(ach.system_id.is_none());

        // Chave grande demais (> 128 bytes) ou com controle e descartada.
        assert!(build_session_started(&"a".repeat(130), None, None, 0, false).is_none());
        assert!(build_session_started("bad\nkey", None, None, 0, false).is_none());
    }

    #[tokio::test]
    async fn posts_signals_with_profile_scope_only_and_fails_silently_on_errors() {
        let _serial = SERIAL.lock().await;
        let server = MockServer::start().await;
        let secrets = Arc::new(MemSecrets::default());
        let db = db::connect_in_memory().await.unwrap();
        let store = Store::new(db.clone(), secrets.clone());
        // Apenas profile:read, sem gamelog nem saves.
        store
            .start_session(&tokens("gas"), &["profile:read"])
            .await
            .unwrap();
        let alva = Alvanista::new(Client::with_api(server.uri(), store));

        let sig =
            build_session_started(SESS_UUID, Some(VALID_GAME), Some("snes"), 0, false).unwrap();
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/gas/signals"))
            .and(header("authorization", "Bearer alp_gas"))
            .and(body_json(json!({ "signals": [sig.clone()] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "accepted": 1 })))
            .expect(1)
            .mount(&server)
            .await;

        assert!(post_gas_signals(&alva, &[sig.clone()]).await);

        // 404 ou 500 falha silenciosamente sem derrubar a sessao.
        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/gas/signals"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        assert!(!post_gas_signals(&alva, &[sig]).await);
        assert!(alva.has_session().await);
    }
}
