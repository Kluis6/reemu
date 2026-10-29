//! "Entrar com Alvanista": OAuth 2.0 com PKCE contra o provedor do Alvanista
//! (rede social de jogos), com o navegador do sistema e um callback em
//! `127.0.0.1` (RFC 8252). Só `profile:read`: o ReEmu espelha o perfil.
//!
//! - `config`: URLs do serviço e URL de autorização;
//! - `pkce`: verifier, challenge S256 e state;
//! - `loopback`: o servidor de uma conexão que recebe o `code`;
//! - `client`: troca do code, renovação, revogação e `GET /me`;
//! - `store`: tokens no chaveiro (reserva no SQLite), `device_id` e o espelho.
//!
//! Toda rede passa por aqui (Rust), nunca pelo webview. Os comandos Tauri
//! ficam no fim deste arquivo.

pub mod client;
pub mod config;
pub mod loopback;
pub mod pkce;
pub mod store;

use client::{Client, ClientError};
use loopback::{Lang, LoopbackError};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use store::{ClearError, Mirror, Store};
use tauri::{AppHandle, Manager};

/// Quanto o navegador tem para o usuário aprovar.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// O que a tela de Perfil recebe. Nunca leva token: só o espelho do perfil
/// e os avisos.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AlvanistaStatus {
    pub connected: bool,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    /// Capa do perfil, para o banner do drawer de perfil.
    pub hero_url: Option<String>,
    pub bio: Option<String>,
    pub profile_url: Option<String>,
    /// Mostrando o último perfil guardado porque o Alvanista não respondeu.
    pub offline: bool,
    /// A sessão está no SQLite porque o chaveiro do sistema falhou.
    pub insecure_storage: bool,
    /// A conexão acabou (revogada no site, por exemplo); os tokens já saíram.
    pub ended: bool,
}

impl AlvanistaStatus {
    fn from_mirror(m: Mirror, offline: bool, insecure_storage: bool) -> Self {
        Self {
            connected: true,
            username: Some(m.username),
            avatar_url: m.avatar_url,
            hero_url: m.hero_url,
            bio: m.bio,
            profile_url: Some(m.profile_url),
            offline,
            insecure_storage,
            ended: false,
        }
    }
}

/// O estado do Tauri: um cliente (e um armazenamento) para todos os
/// comandos, mais o contador que decide qual tentativa de conexão ainda
/// vale. Começar ou cancelar uma tentativa sobe o contador; a tentativa
/// antiga vê a mudança e para antes de trocar o code ou de gravar tokens.
pub struct Alvanista {
    client: Client,
    generation: Arc<AtomicU64>,
}

impl Alvanista {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    fn store(&self) -> &Store {
        self.client.store()
    }

    /// Cancela a tentativa em andamento (se houver).
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    fn is_current(&self, mine: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == mine
    }

    /// Conecta: `open` recebe a URL de consentimento e a abre no navegador.
    /// Os erros são códigos curtos: `cancelled`, `denied`, `timeout`,
    /// `offline`, `failed`.
    pub async fn connect<F>(&self, lang: Lang, open: F) -> Result<AlvanistaStatus, String>
    where
        F: FnOnce(&str) -> Result<(), String>,
    {
        // Uma tentativa por vez: esta cancela a anterior.
        let mine = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let pkce = pkce::new();
        let lb = loopback::listen(lang).map_err(|e| {
            log::warn!("alvanista: não abriu o callback local: {e}");
            "failed".to_owned()
        })?;
        let port = lb.port;
        let url = config::authorize_url(&config::base_urls().0, port, &pkce);
        let state = pkce.state.clone();
        let generation = self.generation.clone();
        // A espera já está de pé quando o navegador abre, para o callback
        // não chegar a uma porta sem ninguém atendendo.
        let waiter = tokio::task::spawn_blocking(move || {
            lb.wait_unless(&state, CONNECT_TIMEOUT, || {
                generation.load(Ordering::SeqCst) != mine
            })
        });
        if let Err(e) = open(&url) {
            log::warn!("alvanista: não abriu o navegador: {e}");
            self.cancel_if_current(mine);
            let _ = waiter.await;
            return Err("failed".into());
        }
        let code = match waiter.await {
            Ok(Ok(code)) => code,
            Ok(Err(LoopbackError::Denied)) => return Err("denied".into()),
            Ok(Err(LoopbackError::Timeout)) => return Err("timeout".into()),
            Ok(Err(LoopbackError::Cancelled)) => return Err("cancelled".into()),
            Ok(Err(LoopbackError::Provider(error))) => {
                log::warn!("alvanista: o provedor recusou: {error}");
                return Err("failed".into());
            }
            Ok(Err(LoopbackError::Io(e))) => {
                log::warn!("alvanista: callback local falhou: {e}");
                return Err("failed".into());
            }
            Err(_) => return Err("failed".into()),
        };
        self.complete(mine, &code, &pkce.verifier, &config::redirect_uri(port))
            .await?;
        Ok(self.status().await)
    }

    /// Só a tentativa vigente derruba o contador: uma que já foi trocada
    /// por outra não cancela a nova.
    fn cancel_if_current(&self, mine: u64) {
        let _ =
            self.generation
                .compare_exchange(mine, mine + 1, Ordering::SeqCst, Ordering::SeqCst);
    }

    /// Troca o code e começa a sessão, a menos que a tentativa tenha sido
    /// cancelada: uma cancelada nunca chama `exchange` nem grava tokens,
    /// mesmo que o callback dela tenha chegado.
    async fn complete(
        &self,
        mine: u64,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<(), String> {
        if !self.is_current(mine) {
            return Err("cancelled".into());
        }
        let device_id = self.store().device_id().await.map_err(|e| {
            log::warn!("alvanista: sem device_id: {e}");
            "failed".to_owned()
        })?;
        let tokens = self
            .client
            .exchange(code, verifier, redirect_uri, &device_id)
            .await
            .map_err(|e| match e {
                ClientError::Transient(_) => "offline".to_owned(),
                _ => "failed".to_owned(),
            })?;
        // Cancelou durante a troca: os tokens recém-emitidos não ficam.
        if !self.is_current(mine) {
            self.client.revoke(&tokens).await;
            return Err("cancelled".into());
        }
        self.store().start_session(&tokens).await.map_err(|e| {
            log::warn!("alvanista: não guardou a sessão: {e}");
            "failed".to_owned()
        })
    }

    /// O estado só com o que está no aparelho (sem rede): há sessão? qual o
    /// espelho? Serve para a tela aparecer na hora; `status` confirma depois.
    /// Lê o chaveiro uma vez só.
    pub async fn cached_status(&self) -> AlvanistaStatus {
        let mirror = self.store().mirror_get().await.ok().flatten();
        match self.store().load().await {
            Ok(Some(session)) => match mirror {
                Some(m) => AlvanistaStatus::from_mirror(m, false, session.from_fallback),
                None => AlvanistaStatus {
                    connected: true,
                    insecure_storage: session.from_fallback,
                    ..Default::default()
                },
            },
            Ok(None) => AlvanistaStatus::default(),
            Err(e) => {
                log::warn!("alvanista: não leu a sessão: {e}");
                // Sem leitura não se sabe onde a sessão está: só o espelho.
                match mirror {
                    Some(m) => AlvanistaStatus::from_mirror(m, true, false),
                    None => AlvanistaStatus::default(),
                }
            }
        }
    }

    /// O estado da conexão, renovando o espelho quando há rede. Lê o
    /// chaveiro uma vez (mais uma só quando o par foi renovado, para saber
    /// onde o par novo ficou).
    pub async fn status(&self) -> AlvanistaStatus {
        let mirror = match self.store().mirror_get().await {
            Ok(m) => m,
            Err(e) => {
                log::warn!("alvanista: espelho ilegível: {e}");
                None
            }
        };
        let session = match self.store().load().await {
            Ok(Some(session)) => session,
            Ok(None) => {
                if mirror.is_some() {
                    let _ = self.store().mirror_clear().await;
                }
                return AlvanistaStatus::default();
            }
            Err(e) => {
                // Falha de leitura não é "desconectado": o espelho, se
                // houver, segue na tela como offline.
                log::warn!("alvanista: não leu a sessão: {e}");
                return match mirror {
                    Some(m) => AlvanistaStatus::from_mirror(m, true, false),
                    None => AlvanistaStatus::default(),
                };
            }
        };
        let insecure = session.from_fallback;
        let mut tokens = session.tokens.clone();
        match self.client.me(&mut tokens).await {
            Ok(me) => AlvanistaStatus {
                connected: true,
                username: Some(me.username),
                avatar_url: me.avatar_url,
                hero_url: me.hero_url,
                bio: me.bio,
                profile_url: Some(me.profile_url),
                offline: false,
                insecure_storage: if tokens == session.tokens {
                    insecure
                } else {
                    // Renovou: o par novo pode ter ido para o outro lugar.
                    match self.store().load().await {
                        Ok(Some(now)) => now.from_fallback,
                        _ => insecure,
                    }
                },
                ended: false,
            },
            Err(ClientError::AuthEnded) => {
                let _ = self.store().mirror_clear().await;
                AlvanistaStatus {
                    ended: true,
                    ..Default::default()
                }
            }
            Err(e) => {
                log::info!("alvanista: sem resposta, usando o espelho: {e}");
                match mirror {
                    Some(m) => AlvanistaStatus::from_mirror(m, true, insecure),
                    None => AlvanistaStatus {
                        connected: true,
                        offline: true,
                        insecure_storage: insecure,
                        ..Default::default()
                    },
                }
            }
        }
    }

    /// Desconecta: revoga no servidor (se não der, segue) e apaga a sessão
    /// e o espelho. O perfil local nunca é tocado.
    ///
    /// Se só o chaveiro recusar o apagar (a reserva no SQLite já saiu), vale
    /// como desconectado quando nada vivo pode ter ficado nele:
    /// - o servidor confirmou a revogação;
    /// - o par vivo veio da reserva: a leitura sempre fica com a cópia mais
    ///   nova, então o que houver no chaveiro é mais velho, com o refresh já
    ///   gasto;
    /// - não havia sessão legível (chaveiro inacessível, reserva vazia): só
    ///   sobrou o espelho. É o que tira do "conectado, sem rede" quem ficou
    ///   preso nesse estado.
    ///
    /// Fora disso, o par do chaveiro é o vivo e a revogação não chegou:
    /// desconectar falha, para não deixar tokens vivos sem ninguém saber.
    pub async fn disconnect(&self) -> Result<(), String> {
        self.cancel();
        let (revoked, nothing_alive_in_keyring) = match self.store().load().await {
            Ok(Some(session)) => (
                self.client.revoke(&session.tokens).await,
                session.from_fallback,
            ),
            Ok(None) => (true, true),
            Err(e) => {
                log::warn!("alvanista: não leu a sessão para desconectar: {e}");
                (false, true)
            }
        };
        let done = match self.store().clear_tokens().await {
            Ok(()) => true,
            Err(ClearError::Keyring(e)) if revoked || nothing_alive_in_keyring => {
                log::warn!("alvanista: chaveiro: {e}");
                true
            }
            Err(e) => {
                log::warn!("alvanista: não apagou a sessão: {e}");
                false
            }
        };
        if !done {
            return Err("failed".into());
        }
        if let Err(e) = self.store().mirror_clear().await {
            log::warn!("alvanista: espelho não apagado: {e}");
        }
        Ok(())
    }

    /// URL do perfil no Alvanista, só se for do próprio site.
    pub async fn profile_url(&self) -> Option<String> {
        let url = self.store().mirror_get().await.ok().flatten()?.profile_url;
        is_site_url(&url).then_some(url)
    }
}

/// `true` só para uma página do site do Alvanista (mesmo esquema, host e
/// porta; em release, sempre `https://www.alvanista.com`).
fn is_site_url(url: &str) -> bool {
    let (Ok(parsed), Ok(site)) = (
        url::Url::parse(url),
        url::Url::parse(&config::base_urls().0),
    ) else {
        return false;
    };
    parsed.scheme() == site.scheme()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.host_str() == site.host_str()
        && parsed.port() == site.port()
}

/// Abre `url` no navegador do sistema, e só se for do site do Alvanista: a
/// página de consentimento e o perfil. Nada que venha do webview chega aqui
/// sem passar por essa conferência.
fn open_in_browser(url: &str) -> Result<(), String> {
    if !is_site_url(url) {
        return Err("URL fora do site do Alvanista".into());
    }
    open::that_detached(url).map_err(|e| e.to_string())
}

/// Evento com o `AlvanistaStatus` da renovação feita ao abrir o app.
pub const STATUS_EVENT: &str = "alvanista-status";

/// FR-11: o espelho do perfil se renova quando o app abre. Roda em segundo
/// plano e volta na hora: o `/me` pode levar até ~15 s e a abertura não
/// espera por ele. Se esta chamada e a da tela de Perfil precisarem renovar
/// o par juntas, o `REFRESH_LOCK` põe uma atrás da outra. Sem banco não há
/// estado gerenciado e nada roda.
/// O resultado vai para a interface em `STATUS_EVENT` (a conexão pode ter
/// acabado no site, ou o nome mudado); sem conexão, nem toca na rede.
pub fn refresh_on_start(app: &AppHandle) {
    if app.try_state::<Alvanista>().is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(alva) = app.try_state::<Alvanista>() else {
            return;
        };
        let status = alva.status().await;
        if let Err(e) = tauri::Emitter::emit(&app, STATUS_EVENT, &status) {
            log::warn!("alvanista: não avisou a interface da renovação: {e}");
        }
    });
}

fn state_of(app: &AppHandle) -> Result<tauri::State<'_, Alvanista>, String> {
    app.try_state::<Alvanista>()
        .ok_or_else(|| "failed".to_owned())
}

#[tauri::command]
pub async fn alvanista_connect(
    app: AppHandle,
    lang: Option<String>,
) -> Result<AlvanistaStatus, String> {
    let alva = state_of(&app)?;
    let lang = Lang::from_tag(lang.as_deref().unwrap_or("pt-BR"));
    alva.connect(lang, open_in_browser).await
}

#[tauri::command]
pub async fn alvanista_status(app: AppHandle) -> Result<AlvanistaStatus, String> {
    Ok(state_of(&app)?.status().await)
}

/// Só o que está no aparelho, sem rede: para a tela abrir na hora.
#[tauri::command]
pub async fn alvanista_cached_status(app: AppHandle) -> Result<AlvanistaStatus, String> {
    Ok(state_of(&app)?.cached_status().await)
}

#[tauri::command]
pub async fn alvanista_disconnect(app: AppHandle) -> Result<(), String> {
    state_of(&app)?.disconnect().await
}

#[tauri::command]
pub fn alvanista_cancel(app: AppHandle) -> Result<(), String> {
    state_of(&app)?.cancel();
    Ok(())
}

/// Abre o perfil no navegador do sistema (a URL vem do espelho guardado,
/// nunca da tela).
#[tauri::command]
pub async fn alvanista_open_profile(app: AppHandle) -> Result<(), String> {
    let url = state_of(&app)?
        .profile_url()
        .await
        .ok_or_else(|| "failed".to_owned())?;
    open_in_browser(&url).map_err(|e| {
        log::warn!("alvanista: não abriu o perfil: {e}");
        "failed".to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::store::test_support::*;
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicUsize;

    /// "API" que só conta as conexões que recebe (qualquer uma é um
    /// `exchange` que não devia ter acontecido).
    fn counting_api() -> (String, Arc<AtomicUsize>) {
        counting_api_with(400)
    }

    fn counting_api_with(status: u16) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = hits.clone();
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(20);
            while std::time::Instant::now() < until {
                if let Ok((mut s, _)) = listener.accept() {
                    hits2.fetch_add(1, Ordering::SeqCst);
                    let _ = s.set_nonblocking(false);
                    let mut buf = [0u8; 2048];
                    let _ = s.read(&mut buf);
                    let _ = write!(
                        s,
                        "HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        });
        (base, hits)
    }

    async fn alva_against(api: String) -> Alvanista {
        let (store, _) = store_with(MemSecrets::default()).await;
        Alvanista::new(Client::with_api(api, store))
    }

    fn get(port: u16, target: &str) {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
    }

    fn query(url: &str, key: &str) -> String {
        url::Url::parse(url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
            .unwrap()
    }

    #[tokio::test]
    async fn a_cancelled_generation_never_exchanges() {
        let _serial = SERIAL.lock().await;
        let (api, hits) = counting_api();
        let alva = alva_against(api).await;
        let mine = alva.generation.fetch_add(1, Ordering::SeqCst) + 1;
        alva.cancel();
        let r = alva.complete(mine, "code", "verifier", "http://x").await;
        assert_eq!(r, Err("cancelled".to_owned()));
        assert_eq!(hits.load(Ordering::SeqCst), 0, "exchange was called");
        assert_eq!(alva.store().load_tokens().await.unwrap(), None);
    }

    #[tokio::test]
    async fn cancel_releases_the_wait_and_a_late_callback_does_nothing() {
        let _serial = SERIAL.lock().await;
        let (api, hits) = counting_api();
        let alva = alva_against(api).await;
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let attempt = alva.connect(Lang::PtBr, move |url| {
            tx.send(url.to_owned()).unwrap();
            Ok(())
        });
        let driver = async {
            let url = tokio::task::spawn_blocking(move || rx.recv().unwrap())
                .await
                .unwrap();
            alva.cancel();
            // O callback chega depois de cancelar, com o state certo.
            let port: u16 = url::Url::parse(&query(&url, "redirect_uri"))
                .unwrap()
                .port()
                .unwrap();
            let state = query(&url, "state");
            tokio::time::sleep(Duration::from_millis(100)).await;
            // A porta já foi liberada ou o servidor ignora: nenhum dos dois
            // pode resultar em troca de code.
            let _ = tokio::task::spawn_blocking(move || {
                if let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", port)) {
                    let _ = write!(
                        s,
                        "GET /callback?code=c&state={state} HTTP/1.1\r\nHost: x\r\n\r\n"
                    );
                }
            })
            .await;
        };
        let (result, ()) = tokio::join!(attempt, driver);
        assert_eq!(result, Err("cancelled".to_owned()));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(hits.load(Ordering::SeqCst), 0);
        assert_eq!(alva.store().load_tokens().await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_new_connect_cancels_the_previous_one() {
        let _serial = SERIAL.lock().await;
        let (api, hits) = counting_api();
        let alva = alva_against(api).await;
        let (tx1, rx1) = std::sync::mpsc::channel::<String>();
        let first = alva.connect(Lang::PtBr, move |url| {
            tx1.send(url.to_owned()).unwrap();
            Ok(())
        });
        let second_start = async {
            tokio::task::spawn_blocking(move || rx1.recv().unwrap())
                .await
                .unwrap();
            // Segunda tentativa: cancela a primeira; o navegador "nega".
            alva.connect(Lang::PtBr, |url| {
                let port = url::Url::parse(&query(url, "redirect_uri"))
                    .unwrap()
                    .port()
                    .unwrap();
                let state = query(url, "state");
                std::thread::spawn(move || {
                    get(
                        port,
                        &format!("/callback?error=access_denied&state={state}"),
                    )
                });
                Ok(())
            })
            .await
        };
        let (a, b) = tokio::join!(first, second_start);
        assert_eq!(a, Err("cancelled".to_owned()));
        assert_eq!(b, Err("denied".to_owned()));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn status_without_tokens_is_disconnected() {
        let _serial = SERIAL.lock().await;
        let (api, _) = counting_api();
        let alva = alva_against(api).await;
        assert_eq!(alva.status().await, AlvanistaStatus::default());
    }

    #[tokio::test]
    async fn status_offline_serves_the_mirror_and_never_carries_tokens() {
        let _serial = SERIAL.lock().await;
        // Porta fechada: erro de transporte (transitório).
        let alva = alva_against("http://127.0.0.1:9".into()).await;
        alva.store().start_session(&tokens("s1")).await.unwrap();
        alva.store()
            .mirror_put(&client::Me {
                id: "alva_1".into(),
                username: "toy".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/users/toy".into(),
            })
            .await
            .unwrap();
        let st = alva.status().await;
        assert!(st.connected && st.offline && !st.ended);
        assert_eq!(st.username.as_deref(), Some("toy"));
        let json = serde_json::to_string(&st).unwrap();
        assert!(!json.contains("alp_") && !json.contains("alr_"));
        // Os tokens seguem guardados.
        assert!(alva.store().load_tokens().await.unwrap().is_some());
    }

    #[tokio::test]
    async fn disconnect_clears_session_and_mirror_even_when_offline() {
        let _serial = SERIAL.lock().await;
        let alva = alva_against("http://127.0.0.1:9".into()).await;
        alva.store().start_session(&tokens("s2")).await.unwrap();
        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(alva.store().load_tokens().await.unwrap(), None);
        assert_eq!(alva.status().await, AlvanistaStatus::default());
    }

    #[tokio::test]
    async fn disconnect_succeeds_when_only_the_keyring_delete_fails_and_the_server_revoked() {
        let _serial = SERIAL.lock().await;
        let (api, hits) = counting_api_with(200);
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store().start_session(&tokens("s3")).await.unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);
        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn disconnect_fails_when_the_keyring_delete_fails_and_the_revoke_did_not_arrive() {
        let _serial = SERIAL.lock().await;
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api("http://127.0.0.1:9".into(), store));
        alva.store().start_session(&tokens("s4")).await.unwrap();
        alva.store()
            .mirror_put(&client::Me {
                id: "a".into(),
                username: "toy".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/users/toy".into(),
            })
            .await
            .unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);
        assert_eq!(alva.disconnect().await, Err("failed".to_owned()));
        // Ainda há sessão viva: o espelho fica e o estado segue conectado.
        assert!(alva.store().mirror_get().await.unwrap().is_some());
        assert!(alva.cached_status().await.connected);
    }

    fn me_named(username: &str) -> client::Me {
        client::Me {
            id: "alva_1".into(),
            username: username.into(),
            avatar_url: None,
            hero_url: None,
            bio: None,
            profile_url: format!("https://www.alvanista.com/users/{username}"),
        }
    }

    #[tokio::test]
    async fn disconnect_without_secret_service_ends_even_if_the_revoke_does_not_arrive() {
        let _serial = SERIAL.lock().await;
        let (store, _) = store_with(MemSecrets {
            broken: true,
            ..Default::default()
        })
        .await;
        // Porta fechada: a revogação não chega.
        let alva = Alvanista::new(Client::with_api("http://127.0.0.1:9".into(), store));
        alva.store().start_session(&tokens("s7")).await.unwrap();
        alva.store().mirror_put(&me_named("toy")).await.unwrap();
        assert!(alva.cached_status().await.insecure_storage);

        // A sessão estava só na reserva, que saiu: nada vivo ficou para trás.
        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        assert_eq!(alva.store().mirror_get().await.unwrap(), None);
        // E desconectar de novo não emperra.
        assert_eq!(alva.disconnect().await, Ok(()));
    }

    #[tokio::test]
    async fn disconnect_recovers_a_machine_left_with_only_the_mirror() {
        let _serial = SERIAL.lock().await;
        // Como uma desconexão antiga deixava a máquina: chaveiro inacessível,
        // reserva vazia e só o espelho, mostrado como "conectado, sem rede".
        let (store, _) = store_with(MemSecrets {
            broken: true,
            ..Default::default()
        })
        .await;
        let alva = Alvanista::new(Client::with_api("http://127.0.0.1:9".into(), store));
        alva.store().mirror_put(&me_named("toy")).await.unwrap();
        let stuck = alva.cached_status().await;
        assert!(stuck.connected && stuck.offline);

        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        assert_eq!(alva.status().await, AlvanistaStatus::default());
    }

    #[tokio::test]
    async fn status_reads_the_keyring_once() {
        let _serial = SERIAL.lock().await;
        // A API recusa o /me (400): o status cai no espelho, sem renovar.
        let (api, _) = counting_api();
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store().start_session(&tokens("s8")).await.unwrap();
        alva.store().mirror_put(&me_named("toy")).await.unwrap();

        secrets.gets.store(0, Ordering::SeqCst);
        assert!(alva.cached_status().await.connected);
        assert_eq!(secrets.gets.load(Ordering::SeqCst), 1, "cached_status");

        secrets.gets.store(0, Ordering::SeqCst);
        assert!(alva.status().await.connected);
        assert_eq!(secrets.gets.load(Ordering::SeqCst), 1, "status");

        secrets.gets.store(0, Ordering::SeqCst);
        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(secrets.gets.load(Ordering::SeqCst), 1, "disconnect");
    }

    #[tokio::test]
    async fn status_reports_insecure_storage_from_where_the_renewed_pair_went() {
        let _serial = SERIAL.lock().await;
        // /me aceita só o par renovado; a renovação grava na reserva porque
        // o chaveiro passou a recusar gravações.
        let renewed = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let api = format!("http://{}", listener.local_addr().unwrap());
        let renewed2 = renewed.clone();
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(20);
            while std::time::Instant::now() < until {
                let Ok((mut s, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let _ = s.set_nonblocking(false);
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let (status, body) = if req.contains("/oauth/token") {
                    renewed2.fetch_add(1, Ordering::SeqCst);
                    (
                        200,
                        r#"{"access_token":"alp_s9b","refresh_token":"alr_s9b"}"#.to_owned(),
                    )
                } else if req.contains("Bearer alp_s9b") {
                    (
                        200,
                        r#"{"id":"alva_1","username":"toy","profile_url":"https://www.alvanista.com/users/toy"}"#
                            .to_owned(),
                    )
                } else {
                    (401, r#"{"error":"invalid_token"}"#.to_owned())
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store().start_session(&tokens("s9")).await.unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);

        let st = alva.status().await;
        assert_eq!(renewed.load(Ordering::SeqCst), 1);
        assert!(st.connected && !st.offline, "{st:?}");
        assert!(st.insecure_storage, "{st:?}");
    }

    #[tokio::test]
    async fn cached_status_never_touches_the_network() {
        let _serial = SERIAL.lock().await;
        let (api, hits) = counting_api();
        let alva = alva_against(api).await;
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        alva.store().start_session(&tokens("s5")).await.unwrap();
        alva.store()
            .mirror_put(&client::Me {
                id: "a".into(),
                username: "toy".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/users/toy".into(),
            })
            .await
            .unwrap();
        let st = alva.cached_status().await;
        assert!(st.connected && !st.offline && !st.ended);
        assert_eq!(st.username.as_deref(), Some("toy"));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_browser_only_opens_the_alvanista_site() {
        let pkce = pkce::new();
        assert!(is_site_url(&config::authorize_url(
            "https://www.alvanista.com",
            4242,
            &pkce
        )));
        assert!(is_site_url("https://www.alvanista.com/users/toy"));
        for bad in [
            "http://www.alvanista.com/users/toy",
            "https://evil.example/users/toy",
            "https://www.alvanista.com.evil.example/",
            "https://www.alvanista.com@evil.example/",
            "https://www.alvanista.com:8443/",
            "file:///C:/Windows/System32/calc.exe",
            "não é url",
        ] {
            assert!(!is_site_url(bad), "{bad}");
            assert!(open_in_browser(bad).is_err(), "{bad}");
        }
    }

    #[tokio::test]
    async fn profile_url_only_opens_the_alvanista_site() {
        let _serial = SERIAL.lock().await;
        let alva = alva_against("http://127.0.0.1:9".into()).await;
        let put = |url: &str| client::Me {
            id: "a".into(),
            username: "u".into(),
            avatar_url: None,
            hero_url: None,
            bio: None,
            profile_url: url.into(),
        };
        alva.store()
            .mirror_put(&put("https://evil.example/users/u"))
            .await
            .unwrap();
        assert_eq!(alva.profile_url().await, None);
        alva.store()
            .mirror_put(&put("https://www.alvanista.com/users/u"))
            .await
            .unwrap();
        assert_eq!(
            alva.profile_url().await.as_deref(),
            Some("https://www.alvanista.com/users/u")
        );
    }
}
