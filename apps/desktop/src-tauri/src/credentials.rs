//! Segredos de emuladores e provedores (token do RetroAchievements, senha do
//! ScreenScraper, chave de API do TheGamesDB) no chaveiro do sistema (Secret
//! Service/GNOME Keyring/KWallet no Linux, Credential Manager no Windows,
//! Keychain no macOS), nunca em texto puro em logs ou diagnósticos.
//!
//! As colunas legadas em `metadata_config` continuam existindo por dois motivos:
//! - **migração**: segredo salvo antes do chaveiro está lá; na primeira
//!   leitura ele vai pro chaveiro e a coluna é zerada;
//! - **fallback**: sem chaveiro disponível para o scraper, o segredo fica no
//!   banco como antes, com `warn!` no log. O token do RetroAchievements exige
//!   chaveiro funcional (como os tokens do Alvanista).
//!
//! Este módulo também mantém os carimbos por provedor (`updated_at`, `cleared`)
//! em `alvanista_state` (`credentials_sync_meta`) para reconciliar com o cofre
//! criptografado do Cloud Save (`/api/v1/saves/credentials`, D152-D157) sem
//! duplicar lógica de chaveiro em outros módulos.

use chrono::{SecondsFormat, Utc};
use db::AlvanistaRepo;
use domain::metadata::{MetadataConfig, MetadataRepository};
use serde::{Deserialize, Serialize};

const SERVICE: &str = "com.reemu.desktop";

pub const ACCOUNT_SCREENSCRAPER: &str = "screenscraper";
pub const ACCOUNT_THEGAMESDB: &str = "thegamesdb";
pub const ACCOUNT_RETROACHIEVEMENTS: &str = "retroachievements";

const RA_CONFIG_KEY: &str = "ra_config";
const SYNC_META_KEY: &str = "credentials_sync_meta";

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

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
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

/// Metadados locais por provedor para reconciliação LWW com o Cloud Save (D155).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSyncMeta {
    pub updated_at: Option<String>,
    #[serde(default)]
    pub cleared: bool,
    /// `true` quando mudou localmente e ainda precisa subir para a nuvem.
    #[serde(default)]
    pub dirty: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialsSyncMeta {
    #[serde(default)]
    pub retroachievements: ProviderSyncMeta,
    #[serde(default)]
    pub screenscraper: ProviderSyncMeta,
    #[serde(default)]
    pub thegamesdb: ProviderSyncMeta,
}

/// Formato de sincronização de cada provedor no cofre do Cloud Save (D152, D155).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncedRaSlot {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub connect_token: String,
    #[serde(default)]
    pub hardcore: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub cleared: bool,
    pub updated_at: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncedScreenScraperSlot {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub cleared: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncedTheGamesDbSlot {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub cleared: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncedCredentialsPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retroachievements: Option<SyncedRaSlot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenscraper: Option<SyncedScreenScraperSlot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thegamesdb: Option<SyncedTheGamesDbSlot>,
}

impl SyncedCredentialsPayload {
    pub fn is_empty(&self) -> bool {
        self.retroachievements.is_none()
            && self.screenscraper.is_none()
            && self.thegamesdb.is_none()
    }
}

pub async fn load_sync_meta(pool: &db::Db) -> Result<CredentialsSyncMeta, String> {
    let raw = AlvanistaRepo::new(pool.clone())
        .state_get(SYNC_META_KEY)
        .await
        .map_err(|e| e.to_string())?;
    Ok(raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

pub async fn save_sync_meta(pool: &db::Db, meta: &CredentialsSyncMeta) -> Result<(), String> {
    let json = serde_json::to_string(meta).map_err(|e| e.to_string())?;
    AlvanistaRepo::new(pool.clone())
        .state_set(SYNC_META_KEY, &json)
        .await
        .map_err(|e| e.to_string())
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

/// Grava a config do scraper e registra a mudança nos carimbos de sync quando
/// os valores realmente mudaram.
pub async fn save_config_tracked(
    pool: &db::Db,
    store: &(impl SecretStore + ?Sized),
    cfg: MetadataConfig,
) -> Result<(), String> {
    let repo = db::MetadataRepo::new(pool.clone());
    let prev = load_config(&repo, store).await.unwrap_or_default();
    let ss_changed = prev.screenscraper_user != cfg.screenscraper_user
        || prev.screenscraper_password != cfg.screenscraper_password;
    let tgdb_changed = prev.thegamesdb_api_key != cfg.thegamesdb_api_key;
    let ss_cleared = cfg.screenscraper_user.is_none() && cfg.screenscraper_password.is_none();
    let tgdb_cleared = cfg.thegamesdb_api_key.is_none();

    save_config(&repo, store, cfg).await?;

    if ss_changed || tgdb_changed {
        let mut meta = load_sync_meta(pool).await.unwrap_or_default();
        let stamp = now_rfc3339();
        if ss_changed {
            meta.screenscraper = ProviderSyncMeta {
                updated_at: Some(stamp.clone()),
                cleared: ss_cleared,
                dirty: true,
            };
        }
        if tgdb_changed {
            meta.thegamesdb = ProviderSyncMeta {
                updated_at: Some(stamp),
                cleared: tgdb_cleared,
                dirty: true,
            };
        }
        save_sync_meta(pool, &meta).await?;
    }
    Ok(())
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

    let mut meta = load_sync_meta(pool).await.unwrap_or_default();
    meta.retroachievements = ProviderSyncMeta {
        updated_at: Some(now_rfc3339()),
        cleared: false,
        dirty: true,
    };
    save_sync_meta(pool, &meta).await?;
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
    let prev_sync_fields = (cfg.enabled, cfg.hardcore);
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

    if cfg.connected && (cfg.enabled, cfg.hardcore) != prev_sync_fields {
        let mut meta = load_sync_meta(pool).await.unwrap_or_default();
        meta.retroachievements = ProviderSyncMeta {
            updated_at: Some(now_rfc3339()),
            cleared: false,
            dirty: true,
        };
        save_sync_meta(pool, &meta).await?;
    }
    Ok(cfg)
}

/// Remove o `connect_token` do chaveiro e grava um tombstone `cleared = true`
/// para propagar o logout às demais máquinas sincronizadas (D155).
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

    let mut meta = load_sync_meta(pool).await.unwrap_or_default();
    meta.retroachievements = ProviderSyncMeta {
        updated_at: Some(now_rfc3339()),
        cleared: true,
        dirty: true,
    };
    save_sync_meta(pool, &meta).await?;
    Ok(cfg)
}

pub async fn set_linked_alvanista_ra_username(
    pool: &db::Db,
    linked: Option<String>,
) -> Result<(), String> {
    let mut cfg = load_ra_config(pool).await?;
    let clean = linked.filter(|s| !s.trim().is_empty());
    if cfg.linked_alvanista_username != clean {
        cfg.linked_alvanista_username = clean;
        write_ra_config(pool, &cfg).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Reconciliação bidirecional com o cofre do Cloud Save (D155, D157)
// ---------------------------------------------------------------------------

/// Aplica um pacote remoto vindo de `GET /api/v1/saves/credentials` sobre o
/// chaveiro local usando LWW por provedor, e devolve os provedores locais mais
/// novos que devem subir num `PUT /api/v1/saves/credentials`.
///
/// Devolve `(to_upload, ra_changed)`.
pub async fn reconcile_with_remote(
    pool: &db::Db,
    store: &(impl SecretStore + ?Sized),
    remote: Option<&SyncedCredentialsPayload>,
    linked_ra_username: Option<String>,
) -> Result<(SyncedCredentialsPayload, bool), String> {
    set_linked_alvanista_ra_username(pool, linked_ra_username).await?;

    let mut meta = load_sync_meta(pool).await.unwrap_or_default();
    let meta_repo = db::MetadataRepo::new(pool.clone());
    let mut scraper = load_config(&meta_repo, store).await?;
    let (mut ra_cfg, mut ra_token) = load_ra_session(pool, store).await?;
    let mut ra_changed = false;
    let mut scraper_changed_from_remote = false;

    // 1. RetroAchievements
    if let Some(rem) = remote.and_then(|r| r.retroachievements.as_ref()) {
        let local_ts = meta.retroachievements.updated_at.as_deref().unwrap_or("");
        if local_ts.is_empty() || rem.updated_at.as_str() > local_ts {
            if rem.cleared {
                if ra_cfg.connected || ra_token.is_some() {
                    store.delete(ACCOUNT_RETROACHIEVEMENTS)?;
                    ra_cfg.connected = false;
                    ra_cfg.username = None;
                    ra_cfg.display_name = None;
                    ra_cfg.avatar_url = None;
                    ra_cfg.score = 0;
                    ra_cfg.score_softcore = 0;
                    write_ra_config(pool, &ra_cfg).await?;
                    ra_token = None;
                    ra_changed = true;
                }
            } else if !rem.username.is_empty() && !rem.connect_token.is_empty() {
                store.set(ACCOUNT_RETROACHIEVEMENTS, &rem.connect_token)?;
                ra_cfg.connected = true;
                ra_cfg.enabled = rem.enabled;
                ra_cfg.hardcore = rem.hardcore;
                if ra_cfg.username.as_deref() != Some(rem.username.as_str()) {
                    ra_cfg.display_name = Some(rem.username.clone());
                }
                ra_cfg.username = Some(rem.username.clone());
                write_ra_config(pool, &ra_cfg).await?;
                ra_token = Some(rem.connect_token.clone());
                ra_changed = true;
            }
            meta.retroachievements = ProviderSyncMeta {
                updated_at: Some(rem.updated_at.clone()),
                cleared: rem.cleared,
                dirty: false,
            };
        }
    }

    // 2. ScreenScraper
    if let Some(rem) = remote.and_then(|r| r.screenscraper.as_ref()) {
        let local_ts = meta.screenscraper.updated_at.as_deref().unwrap_or("");
        if local_ts.is_empty() || rem.updated_at.as_str() > local_ts {
            if rem.cleared {
                scraper.screenscraper_user = None;
                scraper.screenscraper_password = None;
            } else {
                scraper.screenscraper_user =
                    (!rem.username.is_empty()).then(|| rem.username.clone());
                scraper.screenscraper_password =
                    (!rem.password.is_empty()).then(|| rem.password.clone());
            }
            scraper_changed_from_remote = true;
            meta.screenscraper = ProviderSyncMeta {
                updated_at: Some(rem.updated_at.clone()),
                cleared: rem.cleared,
                dirty: false,
            };
        }
    }

    // 3. TheGamesDB
    if let Some(rem) = remote.and_then(|r| r.thegamesdb.as_ref()) {
        let local_ts = meta.thegamesdb.updated_at.as_deref().unwrap_or("");
        if local_ts.is_empty() || rem.updated_at.as_str() > local_ts {
            scraper.thegamesdb_api_key = if rem.cleared || rem.api_key.is_empty() {
                None
            } else {
                Some(rem.api_key.clone())
            };
            scraper_changed_from_remote = true;
            meta.thegamesdb = ProviderSyncMeta {
                updated_at: Some(rem.updated_at.clone()),
                cleared: rem.cleared,
                dirty: false,
            };
        }
    }

    if scraper_changed_from_remote {
        save_config(&meta_repo, store, scraper.clone()).await?;
    }

    // 4. Monta o que esta máquina tem de mais novo (ou primeiro sync com segredo já configurado).
    let now = now_rfc3339();
    let mut upload = SyncedCredentialsPayload::default();

    let remote_ra_ts = remote
        .and_then(|r| r.retroachievements.as_ref())
        .map(|s| s.updated_at.as_str())
        .unwrap_or("");
    if meta.retroachievements.dirty
        || (remote_ra_ts.is_empty() && ra_cfg.connected && ra_token.is_some())
    {
        let ts = meta
            .retroachievements
            .updated_at
            .clone()
            .unwrap_or_else(|| now.clone());
        if ts.as_str() > remote_ra_ts {
            meta.retroachievements.updated_at = Some(ts.clone());
            if meta.retroachievements.cleared {
                upload.retroachievements = Some(SyncedRaSlot {
                    cleared: true,
                    updated_at: ts,
                    ..Default::default()
                });
            } else if let (Some(user), Some(tok)) = (ra_cfg.username.clone(), ra_token) {
                upload.retroachievements = Some(SyncedRaSlot {
                    username: user,
                    connect_token: tok,
                    hardcore: ra_cfg.hardcore,
                    enabled: ra_cfg.enabled,
                    cleared: false,
                    updated_at: ts,
                });
            }
        }
    }

    let remote_ss_ts = remote
        .and_then(|r| r.screenscraper.as_ref())
        .map(|s| s.updated_at.as_str())
        .unwrap_or("");
    let has_local_ss =
        scraper.screenscraper_user.is_some() && scraper.screenscraper_password.is_some();
    if meta.screenscraper.dirty || (remote_ss_ts.is_empty() && has_local_ss) {
        let ts = meta
            .screenscraper
            .updated_at
            .clone()
            .unwrap_or_else(|| now.clone());
        if ts.as_str() > remote_ss_ts {
            meta.screenscraper.updated_at = Some(ts.clone());
            if meta.screenscraper.cleared {
                upload.screenscraper = Some(SyncedScreenScraperSlot {
                    cleared: true,
                    updated_at: ts,
                    ..Default::default()
                });
            } else if let (Some(u), Some(p)) = (
                scraper.screenscraper_user.clone(),
                scraper.screenscraper_password.clone(),
            ) {
                upload.screenscraper = Some(SyncedScreenScraperSlot {
                    username: u,
                    password: p,
                    cleared: false,
                    updated_at: ts,
                });
            }
        }
    }

    let remote_tgdb_ts = remote
        .and_then(|r| r.thegamesdb.as_ref())
        .map(|s| s.updated_at.as_str())
        .unwrap_or("");
    if meta.thegamesdb.dirty || (remote_tgdb_ts.is_empty() && scraper.thegamesdb_api_key.is_some())
    {
        let ts = meta
            .thegamesdb
            .updated_at
            .clone()
            .unwrap_or_else(|| now.clone());
        if ts.as_str() > remote_tgdb_ts {
            meta.thegamesdb.updated_at = Some(ts.clone());
            if meta.thegamesdb.cleared {
                upload.thegamesdb = Some(SyncedTheGamesDbSlot {
                    cleared: true,
                    updated_at: ts,
                    ..Default::default()
                });
            } else if let Some(k) = scraper.thegamesdb_api_key.clone() {
                upload.thegamesdb = Some(SyncedTheGamesDbSlot {
                    api_key: k,
                    cleared: false,
                    updated_at: ts,
                });
            }
        }
    }

    save_sync_meta(pool, &meta).await?;
    Ok((upload, ra_changed))
}

/// Confirma que o `upload` enviado para o servidor foi aceito (limpa `dirty`).
pub async fn mark_sync_uploaded(
    pool: &db::Db,
    uploaded: &SyncedCredentialsPayload,
) -> Result<(), String> {
    let mut meta = load_sync_meta(pool).await.unwrap_or_default();
    if let Some(ra) = &uploaded.retroachievements {
        if meta.retroachievements.updated_at.as_deref() == Some(ra.updated_at.as_str()) {
            meta.retroachievements.dirty = false;
        }
    }
    if let Some(ss) = &uploaded.screenscraper {
        if meta.screenscraper.updated_at.as_deref() == Some(ss.updated_at.as_str()) {
            meta.screenscraper.dirty = false;
        }
    }
    if let Some(tgdb) = &uploaded.thegamesdb {
        if meta.thegamesdb.updated_at.as_deref() == Some(tgdb.updated_at.as_str()) {
            meta.thegamesdb.dirty = false;
        }
    }
    save_sync_meta(pool, &meta).await
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
    async fn ra_credentials_sync_pull_unlocks_locally_and_logout_propagates() {
        let pool_b = db::connect_in_memory().await.unwrap();
        let store_b = MemStore::default();

        let incoming = SyncedCredentialsPayload {
            retroachievements: Some(SyncedRaSlot {
                username: "edward".into(),
                connect_token: "ra_secret_tok".into(),
                hardcore: true,
                enabled: true,
                cleared: false,
                updated_at: "2026-11-26T19:00:00.000Z".into(),
            }),
            screenscraper: Some(SyncedScreenScraperSlot {
                username: "ss_ed".into(),
                password: "ss_pw".into(),
                cleared: false,
                updated_at: "2026-11-26T19:00:00.000Z".into(),
            }),
            thegamesdb: None,
        };

        let (to_upload, ra_changed) =
            reconcile_with_remote(&pool_b, &store_b, Some(&incoming), Some("edward".into()))
                .await
                .unwrap();
        assert!(ra_changed);
        assert!(to_upload.is_empty());
        assert_eq!(
            store_b.secret(ACCOUNT_RETROACHIEVEMENTS).as_deref(),
            Some("ra_secret_tok")
        );
        assert_eq!(
            store_b.secret(ACCOUNT_SCREENSCRAPER).as_deref(),
            Some("ss_pw")
        );
        let (ra_cfg, tok) = load_ra_session(&pool_b, &store_b).await.unwrap();
        assert!(ra_cfg.connected && ra_cfg.enabled && ra_cfg.hardcore);
        assert_eq!(ra_cfg.username.as_deref(), Some("edward"));
        assert_eq!(tok.as_deref(), Some("ra_secret_tok"));

        // Logout em outra máquina limpa localmente sem reenviar o token antigo.
        let tombstone = SyncedCredentialsPayload {
            retroachievements: Some(SyncedRaSlot {
                cleared: true,
                updated_at: "2026-11-26T19:05:00.000Z".into(),
                ..Default::default()
            }),
            screenscraper: incoming.screenscraper.clone(),
            thegamesdb: None,
        };
        let (to_upload2, ra_changed2) =
            reconcile_with_remote(&pool_b, &store_b, Some(&tombstone), Some("edward".into()))
                .await
                .unwrap();
        assert!(ra_changed2);
        assert!(to_upload2.is_empty());
        assert_eq!(store_b.secret(ACCOUNT_RETROACHIEVEMENTS), None);
        let (ra_after, tok_after) = load_ra_session(&pool_b, &store_b).await.unwrap();
        assert!(!ra_after.connected);
        assert_eq!(tok_after, None);
        assert_eq!(
            ra_after.linked_alvanista_username.as_deref(),
            Some("edward")
        );
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
