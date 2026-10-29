//! O que fica no aparelho: o par de tokens (chaveiro do SO, com o SQLite de
//! reserva), o `device_id` e o espelho do último perfil lido.
//!
//! Os tokens seguem o mesmo caminho dos segredos do scraper
//! (`credentials::SecretStore`, conta `alvanista`, o par em JSON). Se o
//! chaveiro falha, o par vai para `secret_fallback`, e a leitura (`load`)
//! diz de onde ele veio, para a tela poder dizer que a sessão está menos
//! protegida.
//!
//! Cada cópia leva `saved_at` (milissegundos, crescente no processo). Como o
//! chaveiro pode falhar só numa gravação e continuar legível, as duas cópias
//! podem divergir; a leitura devolve sempre a mais nova, porque o refresh
//! token da mais velha já foi gasto no servidor.

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
    /// O chaveiro não deixou apagar (a reserva no SQLite já saiu).
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

/// O que vai para o chaveiro/banco: o par mais o carimbo de gravação.
#[derive(Serialize, Deserialize)]
struct Saved {
    #[serde(flatten)]
    tokens: Tokens,
    #[serde(default)]
    saved_at: i64,
}

/// O par guardado, com o carimbo e de onde veio. Uma leitura responde as
/// duas perguntas (há sessão? está no SQLite?): num chaveiro trancado do
/// Linux cada leitura pode pedir a senha, então quem chama lê uma vez só.
pub struct Loaded {
    pub tokens: Tokens,
    pub saved_at: i64,
    /// O par veio da reserva no SQLite (o chaveiro falhou).
    pub from_fallback: bool,
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

fn parse_saved(raw: &str, from_fallback: bool) -> Option<Loaded> {
    match serde_json::from_str::<Saved>(raw) {
        Ok(s) => Some(Loaded {
            tokens: s.tokens,
            saved_at: s.saved_at,
            from_fallback,
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

    /// Lê as duas cópias e fica com a mais nova (empate: o chaveiro).
    /// Erro de leitura do chaveiro com a reserva vazia é erro, não "sem
    /// conexão": quem chama não pode tratar uma falha como desconectado.
    pub async fn load(&self) -> Result<Option<Loaded>, String> {
        let (keyring, keyring_err) = match self.secrets.get(ACCOUNT) {
            Ok(Some(raw)) => (parse_saved(&raw, false), None),
            Ok(None) => (None, None),
            Err(e) => {
                log::warn!("alvanista: chaveiro indisponível para ler a sessão: {e}");
                (None, Some(e))
            }
        };
        let fallback = self
            .repo
            .fallback_get(ACCOUNT)
            .await
            .map_err(|e| e.to_string())?
            .and_then(|raw| parse_saved(&raw, true));
        match (keyring, fallback) {
            (Some(k), Some(f)) => Ok(Some(if f.saved_at > k.saved_at { f } else { k })),
            (Some(one), None) | (None, Some(one)) => Ok(Some(one)),
            (None, None) => keyring_err.map_or(Ok(None), Err),
        }
    }

    /// `None` se não há conexão. Um par ilegível conta como ausente (o
    /// usuário conecta de novo).
    pub async fn load_tokens(&self) -> Result<Option<Tokens>, String> {
        Ok(self.load().await?.map(|l| l.tokens))
    }

    /// Começa uma sessão nova (depois do `exchange`): sob `REFRESH_LOCK`,
    /// esquece as rotações de qualquer sessão anterior e grava o par. É o
    /// caminho do fluxo de conectar.
    pub async fn start_session(&self, tokens: &Tokens) -> Result<(), String> {
        let _one_at_a_time = REFRESH_LOCK.lock().await;
        forget_all_rotations();
        self.save_tokens(tokens).await
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
        match self.secrets.set(ACCOUNT, &json) {
            Ok(()) => {
                // Se a cópia velha da reserva não sair, ela perde pelo carimbo.
                if let Err(e) = self.repo.fallback_delete(ACCOUNT).await {
                    log::warn!("alvanista: reserva antiga não apagada: {e}");
                }
                Ok(())
            }
            Err(e) => {
                log::warn!("alvanista: chaveiro indisponível, a sessão vai para o banco: {e}");
                // O chaveiro pode ainda guardar o par anterior (gasto).
                let _ = self.secrets.delete(ACCOUNT);
                self.repo
                    .fallback_set(ACCOUNT, &json)
                    .await
                    .map_err(|e| e.to_string())
            }
        }
    }

    /// Apaga a sessão, esperando uma renovação em andamento terminar.
    pub async fn clear_tokens(&self) -> Result<(), ClearError> {
        let _one_at_a_time = REFRESH_LOCK.lock().await;
        self.clear_tokens_unlocked().await
    }

    /// Para quem já segura `REFRESH_LOCK`.
    pub(super) async fn clear_tokens_unlocked(&self) -> Result<(), ClearError> {
        // Antes de qualquer leitura: mesmo que o chaveiro falhe aqui, nada
        // da sessão que acabou pode sobreviver na memória.
        forget_all_rotations();
        let keyring = self.secrets.delete(ACCOUNT);
        // A reserva primeiro: `Keyring` promete que ela já saiu.
        self.repo
            .fallback_delete(ACCOUNT)
            .await
            .map_err(|e| ClearError::Other(e.to_string()))?;
        // Cópia que não saiu do chaveiro voltaria a carregar como conectado.
        keyring.map_err(ClearError::Keyring)
    }

    /// `true` se o par que `load_tokens` devolve está no SQLite porque o
    /// chaveiro falhou. Fora dos testes, quem precisa saber usa o
    /// `from_fallback` da leitura que já fez.
    #[cfg(test)]
    pub async fn using_fallback(&self) -> bool {
        matches!(self.load().await, Ok(Some(l)) if l.from_fallback)
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

    pub async fn mirror_put(&self, me: &Me) -> Result<(), String> {
        let mirror = Mirror {
            id: me.id.clone(),
            username: me.username.clone(),
            avatar_url: me.avatar_url.clone(),
            hero_url: me.hero_url.clone(),
            bio: me.bio.clone(),
            profile_url: me.profile_url.clone(),
            fetched_at: now_millis() / 1000,
        };
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
pub(super) mod test_support {
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
    }

    impl SecretStore for MemSecrets {
        fn get(&self, account: &str) -> Result<Option<String>, String> {
            self.gets.fetch_add(1, Ordering::SeqCst);
            if self.broken {
                return Err("sem Secret Service".into());
            }
            Ok(self.secrets.lock().unwrap().get(account).cloned())
        }
        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
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
            if self.broken || self.readonly.load(Ordering::SeqCst) {
                return Err("sem Secret Service".into());
            }
            self.secrets.lock().unwrap().remove(account);
            Ok(())
        }
    }

    pub async fn store_with(secrets: MemSecrets) -> (Store, Arc<MemSecrets>) {
        let secrets = Arc::new(secrets);
        let db = db::connect_in_memory().await.unwrap();
        (Store::new(db, secrets.clone()), secrets)
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
        assert!(!store.using_fallback().await);
        store.clear_tokens().await.unwrap();
        assert_eq!(store.load_tokens().await.unwrap(), None);
    }

    #[tokio::test]
    async fn store_falls_back_to_sqlite_when_keyring_fails() {
        let _serial = SERIAL.lock().await;
        let (store, _) = store_with(MemSecrets {
            broken: true,
            ..Default::default()
        })
        .await;
        store.save_tokens(&tokens("a")).await.unwrap();
        assert!(store.using_fallback().await);
        assert_eq!(store.load_tokens().await.unwrap(), Some(tokens("a")));
        store.save_tokens(&tokens("b")).await.unwrap();
        assert_eq!(store.load_tokens().await.unwrap(), Some(tokens("b")));
        // O chaveiro quebrado não deixa apagar: erro, mas a reserva sai.
        assert!(store.clear_tokens().await.is_err());
        assert!(!store.using_fallback().await);
    }

    #[tokio::test]
    async fn keyring_save_drops_a_stale_fallback_copy() {
        let db = db::connect_in_memory().await.unwrap();
        let broken = Store::new(
            db.clone(),
            Arc::new(MemSecrets {
                broken: true,
                ..Default::default()
            }),
        );
        broken.save_tokens(&tokens("old")).await.unwrap();
        let healthy = Store::new(db, Arc::new(MemSecrets::default()));
        healthy.save_tokens(&tokens("new")).await.unwrap();
        assert!(!healthy.using_fallback().await);
        assert_eq!(healthy.load_tokens().await.unwrap(), Some(tokens("new")));
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
    async fn newest_copy_wins_when_a_keyring_write_fails_once() {
        let (store, secrets) = store_with(MemSecrets::default()).await;
        store.save_tokens(&tokens("p1")).await.unwrap();
        secrets.fail_next_set.store(true, Ordering::SeqCst);
        store.save_tokens(&tokens("p2")).await.unwrap();

        assert_eq!(store.load_tokens().await.unwrap(), Some(tokens("p2")));
        assert!(store.using_fallback().await);
        // Uma gravação boa depois volta tudo para o chaveiro.
        store.save_tokens(&tokens("p3")).await.unwrap();
        assert_eq!(store.load_tokens().await.unwrap(), Some(tokens("p3")));
        assert!(!store.using_fallback().await);
    }

    #[tokio::test]
    async fn stale_keyring_copy_loses_even_if_it_cannot_be_deleted() {
        let (store, secrets) = store_with(MemSecrets::default()).await;
        store.save_tokens(&tokens("p1")).await.unwrap();
        secrets.readonly.store(true, Ordering::SeqCst);
        store.save_tokens(&tokens("p2")).await.unwrap();
        // O chaveiro ainda tem p1 (nem apagar deu), mas p2 é mais novo.
        assert_eq!(store.load_tokens().await.unwrap(), Some(tokens("p2")));
        assert!(store.using_fallback().await);
    }

    #[tokio::test]
    async fn keyring_read_error_with_empty_fallback_is_an_error() {
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

    #[tokio::test]
    async fn clear_reports_a_fallback_that_did_not_go_before_the_keyring() {
        let _serial = SERIAL.lock().await;
        // Chaveiro sem Secret Service e banco fechado: nenhuma das duas
        // cópias sai. `Keyring` diria que a reserva já saiu, e não saiu.
        let db = db::connect_in_memory().await.unwrap();
        db.close().await;
        let store = Store::new(
            db,
            Arc::new(MemSecrets {
                broken: true,
                ..Default::default()
            }),
        );
        assert!(matches!(
            store.clear_tokens().await,
            Err(ClearError::Other(_))
        ));
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
    async fn concurrent_first_device_id_calls_agree() {
        let (store, _) = store_with(MemSecrets::default()).await;
        let (a, b, c) = tokio::join!(store.device_id(), store.device_id(), store.device_id());
        let (a, b, c) = (a.unwrap(), b.unwrap(), c.unwrap());
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(store.device_id().await.unwrap(), a);
    }
}
