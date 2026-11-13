//! A máquina como aparelho do cloud save da conta do Alvanista.
//!
//! - `api`: o cliente REST das rotas de save;
//! - `device`: o payload de hardware e o armazenamento do aparelho (o token
//!   `asv_...` só no chaveiro, o resto no SQLite).
//!
//! O aparelho nasce uma vez, pelo token de acesso do parceiro (o par do
//! "Entrar com Alvanista"). Dali em diante só o token do aparelho fala com
//! `/api/v1/saves`: o do parceiro nunca chega lá.
//!
//! Erros seguem a convenção de `alvanista/`: `Result<_, String>` com o código
//! no texto (`SyncError` mostra o código de i18n).

// Os comandos do sync (próxima etapa) são quem chama esta API; até lá o que
// eles usam fica sem chamador fora dos testes. As funções que recebem um
// `AppHandle` (que o teste não tem) levam o `expect` no próprio item.
#![cfg_attr(not(test), expect(dead_code))]

pub mod api;
pub mod device;

use crate::alvanista::client::ClientError;
use crate::alvanista::Alvanista;
use api::{ApiError, SavesApi};
use device::{hardware_fingerprint, hardware_payload, Device, DeviceStore, HardwarePayload};
use tauri::{AppHandle, Manager};

/// Passado esse tempo sem enviar, o hardware vai de novo mesmo sem mudança.
const HW_RESEND_SECS: i64 = 24 * 3600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// Sem sessão do Alvanista.
    NoAccount,
    /// O plano não libera o sync de saves.
    NoTier,
    /// A sessão não tem o escopo `saves`: a tela pede o consentimento.
    NeedsConsent,
    /// O token do aparelho foi revogado.
    Revoked,
    Paused,
    Quota,
    Offline,
    Keyring,
    Failed(String),
}

impl std::fmt::Display for SyncError {
    /// O código de i18n, o mesmo que a tela traduz.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoAccount => "no_account",
            Self::NoTier => "no_tier",
            Self::NeedsConsent => "needs_consent",
            Self::Revoked => "revoked",
            Self::Paused => "paused",
            Self::Quota => "quota",
            Self::Offline => "offline",
            Self::Keyring => "keyring",
            Self::Failed(_) => "failed",
        })
    }
}

impl std::error::Error for SyncError {}

impl From<ApiError> for SyncError {
    /// Para chamadas com o token do aparelho: 401 é aparelho revogado.
    fn from(e: ApiError) -> Self {
        match e {
            ApiError::Unauthorized => Self::Revoked,
            ApiError::InsufficientScope => Self::NeedsConsent,
            ApiError::TierInsufficient { .. } => Self::NoTier,
            ApiError::Paused => Self::Paused,
            ApiError::Quota => Self::Quota,
            ApiError::Transient(_) => Self::Offline,
            other => failed(other.to_string()),
        }
    }
}

pub struct SyncService {
    api: SavesApi,
    devices: DeviceStore,
    /// Quem cria o aparelho nunca corre em paralelo: duas chamadas juntas
    /// criariam dois aparelhos na conta.
    mint: tokio::sync::Mutex<()>,
}

impl SyncService {
    pub fn new(api: SavesApi, devices: DeviceStore) -> Self {
        Self {
            api,
            devices,
            mint: tokio::sync::Mutex::new(()),
        }
    }

    pub fn devices(&self) -> &DeviceStore {
        &self.devices
    }

    /// O aparelho desta máquina, criando-o na primeira vez. `label` é o nome
    /// que a pessoa escolheu; sem ele vale o nome da máquina.
    ///
    /// `NeedsConsent`: a sessão não tem o escopo `saves`. Quem chama roda
    /// `reconnect_with_saves` e repete uma vez.
    #[expect(dead_code)] // os comandos do sync chamam; o teste não tem `AppHandle`
    pub async fn ensure_device(
        &self,
        app: &AppHandle,
        label: Option<String>,
    ) -> Result<Device, SyncError> {
        let alva = app.state::<Alvanista>();
        let hw = current_hardware(app);
        self.ensure_device_with(&alva, label, hw).await
    }

    async fn ensure_device_with(
        &self,
        alva: &Alvanista,
        label: Option<String>,
        hw: HardwarePayload,
    ) -> Result<Device, SyncError> {
        let _one_at_a_time = self.mint.lock().await;
        let status = alva.cached_status().await;
        if !status.connected {
            return Err(SyncError::NoAccount);
        }
        if status.save_sync == Some(false) {
            return Err(SyncError::NoTier);
        }
        if let Some(device) = self.devices.device().await.map_err(failed)? {
            // Um aparelho sem token (chaveiro limpo por fora) não serve: nasce outro.
            match self.devices.token() {
                Ok(Some(_)) => return Ok(device),
                Ok(None) => {
                    log::warn!("sync: aparelho guardado sem token no chaveiro, criando outro");
                    self.devices
                        .forget()
                        .await
                        .map_err(|_| SyncError::Keyring)?;
                }
                Err(e) => {
                    log::warn!("sync: chaveiro indisponível para ler o token: {e}");
                    return Err(SyncError::Keyring);
                }
            }
        }
        let label = label
            .map(|l| device::clean_text(&l))
            .filter(|l| !l.is_empty())
            .unwrap_or_else(device::default_label);

        let mut tokens = match alva.session_tokens().await {
            Ok(Some(t)) => t,
            Ok(None) => return Err(SyncError::NoAccount),
            Err(e) => {
                log::warn!("sync: não leu a sessão: {e}");
                return Err(SyncError::Keyring);
            }
        };
        let mut minted = self
            .api
            .partner_mint_device(&tokens.access_token, &label, &hw)
            .await;
        if matches!(minted, Err(ApiError::Unauthorized)) {
            // O token de acesso do parceiro vence em minutos: renova uma vez.
            tokens = match alva.renew(&tokens).await {
                Ok(t) => t,
                Err(ClientError::AuthEnded) => return Err(SyncError::NoAccount),
                Err(ClientError::Transient(_)) => return Err(SyncError::Offline),
                Err(e) => return Err(failed(e.to_string())),
            };
            minted = self
                .api
                .partner_mint_device(&tokens.access_token, &label, &hw)
                .await;
        }
        let minted = minted.map_err(|e| match e {
            ApiError::Unauthorized => SyncError::NoAccount,
            other => SyncError::from(other),
        })?;

        let device = Device {
            id: minted.id,
            label: minted.label,
        };
        if let Err(e) = self.devices.save(&device, &minted.token).await {
            log::warn!("sync: não deu pra guardar o aparelho: {e}");
            // O aparelho já existe no servidor: sem onde guardar o token ele
            // ficaria órfão. Revoga com o token que acabou de nascer.
            if let Err(e) = self.api.revoke_own_device(&minted.token).await {
                log::warn!("sync: aparelho órfão não revogado: {e}");
            }
            let _ = self.devices.forget().await;
            return Err(SyncError::Keyring);
        }
        // Acabou de enviar o hardware: o próximo `refresh_hardware` espera.
        self.remember_gpu(&hw).await;
        let _ = self
            .devices
            .set_hw_fingerprint(&hardware_fingerprint(&hw))
            .await;
        let _ = self
            .devices
            .set_hw_sent_at(crate::alvanista::store::now_secs())
            .await;
        log::info!("sync: aparelho criado");
        Ok(device)
    }

    /// Manda o hardware quando mudou ou quando o último envio tem mais de 24 h.
    #[expect(dead_code)] // os comandos do sync chamam; o teste não tem `AppHandle`
    pub async fn refresh_hardware(&self, app: &AppHandle) -> Result<(), SyncError> {
        let alva = app.state::<Alvanista>();
        self.refresh_hardware_with(&alva, current_hardware(app))
            .await
    }

    async fn refresh_hardware_with(
        &self,
        alva: &Alvanista,
        mut hw: HardwarePayload,
    ) -> Result<(), SyncError> {
        // Sem sessão nem chaveiro nem rede: só o SQLite foi lido até aqui.
        if !alva.cached_status().await.connected {
            return Err(SyncError::NoAccount);
        }
        if self.devices.device().await.map_err(failed)?.is_none() {
            return Ok(());
        }
        self.keep_last_gpu(&mut hw).await;
        let fingerprint = hardware_fingerprint(&hw);
        let now = crate::alvanista::store::now_secs();
        let unchanged = self
            .devices
            .hw_fingerprint()
            .await
            .map_err(failed)?
            .as_deref()
            == Some(&fingerprint);
        let fresh = self
            .devices
            .hw_sent_at()
            .await
            .map_err(failed)?
            .is_some_and(|at| now - at < HW_RESEND_SECS);
        if unchanged && fresh {
            return Ok(());
        }
        let token = self
            .devices
            .token()
            .map_err(|_| SyncError::Keyring)?
            .ok_or(SyncError::Revoked)?;
        self.api.patch_own_device(&token, None, Some(&hw)).await?;
        self.remember_gpu(&hw).await;
        self.devices
            .set_hw_fingerprint(&fingerprint)
            .await
            .map_err(failed)?;
        self.devices.set_hw_sent_at(now).await.map_err(failed)
    }

    /// GPU desconhecida agora (o wgpu não abriu): em vez de mandar campos
    /// vazios por cima de valores bons do servidor, repete os últimos enviados.
    async fn keep_last_gpu(&self, hw: &mut HardwarePayload) {
        if !hw.gpu.is_empty() || !hw.gpu_backend.is_empty() || !hw.gpu_driver.is_empty() {
            return;
        }
        if let Ok(Some([gpu, backend, driver])) = self.devices.hw_gpu().await {
            (hw.gpu, hw.gpu_backend, hw.gpu_driver) = (gpu, backend, driver);
        }
    }

    async fn remember_gpu(&self, hw: &HardwarePayload) {
        if hw.gpu.is_empty() && hw.gpu_backend.is_empty() && hw.gpu_driver.is_empty() {
            return;
        }
        let triple = [
            hw.gpu.clone(),
            hw.gpu_backend.clone(),
            hw.gpu_driver.clone(),
        ];
        if let Err(e) = self.devices.set_hw_gpu(&triple).await {
            log::warn!("sync: GPU enviada não guardada: {e}");
        }
    }
}

/// `Failed` leva a mensagem, mas `Display` só mostra o código: o texto
/// (sem segredos) vai para o log onde o erro nasce.
fn failed(e: String) -> SyncError {
    log::warn!("sync: falhou: {e}");
    SyncError::Failed(e)
}

/// O hardware da máquina agora, no formato do servidor.
#[expect(dead_code)] // `ensure_device` e `refresh_hardware`, que os comandos chamam
fn current_hardware(app: &AppHandle) -> HardwarePayload {
    let info = crate::commands::hardware::get_hardware_info(app.state());
    hardware_payload(
        &info,
        env!("CARGO_PKG_VERSION"),
        crate::alvanista::store::now_secs(),
    )
}

#[cfg(test)]
pub(crate) mod test_support {
    use sqlx::Row;

    /// Toda chave e todo valor de `alvanista_state`, para provar o que NÃO
    /// está lá.
    pub async fn all_state_values(db: &db::Db) -> Vec<String> {
        sqlx::query("SELECT key, value FROM alvanista_state")
            .fetch_all(db)
            .await
            .unwrap()
            .into_iter()
            .flat_map(|r| [r.get::<String, _>("key"), r.get::<String, _>("value")])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::{Client, Entitlements, Me};
    use crate::alvanista::store::test_support::{tokens, MemSecrets, SERIAL};
    use crate::alvanista::store::Store;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn hw() -> HardwarePayload {
        HardwarePayload {
            os: "Windows 11".into(),
            arch: "x86_64".into(),
            cpu_cores: 8,
            memory_mb: 16_384,
            app_version: "0.4.0".into(),
            captured_at: "2026-11-13T10:00:00Z".into(),
            ..Default::default()
        }
    }

    struct Rig {
        sync: SyncService,
        alva: Alvanista,
        store: Store,
        secrets: Arc<MemSecrets>,
        db: db::Db,
        server: MockServer,
    }

    /// Alvanista e sync contra o mesmo servidor de mentira, com sessão
    /// (escopos `profile:read saves`) e chaveiro em memória.
    async fn rig() -> Rig {
        let server = MockServer::start().await;
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
        let sync = SyncService::new(
            SavesApi::new(server.uri()),
            DeviceStore::new(db.clone(), secrets.clone()),
        );
        Rig {
            sync,
            alva,
            store,
            secrets,
            db,
            server,
        }
    }

    async fn patches(server: &MockServer) -> usize {
        server.received_requests().await.unwrap().len()
    }

    fn minted_body(token: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "dev1", "label": "Sala", "client_kind": "reemu",
            "token": token, "token_prefix": "asv_se",
            "created_at": "2026-11-13T10:00:00Z"
        })
    }

    async fn mount_mint(server: &MockServer, template: ResponseTemplate, times: u64) {
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/save-devices"))
            .respond_with(template)
            .expect(times)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn ensure_device_mints_once_then_reuses() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        mount_mint(
            &r.server,
            ResponseTemplate::new(201).set_body_json(minted_body("asv_secret")),
            1,
        )
        .await;
        let first = r
            .sync
            .ensure_device_with(&r.alva, Some("Sala".into()), hw())
            .await
            .unwrap();
        assert_eq!(
            first,
            Device {
                id: "dev1".into(),
                label: "Sala".into()
            }
        );
        let second = r
            .sync
            .ensure_device_with(&r.alva, None, hw())
            .await
            .unwrap();
        assert_eq!(second, first);
        assert_eq!(
            r.sync.devices().token().unwrap().as_deref(),
            Some("asv_secret")
        );
        // `expect(1)` do mock confere no drop do servidor.
        let posts = r.server.received_requests().await.unwrap();
        assert_eq!(posts.len(), 1);
    }

    #[tokio::test]
    async fn two_concurrent_calls_mint_a_single_device() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        mount_mint(
            &r.server,
            ResponseTemplate::new(201).set_body_json(minted_body("asv_secret")),
            1,
        )
        .await;
        let (a, b) = tokio::join!(
            r.sync.ensure_device_with(&r.alva, None, hw()),
            r.sync.ensure_device_with(&r.alva, None, hw())
        );
        assert_eq!(a.unwrap(), b.unwrap());
    }

    #[tokio::test]
    async fn ensure_device_maps_insufficient_scope_to_needs_consent() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        mount_mint(
            &r.server,
            ResponseTemplate::new(403)
                .set_body_json(serde_json::json!({"error": "insufficient_scope"})),
            1,
        )
        .await;
        let err = r.sync.ensure_device_with(&r.alva, None, hw()).await;
        assert_eq!(err, Err(SyncError::NeedsConsent));
        assert_eq!(r.sync.devices().device().await.unwrap(), None);
    }

    #[tokio::test]
    async fn ensure_device_maps_402_to_no_tier() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        mount_mint(
            &r.server,
            ResponseTemplate::new(402).set_body_json(serde_json::json!({
                "error": {"code": "tier_insufficient", "current_tier": "free",
                          "upgrade_to": "collector", "required_quota": "saves",
                          "required_value": 1}
            })),
            1,
        )
        .await;
        let err = r.sync.ensure_device_with(&r.alva, None, hw()).await;
        assert_eq!(err, Err(SyncError::NoTier));
    }

    #[tokio::test]
    async fn without_a_session_there_is_no_account_and_no_request() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        r.store.unmark_session().await.unwrap();
        let keyring_calls = || r.secrets.calls.load(std::sync::atomic::Ordering::SeqCst);
        let before = keyring_calls();
        mount_mint(&r.server, ResponseTemplate::new(500), 0).await;
        let err = r.sync.ensure_device_with(&r.alva, None, hw()).await;
        assert_eq!(err, Err(SyncError::NoAccount));
        // O mesmo vale para o hardware, mesmo com um aparelho guardado.
        r.sync
            .devices()
            .save(
                &Device {
                    id: "dev1".into(),
                    label: "Sala".into(),
                },
                "asv_secret",
            )
            .await
            .unwrap();
        let before_refresh = keyring_calls();
        assert_eq!(
            r.sync.refresh_hardware_with(&r.alva, hw()).await,
            Err(SyncError::NoAccount)
        );
        assert_eq!(
            keyring_calls(),
            before_refresh,
            "o refresh tocou o chaveiro"
        );
        assert_eq!(
            before,
            before_refresh - 1,
            "só o save do teste tocou o chaveiro"
        );
        assert!(r.server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_unknown_gpu_keeps_the_last_one_sent() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        r.sync
            .devices()
            .save(
                &Device {
                    id: "dev1".into(),
                    label: "Sala".into(),
                },
                "asv_secret",
            )
            .await
            .unwrap();
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "dev1", "label": "Sala", "client_kind": "reemu",
                "token_prefix": "asv_se", "created_at": "2026-11-13T10:00:00Z"
            })))
            .mount(&r.server)
            .await;
        let mut known = hw();
        (known.gpu, known.gpu_backend, known.gpu_driver) =
            ("RTX".into(), "Vulkan".into(), "555".into());
        r.sync.refresh_hardware_with(&r.alva, known).await.unwrap();
        // A GPU não abriu e a RAM mudou: vai de novo, com a GPU de antes.
        let mut unknown = hw();
        unknown.memory_mb = 32_768;
        r.sync
            .refresh_hardware_with(&r.alva, unknown)
            .await
            .unwrap();
        let reqs = r.server.received_requests().await.unwrap();
        assert_eq!(reqs.len(), 2);
        let body: serde_json::Value = serde_json::from_slice(&reqs[1].body).unwrap();
        assert_eq!(body["hardware"]["memory_mb"], 32_768);
        assert_eq!(body["hardware"]["gpu"], "RTX");
        assert_eq!(body["hardware"]["gpu_backend"], "Vulkan");
        assert_eq!(body["hardware"]["gpu_driver"], "555");
    }

    #[tokio::test]
    async fn a_plan_without_save_sync_is_no_tier_and_no_request() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        r.store
            .mirror_put(&Me {
                id: "u1".into(),
                username: "toy".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/u/toy".into(),
                card: None,
                entitlements: Some(Entitlements { save_sync: false }),
            })
            .await
            .unwrap();
        mount_mint(&r.server, ResponseTemplate::new(500), 0).await;
        let err = r.sync.ensure_device_with(&r.alva, None, hw()).await;
        assert_eq!(err, Err(SyncError::NoTier));
    }

    #[tokio::test]
    async fn an_expired_partner_token_is_renewed_once_and_the_mint_repeated() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/save-devices"))
            .and(header("authorization", "Bearer alp_1"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(serde_json::json!({"error": "invalid_token"})),
            )
            .expect(1)
            .mount(&r.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "alp_2", "refresh_token": "alr_2", "token_type": "Bearer",
                "expires_in": 3600
            })))
            .expect(1)
            .mount(&r.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/save-devices"))
            .and(header("authorization", "Bearer alp_2"))
            .respond_with(ResponseTemplate::new(201).set_body_json(minted_body("asv_secret")))
            .expect(1)
            .mount(&r.server)
            .await;
        let device = r
            .sync
            .ensure_device_with(&r.alva, None, hw())
            .await
            .unwrap();
        assert_eq!(device.id, "dev1");
    }

    #[tokio::test]
    async fn a_keyring_that_refuses_the_token_leaves_no_device_and_revokes_the_orphan() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        mount_mint(
            &r.server,
            ResponseTemplate::new(201).set_body_json(minted_body("asv_secret")),
            1,
        )
        .await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .and(header("authorization", "Bearer asv_secret"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&r.server)
            .await;
        r.secrets
            .fail_next_set
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let err = r.sync.ensure_device_with(&r.alva, None, hw()).await;
        assert_eq!(err, Err(SyncError::Keyring));
        assert_eq!(r.sync.devices().device().await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_stored_device_without_a_token_is_minted_again() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        mount_mint(
            &r.server,
            ResponseTemplate::new(201).set_body_json(minted_body("asv_new")),
            1,
        )
        .await;
        r.sync
            .devices()
            .save(
                &Device {
                    id: "old".into(),
                    label: "Velho".into(),
                },
                "asv_old",
            )
            .await
            .unwrap();
        r.secrets.secrets.lock().unwrap().remove("alvanista-saves");
        let device = r
            .sync
            .ensure_device_with(&r.alva, None, hw())
            .await
            .unwrap();
        assert_eq!(device.id, "dev1");
        assert_eq!(
            r.sync.devices().token().unwrap().as_deref(),
            Some("asv_new")
        );
    }

    #[tokio::test]
    async fn refresh_hardware_patches_only_on_change_or_after_a_day() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        r.sync
            .devices()
            .save(
                &Device {
                    id: "dev1".into(),
                    label: "Sala".into(),
                },
                "asv_secret",
            )
            .await
            .unwrap();
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .and(header("authorization", "Bearer asv_secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "dev1", "label": "Sala", "client_kind": "reemu",
                "token_prefix": "asv_se", "created_at": "2026-11-13T10:00:00Z"
            })))
            .mount(&r.server)
            .await;
        // Nunca enviado: vai.
        r.sync.refresh_hardware_with(&r.alva, hw()).await.unwrap();
        assert_eq!(patches(&r.server).await, 1);
        // Mesma máquina, enviado agora: não vai.
        let mut later = hw();
        later.captured_at = "2026-11-13T11:00:00Z".into();
        later.app_version = "0.5.0".into();
        r.sync.refresh_hardware_with(&r.alva, later).await.unwrap();
        assert_eq!(patches(&r.server).await, 1);
        // GPU nova: vai.
        let mut gpu = hw();
        gpu.gpu = "Outra".into();
        r.sync.refresh_hardware_with(&r.alva, gpu).await.unwrap();
        assert_eq!(patches(&r.server).await, 2);
        // Mesma máquina, mas o último envio tem mais de 24 h: vai.
        r.sync
            .devices()
            .set_hw_sent_at(crate::alvanista::store::now_secs() - HW_RESEND_SECS - 1)
            .await
            .unwrap();
        r.sync.refresh_hardware_with(&r.alva, hw()).await.unwrap();
        assert_eq!(patches(&r.server).await, 3);
    }

    #[test]
    fn the_error_display_is_the_i18n_code() {
        let codes = [
            (SyncError::NoAccount, "no_account"),
            (SyncError::NoTier, "no_tier"),
            (SyncError::NeedsConsent, "needs_consent"),
            (SyncError::Revoked, "revoked"),
            (SyncError::Paused, "paused"),
            (SyncError::Quota, "quota"),
            (SyncError::Offline, "offline"),
            (SyncError::Keyring, "keyring"),
            (SyncError::Failed("x".into()), "failed"),
        ];
        for (err, code) in codes {
            assert_eq!(err.to_string(), code);
        }
        assert_eq!(SyncError::from(ApiError::Unauthorized), SyncError::Revoked);
        assert_eq!(
            SyncError::from(ApiError::Transient("x".into())),
            SyncError::Offline
        );
    }

    /// Logger de teste que guarda o texto de cada linha.
    struct Capture(Mutex<Vec<String>>);
    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            self.0.lock().unwrap().push(format!("{}", record.args()));
        }
        fn flush(&self) {}
    }
    static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

    #[tokio::test]
    async fn no_log_line_and_no_state_row_ever_holds_the_device_token() {
        let _serial = SERIAL.lock().await;
        // Se outro teste do processo instalou o logger antes, falha alto: um
        // teste de "nada vaza no log" que não vê o log não prova nada.
        assert!(
            log::set_logger(&CAPTURE).is_ok(),
            "não deu pra instalar o logger de captura"
        );
        log::set_max_level(log::LevelFilter::Trace);
        let r = rig().await;
        // O caminho feliz, o do chaveiro recusando (que revoga o órfão) e o
        // do 401 do aparelho no refresh de hardware.
        mount_mint(
            &r.server,
            ResponseTemplate::new(201).set_body_json(minted_body("asv_topsecret")),
            2,
        )
        .await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&r.server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(serde_json::json!({"error": "invalid_token"})),
            )
            .mount(&r.server)
            .await;
        r.secrets
            .fail_next_set
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            r.sync.ensure_device_with(&r.alva, None, hw()).await,
            Err(SyncError::Keyring)
        );
        r.sync
            .ensure_device_with(&r.alva, None, hw())
            .await
            .unwrap();
        let mut moved = hw();
        moved.gpu = "Outra".into();
        assert_eq!(
            r.sync.refresh_hardware_with(&r.alva, moved).await,
            Err(SyncError::Revoked)
        );
        r.sync.devices().set_enabled(true).await.unwrap();

        for v in test_support::all_state_values(&r.db).await {
            assert!(!v.contains("asv_"), "token no SQLite: {v}");
        }
        let lines = CAPTURE.0.lock().unwrap().clone();
        assert!(
            lines.iter().any(|l| l.starts_with("sync:")),
            "o logger não viu as linhas do sync"
        );
        for l in lines.iter() {
            assert!(!l.contains("asv_"), "token no log: {l}");
        }
        // O par da sessão também não aparece no SQLite.
        for v in test_support::all_state_values(&r.db).await {
            assert!(!v.contains("alp_") && !v.contains("alr_"), "{v}");
        }
        let _ = &r.store;
    }
}
