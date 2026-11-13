//! "Entrar com Alvanista": OAuth 2.0 com PKCE contra o provedor do Alvanista
//! (rede social de jogos), com o navegador do sistema e um callback em
//! `127.0.0.1` (RFC 8252). Só `profile:read`: o ReEmu espelha o perfil.
//!
//! Núcleo trazido do PR #1, de Edward Facundo (Tellurian), sem a reserva dos
//! tokens no SQLite e sem a adoção de bancos de pré-lançamento.
//!
//! - `config`: URLs do serviço e URL de autorização;
//! - `pkce`: verifier, challenge S256 e state;
//! - `loopback`: o servidor de uma conexão que recebe o `code`;
//! - `client`: troca do code, renovação, revogação e `GET /me`;
//! - `store`: tokens no chaveiro, `device_id` e o espelho.
//!
//! Toda rede passa por aqui (Rust), nunca pelo webview. Os comandos Tauri
//! ficam no fim deste arquivo.

pub mod client;
pub mod config;
pub mod loopback;
pub mod pkce;
pub mod store;

use crate::sync::SyncService;
use client::{Client, ClientError};
use loopback::{Lang, LoopbackError};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use store::{ClearError, Loaded, Mirror, Store, Tokens};
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
    /// A conexão acabou (revogada no site, por exemplo); os tokens já saíram.
    pub ended: bool,
    /// O cartão do perfil que o `/me` mandou (ou o último guardado).
    pub card: Option<serde_json::Value>,
    /// Quando o perfil foi lido do Alvanista (segundos Unix).
    pub fetched_at: Option<i64>,
    /// Quando esta conexão começou (segundos Unix); `None` em sessões antigas.
    pub connected_at: Option<i64>,
    /// O rótulo deste aparelho no Alvanista, quando conectado.
    pub device_label: Option<String>,
    /// O plano da conta libera o sync de saves? `None` quando o `/me` ainda
    /// não disse (espelho de antes do campo, ou sem espelho).
    pub save_sync: Option<bool>,
    /// Escopos desta sessão; vazio sem sessão.
    pub scopes: Vec<String>,
}

impl AlvanistaStatus {
    fn from_mirror(m: Mirror, offline: bool, connected_at: Option<i64>) -> Self {
        Self {
            save_sync: m.save_sync,
            scopes: Vec::new(),
            connected: true,
            username: Some(m.username),
            avatar_url: m.avatar_url,
            hero_url: m.hero_url,
            bio: m.bio,
            profile_url: Some(m.profile_url),
            offline,
            ended: false,
            card: m.card,
            fetched_at: Some(m.fetched_at),
            connected_at,
            device_label: Some(config::device_label().to_owned()),
        }
    }

    /// Conectado sem espelho (o `/me` ainda não respondeu).
    fn without_mirror(offline: bool, connected_at: Option<i64>) -> Self {
        Self {
            connected: true,
            offline,
            connected_at,
            device_label: Some(config::device_label().to_owned()),
            ..Default::default()
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
    /// O aparelho do cloud save desta máquina: sai junto com a conta.
    sync: OnceLock<Arc<SyncService>>,
}

impl Alvanista {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            generation: Arc::new(AtomicU64::new(0)),
            sync: OnceLock::new(),
        }
    }

    /// Liga o cloud save: desconectar revoga o aparelho, e uma conexão que
    /// acabou no site o esquece. Só a primeira chamada vale.
    pub fn attach_sync(&self, sync: Arc<SyncService>) {
        let _ = self.sync.set(sync);
    }

    /// O aparelho do cloud save sai com a conta. `revoke`: avisa o servidor
    /// com o token do aparelho antes (quando a conta some no site, o par do
    /// parceiro já não existe, e o aparelho só se esquece). Nunca falha
    /// quem chama: o que sobrar vai para o log.
    async fn forget_save_device(&self, revoke: bool) {
        let Some(sync) = self.sync.get() else {
            return;
        };
        if let Err(e) = sync.forget_device(revoke).await {
            log::warn!("alvanista: aparelho do cloud save não esquecido: {e}");
        }
    }

    fn store(&self) -> &Store {
        self.client.store()
    }

    /// O par de tokens da sessão, para o sync pedir o aparelho em nome da
    /// conta. `None` sem sessão (e sem tocar o chaveiro nem a rede).
    pub(crate) async fn session_tokens(&self) -> Result<Option<Tokens>, String> {
        Ok(self.stored_session().await?.map(|l| l.tokens))
    }

    /// Renova o par (gravando o novo), para o sync repetir o pedido que
    /// voltou 401.
    pub(crate) async fn renew(&self, current: &Tokens) -> Result<Tokens, ClientError> {
        self.client.refresh(current).await
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
    /// `offline`, `keyring`, `failed`.
    pub async fn connect<F>(&self, lang: Lang, open: F) -> Result<AlvanistaStatus, String>
    where
        F: FnOnce(&str) -> Result<(), String>,
    {
        self.connect_with(config::SCOPE_PROFILE, false, lang, open)
            .await
    }

    /// Refaz a conexão pedindo também `saves`. O par anterior só é revogado
    /// depois que a sessão nova foi guardada, e o resultado da revogação
    /// não muda nada. Os erros são os de `connect`.
    pub async fn reconnect_with_saves<F>(
        &self,
        lang: Lang,
        open: F,
    ) -> Result<AlvanistaStatus, String>
    where
        F: FnOnce(&str) -> Result<(), String>,
    {
        let scope = format!("{} {}", config::SCOPE_PROFILE, config::SCOPE_SAVES);
        self.connect_with(&scope, true, lang, open).await
    }

    /// `scope`: lista separada por espaço, como vai na URL de consentimento.
    /// `replace`: revoga o par anterior depois de guardar a sessão nova.
    async fn connect_with<F>(
        &self,
        scope: &str,
        replace: bool,
        lang: Lang,
        open: F,
    ) -> Result<AlvanistaStatus, String>
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
        let url = config::authorize_url(&config::base_urls().0, port, &pkce, scope);
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
        let scopes: Vec<&str> = scope.split_whitespace().collect();
        let previous = self
            .complete(
                mine,
                &code,
                &pkce.verifier,
                &config::redirect_uri(port),
                &scopes,
                replace,
            )
            .await?;
        if let Some(old) = previous {
            let _ = self.client.revoke(&old).await;
        }
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
        scopes: &[&str],
        take_previous: bool,
    ) -> Result<Option<Tokens>, String> {
        if !self.is_current(mine) {
            return Err("cancelled".into());
        }
        let device_id = self.store().device_id().await.map_err(|e| {
            log::warn!("alvanista: sem device_id: {e}");
            "failed".to_owned()
        })?;
        let (tokens, granted) = self
            .client
            .exchange_scoped(code, verifier, redirect_uri, &device_id)
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
        // O que o servidor disse ter concedido vale; sem isso, o que se pediu.
        let granted: Option<Vec<&str>> = granted
            .as_deref()
            .map(|g| g.split_whitespace().collect::<Vec<_>>())
            .filter(|g| !g.is_empty());
        let scopes = granted.as_deref().unwrap_or(scopes);
        // O par anterior é lido só agora: uma renovação que o girou durante o
        // consentimento é o par que se revoga.
        let previous = if take_previous && self.store().has_session().await.unwrap_or(false) {
            self.store().load_tokens().await.unwrap_or(None)
        } else {
            None
        };
        // Sem chaveiro não há onde guardar: os tokens emitidos não ficam vivos.
        if let Err(e) = self.store().start_session(&tokens, scopes).await {
            log::warn!("alvanista: não guardou a sessão: {e}");
            self.client.revoke(&tokens).await;
            return Err("keyring".into());
        }
        Ok(previous)
    }

    /// A sessão guardada, só quando há o marcador: sem ele não se toca no
    /// chaveiro (nem, depois, na rede). Marcador sem par (o app caiu no meio
    /// de conectar, ou o chaveiro foi limpo por fora) sai aqui.
    async fn stored_session(&self) -> Result<Option<Loaded>, String> {
        if !self.store().has_session().await? {
            return Ok(None);
        }
        let loaded = self.store().load().await?;
        if loaded.is_none() {
            if let Err(e) = self.store().unmark_session().await {
                log::warn!("alvanista: marcador de sessão não apagado: {e}");
            }
        }
        Ok(loaded)
    }

    /// A data da conexão; se não der para ler, a tela só fica sem ela.
    async fn connected_at(&self) -> Option<i64> {
        match self.store().connected_at().await {
            Ok(at) => at,
            Err(e) => {
                log::warn!("alvanista: data da conexão ilegível: {e}");
                None
            }
        }
    }

    /// O estado só com o que está no aparelho (sem rede): há sessão? qual o
    /// espelho? Serve para a tela aparecer na hora; `status` confirma depois.
    /// Lê o chaveiro uma vez só, e só com sessão marcada.
    pub async fn cached_status(&self) -> AlvanistaStatus {
        let status = self.cached_status_inner().await;
        self.with_scopes(status).await
    }

    /// Junta os escopos da sessão ao estado; sem conexão, ficam vazios.
    async fn with_scopes(&self, mut status: AlvanistaStatus) -> AlvanistaStatus {
        if status.connected {
            match self.store().scopes().await {
                Ok(scopes) => status.scopes = scopes,
                Err(e) => log::warn!("alvanista: escopos ilegíveis: {e}"),
            }
        }
        status
    }

    async fn cached_status_inner(&self) -> AlvanistaStatus {
        let mirror = self.store().mirror_get().await.ok().flatten();
        let connected_at = self.connected_at().await;
        match self.stored_session().await {
            Ok(Some(_)) => match mirror {
                Some(m) => AlvanistaStatus::from_mirror(m, false, connected_at),
                None => AlvanistaStatus::without_mirror(false, connected_at),
            },
            Ok(None) => AlvanistaStatus::default(),
            Err(e) => {
                log::warn!("alvanista: não leu a sessão: {e}");
                // Sem leitura não se sabe onde a sessão está: só o espelho.
                match mirror {
                    Some(m) => AlvanistaStatus::from_mirror(m, true, connected_at),
                    None => AlvanistaStatus::default(),
                }
            }
        }
    }

    /// O estado da conexão, renovando o espelho quando há rede. Sem sessão
    /// marcada, volta sem tocar no chaveiro nem na rede. Com ela, lê o
    /// chaveiro uma vez.
    pub async fn status(&self) -> AlvanistaStatus {
        let status = self.status_inner().await;
        self.with_scopes(status).await
    }

    async fn status_inner(&self) -> AlvanistaStatus {
        let mirror = match self.store().mirror_get().await {
            Ok(m) => m,
            Err(e) => {
                log::warn!("alvanista: espelho ilegível: {e}");
                None
            }
        };
        let session = match self.stored_session().await {
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
                    Some(m) => AlvanistaStatus::from_mirror(m, true, self.connected_at().await),
                    None => AlvanistaStatus::default(),
                };
            }
        };
        let mut tokens = session.tokens;
        let connected_at = self.connected_at().await;
        match self.client.me(&mut tokens).await {
            Ok(me) => AlvanistaStatus {
                connected: true,
                username: Some(me.username),
                avatar_url: me.avatar_url,
                hero_url: me.hero_url,
                bio: me.bio,
                profile_url: Some(me.profile_url),
                offline: false,
                ended: false,
                card: me.card,
                fetched_at: Some(store::now_secs()),
                connected_at,
                device_label: Some(config::device_label().to_owned()),
                save_sync: me.entitlements.map(|e| e.save_sync),
                scopes: Vec::new(),
            },
            Err(ClientError::AuthEnded) => {
                let _ = self.store().mirror_clear().await;
                // `client.me` já devolveu: nenhum trava de renovação está
                // preso aqui. O par do parceiro acabou, então só se esquece.
                self.forget_save_device(false).await;
                AlvanistaStatus {
                    ended: true,
                    ..Default::default()
                }
            }
            Err(e) => {
                log::info!("alvanista: sem resposta, usando o espelho: {e}");
                match mirror {
                    Some(m) => AlvanistaStatus::from_mirror(m, true, connected_at),
                    None => AlvanistaStatus::without_mirror(true, connected_at),
                }
            }
        }
    }

    /// Desconecta: revoga no servidor (se não der, segue) e apaga a sessão
    /// e o espelho. O perfil local nunca é tocado.
    ///
    /// Se o chaveiro recusar o apagar, vale como desconectado quando nada vivo
    /// pode ter ficado nele:
    /// - o servidor confirmou a revogação;
    /// - não havia sessão legível: só sobrou o espelho. É o que tira do
    ///   "conectado, sem rede" quem ficou preso nesse estado.
    ///
    /// Fora disso, o par do chaveiro é o vivo e a revogação não chegou:
    /// desconectar falha, para não deixar tokens vivos sem ninguém saber.
    ///
    /// Sem sessão marcada não há o que revogar nem apagar: só sai o espelho.
    pub async fn disconnect(&self) -> Result<(), String> {
        self.cancel();
        // O aparelho do cloud save vai primeiro, com o token dele: o par do
        // parceiro, que revoga abaixo, não é o que o autoriza.
        self.forget_save_device(true).await;
        if matches!(self.store().has_session().await, Ok(false)) {
            if let Err(e) = self.store().mirror_clear().await {
                log::warn!("alvanista: espelho não apagado: {e}");
            }
            return Ok(());
        }
        let (revoked, nothing_alive_in_keyring) = match self.store().load().await {
            Ok(Some(session)) => (self.client.revoke(&session.tokens).await, false),
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
        // Quando o chaveiro recusou o apagar, `clear_tokens` deixou o
        // marcador; aqui a sessão já vale como encerrada.
        if let Err(e) = self.store().unmark_session().await {
            log::warn!("alvanista: marcador de sessão não apagado: {e}");
        }
        if let Err(e) = self.store().mirror_clear().await {
            log::warn!("alvanista: espelho não apagado: {e}");
        }
        Ok(())
    }

    /// O `status` da abertura do app, só com sessão marcada. Sem ela devolve
    /// `None` sem ler o chaveiro nem chamar a rede.
    pub async fn refresh_if_connected(&self) -> Option<AlvanistaStatus> {
        match self.store().has_session().await {
            Ok(true) => Some(self.status().await),
            Ok(false) => None,
            Err(e) => {
                log::warn!("alvanista: não leu o marcador de sessão: {e}");
                None
            }
        }
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
pub(crate) fn open_in_browser(url: &str) -> Result<(), String> {
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
/// acabado no site, ou o nome mudado). Sem sessão marcada não lê o chaveiro,
/// não toca na rede e não manda o evento.
pub fn refresh_on_start(app: &AppHandle) {
    if app.try_state::<Alvanista>().is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(alva) = app.try_state::<Alvanista>() else {
            return;
        };
        let Some(status) = alva.refresh_if_connected().await else {
            return;
        };
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

/// A página do site para cada nome que a tela conhece. Qualquer outro nome é
/// recusado: a tela escolhe a página, nunca a URL.
fn site_page_url(page: &str) -> Option<String> {
    let path = match page {
        "edit" => "/settings/profile",
        "apps" => "/settings/apps",
        _ => return None,
    };
    Some(format!("{}{path}", config::base_urls().0))
}

/// Abre "editar perfil" ou "apps conectados" no navegador do sistema.
#[tauri::command]
pub async fn alvanista_open_site(page: String) -> Result<(), String> {
    let url = site_page_url(&page).ok_or_else(|| "failed".to_owned())?;
    open_in_browser(&url).map_err(|e| {
        log::warn!("alvanista: não abriu a página do site: {e}");
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
        let r = alva
            .complete(
                mine,
                "code",
                "verifier",
                "http://x",
                &["profile:read"],
                false,
            )
            .await;
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

    /// Alvanista de mentira que emite tokens e conta as revogações.
    fn issuing_api() -> (String, Arc<AtomicUsize>) {
        let revokes = Arc::new(AtomicUsize::new(0));
        let counter = revokes.clone();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let api = format!("http://{}", listener.local_addr().unwrap());
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
                if req.contains("/oauth/revoke") {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                let body = r#"{"access_token":"alp_k","refresh_token":"alr_k"}"#;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (api, revokes)
    }

    #[tokio::test]
    async fn without_a_keyring_connecting_fails_and_the_new_tokens_are_revoked() {
        let _serial = SERIAL.lock().await;
        let (api, revokes) = issuing_api();
        let (store, _) = store_with(MemSecrets {
            broken: true,
            ..Default::default()
        })
        .await;
        let alva = Alvanista::new(Client::with_api(api, store));
        let mine = alva.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let r = alva
            .complete(
                mine,
                "code",
                "verifier",
                "http://x",
                &["profile:read"],
                false,
            )
            .await;
        assert_eq!(r, Err("keyring".to_owned()));
        assert_eq!(revokes.load(Ordering::SeqCst), 1);
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
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
        alva.store()
            .start_session(&tokens("s1"), &["profile:read"])
            .await
            .unwrap();
        alva.store()
            .mirror_put(&client::Me {
                id: "alva_1".into(),
                username: "toy".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/users/toy".into(),
                card: None,
                entitlements: None,
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
        alva.store()
            .start_session(&tokens("s2"), &["profile:read"])
            .await
            .unwrap();
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
        alva.store()
            .start_session(&tokens("s3"), &["profile:read"])
            .await
            .unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);
        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn disconnect_fails_when_the_keyring_delete_fails_and_the_revoke_did_not_arrive() {
        let _serial = SERIAL.lock().await;
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api("http://127.0.0.1:9".into(), store));
        alva.store()
            .start_session(&tokens("s4"), &["profile:read"])
            .await
            .unwrap();
        alva.store()
            .mirror_put(&client::Me {
                id: "a".into(),
                username: "toy".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/users/toy".into(),
                card: None,
                entitlements: None,
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
            card: None,
            entitlements: None,
        }
    }

    #[tokio::test]
    async fn disconnect_recovers_a_machine_left_with_only_the_mirror() {
        let _serial = SERIAL.lock().await;
        // Chaveiro inacessível e só o espelho, mostrado como "conectado, sem rede".
        let (store, _, db) = store_and_db(MemSecrets {
            broken: true,
            ..Default::default()
        })
        .await;
        mark_session(&db).await;
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
        alva.store()
            .start_session(&tokens("s8"), &["profile:read"])
            .await
            .unwrap();
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
    async fn status_keeps_working_when_the_keyring_refuses_the_renewed_pair() {
        let _serial = SERIAL.lock().await;
        // /me aceita só o par renovado, e o chaveiro passou a recusar gravações.
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
        alva.store()
            .start_session(&tokens("s9"), &["profile:read"])
            .await
            .unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);

        let st = alva.status().await;
        assert_eq!(renewed.load(Ordering::SeqCst), 1);
        assert!(st.connected && !st.offline, "{st:?}");
    }

    #[tokio::test]
    async fn cached_status_never_touches_the_network() {
        let _serial = SERIAL.lock().await;
        let (api, hits) = counting_api();
        let alva = alva_against(api).await;
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        alva.store()
            .start_session(&tokens("s5"), &["profile:read"])
            .await
            .unwrap();
        alva.store()
            .mirror_put(&client::Me {
                id: "a".into(),
                username: "toy".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/users/toy".into(),
                card: None,
                entitlements: None,
            })
            .await
            .unwrap();
        let st = alva.cached_status().await;
        assert!(st.connected && !st.offline && !st.ended);
        assert_eq!(st.username.as_deref(), Some("toy"));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    fn me_full(username: &str) -> client::Me {
        client::Me {
            hero_url: Some(format!("https://cdn.alvanista.com/h/{username}.png")),
            bio: Some(format!("bio de {username}")),
            avatar_url: Some(format!("https://cdn.alvanista.com/a/{username}.png")),
            ..me_named(username)
        }
    }

    #[tokio::test]
    async fn without_a_session_the_keyring_and_the_network_are_never_called() {
        let _serial = SERIAL.lock().await;
        let (api, hits) = counting_api_with(200);
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        // Um espelho perdido (de uma desconexão que não o apagou) não muda nada.
        alva.store().mirror_put(&me_named("toy")).await.unwrap();

        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        assert_eq!(alva.status().await, AlvanistaStatus::default());
        assert_eq!(alva.refresh_if_connected().await, None);
        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());

        assert_eq!(secrets.calls.load(Ordering::SeqCst), 0, "SecretStore");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "HTTP");
    }

    #[tokio::test]
    async fn a_marker_without_a_pair_is_dropped_on_the_first_read() {
        let _serial = SERIAL.lock().await;
        let (api, hits) = counting_api();
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store()
            .start_session(&tokens("m4"), &["profile:read"])
            .await
            .unwrap();
        // O chaveiro foi limpo por fora do ReEmu.
        secrets.secrets.lock().unwrap().clear();

        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        assert!(!alva.store().has_session().await.unwrap());
        // Dali em diante, nem o chaveiro.
        secrets.calls.store(0, Ordering::SeqCst);
        assert_eq!(alva.refresh_if_connected().await, None);
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        assert_eq!(secrets.calls.load(Ordering::SeqCst), 0);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn offline_the_app_opens_with_the_right_profile() {
        let _serial = SERIAL.lock().await;
        // Porta fechada: sem rede.
        let alva = alva_against("http://127.0.0.1:9".into()).await;

        // Sem sessão: nada do Alvanista (a tela fica com o perfil local).
        assert_eq!(alva.refresh_if_connected().await, None);
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());

        // Conectado: o último perfil guardado, inteiro, como offline.
        alva.store()
            .start_session(&tokens("m5"), &["profile:read"])
            .await
            .unwrap();
        alva.store().mirror_put(&me_full("toy")).await.unwrap();
        let cached = alva.cached_status().await;
        assert!(cached.connected && !cached.offline, "{cached:?}");
        let boot = alva.refresh_if_connected().await.unwrap();
        assert!(boot.connected && boot.offline && !boot.ended, "{boot:?}");
        for st in [&cached, &boot] {
            assert_eq!(st.username.as_deref(), Some("toy"));
            assert_eq!(
                st.avatar_url.as_deref(),
                Some("https://cdn.alvanista.com/a/toy.png")
            );
            assert_eq!(st.bio.as_deref(), Some("bio de toy"));
            assert_eq!(
                st.hero_url.as_deref(),
                Some("https://cdn.alvanista.com/h/toy.png")
            );
        }
        // Sem rede a sessão continua.
        assert!(alva.store().has_session().await.unwrap());
    }

    #[tokio::test]
    async fn disconnect_gives_back_the_local_profile_exactly_as_it_was() {
        use domain::profile::{Profile, ProfileRepository};
        let _serial = SERIAL.lock().await;
        let (api, _) = counting_api_with(200);
        let (store, _, db) = store_and_db(MemSecrets::default()).await;
        let profiles = db::ProfileRepo::new(db);
        profiles
            .update(&Profile {
                name: "Local".into(),
                bio: Some("bio local".into()),
                avatar: "preset:3".into(),
                onboarded: true,
            })
            .await
            .unwrap();
        let before = profiles.get().await.unwrap();

        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store()
            .start_session(&tokens("m6"), &["profile:read"])
            .await
            .unwrap();
        alva.store().mirror_put(&me_full("toy")).await.unwrap();
        assert_eq!(profiles.get().await.unwrap(), before, "conectado");

        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(profiles.get().await.unwrap(), before, "desconectado");
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        assert!(!alva.store().has_session().await.unwrap());
        assert_eq!(alva.store().mirror_get().await.unwrap(), None);
    }

    #[tokio::test]
    async fn disconnect_with_a_keyring_that_refuses_the_delete_still_drops_the_marker() {
        let _serial = SERIAL.lock().await;
        // O servidor confirma a revogação; o chaveiro não deixa apagar.
        let (api, _) = counting_api_with(200);
        let (store, secrets) = store_with(MemSecrets::default()).await;
        let alva = Alvanista::new(Client::with_api(api, store));
        alva.store()
            .start_session(&tokens("m7"), &["profile:read"])
            .await
            .unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);
        assert_eq!(alva.disconnect().await, Ok(()));
        assert!(!alva.store().has_session().await.unwrap());
        // E a próxima abertura não lê o chaveiro.
        secrets.calls.store(0, Ordering::SeqCst);
        assert_eq!(alva.cached_status().await, AlvanistaStatus::default());
        assert_eq!(secrets.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn open_site_refuses_unknown_pages() {
        for page in ["edit", "apps"] {
            assert!(is_site_url(&site_page_url(page).unwrap()), "{page}");
        }
        for bad in ["", "EDIT", "../admin", "https://evil.example/"] {
            assert_eq!(site_page_url(bad), None, "{bad}");
        }
    }

    #[tokio::test]
    async fn the_cached_status_carries_the_card_and_the_connection_data() {
        let _serial = SERIAL.lock().await;
        let alva = alva_against("http://127.0.0.1:9".into()).await;
        alva.store()
            .start_session(&tokens("c1"), &["profile:read"])
            .await
            .unwrap();
        alva.store()
            .mirror_put(&client::Me {
                card: Some(serde_json::json!({ "member_since": 2010 })),
                ..me_named("toy")
            })
            .await
            .unwrap();
        for st in [alva.cached_status().await, alva.status().await] {
            assert_eq!(st.card.unwrap()["member_since"], 2010);
            assert!(st.fetched_at.unwrap() > 1_700_000_000);
            assert!(st.connected_at.unwrap() > 1_700_000_000);
            assert_eq!(st.device_label.as_deref(), Some(config::device_label()));
        }
    }

    #[test]
    fn the_browser_only_opens_the_alvanista_site() {
        let pkce = pkce::new();
        assert!(is_site_url(&config::authorize_url(
            "https://www.alvanista.com",
            4242,
            &pkce,
            config::SCOPE_PROFILE
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
            card: None,
            entitlements: None,
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

    #[tokio::test]
    async fn the_cached_status_carries_scopes_and_the_save_sync_flag() {
        let _serial = SERIAL.lock().await;
        let alva = alva_against("http://127.0.0.1:1".into()).await;
        assert!(alva.cached_status().await.scopes.is_empty());
        alva.store()
            .start_session(&tokens("sc"), &["profile:read", "saves"])
            .await
            .unwrap();
        let mut me = me_named("toy");
        me.entitlements = Some(client::Entitlements { save_sync: true });
        alva.store().mirror_put(&me).await.unwrap();
        let st = alva.cached_status().await;
        assert_eq!(st.scopes, vec!["profile:read", "saves"]);
        assert_eq!(st.save_sync, Some(true));
    }

    /// API do fluxo de conectar: `/token` devolve `alr_new` (com o `scope`
    /// dado, se houver) e `/revoke` responde `revoke_status`. Cada chamada
    /// entra em `events` (`token`, `revoke:<refresh>`), o mesmo registro em
    /// que o chaveiro anota `save`: dá a ordem entre os três.
    fn flow_api(
        events: Arc<std::sync::Mutex<Vec<String>>>,
        token_scope: Option<&'static str>,
        revoke_status: u16,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(30);
            while std::time::Instant::now() < until {
                let Ok((mut s, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let _ = s.set_nonblocking(false);
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let (status, body) = if req.contains("/v1/me") {
                    (404, String::new())
                } else if req.contains("/oauth/revoke") {
                    let token = req
                        .split("token=")
                        .nth(1)
                        .and_then(|t| t.split('&').next())
                        .unwrap_or("")
                        .to_owned();
                    events.lock().unwrap().push(format!("revoke:{token}"));
                    (revoke_status, String::new())
                } else {
                    events.lock().unwrap().push("token".into());
                    let scope = token_scope
                        .map(|v| format!(r#","scope":"{v}""#))
                        .unwrap_or_default();
                    (
                        200,
                        format!(
                            r#"{{"access_token":"alp_new","token_type":"Bearer","refresh_token":"alr_new"{scope}}}"#
                        ),
                    )
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        base
    }

    /// Um navegador de mentira: aprova (`code`) ou nega o consentimento.
    fn browser(approve: bool) -> impl FnOnce(&str) -> Result<(), String> {
        move |url: &str| {
            let port: u16 = url::Url::parse(&query(url, "redirect_uri"))
                .unwrap()
                .port()
                .unwrap();
            let state = query(url, "state");
            std::thread::spawn(move || {
                let target = if approve {
                    format!("/callback?code=c&state={state}")
                } else {
                    format!("/callback?error=access_denied&state={state}")
                };
                get(port, &target);
            });
            Ok(())
        }
    }

    async fn alva_with_events(
        api: String,
        events: Arc<std::sync::Mutex<Vec<String>>>,
    ) -> Alvanista {
        let (store, _) = store_with(MemSecrets {
            events: Some(events),
            ..Default::default()
        })
        .await;
        Alvanista::new(Client::with_api(api, store))
    }

    #[tokio::test]
    async fn reconnect_with_saves_exchanges_stores_then_revokes_the_old_pair() {
        let _serial = SERIAL.lock().await;
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let alva = alva_with_events(flow_api(events.clone(), None, 200), events.clone()).await;
        alva.store()
            .start_session(&tokens("old"), &["profile:read"])
            .await
            .unwrap();
        events.lock().unwrap().clear();
        let r = alva.reconnect_with_saves(Lang::PtBr, browser(true)).await;
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(
            *events.lock().unwrap(),
            vec!["token", "save", "revoke:alr_old"]
        );
        assert_eq!(
            alva.store().load_tokens().await.unwrap(),
            Some(Tokens {
                access_token: "alp_new".into(),
                refresh_token: "alr_new".into()
            })
        );
        // O servidor não informou o escopo: vale o que se pediu.
        assert_eq!(
            alva.store().scopes().await.unwrap(),
            vec!["profile:read", "saves"]
        );
    }

    #[tokio::test]
    async fn a_failing_revoke_still_succeeds_and_keeps_the_new_session() {
        let _serial = SERIAL.lock().await;
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let alva = alva_with_events(flow_api(events.clone(), None, 500), events.clone()).await;
        alva.store()
            .start_session(&tokens("old"), &["profile:read"])
            .await
            .unwrap();
        let r = alva.reconnect_with_saves(Lang::PtBr, browser(true)).await;
        assert!(r.is_ok(), "{r:?}");
        assert!(events
            .lock()
            .unwrap()
            .contains(&"revoke:alr_old".to_owned()));
        assert_eq!(
            alva.store()
                .load_tokens()
                .await
                .unwrap()
                .unwrap()
                .refresh_token,
            "alr_new"
        );
    }

    #[tokio::test]
    async fn a_denied_reconnect_leaves_the_old_session_and_revokes_nothing() {
        let _serial = SERIAL.lock().await;
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let alva = alva_with_events(flow_api(events.clone(), None, 200), events.clone()).await;
        alva.store()
            .start_session(&tokens("old"), &["profile:read"])
            .await
            .unwrap();
        events.lock().unwrap().clear();
        let r = alva.reconnect_with_saves(Lang::PtBr, browser(false)).await;
        assert_eq!(r, Err("denied".to_owned()));
        assert!(
            events.lock().unwrap().is_empty(),
            "{:?}",
            events.lock().unwrap()
        );
        assert_eq!(
            alva.store().load_tokens().await.unwrap(),
            Some(tokens("old"))
        );
        assert_eq!(alva.store().scopes().await.unwrap(), vec!["profile:read"]);
    }

    #[tokio::test]
    async fn the_pair_rotated_during_consent_is_the_one_revoked() {
        let _serial = SERIAL.lock().await;
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let alva = alva_with_events(flow_api(events.clone(), None, 200), events.clone()).await;
        alva.store()
            .start_session(&tokens("old"), &["profile:read"])
            .await
            .unwrap();
        // Uma renovação gira o par enquanto o navegador está aberto.
        let store_ref = alva.store();
        let rotate = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            store_ref.save_tokens(&tokens("rot")).await.unwrap();
        };
        let (r, ()) = tokio::join!(
            alva.reconnect_with_saves(Lang::PtBr, |url: &str| {
                let url = url.to_owned();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(400));
                    let port: u16 = url::Url::parse(&query(&url, "redirect_uri"))
                        .unwrap()
                        .port()
                        .unwrap();
                    get(
                        port,
                        &format!("/callback?code=c&state={}", query(&url, "state")),
                    );
                });
                Ok(())
            }),
            rotate
        );
        assert!(r.is_ok(), "{r:?}");
        assert!(events
            .lock()
            .unwrap()
            .contains(&"revoke:alr_rot".to_owned()));
    }

    #[tokio::test]
    async fn the_scopes_the_server_granted_win_over_the_requested_ones() {
        let _serial = SERIAL.lock().await;
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let api = flow_api(events.clone(), Some("profile:read"), 200);
        let alva = alva_with_events(api, events).await;
        let r = alva.reconnect_with_saves(Lang::PtBr, browser(true)).await;
        assert!(r.is_ok(), "{r:?}");
        // Pediu `profile:read saves`, o servidor só concedeu `profile:read`.
        assert_eq!(alva.store().scopes().await.unwrap(), vec!["profile:read"]);
    }

    /// Alvanista com sessão e com o cloud save ligado, os dois contra o
    /// mesmo servidor de mentira; o aparelho `dev1` já está guardado.
    async fn alva_with_save_device(
        server: &wiremock::MockServer,
    ) -> (Alvanista, Arc<SyncService>, Arc<MemSecrets>) {
        use crate::sync::device::{Device, DeviceStore};
        let secrets = Arc::new(MemSecrets::default());
        let db = db::connect_in_memory().await.unwrap();
        let store = Store::new(db.clone(), secrets.clone());
        store
            .start_session(&tokens("1"), &["profile:read", "saves"])
            .await
            .unwrap();
        let alva = Alvanista::new(Client::with_api(
            server.uri(),
            Store::new(db.clone(), secrets.clone()),
        ));
        let sync = Arc::new(SyncService::new(
            crate::sync::api::SavesApi::new(server.uri()),
            DeviceStore::new(db, secrets.clone()),
        ));
        sync.devices()
            .save(
                &Device {
                    id: "dev1".into(),
                    label: "Sala".into(),
                },
                "asv_secret",
            )
            .await
            .unwrap();
        sync.devices().set_enabled(true).await.unwrap();
        alva.attach_sync(sync.clone());
        (alva, sync, secrets)
    }

    #[tokio::test]
    async fn disconnect_revokes_the_device_before_the_partner_pair() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let _serial = SERIAL.lock().await;
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .and(header("authorization", "Bearer asv_secret"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/revoke"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let (alva, sync, secrets) = alva_with_save_device(&server).await;
        assert_eq!(alva.disconnect().await, Ok(()));
        let order: Vec<String> = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|r| format!("{} {}", r.method, r.url.path()))
            .collect();
        assert_eq!(
            order,
            vec![
                "DELETE /api/v1/saves/device".to_owned(),
                "POST /api/partner/oauth/revoke".to_owned()
            ]
        );
        assert_eq!(sync.devices().device().await.unwrap(), None);
        assert!(!secrets
            .secrets
            .lock()
            .unwrap()
            .contains_key("alvanista-saves"));
    }

    #[tokio::test]
    async fn disconnect_still_disconnects_when_the_device_revoke_is_unauthorized() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let _serial = SERIAL.lock().await;
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/revoke"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let (alva, sync, _) = alva_with_save_device(&server).await;
        assert_eq!(alva.disconnect().await, Ok(()));
        assert_eq!(sync.devices().device().await.unwrap(), None);
    }

    /// O `/me` dá 401 e a renovação é recusada: a conexão acabou. O aparelho
    /// se esquece sem aviso ao servidor, e isso não pode travar mesmo com o
    /// trinco de quem cria o aparelho preso por outra chamada.
    #[tokio::test]
    async fn an_ended_session_forgets_the_device_without_deadlocking() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let _serial = SERIAL.lock().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/me"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/token"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"error": "invalid_grant"})),
            )
            .mount(&server)
            .await;
        let (alva, sync, secrets) = alva_with_save_device(&server).await;
        let _minting = sync.hold_mint_for_test().await;
        let status = tokio::time::timeout(Duration::from_secs(10), alva.refresh_if_connected())
            .await
            .expect("travou: o esquecer do aparelho tomou o trinco do mint")
            .expect("havia sessão marcada");
        assert!(status.ended);
        assert_eq!(sync.devices().device().await.unwrap(), None);
        assert!(!secrets
            .secrets
            .lock()
            .unwrap()
            .contains_key("alvanista-saves"));
        let deletes = server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.method.as_str() == "DELETE")
            .count();
        assert_eq!(deletes, 0, "sem par do parceiro não há o que avisar");
    }
}
