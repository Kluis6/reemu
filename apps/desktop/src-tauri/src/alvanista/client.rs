//! Cliente HTTP do provedor OAuth do Alvanista (`/api/partner`).
//!
//! Classes de erro (`ClientError`):
//! - `AuthEnded`: a conexão acabou de vez. Só quando a renovação responde 400
//!   `invalid_grant`, ou quando o `/me` ainda dá 401 logo depois de uma
//!   renovação que deu certo. Os tokens guardados são apagados.
//! - `Transient`: 429, 5xx, timeout, rede fora do ar, `/me` 404. Os tokens
//!   ficam como estão; a tela mostra "sem conexão" e tenta depois.
//! - `Rejected`: o servidor recusou o pedido (400 `invalid_request`, 403
//!   `insufficient_scope`...). Não adianta repetir igual.
//!
//! O refresh token roda a cada uso; usar um antigo encerra a sessão do
//! aparelho. Por isso a renovação é uma de cada vez e o par novo é gravado
//! antes de ser usado.

use super::config::{base_urls, CLIENT_ID};
use super::store::{
    is_latest_rotation, remember_rotation, rotated_from, Store, Tokens, REFRESH_LOCK,
};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClientError {
    #[error("a conexão com o Alvanista acabou")]
    AuthEnded,
    #[error("Alvanista indisponível: {0}")]
    Transient(String),
    #[error("o Alvanista recusou o pedido: {0}")]
    Rejected(String),
}

type Result<T> = std::result::Result<T, ClientError>;

/// Perfil público devolvido por `GET /api/partner/v1/me`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Me {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub hero_url: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    pub profile_url: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: String,
}

pub struct Client {
    http: reqwest::Client,
    api: String,
    store: Store,
}

impl Client {
    /// Cliente das URLs de `config::base_urls()`.
    pub fn new(store: Store) -> Self {
        Self::with_api(base_urls().1, store)
    }

    pub fn with_api(api: String, store: Store) -> Self {
        let builder = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent(concat!("ReEmu/", env!("CARGO_PKG_VERSION")));
        // Os testes falam com um servidor local: sem proxy do ambiente.
        #[cfg(test)]
        let builder = builder.no_proxy();
        Self {
            // Sem TLS/backend o cliente não sobe; melhor falhar alto do que
            // cair num cliente sem timeout nem User-Agent.
            http: builder
                .build()
                .expect("cliente HTTP do Alvanista (reqwest) não pôde ser criado"),
            api: api.trim_end_matches('/').to_owned(),
            store,
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Troca o `code` do callback pelos tokens. Não grava: quem chama grava
    /// com `store().save_tokens` quando for usar.
    pub async fn exchange(
        &self,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
        device_id: &str,
    ) -> Result<Tokens> {
        self.token_request(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
            ("device_id", device_id),
        ])
        .await
    }

    /// Renova o par e grava o novo antes de devolvê-lo. Uma renovação por vez
    /// no processo (`REFRESH_LOCK`). Se `current` já foi trocado (por outra
    /// chamada deste processo, ou o par guardado já é outro), devolve o par
    /// mais novo em vez de gastar o refresh token de novo.
    pub async fn refresh(&self, current: &Tokens) -> Result<Tokens> {
        let _one_at_a_time = REFRESH_LOCK.lock().await;
        if let Some((done, received_at)) = rotated_from(&current.refresh_token) {
            match self.store.load().await {
                Ok(Some(stored)) if stored.tokens == done => {}
                // Gravado depois de o par da memória chegar: outra instância
                // do ReEmu renovou (ou conectou) por cima. O guardado ganha, e
                // o da memória não volta pro disco (o refresh dele pode já
                // ter sido gasto lá).
                Ok(Some(stored)) if stored.saved_at > received_at => return Ok(stored.tokens),
                // A troca aconteceu aqui e a gravação falhou: tenta de novo.
                _ => self.save_with_retry(&done).await,
            }
            return Ok(done);
        }
        if !is_latest_rotation(&current.refresh_token) {
            match self.store.load_tokens().await {
                Ok(Some(stored)) if stored.refresh_token != current.refresh_token => {
                    return Ok(stored);
                }
                Ok(_) => {}
                Err(e) => log::warn!("alvanista: não deu pra ler a sessão guardada: {e}"),
            }
        }
        match self
            .token_request(&[
                ("grant_type", "refresh_token"),
                ("client_id", CLIENT_ID),
                ("refresh_token", &current.refresh_token),
            ])
            .await
        {
            Ok(new) => {
                // Primeiro na memória: o refresh token antigo já foi gasto no
                // servidor, e o par novo não pode se perder se a gravação falhar.
                remember_rotation(&current.refresh_token, &new);
                self.save_with_retry(&new).await;
                Ok(new)
            }
            Err(ClientError::Rejected(code)) if code == "invalid_grant" => {
                if let Err(e) = self.store.clear_tokens_unlocked().await {
                    log::warn!("alvanista: não deu pra apagar a sessão: {e}");
                }
                Err(ClientError::AuthEnded)
            }
            Err(e) => Err(e),
        }
    }

    /// Grava o par, com uma segunda tentativa. Se as duas falham o par segue
    /// valendo na memória do processo (`refresh` o devolve), e o erro vai
    /// para o log.
    async fn save_with_retry(&self, tokens: &Tokens) {
        if self.store.save_tokens(tokens).await.is_ok() {
            return;
        }
        if let Err(e) = self.store.save_tokens(tokens).await {
            log::warn!("alvanista: não deu pra guardar a sessão renovada: {e}");
        }
    }

    /// Avisa o Alvanista que a sessão deste aparelho acabou (revoga o refresh
    /// token, e com ele a família). O Alvanista sempre responde 200; falha de
    /// rede aqui não impede o usuário de desconectar localmente. Devolve se o
    /// servidor confirmou (resposta 2xx).
    pub async fn revoke(&self, tokens: &Tokens) -> bool {
        let sent = self
            .http
            .post(format!("{}/api/partner/oauth/revoke", self.api))
            .form(&[
                ("token", tokens.refresh_token.as_str()),
                ("client_id", CLIENT_ID),
            ])
            .send()
            .await;
        match sent {
            Ok(r) => {
                log::info!("alvanista: revogação respondeu {}", r.status().as_u16());
                r.status().is_success()
            }
            Err(e) => {
                log::warn!("alvanista: revogação não chegou: {}", e.without_url());
                false
            }
        }
    }

    /// Perfil da conta conectada. Num 401 renova uma vez (gravando o par
    /// novo), tenta de novo e, se ainda for 401, a conexão acabou. O
    /// `tokens` do chamador fica com o par atual.
    pub async fn me(&self, tokens: &mut Tokens) -> Result<Me> {
        let mut result = self.get_me(&tokens.access_token).await;
        if matches!(result, Err(MeError::Unauthorized)) {
            *tokens = self.refresh(tokens).await?;
            result = self.get_me(&tokens.access_token).await;
        }
        match result {
            Ok(me) => {
                if let Err(e) = self.store.mirror_put(&me).await {
                    log::warn!("alvanista: não deu pra guardar o espelho do perfil: {e}");
                }
                Ok(me)
            }
            // Só chega aqui um 401 logo depois de uma renovação que deu certo.
            Err(MeError::Unauthorized) => {
                if let Err(e) = self.store.clear_tokens().await {
                    log::warn!("alvanista: não deu pra apagar a sessão: {e}");
                }
                Err(ClientError::AuthEnded)
            }
            Err(MeError::Failed(e)) => Err(e),
        }
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<Tokens> {
        let resp = self
            .http
            .post(format!("{}/api/partner/oauth/token", self.api))
            .form(form)
            .send()
            .await
            .map_err(transport)?;
        let status = resp.status();
        log::info!("alvanista: /token respondeu {}", status.as_u16());
        if status.is_success() {
            let body: TokenResponse = resp
                .json()
                .await
                .map_err(|_| ClientError::Transient("resposta do /token ilegível".into()))?;
            return Ok(Tokens {
                access_token: body.access_token,
                refresh_token: body.refresh_token,
            });
        }
        Err(classify(status, error_code(resp).await))
    }

    async fn get_me(&self, access_token: &str) -> std::result::Result<Me, MeError> {
        let resp = self
            .http
            .get(format!("{}/api/partner/v1/me", self.api))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| MeError::Failed(transport(e)))?;
        let status = resp.status();
        log::info!("alvanista: /me respondeu {}", status.as_u16());
        if status == StatusCode::UNAUTHORIZED {
            return Err(MeError::Unauthorized);
        }
        if status.is_success() {
            return resp.json().await.map_err(|_| {
                MeError::Failed(ClientError::Transient("resposta do /me ilegível".into()))
            });
        }
        // Um 404 no /me não é "a conta acabou": pode ser deploy no meio.
        let err = if status == StatusCode::NOT_FOUND {
            ClientError::Transient("not_found".into())
        } else {
            classify(status, error_code(resp).await)
        };
        Err(MeError::Failed(err))
    }
}

/// 401 é o único status que o `/me` trata à parte.
enum MeError {
    Unauthorized,
    Failed(ClientError),
}

fn transport(e: reqwest::Error) -> ClientError {
    ClientError::Transient(e.without_url().to_string())
}

async fn error_code(resp: reqwest::Response) -> String {
    resp.json::<ErrorBody>()
        .await
        .map(|b| b.error)
        .unwrap_or_default()
}

fn classify(status: StatusCode, code: String) -> ClientError {
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        return ClientError::Transient(format!("{}", status.as_u16()));
    }
    ClientError::Rejected(if code.is_empty() {
        status.as_u16().to_string()
    } else {
        code
    })
}

#[cfg(test)]
mod tests {
    use super::super::store::test_support::*;
    use super::*;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug)]
    struct Req {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: String,
    }

    impl Req {
        fn form(&self, key: &str) -> Option<String> {
            url::form_urlencoded::parse(self.body.as_bytes())
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
        }
        fn bearer(&self) -> String {
            self.headers
                .get("authorization")
                .and_then(|h| h.strip_prefix("Bearer "))
                .unwrap_or_default()
                .to_owned()
        }
    }

    /// Alvanista de mentira em `std::net::TcpListener`: uma conexão por vez,
    /// `Connection: close`. Cada requisição vai para `log` (que o chaveiro de
    /// teste também usa, pra provar a ordem).
    struct Fake {
        base: String,
        log: Arc<Mutex<Vec<String>>>,
        reqs: Arc<Mutex<Vec<Req>>>,
        stop: Arc<AtomicBool>,
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
        }
    }

    fn start_fake(
        log: Arc<Mutex<Vec<String>>>,
        handler: impl Fn(&Req) -> (u16, String) + Send + 'static,
    ) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let reqs = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (log2, reqs2, stop2) = (log.clone(), reqs.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                let Ok((mut s, _)) = listener.accept() else {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                };
                s.set_nonblocking(false).unwrap();
                let req = read_req(&mut s);
                let tag = if req.path.contains("token") {
                    format!("POST token {}", req.form("grant_type").unwrap_or_default())
                } else if req.path.ends_with("/me") {
                    format!("GET me {}", req.bearer())
                } else {
                    format!("{} {}", req.method, req.path)
                };
                log2.lock().unwrap().push(tag);
                reqs2.lock().unwrap().push(req.clone());
                let (status, body) = handler(&req);
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Fake {
            base,
            log,
            reqs,
            stop,
        }
    }

    fn read_req(s: &mut std::net::TcpStream) -> Req {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 2048];
        let (head_end, content_len) = loop {
            let n = s.read(&mut chunk).unwrap();
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                let len = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                break (pos + 4, len);
            }
            assert!(n > 0, "conexão fechada antes do cabeçalho");
        };
        while buf.len() < head_end + content_len {
            let n = s.read(&mut chunk).unwrap();
            buf.extend_from_slice(&chunk[..n]);
        }
        let text = String::from_utf8_lossy(&buf[..head_end]).into_owned();
        let mut lines = text.lines();
        let mut first = lines.next().unwrap().split_whitespace();
        let (method, path) = (
            first.next().unwrap().to_owned(),
            first.next().unwrap().to_owned(),
        );
        let headers = lines
            .filter_map(|l| l.split_once(':'))
            .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_owned()))
            .collect();
        Req {
            method,
            path,
            headers,
            body: String::from_utf8_lossy(&buf[head_end..head_end + content_len]).into_owned(),
        }
    }

    /// Nomes novos por teste: a memória da última rotação é do processo, e
    /// os testes rodam juntos.
    fn names() -> (String, String) {
        use std::sync::atomic::AtomicUsize;
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        (format!("old{n}"), format!("new{n}"))
    }

    const ME_JSON: &str = r#"{"id":"alva_1","username":"toy","avatar_url":null,"hero_url":null,"bio":null,"profile_url":"https://www.alvanista.com/users/toy"}"#;

    fn pair_json(n: &str) -> String {
        format!(
            r#"{{"access_token":"alp_{n}","token_type":"Bearer","expires_in":3600,"refresh_token":"alr_{n}","scope":"profile:read"}}"#
        )
    }

    /// `/me` aceita só o access `alp_new`; o refresh `alr_old` troca por
    /// `new` uma vez (a segunda vez é reuso: `invalid_grant`).
    fn rotating_server(log: Arc<Mutex<Vec<String>>>, old: &str, new: &str) -> Fake {
        let used = Mutex::new(false);
        let (old_refresh, new_access, new) =
            (format!("alr_{old}"), format!("alp_{new}"), new.to_owned());
        start_fake(log, move |req| {
            if req.path.contains("token") {
                let mut used = used.lock().unwrap();
                if req.form("refresh_token").as_deref() == Some(old_refresh.as_str()) && !*used {
                    *used = true;
                    (200, pair_json(&new))
                } else {
                    (400, r#"{"error":"invalid_grant"}"#.into())
                }
            } else if req.bearer() == new_access {
                (200, ME_JSON.into())
            } else {
                (401, r#"{"error":"invalid_token"}"#.into())
            }
        })
    }

    async fn client_for(
        fake: &Fake,
        events: Arc<Mutex<Vec<String>>>,
        seed: Option<&Tokens>,
    ) -> Client {
        let (store, _) = store_with(MemSecrets {
            events: Some(events),
            ..Default::default()
        })
        .await;
        if let Some(t) = seed {
            store.save_tokens(t).await.unwrap();
            // O save semeado não conta na ordem do teste.
            fake.log.lock().unwrap().clear();
        }
        Client::with_api(fake.base.clone(), store)
    }

    #[tokio::test]
    async fn me_refreshes_once_on_401_and_saves_new_pair_before_retry() {
        let _serial = SERIAL.lock().await;
        let (old, new) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = rotating_server(log.clone(), &old, &new);
        let client = client_for(&fake, log.clone(), Some(&tokens(&old))).await;

        let mut t = tokens(&old);
        let me = client.me(&mut t).await.unwrap();

        assert_eq!(me.username, "toy");
        assert_eq!(t, tokens(&new));
        assert_eq!(
            client.store().load_tokens().await.unwrap(),
            Some(tokens(&new))
        );
        // A gravação do par novo vem antes do /me com ele.
        assert_eq!(
            *log.lock().unwrap(),
            [
                format!("GET me alp_{old}"),
                "POST token refresh_token".to_owned(),
                "save".to_owned(),
                format!("GET me alp_{new}")
            ]
        );
        // O espelho foi guardado.
        assert_eq!(
            client.store().mirror_get().await.unwrap().unwrap().username,
            "toy"
        );
    }

    #[tokio::test]
    async fn me_second_401_ends_connection() {
        let _serial = SERIAL.lock().await;
        let (old, new) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let new2 = new.clone();
        let fake = start_fake(log.clone(), move |req| {
            if req.path.contains("token") {
                (200, pair_json(&new2))
            } else {
                (401, r#"{"error":"invalid_token"}"#.into())
            }
        });
        let client = client_for(&fake, log.clone(), Some(&tokens(&old))).await;

        let mut t = tokens(&old);
        assert_eq!(client.me(&mut t).await, Err(ClientError::AuthEnded));
        // Renovou uma vez só, e a sessão guardada foi apagada.
        let refreshes = log
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.starts_with("POST token"))
            .count();
        assert_eq!(refreshes, 1);
        assert_eq!(client.store().load_tokens().await.unwrap(), None);
    }

    #[tokio::test]
    async fn refresh_invalid_grant_ends_connection() {
        let _serial = SERIAL.lock().await;
        let (old, _new) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = start_fake(log.clone(), |req| {
            if req.path.contains("token") {
                (
                    400,
                    r#"{"error":"invalid_grant","error_description":"x"}"#.into(),
                )
            } else {
                (401, r#"{"error":"invalid_token"}"#.into())
            }
        });
        let client = client_for(&fake, log, Some(&tokens(&old))).await;
        let mut t = tokens(&old);
        assert_eq!(client.me(&mut t).await, Err(ClientError::AuthEnded));
        assert_eq!(client.store().load_tokens().await.unwrap(), None);
    }

    #[tokio::test]
    async fn transient_failures_keep_the_tokens() {
        let (old, _new) = names();
        for (status, on_refresh) in [
            (503, false),
            (429, false),
            (404, false),
            (503, true),
            (429, true),
        ] {
            let log = Arc::new(Mutex::new(Vec::new()));
            let fake = start_fake(log.clone(), move |req| {
                if req.path.contains("token") {
                    (status, r#"{"error":"temporarily_unavailable"}"#.into())
                } else if on_refresh {
                    (401, r#"{"error":"invalid_token"}"#.into())
                } else {
                    (status, r#"{"error":"x"}"#.into())
                }
            });
            let client = client_for(&fake, log, Some(&tokens(&old))).await;
            let mut t = tokens(&old);
            let err = client.me(&mut t).await.unwrap_err();
            assert!(
                matches!(err, ClientError::Transient(_)),
                "{status}/{on_refresh}: {err:?}"
            );
            assert_eq!(
                client.store().load_tokens().await.unwrap(),
                Some(tokens(&old)),
                "{status}/{on_refresh}"
            );
        }
    }

    #[tokio::test]
    async fn unreachable_server_is_transient() {
        let (old, _new) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = start_fake(log.clone(), |_| (200, "{}".into()));
        let base = fake.base.clone();
        drop(fake); // porta fechada
        std::thread::sleep(std::time::Duration::from_millis(50));
        let (store, _) = store_with(MemSecrets::default()).await;
        store.save_tokens(&tokens(&old)).await.unwrap();
        let client = Client::with_api(base, store);
        let mut t = tokens(&old);
        assert!(matches!(
            client.me(&mut t).await,
            Err(ClientError::Transient(_))
        ));
        assert_eq!(
            client.store().load_tokens().await.unwrap(),
            Some(tokens(&old))
        );
    }

    #[tokio::test]
    async fn concurrent_refreshes_spend_the_refresh_token_once() {
        let _serial = SERIAL.lock().await;
        let (old, new) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = rotating_server(log.clone(), &old, &new);
        let client = client_for(&fake, log.clone(), Some(&tokens(&old))).await;

        let (mut a, mut b) = (tokens(&old), tokens(&old));
        let (ra, rb) = tokio::join!(client.me(&mut a), client.me(&mut b));
        assert!(ra.is_ok() && rb.is_ok(), "{ra:?} {rb:?}");
        let refreshes = log
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.starts_with("POST token"))
            .count();
        assert_eq!(refreshes, 1);
        assert_eq!(a, tokens(&new));
        assert_eq!(b, tokens(&new));
    }

    #[tokio::test]
    async fn exchange_posts_the_authorization_code_form() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = start_fake(log.clone(), |_| (200, pair_json("x")));
        let client = client_for(&fake, log, None).await;
        let got = client
            .exchange(
                "the-code",
                "the-verifier",
                "http://127.0.0.1:99/callback",
                "dev_1",
            )
            .await
            .unwrap();
        assert_eq!(got, tokens("x"));

        let req = fake.reqs.lock().unwrap()[0].clone();
        assert_eq!(
            (req.method.as_str(), req.path.as_str()),
            ("POST", "/api/partner/oauth/token")
        );
        assert_eq!(
            req.headers.get("content-type").map(String::as_str),
            Some("application/x-www-form-urlencoded")
        );
        assert!(req.headers["user-agent"].starts_with("ReEmu/"));
        for (k, v) in [
            ("grant_type", "authorization_code"),
            ("client_id", "reemu"),
            ("code", "the-code"),
            ("code_verifier", "the-verifier"),
            ("redirect_uri", "http://127.0.0.1:99/callback"),
            ("device_id", "dev_1"),
        ] {
            assert_eq!(req.form(k).as_deref(), Some(v), "{k}");
        }
    }

    #[tokio::test]
    async fn exchange_rejection_is_not_an_ended_connection() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = start_fake(log.clone(), |_| {
            (400, r#"{"error":"invalid_grant"}"#.into())
        });
        let client = client_for(&fake, log, None).await;
        assert_eq!(
            client
                .exchange("c", "v", "http://127.0.0.1:1/callback", "d")
                .await,
            Err(ClientError::Rejected("invalid_grant".into()))
        );
    }

    #[tokio::test]
    async fn revoke_sends_the_refresh_token() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = start_fake(log.clone(), |_| (200, "{}".into()));
        let client = client_for(&fake, log, None).await;
        client.revoke(&tokens("a")).await;
        let req = fake.reqs.lock().unwrap()[0].clone();
        assert_eq!(req.path, "/api/partner/oauth/revoke");
        assert_eq!(req.form("token").as_deref(), Some("alr_a"));
        assert_eq!(req.form("client_id").as_deref(), Some("reemu"));
    }

    #[tokio::test]
    async fn refresh_survives_a_keyring_that_cannot_be_written() {
        let _serial = SERIAL.lock().await;
        let (old, new) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = rotating_server(log.clone(), &old, &new);
        let (store, secrets) = store_with(MemSecrets::default()).await;
        store.save_tokens(&tokens(&old)).await.unwrap();
        // O chaveiro continua com o par velho, e nada mais grava nele.
        secrets.readonly.store(true, Ordering::SeqCst);
        let client = Client::with_api(fake.base.clone(), store);

        let mut t = tokens(&old);
        client.me(&mut t).await.unwrap();
        assert_eq!(t, tokens(&new));
        assert_eq!(
            client.store().load_tokens().await.unwrap(),
            Some(tokens(&new))
        );
        assert!(client.store().using_fallback().await);

        // Quem ainda segura o par velho recebe o novo, nunca o velho.
        assert_eq!(client.refresh(&tokens(&old)).await, Ok(tokens(&new)));
        let spent = log
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.starts_with("POST token"))
            .count();
        assert_eq!(spent, 1);
    }

    #[tokio::test]
    async fn rotated_pair_survives_failed_saves_in_memory() {
        let _serial = SERIAL.lock().await;
        let (old, new) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = rotating_server(log.clone(), &old, &new);
        // Chaveiro sem gravar e banco fechado: nenhuma das duas
        // cópias aceita gravação.
        let db = db::connect_in_memory().await.unwrap();
        db.close().await;
        let secrets = Arc::new(MemSecrets::default());
        let store = Store::new(db, secrets.clone());
        let client = Client::with_api(fake.base.clone(), store);
        secrets.readonly.store(true, Ordering::SeqCst);

        // O par velho nunca foi gravado aqui; o chamador o traz na mão.
        let mut t = tokens(&old);
        client.me(&mut t).await.unwrap();
        assert_eq!(t, tokens(&new));
        // Nada foi gravado, mas a memória do processo entrega o par novo.
        assert_eq!(client.refresh(&tokens(&old)).await, Ok(tokens(&new)));
    }

    #[tokio::test]
    async fn refresh_never_rewrites_an_older_in_memory_pair_over_a_newer_stored_one() {
        let _serial = SERIAL.lock().await;
        let (old, new) = names();
        let (other, _) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = start_fake(log.clone(), |_| {
            (400, r#"{"error":"invalid_grant"}"#.into())
        });
        let (store, _) = store_with(MemSecrets::default()).await;
        // Este processo trocou old -> new e gravou.
        super::super::store::remember_rotation(&format!("alr_{old}"), &tokens(&new));
        store.save_tokens(&tokens(&new)).await.unwrap();
        // Outra instância do ReEmu renovou new -> other e gravou depois.
        store.save_tokens(&tokens(&other)).await.unwrap();
        let client = Client::with_api(fake.base.clone(), store);

        // Quem ainda segura old recebe o par guardado, que é mais novo, e o
        // new (refresh já gasto pela outra instância) não volta pro disco.
        assert_eq!(client.refresh(&tokens(&old)).await, Ok(tokens(&other)));
        assert_eq!(
            client.store().load_tokens().await.unwrap(),
            Some(tokens(&other))
        );
        assert!(log.lock().unwrap().is_empty(), "{:?}", log.lock().unwrap());
    }

    #[tokio::test]
    async fn refresh_rewrites_the_in_memory_pair_over_an_older_stored_one() {
        let _serial = SERIAL.lock().await;
        let (old, new) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = start_fake(log.clone(), |_| {
            (400, r#"{"error":"invalid_grant"}"#.into())
        });
        let (store, _) = store_with(MemSecrets::default()).await;
        // O par velho está guardado; a troca old -> new veio depois e a
        // gravação dela falhou.
        store.save_tokens(&tokens(&old)).await.unwrap();
        super::super::store::remember_rotation(&format!("alr_{old}"), &tokens(&new));
        let client = Client::with_api(fake.base.clone(), store);

        assert_eq!(client.refresh(&tokens(&old)).await, Ok(tokens(&new)));
        assert_eq!(
            client.store().load_tokens().await.unwrap(),
            Some(tokens(&new))
        );
        assert!(log.lock().unwrap().is_empty(), "{:?}", log.lock().unwrap());
    }

    #[tokio::test]
    async fn disconnect_forgets_every_rotation_and_never_rewrites_a_dead_pair() {
        let _serial = SERIAL.lock().await;
        let (old0, p1) = names();
        let (_, p2) = names();
        let log = Arc::new(Mutex::new(Vec::new()));
        let fake = start_fake(log.clone(), |_| {
            (400, r#"{"error":"invalid_grant"}"#.into())
        });
        let (store, _) = store_with(MemSecrets::default()).await;
        // Cadeia old0 -> P1 -> P2; P2 é o par guardado.
        super::super::store::remember_rotation(&format!("alr_{old0}"), &tokens(&p1));
        super::super::store::remember_rotation(&format!("alr_{p1}"), &tokens(&p2));
        store.save_tokens(&tokens(&p2)).await.unwrap();
        let client = Client::with_api(fake.base.clone(), store);
        assert_eq!(client.refresh(&tokens(&old0)).await, Ok(tokens(&p2)));

        client.store().clear_tokens().await.unwrap();
        // Uma conta nova entra.
        let (fresh, _) = names();
        client.store().start_session(&tokens(&fresh)).await.unwrap();

        // Quem ainda segura old0 não recebe P1 nem P2, e nada é regravado.
        let got = client.refresh(&tokens(&old0)).await;
        assert!(!matches!(got, Ok(ref t) if *t == tokens(&p1) || *t == tokens(&p2)));
        assert_eq!(
            client.store().load_tokens().await.unwrap(),
            Some(tokens(&fresh))
        );
    }

    #[tokio::test]
    async fn start_session_forgets_rotations_of_the_previous_session() {
        let _serial = SERIAL.lock().await;
        let (old0, p1) = names();
        let (store, _) = store_with(MemSecrets::default()).await;
        super::super::store::remember_rotation(&format!("alr_{old0}"), &tokens(&p1));
        let (fresh, _) = names();
        store.start_session(&tokens(&fresh)).await.unwrap();
        assert_eq!(
            super::super::store::rotated_from(&format!("alr_{old0}")),
            None
        );
        assert_eq!(store.load_tokens().await.unwrap(), Some(tokens(&fresh)));
    }
}
