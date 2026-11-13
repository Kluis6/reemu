//! O que fica no aparelho: o par de tokens (só no chaveiro do SO), o
//! `device_id` e o espelho do último perfil lido.
//!
//! Diferente dos segredos do scraper, o par nunca vai para o SQLite: sem
//! chaveiro não há sessão, e a pessoa conecta de novo quando ele voltar.
//!
//! O par leva `saved_at` (milissegundos, crescente no processo), que diz se o
//! guardado é mais novo que o da memória.
//!
//! O marcador `session` em `alvanista_state` diz se há sessão. Sem ele
//! ninguém lê o chaveiro nem chama a rede: num chaveiro trancado do Linux a
//! leitura pode pedir a senha, e quem nunca conectou não pode ser incomodado.
//! Ele entra antes do par ao conectar e sai quando a sessão acaba.

use super::client::Me;
use super::pkce::random_token;
use crate::credentials::SecretStore;
use db::AlvanistaRepo;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

const ACCOUNT: &str = "alvanista";
const DEVICE_ID_KEY: &str = "device_id";
const MIRROR_KEY: &str = "mirror";
/// Marcador de sessão (ver o topo do arquivo). O valor não importa.
const SESSION_KEY: &str = "session";
/// Quando esta conexão começou (segundos Unix). Só informa: nunca decide sessão.
const CONNECTED_AT_KEY: &str = "connected_at";
const SCOPES_KEY: &str = "scopes";

/// Uma renovação (ou desconexão) por vez em todo o processo, mesmo que
/// existam vários `Client`/`Store`.
pub(super) static REFRESH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Uma rotação feita neste processo: o refresh token gasto, o par novo e o
/// carimbo de quando ele chegou (na mesma escala do `saved_at`).
struct Rotation {
    spent: String,
    new: Tokens,
    received_at: i64,
}

/// Rotações feitas neste processo, as mais recentes. Valem mesmo quando a
/// gravação do par novo falhou.
static ROTATIONS: std::sync::Mutex<Vec<Rotation>> = std::sync::Mutex::new(Vec::new());
const ROTATIONS_KEPT: usize = 8;

pub(super) fn remember_rotation(spent_refresh: &str, new: &Tokens) {
    if let Ok(mut all) = ROTATIONS.lock() {
        all.push(Rotation {
            spent: spent_refresh.to_owned(),
            new: new.clone(),
            received_at: next_stamp(),
        });
        if all.len() > ROTATIONS_KEPT {
            all.remove(0);
        }
    }
}

/// O par mais novo que sucedeu a `spent_refresh`, se foi este processo que
/// o trocou (segue a cadeia se o par novo também já foi trocado), com o
/// carimbo de quando chegou.
pub(super) fn rotated_from(spent_refresh: &str) -> Option<(Tokens, i64)> {
    let all = ROTATIONS.lock().ok()?;
    let mut found: Option<(Tokens, i64)> = None;
    let mut key = spent_refresh.to_owned();
    for _ in 0..ROTATIONS_KEPT {
        let Some(next) = all.iter().rev().find(|r| r.spent == key) else {
            break;
        };
        key = next.new.refresh_token.clone();
        found = Some((next.new.clone(), next.received_at));
    }
    found
}

/// `true` se `refresh` é o refresh token de um par que este processo
/// acabou de receber e ainda não gastou.
pub(super) fn is_latest_rotation(refresh: &str) -> bool {
    ROTATIONS.lock().is_ok_and(|all| {
        all.iter().any(|r| r.new.refresh_token == refresh)
            && !all.iter().any(|r| r.spent == refresh)
    })
}

/// Esquece todas as rotações: a sessão acabou ou outra vai começar, e
/// nenhum par antigo pode voltar a ser servido (nem regravado).
fn forget_all_rotations() {
    if let Ok(mut all) = ROTATIONS.lock() {
        all.clear();
    }
}

/// Por que apagar a sessão falhou.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ClearError {
    /// O chaveiro não deixou apagar.
    #[error("chaveiro: {0}")]
    Keyring(String),
    #[error("{0}")]
    Other(String),
}

/// O par de tokens. Nunca vai para o log: o `Debug` esconde os valores.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tokens(..)")
    }
}

/// O que vai para o chaveiro: o par mais o carimbo de gravação.
#[derive(Serialize, Deserialize)]
struct Saved {
    #[serde(flatten)]
    tokens: Tokens,
    #[serde(default)]
    saved_at: i64,
}

/// O par guardado, com o carimbo. Num chaveiro trancado do Linux cada leitura
/// pode pedir a senha, então quem chama lê uma vez só.
pub struct Loaded {
    pub tokens: Tokens,
    pub saved_at: i64,
}

/// Carimbo em milissegundos, estritamente crescente neste processo (duas
/// gravações no mesmo milissegundo não empatam).
fn next_stamp() -> i64 {
    static LAST: AtomicI64 = AtomicI64::new(0);
    let now = now_millis();
    let mut prev = LAST.load(Ordering::SeqCst);
    loop {
        let next = now.max(prev + 1);
        match LAST.compare_exchange(prev, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return next,
            Err(actual) => prev = actual,
        }
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Segundos desde a época Unix.
pub(crate) fn now_secs() -> i64 {
    now_millis() / 1000
}

fn parse_saved(raw: &str) -> Option<Loaded> {
    match serde_json::from_str::<Saved>(raw) {
        Ok(s) => Some(Loaded {
            tokens: s.tokens,
            saved_at: s.saved_at,
        }),
        Err(_) => {
            log::warn!("alvanista: sessão guardada ilegível, ignorada");
            None
        }
    }
}

/// Último perfil lido do Alvanista, para mostrar sem rede.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirror {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
    /// Capa do perfil (o banner do drawer de perfil). `default`: espelhos
    /// gravados antes de a capa entrar não têm o campo.
    #[serde(default)]
    pub hero_url: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    pub profile_url: String,
    /// Segundos desde a época Unix.
    pub fetched_at: i64,
    /// O cartão do perfil. `default`: espelhos de antes do cartão não têm o campo.
    #[serde(default)]
    pub card: Option<serde_json::Value>,
    /// O plano libera o sync de saves? `None` em espelhos de antes do campo.
    #[serde(default)]
    pub save_sync: Option<bool>,
}

pub type SharedSecrets = Arc<dyn SecretStore + Send + Sync>;

pub struct Store {
    repo: AlvanistaRepo,
    secrets: SharedSecrets,
}

impl Store {
    pub fn new(db: db::Db, secrets: SharedSecrets) -> Self {
        Self {
            repo: AlvanistaRepo::new(db),
            secrets,
        }
    }

    /// Lê o par. Erro de leitura do chaveiro é erro, não "sem conexão": quem
    /// chama não pode tratar uma falha como desconectado.
    pub async fn load(&self) -> Result<Option<Loaded>, String> {
        match self.secrets.get(ACCOUNT) {
            Ok(Some(raw)) => Ok(parse_saved(&raw)),
            Ok(None) => Ok(None),
            Err(e) => {
                log::warn!("alvanista: chaveiro indisponível para ler a sessão: {e}");
                Err(e)
            }
        }
    }

    /// `None` se não há conexão. Um par ilegível conta como ausente (o
    /// usuário conecta de novo).
    pub async fn load_tokens(&self) -> Result<Option<Tokens>, String> {
        Ok(self.load().await?.map(|l| l.tokens))
    }

    /// Começa uma sessão nova (depois do `exchange`): sob `REFRESH_LOCK`,
    /// esquece as rotações de qualquer sessão anterior, marca a sessão e grava
    /// o par. É o caminho do fluxo de conectar.
    ///
    /// O marcador vem antes do par: se o app cair entre os dois, sobra um
    /// marcador sem sessão, que a próxima leitura apaga. Na ordem inversa
    /// sobraria uma sessão que ninguém lê.
    ///
    /// O espelho anterior sai antes de tudo: a conta nova pode ser outra, e o
    /// perfil de uma conta não aparece na sessão de outra.
    ///
    /// `scopes` são os escopos que o usuário aprovou; ficam em `scopes`.
    pub async fn start_session(&self, tokens: &Tokens, scopes: &[&str]) -> Result<(), String> {
        let _one_at_a_time = REFRESH_LOCK.lock().await;
        forget_all_rotations();
        if let Err(e) = self.mirror_clear().await {
            log::warn!("alvanista: espelho anterior não apagado: {e}");
        }
        self.repo
            .state_set(SESSION_KEY, "1")
            .await
            .map_err(|e| e.to_string())?;
        if let Err(e) = self.save_tokens(tokens).await {
            // Sem chaveiro não há sessão: o marcador não fica apontando para nada.
            if let Err(e) = self.unmark_session().await {
                log::warn!("alvanista: marcador de sessão não apagado: {e}");
            }
            return Err(e);
        }
        // Os escopos só informam a tela; sem eles vale `profile:read`.
        if let Err(e) = self.repo.state_set(SCOPES_KEY, &scopes.join(" ")).await {
            log::warn!("alvanista: escopos não gravados: {e}");
        }
        // A data só informa: se não gravar, a conexão vale do mesmo jeito.
        if let Err(e) = self
            .repo
            .state_set(CONNECTED_AT_KEY, &now_secs().to_string())
            .await
        {
            log::warn!("alvanista: data da conexão não gravada: {e}");
        }
        Ok(())
    }

    /// Troca o par de uma sessão que continua (a conta pediu mais um escopo).
    /// No Alvanista o par novo é da mesma família do guardado (mesma concessão,
    /// mesmo `device_id`), então o antigo não se revoga: revogar um encerra os
    /// dois. Ele só sai do chaveiro e vence sozinho.
    ///
    /// O par novo vai primeiro: se o chaveiro recusar, a sessão anterior fica
    /// inteira (marcador, escopos, data, espelho e par) e segue valendo.
    pub async fn replace_session(&self, tokens: &Tokens, scopes: &[&str]) -> Result<(), String> {
        let _one_at_a_time = REFRESH_LOCK.lock().await;
        self.save_tokens(tokens).await?;
        // Nenhuma rotação do par antigo volta a ser servida nem regravada.
        forget_all_rotations();
        // A aprovação pode ter vindo de outra conta: o perfil se relê.
        if let Err(e) = self.mirror_clear().await {
            log::warn!("alvanista: espelho anterior não apagado: {e}");
        }
        self.repo
            .state_set(SESSION_KEY, "1")
            .await
            .map_err(|e| e.to_string())?;
        if let Err(e) = self.repo.state_set(SCOPES_KEY, &scopes.join(" ")).await {
            log::warn!("alvanista: escopos não gravados: {e}");
        }
        Ok(())
    }

    /// Os escopos da sessão. Sessão de antes do registro (ou sem ele) conta
    /// como só `profile:read`.
    pub async fn scopes(&self) -> Result<Vec<String>, String> {
        let raw = self
            .repo
            .state_get(SCOPES_KEY)
            .await
            .map_err(|e| e.to_string())?;
        let list: Vec<String> = raw
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        if list.is_empty() {
            return Ok(vec![super::config::SCOPE_PROFILE.to_owned()]);
        }
        Ok(list)
    }

    /// Quando esta conexão começou; `None` em sessões de antes da data.
    pub async fn connected_at(&self) -> Result<Option<i64>, String> {
        let raw = self
            .repo
            .state_get(CONNECTED_AT_KEY)
            .await
            .map_err(|e| e.to_string())?;
        Ok(raw.and_then(|v| v.parse().ok()))
    }

    /// `true` se há sessão marcada. Só lê o SQLite.
    pub async fn has_session(&self) -> Result<bool, String> {
        self.repo
            .state_get(SESSION_KEY)
            .await
            .map(|v| v.is_some())
            .map_err(|e| e.to_string())
    }

    /// A sessão acabou (desconectou, o servidor a encerrou, ou o marcador
    /// ficou sem par): sai o marcador.
    pub async fn unmark_session(&self) -> Result<(), String> {
        if let Err(e) = self.repo.state_delete(CONNECTED_AT_KEY).await {
            log::warn!("alvanista: data da conexão não apagada: {e}");
        }
        if let Err(e) = self.repo.state_delete(SCOPES_KEY).await {
            log::warn!("alvanista: escopos não apagados: {e}");
        }
        self.repo
            .state_delete(SESSION_KEY)
            .await
            .map_err(|e| e.to_string())
    }

    /// Só para dentro do módulo: a renovação (que já segura `REFRESH_LOCK`) e
    /// `start_session`. Quem conecta usa `start_session`; gravar por fora do
    /// lock poderia pisar numa renovação em andamento.
    pub(super) async fn save_tokens(&self, tokens: &Tokens) -> Result<(), String> {
        let json = serde_json::to_string(&Saved {
            tokens: tokens.clone(),
            saved_at: next_stamp(),
        })
        .map_err(|e| e.to_string())?;
        self.secrets.set(ACCOUNT, &json)
    }

    /// Apaga a sessão, esperando uma renovação em andamento terminar.
    pub async fn clear_tokens(&self) -> Result<(), ClearError> {
        let _one_at_a_time = REFRESH_LOCK.lock().await;
        self.clear_tokens_unlocked().await
    }

    /// Para quem já segura `REFRESH_LOCK`. Com o par apagado, sai também o
    /// marcador; se o chaveiro recusou, ele fica (uma cópia viva pode ter
    /// ficado lá) e quem chama decide.
    pub(super) async fn clear_tokens_unlocked(&self) -> Result<(), ClearError> {
        // Antes de qualquer leitura: mesmo que o chaveiro falhe aqui, nada
        // da sessão que acabou pode sobreviver na memória.
        forget_all_rotations();
        self.secrets.delete(ACCOUNT).map_err(ClearError::Keyring)?;
        self.unmark_session().await.map_err(ClearError::Other)
    }

    /// Identificador deste aparelho (16 bytes aleatórios em base64url),
    /// gerado na primeira vez e mantido; o Alvanista separa as sessões por
    /// aparelho com ele. Chamadas simultâneas na primeira vez concordam.
    pub async fn device_id(&self) -> Result<String, String> {
        if let Some(id) = self
            .repo
            .state_get(DEVICE_ID_KEY)
            .await
            .map_err(|e| e.to_string())?
        {
            return Ok(id);
        }
        self.repo
            .state_insert_if_absent(DEVICE_ID_KEY, &random_token(16))
            .await
            .map_err(|e| e.to_string())?;
        self.repo
            .state_get(DEVICE_ID_KEY)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "device_id sumiu depois de gravado".to_owned())
    }

    pub async fn mirror_get(&self) -> Result<Option<Mirror>, String> {
        let raw = self
            .repo
            .state_get(MIRROR_KEY)
            .await
            .map_err(|e| e.to_string())?;
        Ok(raw.and_then(|raw| serde_json::from_str(&raw).ok()))
    }

    /// Grava o espelho e devolve o que ficou. Um `/me` sem cartão mantém o
    /// guardado, só se for da mesma conta.
    pub async fn mirror_put(&self, me: &Me) -> Result<Mirror, String> {
        let previous = self.mirror_get().await?.filter(|old| old.id == me.id);
        let card = match &me.card {
            Some(card) => Some(card.clone()),
            None => previous.as_ref().and_then(|old| old.card.clone()),
        };
        // Um `/me` sem `entitlements` mantém o que se sabia da mesma conta.
        let save_sync = match me.entitlements {
            Some(e) => Some(e.save_sync),
            None => previous.and_then(|old| old.save_sync),
        };
        let mirror = Mirror {
            id: me.id.clone(),
            username: me.username.clone(),
            avatar_url: me.avatar_url.clone(),
            hero_url: me.hero_url.clone(),
            bio: me.bio.clone(),
            profile_url: me.profile_url.clone(),
            fetched_at: now_secs(),
            card,
            save_sync,
        };
        let json = serde_json::to_string(&mirror).map_err(|e| e.to_string())?;
        self.repo
            .state_set(MIRROR_KEY, &json)
            .await
            .map_err(|e| e.to_string())?;
        Ok(mirror)
    }

    /// Muda só o `save_sync` do espelho guardado (o servidor disse o plano por
    /// outra rota que não o `/me`). Sem espelho não há o que mudar.
    pub async fn mirror_set_save_sync(&self, on: bool) -> Result<(), String> {
        let Some(mut mirror) = self.mirror_get().await? else {
            return Ok(());
        };
        mirror.save_sync = Some(on);
        let json = serde_json::to_string(&mirror).map_err(|e| e.to_string())?;
        self.repo
            .state_set(MIRROR_KEY, &json)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn mirror_clear(&self) -> Result<(), String> {
        self.repo
            .state_delete(MIRROR_KEY)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    use std::sync::Mutex;

    /// Chaveiro em memória; `broken` simula "sem Secret Service". `events`
    /// (opcional) registra cada gravação, para provar ordem.
    #[derive(Default)]
    pub struct MemSecrets {
        pub secrets: Mutex<HashMap<String, String>>,
        pub broken: bool,
        /// Leitura funciona, mas gravar e apagar falham.
        pub readonly: AtomicBool,
        /// A próxima gravação falha (uma só); a leitura segue funcionando.
        pub fail_next_set: AtomicBool,
        pub events: Option<Arc<Mutex<Vec<String>>>>,
        /// Quantas leituras o chaveiro recebeu (cada uma pode pedir a senha
        /// num chaveiro trancado).
        pub gets: AtomicUsize,
        /// Quantas chamadas de qualquer tipo (ler, gravar, apagar).
        pub calls: AtomicUsize,
    }

    impl SecretStore for MemSecrets {
        fn get(&self, account: &str) -> Result<Option<String>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.gets.fetch_add(1, Ordering::SeqCst);
            if self.broken {
                return Err("sem Secret Service".into());
            }
            Ok(self.secrets.lock().unwrap().get(account).cloned())
        }
        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.broken
                || self.readonly.load(Ordering::SeqCst)
                || self.fail_next_set.swap(false, Ordering::SeqCst)
            {
                return Err("sem Secret Service".into());
            }
            if let Some(events) = &self.events {
                events.lock().unwrap().push("save".into());
            }
            self.secrets
                .lock()
                .unwrap()
                .insert(account.into(), secret.into());
            Ok(())
        }
        fn delete(&self, account: &str) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.broken || self.readonly.load(Ordering::SeqCst) {
                return Err("sem Secret Service".into());
            }
            self.secrets.lock().unwrap().remove(account);
            Ok(())
        }
    }

    pub async fn store_with(secrets: MemSecrets) -> (Store, Arc<MemSecrets>) {
        let (store, secrets, _) = store_and_db(secrets).await;
        (store, secrets)
    }

    /// Como `store_with`, mais o banco (para conferir outras tabelas).
    pub async fn store_and_db(secrets: MemSecrets) -> (Store, Arc<MemSecrets>, db::Db) {
        let secrets = Arc::new(secrets);
        let db = db::connect_in_memory().await.unwrap();
        (Store::new(db.clone(), secrets.clone()), secrets, db)
    }

    /// Marca a sessão sem gravar par (como um chaveiro que não abre).
    pub async fn mark_session(db: &db::Db) {
        AlvanistaRepo::new(db.clone())
            .state_set(SESSION_KEY, "1")
            .await
            .unwrap();
    }

    /// Os testes que mexem na memória de rotações (que é do processo) ou
    /// desconectam rodam um de cada vez.
    pub static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    pub fn tokens(n: &str) -> Tokens {
        Tokens {
            access_token: format!("alp_{n}"),
            refresh_token: format!("alr_{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[tokio::test]
    async fn store_keeps_tokens_in_keyring_when_it_works() {
        let _serial = SERIAL.lock().await;
        let (store, secrets) = store_with(MemSecrets::default()).await;
        assert_eq!(store.load_tokens().await.unwrap(), None);
        store.save_tokens(&tokens("a")).await.unwrap();
        assert_eq!(store.load_tokens().await.unwrap(), Some(tokens("a")));
        assert!(secrets.secrets.lock().unwrap().contains_key(ACCOUNT));
        store.clear_tokens().await.unwrap();
        assert_eq!(store.load_tokens().await.unwrap(), None);
    }

    #[tokio::test]
    async fn no_session_starts_without_the_keyring() {
        let _serial = SERIAL.lock().await;
        let (store, _, db) = store_and_db(MemSecrets {
            broken: true,
            ..Default::default()
        })
        .await;
        assert!(store
            .start_session(&tokens("a"), &["profile:read"])
            .await
            .is_err());
        assert!(!store.has_session().await.unwrap());
        // Só o marcador e o espelho iriam ao banco, e nem o marcador ficou.
        assert_eq!(
            AlvanistaRepo::new(db).state_get(SESSION_KEY).await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn device_id_is_generated_once_and_url_safe() {
        let (store, _) = store_with(MemSecrets::default()).await;
        let id = store.device_id().await.unwrap();
        assert_eq!(id.len(), 22);
        assert!(id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
        assert_eq!(store.device_id().await.unwrap(), id);
    }

    #[tokio::test]
    async fn mirror_round_trips_and_clears() {
        let (store, _) = store_with(MemSecrets::default()).await;
        assert_eq!(store.mirror_get().await.unwrap(), None);
        let me = Me {
            id: "alva_1".into(),
            username: "toy".into(),
            avatar_url: None,
            hero_url: Some("https://x/hero.png".into()),
            bio: Some("oi".into()),
            profile_url: "https://www.alvanista.com/users/toy".into(),
            card: None,
            entitlements: None,
        };
        store.mirror_put(&me).await.unwrap();
        let m = store.mirror_get().await.unwrap().unwrap();
        assert_eq!((m.id.as_str(), m.username.as_str()), ("alva_1", "toy"));
        assert_eq!(m.avatar_url, None);
        assert_eq!(m.hero_url.as_deref(), Some("https://x/hero.png"));
        assert_eq!(m.bio.as_deref(), Some("oi"));
        assert!(m.fetched_at > 0);
        store.mirror_clear().await.unwrap();
        assert_eq!(store.mirror_get().await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_mirror_saved_before_the_cover_and_bio_still_reads() {
        let (store, _) = store_with(MemSecrets::default()).await;
        store
            .repo
            .state_set(
                MIRROR_KEY,
                r#"{"id":"alva_1","username":"toy","avatar_url":null,"profile_url":"https://www.alvanista.com/users/toy","fetched_at":1}"#,
            )
            .await
            .unwrap();
        let m = store.mirror_get().await.unwrap().unwrap();
        assert_eq!(m.username, "toy");
        assert_eq!((m.hero_url, m.bio), (None, None));
    }

    #[tokio::test]
    async fn a_me_without_a_card_keeps_the_card_only_for_the_same_account() {
        let (store, _) = store_with(MemSecrets::default()).await;
        let me = |id: &str, card| Me {
            id: id.into(),
            username: "toy".into(),
            avatar_url: None,
            hero_url: None,
            bio: None,
            profile_url: "https://www.alvanista.com/users/toy".into(),
            card,
            entitlements: None,
        };
        let card = serde_json::json!({ "member_since": 2010 });
        store
            .mirror_put(&me("a", Some(card.clone())))
            .await
            .unwrap();
        let kept = store.mirror_put(&me("a", None)).await.unwrap();
        assert_eq!(kept.card, Some(card));
        let other = store.mirror_put(&me("b", None)).await.unwrap();
        assert_eq!(other.card, None);
        assert_eq!(store.mirror_get().await.unwrap().unwrap().card, None);
    }

    #[tokio::test]
    async fn a_new_session_starts_without_the_previous_account() {
        let _serial = SERIAL.lock().await;
        let (store, _) = store_with(MemSecrets::default()).await;
        store
            .mirror_put(&Me {
                id: "a".into(),
                username: "antiga".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/users/antiga".into(),
                card: None,
                entitlements: None,
            })
            .await
            .unwrap();
        store
            .start_session(&tokens("n1"), &["profile:read"])
            .await
            .unwrap();
        assert_eq!(store.mirror_get().await.unwrap(), None);
        assert!(store.connected_at().await.unwrap().unwrap() > 1_700_000_000);
        store.clear_tokens().await.unwrap();
        assert_eq!(store.connected_at().await.unwrap(), None);
    }

    #[tokio::test]
    async fn keyring_read_error_is_an_error() {
        let (store, _) = store_with(MemSecrets {
            broken: true,
            ..Default::default()
        })
        .await;
        assert!(store.load_tokens().await.is_err());
    }

    #[tokio::test]
    async fn clear_fails_when_the_keyring_entry_cannot_be_deleted() {
        let _serial = SERIAL.lock().await;
        let (store, secrets) = store_with(MemSecrets::default()).await;
        store.save_tokens(&tokens("a")).await.unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);
        assert!(store.clear_tokens().await.is_err());
        // A cópia continua ali: não some em silêncio.
        assert_eq!(store.load_tokens().await.unwrap(), Some(tokens("a")));
    }

    #[test]
    fn tokens_debug_never_shows_the_values() {
        let shown = format!("{:?}", tokens("segredo"));
        assert!(
            !shown.contains("alp_") && !shown.contains("alr_"),
            "{shown}"
        );
    }

    #[tokio::test]
    async fn the_session_marker_follows_the_session() {
        let _serial = SERIAL.lock().await;
        let (store, _) = store_with(MemSecrets::default()).await;
        assert!(!store.has_session().await.unwrap());
        store
            .start_session(&tokens("m1"), &["profile:read"])
            .await
            .unwrap();
        assert!(store.has_session().await.unwrap());
        store.clear_tokens().await.unwrap();
        assert!(!store.has_session().await.unwrap());
    }

    #[tokio::test]
    async fn the_marker_stays_when_the_keyring_refuses_the_delete() {
        let _serial = SERIAL.lock().await;
        let (store, secrets) = store_with(MemSecrets::default()).await;
        store
            .start_session(&tokens("m2"), &["profile:read"])
            .await
            .unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);
        assert!(store.clear_tokens().await.is_err());
        // A cópia viva continua no chaveiro: a sessão não pode sumir da vista.
        assert!(store.has_session().await.unwrap());
    }

    #[tokio::test]
    async fn concurrent_first_device_id_calls_agree() {
        let (store, _) = store_with(MemSecrets::default()).await;
        let (a, b, c) = tokio::join!(store.device_id(), store.device_id(), store.device_id());
        let (a, b, c) = (a.unwrap(), b.unwrap(), c.unwrap());
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(store.device_id().await.unwrap(), a);
    }

    #[tokio::test]
    async fn start_session_records_scopes_and_scopes_default_to_profile() {
        let _serial = SERIAL.lock().await;
        let (store, _) = store_with(MemSecrets::default()).await;
        assert_eq!(store.scopes().await.unwrap(), vec!["profile:read"]);
        store
            .start_session(&tokens("sc1"), &["profile:read", "saves"])
            .await
            .unwrap();
        assert_eq!(store.scopes().await.unwrap(), vec!["profile:read", "saves"]);
        store
            .start_session(&tokens("sc2"), &["profile:read"])
            .await
            .unwrap();
        assert_eq!(store.scopes().await.unwrap(), vec!["profile:read"]);
    }

    #[tokio::test]
    async fn mirror_put_keeps_the_save_sync_entitlement() {
        let (store, _) = store_with(MemSecrets::default()).await;
        let mut me = Me {
            id: "alva_1".into(),
            username: "toy".into(),
            avatar_url: None,
            hero_url: None,
            bio: None,
            profile_url: "https://www.alvanista.com/users/toy".into(),
            card: None,
            entitlements: None,
        };
        assert_eq!(store.mirror_put(&me).await.unwrap().save_sync, None);
        me.entitlements = Some(super::super::client::Entitlements { save_sync: true });
        assert_eq!(store.mirror_put(&me).await.unwrap().save_sync, Some(true));
        assert_eq!(
            store.mirror_get().await.unwrap().unwrap().save_sync,
            Some(true)
        );
    }

    #[tokio::test]
    async fn a_me_without_entitlements_keeps_the_save_sync_of_the_same_account() {
        let (store, _) = store_with(MemSecrets::default()).await;
        let me = |id: &str, entitlements| Me {
            id: id.into(),
            username: "toy".into(),
            avatar_url: None,
            hero_url: None,
            bio: None,
            profile_url: "https://www.alvanista.com/users/toy".into(),
            card: None,
            entitlements,
        };
        let on = Some(super::super::client::Entitlements { save_sync: true });
        store.mirror_put(&me("a", on)).await.unwrap();
        assert_eq!(
            store.mirror_put(&me("a", None)).await.unwrap().save_sync,
            Some(true)
        );
        assert_eq!(
            store.mirror_put(&me("b", None)).await.unwrap().save_sync,
            None
        );
    }

    #[tokio::test]
    async fn ending_the_session_makes_scopes_read_as_profile_only_again() {
        let _serial = SERIAL.lock().await;
        let (store, _) = store_with(MemSecrets::default()).await;
        store
            .start_session(&tokens("e1"), &["profile:read", "saves"])
            .await
            .unwrap();
        assert_eq!(store.scopes().await.unwrap().len(), 2);
        store.unmark_session().await.unwrap();
        assert_eq!(store.scopes().await.unwrap(), vec!["profile:read"]);
    }
}
