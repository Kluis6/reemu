//! Segredos do RetroAchievements (`connect_token`) e dos provedores de
//! metadados (senha do ScreenScraper, chave de API do TheGamesDB) no chaveiro
//! do sistema (Secret Service/GNOME Keyring/KWallet no Linux, Credential
//! Manager no Windows, Keychain no macOS), nunca em texto puro em logs ou
//! diagnósticos.
//!
//! As colunas legadas em `metadata_config` continuam existindo por dois motivos:
//! - **migração**: segredo salvo antes do chaveiro está lá; na primeira
//!   leitura ele vai pro chaveiro e a coluna é zerada;
//! - **fallback**: sem chaveiro disponível para o scraper, o segredo fica no
//!   banco como antes, com `warn!` no log. O token do RetroAchievements exige
//!   chaveiro funcional (como os tokens do Alvanista).

use db::AlvanistaRepo;
use domain::metadata::{MetadataConfig, MetadataRepository};
use serde::{Deserialize, Serialize};

const SERVICE: &str = "com.reemu.desktop";

pub const ACCOUNT_SCREENSCRAPER: &str = "screenscraper";
pub const ACCOUNT_THEGAMESDB: &str = "thegamesdb";
pub const ACCOUNT_RETROACHIEVEMENTS: &str = "retroachievements";

const RA_CONFIG_KEY: &str = "ra_config";

fn screenscraper_password(c: &mut MetadataConfig) -> &mut Option<String> {
    &mut c.screenscraper_password
}

fn thegamesdb_api_key(c: &mut MetadataConfig) -> &mut Option<String> {
    &mut c.thegamesdb_api_key
}

/// Conta no chaveiro -> campo do `MetadataConfig` que ela guarda.
type SecretField = fn(&mut MetadataConfig) -> &mut Option<String>;
const SECRETS: &[(&str, SecretField)] = &[
    (ACCOUNT_SCREENSCRAPER, screenscraper_password),
    (ACCOUNT_THEGAMESDB, thegamesdb_api_key),
];

/// Onde os segredos moram. Trait permite trocar o chaveiro real por um em
/// memória nos testes.
pub trait SecretStore {
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn set(&self, account: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// O chaveiro do sistema operacional.
pub struct OsKeyring;

impl OsKeyring {
    fn entry(account: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, account).map_err(|e| e.to_string())
    }
}

impl SecretStore for OsKeyring {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        match Self::entry(account)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        Self::entry(account)?
            .set_password(secret)
            .map_err(|e| e.to_string())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Estado local não-secreto do RetroAchievements. O `connect_token` vive só no
/// chaveiro do SO (`ACCOUNT_RETROACHIEVEMENTS`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaConfig {
    pub enabled: bool,
    pub hardcore: bool,
    #[serde(default)]
    pub unofficial: bool,
    #[serde(default)]
    pub encore: bool,
    pub username: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub score_softcore: u32,
    /// Marcador de sessão ativa no chaveiro: sem ele não lemos o chaveiro.
    #[serde(default)]
    pub connected: bool,
    /// Usuário sugerido quando a conta do Alvanista já vinculou `retroachievements`.
    #[serde(default)]
    pub linked_alvanista_username: Option<String>,
}

impl Default for RaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hardcore: false,
            unofficial: false,
            encore: false,
            username: None,
            display_name: None,
            avatar_url: None,
            score: 0,
            score_softcore: 0,
            connected: false,
            linked_alvanista_username: None,
        }
    }
}

/// Config do scraper com os segredos resolvidos: do banco se ainda estiverem
/// lá (migrando pro chaveiro no caminho), senão do chaveiro.
pub async fn load_config(
    repo: &impl MetadataRepository,
    store: &(impl SecretStore + ?Sized),
) -> Result<MetadataConfig, String> {
    let mut cfg = repo.get_config().await.map_err(|e| e.to_string())?;
    let mut cleared = cfg.clone();
    let mut migrated = false;
    for (account, field) in SECRETS {
        if let Some(secret) = field(&mut cfg).clone() {
            match store.set(account, &secret) {
                Ok(()) => {
                    *field(&mut cleared) = None;
                    migrated = true;
                    log::info!("scraper: segredo `{account}` migrado do banco pro chaveiro");
                }
                Err(e) => {
                    log::warn!("scraper: chaveiro indisponível, `{account}` segue no banco: {e}")
                }
            }
            continue;
        }
        *field(&mut cfg) = store.get(account).unwrap_or_else(|e| {
            log::warn!("scraper: não deu pra ler `{account}` do chaveiro: {e}");
            None
        });
    }
    if migrated {
        repo.set_config(&cleared).await.map_err(|e| e.to_string())?;
    }
    Ok(cfg)
}

/// Grava a config; cada segredo vai pro chaveiro (ou pro banco, se o chaveiro
/// falhar). Segredo `None` apaga do chaveiro também.
pub async fn save_config(
    repo: &impl MetadataRepository,
    store: &(impl SecretStore + ?Sized),
    mut cfg: MetadataConfig,
) -> Result<(), String> {
    for (account, field) in SECRETS {
        match field(&mut cfg).take() {
            Some(secret) => {
                if let Err(e) = store.set(account, &secret) {
                    log::warn!("scraper: chaveiro indisponível, `{account}` vai pro banco: {e}");
                    *field(&mut cfg) = Some(secret);
                }
            }
            None => {
                if let Err(e) = store.delete(account) {
                    log::warn!("scraper: não deu pra apagar `{account}` do chaveiro: {e}");
                }
            }
        }
    }
    repo.set_config(&cfg).await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// RetroAchievements (mesmo chaveiro `OsKeyring`, estado em `alvanista_state`)
// ---------------------------------------------------------------------------

pub async fn load_ra_config(pool: &db::Db) -> Result<RaConfig, String> {
    let raw = AlvanistaRepo::new(pool.clone())
        .state_get(RA_CONFIG_KEY)
        .await
        .map_err(|e| e.to_string())?;
    Ok(raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

async fn write_ra_config(pool: &db::Db, cfg: &RaConfig) -> Result<(), String> {
    let json = serde_json::to_string(cfg).map_err(|e| e.to_string())?;
    AlvanistaRepo::new(pool.clone())
        .state_set(RA_CONFIG_KEY, &json)
        .await
        .map_err(|e| e.to_string())
}

/// Lê a config e o `connect_token` quando `connected` está marcado. Se o
/// marcador estiver órfão (token sumiu do chaveiro), desmarca `connected`.
pub async fn load_ra_session(
    pool: &db::Db,
    store: &(impl SecretStore + ?Sized),
) -> Result<(RaConfig, Option<String>), String> {
    let mut cfg = load_ra_config(pool).await?;
    if !cfg.connected {
        return Ok((cfg, None));
    }
    match store.get(ACCOUNT_RETROACHIEVEMENTS)? {
        Some(tok) if !tok.trim().is_empty() => Ok((cfg, Some(tok))),
        _ => {
            cfg.connected = false;
            let _ = write_ra_config(pool, &cfg).await;
            Ok((cfg, None))
        }
    }
}

/// Persiste o resultado de um login no RetroAchievements: o `connect_token`
/// entra no chaveiro e os metadados públicos entram em `ra_config`.
#[allow(clippy::too_many_arguments)]
pub async fn save_ra_login(
    pool: &db::Db,
    store: &(impl SecretStore + ?Sized),
    username: &str,
    display_name: Option<String>,
    avatar_url: Option<String>,
    score: u32,
    score_softcore: u32,
    connect_token: &str,
) -> Result<RaConfig, String> {
    store.set(ACCOUNT_RETROACHIEVEMENTS, connect_token)?;
    let prev = load_ra_config(pool).await.unwrap_or_default();
    let cfg = RaConfig {
        enabled: true,
        hardcore: prev.hardcore,
        unofficial: prev.unofficial,
        encore: prev.encore,
        username: Some(username.to_owned()),
        display_name: display_name.or_else(|| Some(username.to_owned())),
        avatar_url,
        score,
        score_softcore,
        connected: true,
        linked_alvanista_username: prev.linked_alvanista_username,
    };
    write_ra_config(pool, &cfg).await?;
    Ok(cfg)
}

pub async fn update_ra_preferences(
    pool: &db::Db,
    enabled: Option<bool>,
    hardcore: Option<bool>,
    unofficial: Option<bool>,
    encore: Option<bool>,
) -> Result<RaConfig, String> {
    let mut cfg = load_ra_config(pool).await?;
    if let Some(v) = enabled {
        cfg.enabled = v;
    }
    if let Some(v) = hardcore {
        cfg.hardcore = v;
    }
    if let Some(v) = unofficial {
        cfg.unofficial = v;
    }
    if let Some(v) = encore {
        cfg.encore = v;
    }
    write_ra_config(pool, &cfg).await?;
    Ok(cfg)
}

/// Remove o `connect_token` do chaveiro e limpa a sessão local.
pub async fn clear_ra_login(
    pool: &db::Db,
    store: &(impl SecretStore + ?Sized),
) -> Result<RaConfig, String> {
    store.delete(ACCOUNT_RETROACHIEVEMENTS)?;
    let prev = load_ra_config(pool).await.unwrap_or_default();
    let cfg = RaConfig {
        enabled: prev.enabled,
        hardcore: prev.hardcore,
        unofficial: prev.unofficial,
        encore: prev.encore,
        username: None,
        display_name: None,
        avatar_url: None,
        score: 0,
        score_softcore: 0,
        connected: false,
        linked_alvanista_username: prev.linked_alvanista_username,
    };
    write_ra_config(pool, &cfg).await?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemStore {
        secrets: RefCell<HashMap<String, String>>,
        broken: bool,
    }

    impl MemStore {
        fn secret(&self, account: &str) -> Option<String> {
            self.secrets.borrow().get(account).cloned()
        }
    }

    impl SecretStore for MemStore {
        fn get(&self, account: &str) -> Result<Option<String>, String> {
            if self.broken {
                return Err("sem Secret Service".into());
            }
            Ok(self.secret(account))
        }
        fn set(&self, account: &str, secret: &str) -> Result<(), String> {
            if self.broken {
                return Err("sem Secret Service".into());
            }
            self.secrets
                .borrow_mut()
                .insert(account.into(), secret.into());
            Ok(())
        }
        fn delete(&self, account: &str) -> Result<(), String> {
            if self.broken {
                return Err("sem Secret Service".into());
            }
            self.secrets.borrow_mut().remove(account);
            Ok(())
        }
    }

    fn cfg(pw: Option<&str>, tgdb: Option<&str>) -> MetadataConfig {
        MetadataConfig {
            provider: "screenscraper".into(),
            screenscraper_user: Some("user".into()),
            screenscraper_password: pw.map(Into::into),
            thegamesdb_api_key: tgdb.map(Into::into),
        }
    }

    async fn repo() -> db::MetadataRepo {
        db::MetadataRepo::new(db::connect_in_memory().await.unwrap())
    }

    #[tokio::test]
    async fn save_puts_secrets_in_keyring_not_db() {
        let (repo, store) = (repo().await, MemStore::default());
        save_config(&repo, &store, cfg(Some("segredo"), Some("chave")))
            .await
            .unwrap();

        let raw = repo.get_config().await.unwrap();
        assert_eq!(raw.screenscraper_password, None);
        assert_eq!(raw.thegamesdb_api_key, None);
        assert_eq!(store.secret("screenscraper").as_deref(), Some("segredo"));
        assert_eq!(store.secret("thegamesdb").as_deref(), Some("chave"));
        let loaded = load_config(&repo, &store).await.unwrap();
        assert_eq!(loaded.screenscraper_password.as_deref(), Some("segredo"));
        assert_eq!(loaded.thegamesdb_api_key.as_deref(), Some("chave"));
        assert_eq!(loaded.screenscraper_user.as_deref(), Some("user"));
    }

    #[tokio::test]
    async fn legacy_db_secrets_migrate_on_load() {
        let (repo, store) = (repo().await, MemStore::default());
        repo.set_config(&cfg(Some("antiga"), Some("velha")))
            .await
            .unwrap();

        let loaded = load_config(&repo, &store).await.unwrap();
        assert_eq!(loaded.screenscraper_password.as_deref(), Some("antiga"));
        assert_eq!(loaded.thegamesdb_api_key.as_deref(), Some("velha"));
        assert_eq!(store.secret("screenscraper").as_deref(), Some("antiga"));
        assert_eq!(store.secret("thegamesdb").as_deref(), Some("velha"));
        let raw = repo.get_config().await.unwrap();
        assert_eq!(raw.screenscraper_password, None);
        assert_eq!(raw.thegamesdb_api_key, None);
        assert_eq!(raw.screenscraper_user.as_deref(), Some("user"));
    }

    #[tokio::test]
    async fn clearing_one_secret_keeps_the_other() {
        let (repo, store) = (repo().await, MemStore::default());
        save_config(&repo, &store, cfg(Some("x"), Some("k")))
            .await
            .unwrap();
        save_config(&repo, &store, cfg(None, Some("k")))
            .await
            .unwrap();

        assert_eq!(store.secret("screenscraper"), None);
        assert_eq!(store.secret("thegamesdb").as_deref(), Some("k"));
        let loaded = load_config(&repo, &store).await.unwrap();
        assert_eq!(loaded.screenscraper_password, None);
        assert_eq!(loaded.thegamesdb_api_key.as_deref(), Some("k"));
    }

    #[tokio::test]
    async fn broken_keyring_falls_back_to_db() {
        let repo = repo().await;
        let store = MemStore {
            broken: true,
            ..Default::default()
        };
        save_config(&repo, &store, cfg(Some("fallback"), Some("kf")))
            .await
            .unwrap();

        let raw = repo.get_config().await.unwrap();
        assert_eq!(raw.screenscraper_password.as_deref(), Some("fallback"));
        assert_eq!(raw.thegamesdb_api_key.as_deref(), Some("kf"));
        let loaded = load_config(&repo, &store).await.unwrap();
        assert_eq!(loaded.screenscraper_password.as_deref(), Some("fallback"));
        assert_eq!(loaded.thegamesdb_api_key.as_deref(), Some("kf"));
    }

    #[tokio::test]
    async fn ra_login_persists_token_in_keyring_and_logout_wipes_it() {
        let pool = db::connect_in_memory().await.unwrap();
        let store = MemStore::default();

        let saved = save_ra_login(
            &pool,
            &store,
            "edward",
            Some("Edward".into()),
            Some("https://media.retroachievements.org/UserPic/edward.png".into()),
            1420,
            300,
            "tok_secret_abc",
        )
        .await
        .unwrap();
        assert!(saved.connected && saved.enabled && !saved.hardcore);
        assert_eq!(
            store.secret(ACCOUNT_RETROACHIEVEMENTS).as_deref(),
            Some("tok_secret_abc")
        );

        let prefs = update_ra_preferences(&pool, None, Some(true), Some(true), None)
            .await
            .unwrap();
        assert!(prefs.hardcore && prefs.unofficial);

        let (loaded, tok) = load_ra_session(&pool, &store).await.unwrap();
        assert!(loaded.connected && loaded.hardcore);
        assert_eq!(tok.as_deref(), Some("tok_secret_abc"));

        let cleared = clear_ra_login(&pool, &store).await.unwrap();
        assert!(!cleared.connected);
        assert_eq!(store.secret(ACCOUNT_RETROACHIEVEMENTS), None);
    }

    #[test]
    #[ignore]
    fn os_keyring_roundtrip() {
        const ACCOUNT: &str = "teste-reemu";
        OsKeyring
            .set(ACCOUNT, "teste-reemu")
            .expect("gravar no chaveiro");
        assert_eq!(
            OsKeyring.get(ACCOUNT).unwrap().as_deref(),
            Some("teste-reemu")
        );
        OsKeyring.delete(ACCOUNT).unwrap();
        assert_eq!(OsKeyring.get(ACCOUNT).unwrap(), None);
    }
}
