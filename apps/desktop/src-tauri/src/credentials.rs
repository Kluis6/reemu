//! Senha do ScreenScraper no chaveiro do sistema (Secret Service/GNOME
//! Keyring/KWallet no Linux, Credential Manager no Windows, Keychain no
//! macOS), e não mais em texto puro no SQLite (`metadata_config.
//! screenscraper_password`).
//!
//! A coluna continua existindo por dois motivos:
//! - **migração**: senha salva antes desta mudança está lá; na primeira
//!   leitura ela vai pro chaveiro e a coluna é zerada;
//! - **fallback**: sem chaveiro disponível (sessão sem Secret Service rodando,
//!   ex.: WM minimalista sem gnome-keyring), a senha fica no banco como antes
//!   — o scraping continua funcionando, com um `warn!` no log.
//!
//! Todo acesso à config do scraper passa por `load_config`/`save_config`,
//! nunca direto pelo `MetadataRepository::{get,set}_config`.

use domain::metadata::{MetadataConfig, MetadataRepository};

const SERVICE: &str = "com.reemu.desktop";
const ACCOUNT: &str = "screenscraper";

/// Onde a senha mora. Trait só pra teste poder trocar o chaveiro real por um
/// em memória (o mock do crate `keyring` não compartilha estado entre
/// `Entry`s, não serve pra testar ida e volta).
pub trait SecretStore {
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, secret: &str) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

/// O chaveiro do sistema operacional.
pub struct OsKeyring;

impl OsKeyring {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
    }
}

impl SecretStore for OsKeyring {
    fn get(&self) -> Result<Option<String>, String> {
        match Self::entry()?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn set(&self, secret: &str) -> Result<(), String> {
        Self::entry()?
            .set_password(secret)
            .map_err(|e| e.to_string())
    }

    fn delete(&self) -> Result<(), String> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Config do scraper com a senha resolvida: do banco se ainda estiver lá
/// (migrando pro chaveiro no caminho), senão do chaveiro.
pub async fn load_config(
    repo: &impl MetadataRepository,
    store: &impl SecretStore,
) -> Result<MetadataConfig, String> {
    let mut cfg = repo.get_config().await.map_err(|e| e.to_string())?;
    if let Some(pw) = &cfg.screenscraper_password {
        match store.set(pw) {
            Ok(()) => {
                let cleared = MetadataConfig {
                    screenscraper_password: None,
                    ..cfg.clone()
                };
                repo.set_config(&cleared).await.map_err(|e| e.to_string())?;
                log::info!("scraper: senha migrada do banco pro chaveiro do sistema");
            }
            Err(e) => log::warn!("scraper: chaveiro indisponível, senha segue no banco: {e}"),
        }
        return Ok(cfg);
    }
    cfg.screenscraper_password = store.get().unwrap_or_else(|e| {
        log::warn!("scraper: não deu pra ler a senha do chaveiro: {e}");
        None
    });
    Ok(cfg)
}

/// Grava a config; a senha vai pro chaveiro (ou pro banco, se o chaveiro
/// falhar). Senha `None` apaga do chaveiro também.
pub async fn save_config(
    repo: &impl MetadataRepository,
    store: &impl SecretStore,
    mut cfg: MetadataConfig,
) -> Result<(), String> {
    match cfg.screenscraper_password.take() {
        Some(pw) => {
            if let Err(e) = store.set(&pw) {
                log::warn!("scraper: chaveiro indisponível, senha vai pro banco: {e}");
                cfg.screenscraper_password = Some(pw);
            }
        }
        None => {
            if let Err(e) = store.delete() {
                log::warn!("scraper: não deu pra apagar a senha do chaveiro: {e}");
            }
        }
    }
    repo.set_config(&cfg).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct MemStore {
        secret: RefCell<Option<String>>,
        broken: bool,
    }

    impl SecretStore for MemStore {
        fn get(&self) -> Result<Option<String>, String> {
            if self.broken {
                return Err("sem Secret Service".into());
            }
            Ok(self.secret.borrow().clone())
        }
        fn set(&self, secret: &str) -> Result<(), String> {
            if self.broken {
                return Err("sem Secret Service".into());
            }
            *self.secret.borrow_mut() = Some(secret.into());
            Ok(())
        }
        fn delete(&self) -> Result<(), String> {
            if self.broken {
                return Err("sem Secret Service".into());
            }
            *self.secret.borrow_mut() = None;
            Ok(())
        }
    }

    fn cfg(pw: Option<&str>) -> MetadataConfig {
        MetadataConfig {
            provider: "screenscraper".into(),
            screenscraper_user: Some("user".into()),
            screenscraper_password: pw.map(Into::into),
        }
    }

    async fn repo() -> db::MetadataRepo {
        db::MetadataRepo::new(db::connect_in_memory().await.unwrap())
    }

    #[tokio::test]
    async fn save_puts_password_in_keyring_not_db() {
        let (repo, store) = (repo().await, MemStore::default());
        save_config(&repo, &store, cfg(Some("segredo")))
            .await
            .unwrap();

        assert_eq!(
            repo.get_config().await.unwrap().screenscraper_password,
            None
        );
        assert_eq!(store.secret.borrow().as_deref(), Some("segredo"));
        let loaded = load_config(&repo, &store).await.unwrap();
        assert_eq!(loaded.screenscraper_password.as_deref(), Some("segredo"));
        assert_eq!(loaded.screenscraper_user.as_deref(), Some("user"));
    }

    #[tokio::test]
    async fn legacy_db_password_migrates_on_load() {
        let (repo, store) = (repo().await, MemStore::default());
        repo.set_config(&cfg(Some("antiga"))).await.unwrap(); // como antes do chaveiro

        let loaded = load_config(&repo, &store).await.unwrap();
        assert_eq!(loaded.screenscraper_password.as_deref(), Some("antiga"));
        assert_eq!(store.secret.borrow().as_deref(), Some("antiga"));
        let raw = repo.get_config().await.unwrap();
        assert_eq!(raw.screenscraper_password, None);
        assert_eq!(raw.screenscraper_user.as_deref(), Some("user"));
    }

    #[tokio::test]
    async fn clearing_password_deletes_from_keyring() {
        let (repo, store) = (repo().await, MemStore::default());
        save_config(&repo, &store, cfg(Some("x"))).await.unwrap();
        save_config(&repo, &store, cfg(None)).await.unwrap();

        assert_eq!(*store.secret.borrow(), None);
        assert_eq!(
            load_config(&repo, &store)
                .await
                .unwrap()
                .screenscraper_password,
            None
        );
    }

    #[tokio::test]
    async fn broken_keyring_falls_back_to_db() {
        let repo = repo().await;
        let store = MemStore {
            broken: true,
            ..Default::default()
        };
        save_config(&repo, &store, cfg(Some("fallback")))
            .await
            .unwrap();

        let raw = repo.get_config().await.unwrap();
        assert_eq!(raw.screenscraper_password.as_deref(), Some("fallback"));
        let loaded = load_config(&repo, &store).await.unwrap();
        assert_eq!(loaded.screenscraper_password.as_deref(), Some("fallback"));
    }

    /// Ida e volta no chaveiro REAL do SO (precisa de Secret Service rodando
    /// no Linux / Credential Manager no Windows). Fora do CI; restaura o que
    /// já estava salvo. `cargo test -p reemu-desktop --lib os_keyring -- --ignored`
    #[test]
    #[ignore]
    fn os_keyring_roundtrip() {
        let before = OsKeyring.get().expect("ler chaveiro");
        OsKeyring.set("teste-reemu").expect("gravar no chaveiro");
        assert_eq!(OsKeyring.get().unwrap().as_deref(), Some("teste-reemu"));
        match before {
            Some(p) => OsKeyring.set(&p).unwrap(),
            None => {
                OsKeyring.delete().unwrap();
                assert_eq!(OsKeyring.get().unwrap(), None);
            }
        }
    }
}
