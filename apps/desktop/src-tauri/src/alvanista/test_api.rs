//! Um Alvanista de mentira com a semântica de tokens do servidor, para os
//! testes de conectar e de ligar o sync (`partner/service.go` e
//! `repository/partner_pg.go` do Alvanista):
//!
//! - a concessão é uma por conta e app: cada consentimento soma escopos a ela;
//! - todo token vale com os escopos da concessão na hora do uso (os que o app
//!   pode pedir), não com os da hora em que nasceu;
//! - revogar qualquer token encerra a família: todo token da mesma concessão
//!   e do mesmo `device_id`, inclusive os que nasceram de outro consentimento;
//! - um refresh já gasto, apresentado de novo, também encerra a família.
//!
//! O `code` que o navegador de mentira (`browser`) devolve é a lista de
//! escopos pedidos: a troca soma esses escopos à concessão, como o
//! consentimento faria.

use super::store::Tokens;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

struct Issued {
    access: String,
    refresh: String,
    device_id: String,
    revoked: bool,
    spent: bool,
}

struct State {
    /// Os escopos da concessão (uma só: uma conta, um app).
    grant: Vec<String>,
    /// O que o app pode pedir (`allowed_scopes`).
    allowed: Vec<String>,
    issued: Vec<Issued>,
    next: u32,
    save_sync: bool,
    /// A troca informa `scope` (o servidor de hoje informa).
    report_scope: bool,
    token_delay: Option<Duration>,
    minted: Vec<String>,
    me_calls: usize,
}

impl State {
    fn effective(&self) -> Vec<String> {
        self.grant
            .iter()
            .filter(|s| self.allowed.contains(s))
            .cloned()
            .collect()
    }

    fn live_access(&self, access: &str) -> bool {
        self.issued.iter().any(|t| t.access == access && !t.revoked)
    }

    fn issue(&mut self, device_id: &str) -> serde_json::Value {
        self.next += 1;
        let n = self.next;
        let (access, refresh) = (format!("alp_f{n}"), format!("alr_f{n}"));
        self.issued.push(Issued {
            access: access.clone(),
            refresh: refresh.clone(),
            device_id: device_id.to_owned(),
            revoked: false,
            spent: false,
        });
        let mut body = serde_json::json!({
            "access_token": access, "token_type": "Bearer",
            "refresh_token": refresh, "expires_in": 3600,
        });
        if self.report_scope {
            body["scope"] = self.effective().join(" ").into();
        }
        body
    }

    /// Revoga a família de `token` (de acesso ou de renovação).
    fn revoke_family(&mut self, token: &str) {
        let Some(device) = self
            .issued
            .iter()
            .find(|t| t.access == token || t.refresh == token)
            .map(|t| t.device_id.clone())
        else {
            return;
        };
        for t in self.issued.iter_mut().filter(|t| t.device_id == device) {
            t.revoked = true;
        }
    }
}

/// O servidor de mentira e o que ele viu. `events` recebe `token` (troca do
/// code), `refresh`, `revoke:<token>` e `mint`, na ordem; o chaveiro de teste
/// pode anotar `save` no mesmo registro.
pub struct FamilyApi {
    pub server: MockServer,
    state: Arc<Mutex<State>>,
    pub events: Arc<Mutex<Vec<String>>>,
}

impl FamilyApi {
    /// Um app que pode pedir `profile:read` e `saves`, numa conta com o sync
    /// no plano.
    pub async fn start(events: Arc<Mutex<Vec<String>>>) -> Self {
        let state = Arc::new(Mutex::new(State {
            grant: Vec::new(),
            allowed: vec!["profile:read".into(), "saves".into()],
            issued: Vec::new(),
            next: 0,
            save_sync: true,
            report_scope: true,
            token_delay: None,
            minted: Vec::new(),
            me_calls: 0,
        }));
        let server = MockServer::start().await;
        Mock::given(any())
            .respond_with(Responder {
                state: state.clone(),
                events: events.clone(),
            })
            .mount(&server)
            .await;
        Self {
            server,
            state,
            events,
        }
    }

    pub fn uri(&self) -> String {
        self.server.uri()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }

    /// Um par que já existe (a sessão guardada), com os escopos que a
    /// concessão tinha quando ele nasceu.
    pub fn seed(&self, tokens: &Tokens, device_id: &str, scopes: &[&str]) {
        let mut st = self.state();
        for s in scopes {
            if !st.grant.iter().any(|g| g == s) {
                st.grant.push((*s).to_owned());
            }
        }
        st.issued.push(Issued {
            access: tokens.access_token.clone(),
            refresh: tokens.refresh_token.clone(),
            device_id: device_id.to_owned(),
            revoked: false,
            spent: false,
        });
    }

    /// O que o app pode pedir; o que sobrar da concessão não vale.
    pub fn allow(&self, scopes: &[&str]) {
        self.state().allowed = scopes.iter().map(|s| (*s).to_owned()).collect();
    }

    pub fn set_save_sync(&self, on: bool) {
        self.state().save_sync = on;
    }

    pub fn report_scope(&self, on: bool) {
        self.state().report_scope = on;
    }

    /// A troca do code demora isto para responder.
    pub fn delay_exchange(&self, by: Duration) {
        self.state().token_delay = Some(by);
    }

    /// O token (de acesso ou de renovação) ainda vale no servidor?
    pub fn is_live(&self, token: &str) -> bool {
        self.state()
            .issued
            .iter()
            .any(|t| (t.access == token || t.refresh == token) && !t.revoked)
    }

    pub fn minted(&self) -> usize {
        self.state().minted.len()
    }

    pub fn me_calls(&self) -> usize {
        self.state().me_calls
    }

    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}

struct Responder {
    state: Arc<Mutex<State>>,
    events: Arc<Mutex<Vec<String>>>,
}

fn json(status: u16, body: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(body)
}

fn unauthorized() -> ResponseTemplate {
    json(401, serde_json::json!({"error": "invalid_token"}))
}

impl Respond for Responder {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let form: Vec<(String, String)> = url::form_urlencoded::parse(&req.body)
            .into_owned()
            .collect();
        let field = |k: &str| {
            form.iter()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let bearer = req
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or_default()
            .to_owned();
        let mut st = self.state.lock().unwrap();
        let log = |e: String| self.events.lock().unwrap().push(e);
        match (req.method.as_str(), req.url.path()) {
            ("POST", "/api/partner/oauth/token") if field("grant_type") == "authorization_code" => {
                log("token".into());
                for s in field("code").split_whitespace() {
                    if !st.grant.iter().any(|g| g == s) {
                        st.grant.push(s.to_owned());
                    }
                }
                let body = st.issue(&field("device_id"));
                let template = json(200, body);
                match st.token_delay {
                    Some(by) => template.set_delay(by),
                    None => template,
                }
            }
            ("POST", "/api/partner/oauth/token") => {
                log("refresh".into());
                let presented = field("refresh_token");
                let Some(i) = st.issued.iter().position(|t| t.refresh == presented) else {
                    return json(400, serde_json::json!({"error": "invalid_grant"}));
                };
                if st.issued[i].revoked {
                    return json(400, serde_json::json!({"error": "invalid_grant"}));
                }
                if st.issued[i].spent {
                    st.revoke_family(&presented);
                    return json(400, serde_json::json!({"error": "invalid_grant"}));
                }
                st.issued[i].spent = true;
                let device = st.issued[i].device_id.clone();
                json(200, st.issue(&device))
            }
            ("POST", "/api/partner/oauth/revoke") => {
                let token = field("token");
                log(format!("revoke:{token}"));
                st.revoke_family(&token);
                ResponseTemplate::new(200)
            }
            ("GET", "/api/partner/v1/me") => {
                st.me_calls += 1;
                if !st.live_access(&bearer) {
                    return unauthorized();
                }
                json(
                    200,
                    serde_json::json!({
                        "id": "alva_1", "username": "toy",
                        "profile_url": "https://www.alvanista.com/users/toy",
                        "entitlements": {"save_sync": st.save_sync},
                    }),
                )
            }
            ("POST", "/api/partner/v1/save-devices") => {
                if !st.live_access(&bearer) {
                    return unauthorized();
                }
                if !st.effective().iter().any(|s| s == "saves") {
                    return json(403, serde_json::json!({"error": "insufficient_scope"}));
                }
                if !st.save_sync {
                    // O corpo da porta do plano (`writeSaveDenial`).
                    return json(
                        402,
                        serde_json::json!({"error": {"code": "tier_insufficient",
                            "current_tier": "gamer", "required_quota": "save_sync_enabled",
                            "required_value": true, "upgrade_to": "collector"}}),
                    );
                }
                log("mint".into());
                let label = req
                    .body_json::<serde_json::Value>()
                    .ok()
                    .and_then(|b| b["label"].as_str().map(str::to_owned))
                    .unwrap_or_default();
                st.minted.push(label.clone());
                let id = format!("dev{}", st.minted.len());
                json(
                    201,
                    serde_json::json!({
                        "id": id, "label": label, "client_kind": "reemu",
                        "token": format!("asv_{id}"), "token_prefix": "asv_de",
                        "created_at": "2026-11-13T10:00:00Z",
                    }),
                )
            }
            ("GET", "/api/partner/v1/save-devices") => {
                if !st.live_access(&bearer) {
                    return unauthorized();
                }
                let devices: Vec<serde_json::Value> = st
                    .minted
                    .iter()
                    .enumerate()
                    .map(|(i, label)| {
                        serde_json::json!({
                            "id": format!("dev{}", i + 1), "label": label,
                            "client_kind": "reemu", "created_at": "2026-11-13T10:00:00Z",
                        })
                    })
                    .collect();
                json(200, serde_json::json!({ "devices": devices }))
            }
            ("GET", "/api/v1/saves/quota") if bearer.starts_with("asv_") => json(
                200,
                serde_json::json!({"used_bytes": 1, "limit_bytes": 100}),
            ),
            _ => ResponseTemplate::new(404),
        }
    }
}

pub fn query(url: &str, key: &str) -> String {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
        .unwrap()
}

/// Um GET cru no callback local.
pub fn get(port: u16, target: &str) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
}

/// Um navegador de mentira: aprova (o `code` é a lista de escopos da URL de
/// consentimento) ou nega.
pub fn browser(approve: bool) -> impl FnOnce(&str) -> Result<(), String> {
    move |url: &str| {
        let port: u16 = url::Url::parse(&query(url, "redirect_uri"))
            .unwrap()
            .port()
            .unwrap();
        let state = query(url, "state");
        let code: String =
            url::form_urlencoded::byte_serialize(query(url, "scope").as_bytes()).collect();
        std::thread::spawn(move || {
            let target = if approve {
                format!("/callback?code={code}&state={state}")
            } else {
                format!("/callback?error=access_denied&state={state}")
            };
            get(port, &target);
        });
        Ok(())
    }
}
