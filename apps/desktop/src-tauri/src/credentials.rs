//! Segredos do scraper (senha do ScreenScraper, chave de API do TheGamesDB)
//! no chaveiro do sistema (Secret Service/GNOME Keyring/KWallet no Linux,
//! Credential Manager no Windows, Keychain no macOS), e não em texto puro no
//! SQLite (`metadata_config.screenscraper_password` / `.thegamesdb_api_key`).
//! Cada segredo é uma conta própria no chaveiro (`SECRETS`).
//!
//! As colunas continuam existindo por dois motivos:
//! - **migração**: segredo salvo antes do chaveiro está lá; na primeira
//!   leitura ele vai pro chaveiro e a coluna é zerada;
//! - **fallback**: sem chaveiro disponível (sessão sem Secret Service rodando,
//!   ex.: WM minimalista sem gnome-keyring), o segredo fica no banco como
//!   antes — o scraping continua funcionando, com um `warn!` no log.
//!
//! Todo acesso à config do scraper passa por `load_config`/`save_config`,
//! nunca direto pelo `MetadataRepository::{get,set}_config`.

use domain::metadata::{MetadataConfig, MetadataRepository};

const SERVICE: &str = "com.reemu.desktop";

fn screenscraper_password(c: &mut MetadataConfig) -> &mut Option<String> {
    &mut c.screenscraper_password
}

fn thegamesdb_api_key(c: &mut MetadataConfig) -> &mut Option<String> {
    &mut c.thegamesdb_api_key
}

/// Conta no chaveiro → campo do `MetadataConfig` que ela guarda.
type SecretField = fn(&mut MetadataConfig) -> &mut Option<String>;
const SECRETS: &[(&str, SecretField)] = &[
    ("screenscraper", screenscraper_password),
    ("thegamesdb", thegamesdb_api_key),
];

/// Onde os segredos moram. Trait só pra teste poder trocar o chaveiro real
/// por um em memória (o mock do crate `keyring` não compartilha estado entre
/// `Entry`s, não serve pra testar ida e volta).
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

/// Config do scraper com os segredos resolvidos: do banco se ainda estiverem
/// lá (migrando pro chaveiro no caminho), senão do chaveiro.
pub async fn load_config(
    repo: &impl MetadataRepository,
    store: &impl SecretStore,
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
    store: &impl SecretStore,
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
            .unwrap(); // como antes do chaveiro

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

    /// Ida e volta no chaveiro REAL do SO (precisa de Secret Service rodando
    /// no Linux / Credential Manager no Windows). Fora do CI; usa uma conta
    /// só de teste, não mexe nos segredos de verdade.
    /// `cargo test -p reemu-desktop --lib os_keyring -- --ignored`
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
