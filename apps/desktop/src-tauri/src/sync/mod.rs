//! A máquina como aparelho do cloud save da conta do Alvanista.
//!
//! - `api`: o cliente REST das rotas de save;
//! - `device`: o payload de hardware e o armazenamento do aparelho (o token
//!   `asv_...` só no chaveiro, o resto no SQLite);
//! - `local`, `reconcile` e `engine`: os game saves deste disco, a regra de
//!   três lados e as passadas que sobem, descem e guardam o conflito; e, a
//!   pedido, as configurações desta máquina enviadas e as de outra copiadas;
//! - `settings_groups`: as configurações de cada grupo como documento
//!   portátil, que outra máquina aplica.
//!
//! O aparelho nasce uma vez, pelo token de acesso do parceiro (o par do
//! "Entrar com Alvanista"). Dali em diante só o token do aparelho fala com
//! `/api/v1/saves`: o do parceiro nunca chega lá.
//!
//! Erros seguem a convenção de `alvanista/`: `Result<_, String>` com o código
//! no texto (`SyncError` mostra o código de i18n).

pub mod api;
pub mod device;
pub mod engine;
pub mod local;
pub mod platform;
pub mod reconcile;
pub mod settings_groups;

use crate::alvanista::client::ClientError;
use crate::alvanista::loopback::Lang;
use crate::alvanista::store::Tokens;
use crate::alvanista::{Alvanista, AlvanistaStatus};
use api::{ApiError, RemoteDevice, SavesApi};
use db::SyncRepo;
use device::{
    hardware_fingerprint, hardware_payload, Device, DeviceStore, HardwarePayload, LABEL_MAX_CHARS,
};
use engine::{
    CopyReport, Engine, Keep, RemoteStates, SaveLineStatus, SettingsPreview, SettingsSent,
    SettingsUndoInfo, SettingsUpload, SyncConflict, SyncVersion,
};
use serde::Serialize;
use std::future::Future;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

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
    /// 413: o save passa do limite que a nuvem aceita para a plataforma.
    TooLarge,
    /// 413 de um save state: passa do maior objeto que a nuvem aceita.
    StateTooLarge,
    /// 400 ou 422: a nuvem recusou o save (nome, campos ou conteúdo).
    Rejected,
    Offline,
    Keyring,
    /// O nome do aparelho está fora de 1..=80 caracteres ou o servidor o recusou.
    InvalidLabel,
    /// Não há aparelho nesta máquina (o sync está desligado).
    NotEnabled,
    /// A reconexão para pedir o escopo `saves` não terminou: um de
    /// `cancelled`, `denied` ou `timeout` (o resto vira `Failed`).
    Connect(String),
    /// Copiar ou desfazer configurações com um jogo aberto (ou abrindo): o
    /// core lê opções, atalhos e áudio enquanto roda.
    GameOpen,
    /// O documento de configurações é de uma versão que esta não conhece (a
    /// outra máquina tem um ReEmu mais novo).
    SettingsVersion,
    /// O documento de configurações não tem o formato do grupo.
    SettingsInvalid,
    /// A outra máquina não enviou o grupo pedido (ou não está mais na conta).
    SettingsMissing,
    /// O backup das configurações desta máquina não pôde ser gravado (a cópia
    /// não aconteceu) ou lido (o "Desfazer" não aconteceu).
    BackupFailed,
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
            Self::TooLarge => "too_large",
            Self::StateTooLarge => "state_too_large",
            Self::Rejected => "rejected",
            Self::Offline => "offline",
            Self::Keyring => "keyring",
            Self::InvalidLabel => "invalid_label",
            Self::NotEnabled => "not_enabled",
            Self::Connect(code) => code.as_str(),
            Self::GameOpen => "game_open",
            Self::SettingsVersion => "settings_version",
            Self::SettingsInvalid => "settings_invalid",
            Self::SettingsMissing => "settings_missing",
            Self::BackupFailed => "backup_failed",
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
            ApiError::TooLarge { .. } => Self::TooLarge,
            ApiError::Transient(_) => Self::Offline,
            other => failed(other.to_string()),
        }
    }
}

pub struct SyncService {
    api: SavesApi,
    devices: DeviceStore,
    /// Os conflitos abertos, que a página conta pelo SQLite.
    conflicts: SyncRepo,
    /// Quem roda uma passada quando o sync ou os game saves acabam de ligar
    /// (o motor se registra aqui ao subir).
    on_enabled: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>>,
    /// Quem cria o aparelho nunca corre em paralelo: duas chamadas juntas
    /// criariam dois aparelhos na conta.
    mint: tokio::sync::Mutex<()>,
    /// O que só existe enquanto o app roda (a cota e o resultado da última
    /// rodada); o resto fica no SQLite.
    runtime: std::sync::Mutex<Runtime>,
}

#[derive(Default)]
struct Runtime {
    quota: Option<SyncQuota>,
    last_pass_at: Option<i64>,
    last_error: Option<String>,
    /// O arquivo que a nuvem recusou, quando `last_error` é de um arquivo só
    /// (`too_large`, `state_too_large`, `rejected`): a página diz qual.
    last_error_file: Option<String>,
    /// Os save states que ficam só nesta máquina, por (ROM, chave), com o
    /// motivo (`unversioned`, `wiped`, `too_large`, `rejected`, `duplicate`,
    /// `deferred`) e o id do state (a vaga diz o dela). A passada dos states
    /// os conta.
    states_local: std::collections::HashMap<(String, String), (&'static str, String)>,
    /// O que as passadas de states têm a dizer à página (um state que a
    /// nuvem recusou, a cota, o plano), por ROM, com o nome do arquivo quando
    /// é um só. Fica à parte do `last_error` dos game saves: uma passada de
    /// states nunca apaga nem troca o que a dos game saves disse, e a de uma
    /// ROM só troca o da ROM dela.
    state_errors: std::collections::HashMap<String, (SyncError, Option<String>)>,
    /// Muda quando os states desligam ou o aparelho é esquecido: uma passada
    /// de states que começou antes não grava mais nada aqui.
    states_epoch: u64,
}

impl Runtime {
    /// O aviso de states que a página mostra: o mais grave, e entre iguais o
    /// da primeira ROM pelo id (sempre o mesmo).
    fn state_error(&self) -> Option<&(SyncError, Option<String>)> {
        let mut roms: Vec<&String> = self.state_errors.keys().collect();
        roms.sort();
        roms.into_iter()
            .map(|rom| &self.state_errors[rom])
            .rev()
            .max_by_key(|(e, _)| severity(e))
    }

    /// Esquece o que os states disseram e deixa para trás a passada em curso.
    fn forget_states(&mut self) {
        self.states_local.clear();
        self.state_errors.clear();
        self.states_epoch += 1;
    }
}

impl SyncService {
    pub fn new(api: SavesApi, devices: DeviceStore) -> Self {
        Self {
            api,
            conflicts: SyncRepo::new(devices.db()),
            devices,
            on_enabled: std::sync::OnceLock::new(),
            mint: tokio::sync::Mutex::new(()),
            runtime: std::sync::Mutex::new(Runtime::default()),
        }
    }

    fn runtime(&self) -> std::sync::MutexGuard<'_, Runtime> {
        self.runtime.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Registra quem roda a passada ao ligar. Só o primeiro vale.
    pub fn on_enabled(&self, run_pass: Box<dyn Fn() + Send + Sync>) {
        let _ = self.on_enabled.set(run_pass);
    }

    /// O sync ou os game saves acabaram de ligar: uma passada agora, em vez
    /// de esperar a de 10 minutos (M3).
    fn kick_pass(&self) {
        if let Some(run_pass) = self.on_enabled.get() {
            run_pass();
        }
    }

    /// Para o teste do `alvanista/` segurar o trinco de quem cria o aparelho.
    #[cfg(test)]
    pub(crate) async fn hold_mint_for_test(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.mint.lock().await
    }

    #[cfg(test)]
    pub fn devices(&self) -> &DeviceStore {
        &self.devices
    }

    /// O aparelho desta máquina, criando-o na primeira vez. `label` é o nome
    /// que a pessoa escolheu; sem ele vale o nome da máquina.
    ///
    /// `NeedsConsent`: a sessão não tem o escopo `saves`. Quem chama roda
    /// `reconnect_with_saves` e repete uma vez.
    async fn ensure_device_with(
        &self,
        alva: &Alvanista,
        mut label: Option<String>,
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
                    // O nome que a pessoa escolheu passa para o aparelho novo.
                    label = label.or(Some(device.label));
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
        // Sem o escopo `saves` o servidor recusaria; e o rótulo e o hardware só
        // saem da máquina depois do consentimento que os nomeia.
        if !status.scopes.iter().any(|s| s == config_scope_saves()) {
            return Err(SyncError::NeedsConsent);
        }
        let label = label
            .map(|l| device::clean_label(&l))
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
            tokens = alva.renew(&tokens).await.map_err(renew_error)?;
            minted = self
                .api
                .partner_mint_device(&tokens.access_token, &label, &hw)
                .await;
        }
        let minted = match minted {
            Ok(minted) => minted,
            Err(ApiError::Unauthorized) => return Err(SyncError::NoAccount),
            Err(ApiError::TierInsufficient { .. }) => {
                // O plano não libera (ou deixou de liberar) o sync: o espelho
                // passa a dizer isso, e a página confere no `/me`.
                alva.note_no_save_sync().await;
                return Err(SyncError::NoTier);
            }
            Err(other) => return Err(SyncError::from(other)),
        };

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
    async fn refresh_hardware_with(
        &self,
        alva: &Alvanista,
        mut hw: HardwarePayload,
    ) -> Result<(), SyncError> {
        // Sem sessão nem chaveiro nem rede: só o SQLite foi lido até aqui.
        if !alva.has_session().await {
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

    // --- o que os comandos da tela usam ---------------------------------

    /// A lista de aparelhos da conta, pelo par do parceiro (renovado uma vez
    /// se vencido). Sem sessão não toca o chaveiro nem a rede. Se a conta
    /// acabou no site (a renovação foi recusada), o aparelho daqui só se
    /// esquece: o par do parceiro já não existe.
    async fn partner_devices(&self, alva: &Alvanista) -> Result<Vec<RemoteDevice>, SyncError> {
        let mut tokens = session_pair(alva).await?;
        self.partner_devices_with(alva, &mut tokens).await
    }

    /// Como `partner_devices`, com o par que quem chama já leu; `tokens` fica
    /// com o par renovado, se houve renovação.
    async fn partner_devices_with(
        &self,
        alva: &Alvanista,
        tokens: &mut Tokens,
    ) -> Result<Vec<RemoteDevice>, SyncError> {
        let mut listed = self.api.partner_list_devices(&tokens.access_token).await;
        if matches!(listed, Err(ApiError::Unauthorized)) {
            *tokens = match alva.renew(tokens).await {
                Ok(t) => t,
                Err(ClientError::AuthEnded) => {
                    // `renew` já soltou o trinco de renovação, e `forget_device` não
                    // toma `mint` (quem chama pode estar dentro dele). O DELETE usa
                    // o token do aparelho, que não depende do par que acabou.
                    if let Err(e) = self.forget_device(true).await {
                        log::warn!("sync: aparelho não esquecido: {e}");
                    }
                    return Err(SyncError::NoAccount);
                }
                Err(e) => return Err(renew_error(e)),
            };
            listed = self.api.partner_list_devices(&tokens.access_token).await;
        }
        listed.map_err(|e| match e {
            ApiError::Unauthorized => SyncError::NoAccount,
            other => SyncError::from(other),
        })
    }

    /// Guarda o que a lista do site diz deste aparelho (D128).
    async fn absorb_site_state(&self, list: &[RemoteDevice]) {
        let Ok(Some(this)) = self.devices.device().await else {
            return;
        };
        // O servidor lista também os revogados e não corta a lista: quem não
        // aparece foi apagado ou é de outra conta.
        let (paused, revoked) = match list.iter().find(|d| d.id == this.id) {
            Some(mine) => (mine.paused_at.is_some(), mine.revoked_at.is_some()),
            None => (self.devices.site_state().await.is_ok_and(|s| s.0), true),
        };
        if let Err(e) = self.devices.set_site_state(paused, revoked).await {
            log::warn!("sync: estado do site não guardado: {e}");
        }
        if !revoked {
            let mut runtime = self.runtime();
            if runtime.last_error.as_deref() == Some("revoked") {
                runtime.last_error = None;
            }
        }
    }

    /// Um 401 com o token do aparelho, fora de uma passada: a lista do site
    /// diz se a máquina foi mesmo removida. Viva na lista (ou lista sem
    /// resposta) é `offline`.
    async fn row_after_401(&self, alva: &Alvanista) -> SyncError {
        match self.partner_devices(alva).await {
            Ok(list) => {
                self.absorb_site_state(&list).await;
                match self.devices.site_state().await {
                    Ok((_, true)) => SyncError::Revoked,
                    _ => SyncError::Offline,
                }
            }
            Err(SyncError::NoAccount) => SyncError::NoAccount,
            Err(e) => {
                log::info!("sync: 401 e a lista de aparelhos não respondeu ({e})");
                SyncError::Offline
            }
        }
    }

    async fn mark_revoked(&self) {
        let paused = self.devices.site_state().await.is_ok_and(|s| s.0);
        if let Err(e) = self.devices.set_site_state(paused, true).await {
            log::warn!("sync: revogação não guardada: {e}");
        }
    }

    /// Esquece o aparelho daqui. Com `revoke`, avisa o servidor antes, com o
    /// token do aparelho (401 e 404 valem como "já revogado"; uma falha de
    /// rede vai para o log e o aparelho local sai assim mesmo: desligar tem
    /// de funcionar sem rede, e o aparelho que sobrar a pessoa revoga no
    /// site). Sem aparelho guardado não toca o chaveiro.
    ///
    /// Não toma `mint`: a partir de `ensure_device` o caminho `renew` segura
    /// o trinco de renovação do `alvanista/`, e a sessão que acaba lá chama
    /// esta função.
    ///
    /// A revogação e o esquecer rodam numa tarefa própria: quem chamou pode
    /// desistir de esperar (o prazo de antes de abrir um jogo), e o token não
    /// pode ficar no chaveiro por isso.
    pub async fn forget_device(&self, revoke: bool) -> Result<(), SyncError> {
        let (api, devices) = (self.api.clone(), self.devices.clone());
        let joined = tokio::spawn(async move { forget_now(&api, &devices, revoke).await }).await;
        match joined {
            Err(e) => Err(failed(format!("esquecer interrompido: {e}"))),
            Ok(Err(e)) => Err(e),
            Ok(Ok(Forgot::NoDevice)) => Ok(()),
            Ok(Ok(Forgot::Done {
                result,
                revoke_failed,
            })) => {
                // O aparelho que sobrou no servidor a pessoa revoga no site: a
                // tela lê `offline` para dizer isso.
                let mut runtime = self.runtime();
                let epoch = runtime.states_epoch;
                *runtime = Runtime {
                    last_error: revoke_failed.then(|| SyncError::Offline.to_string()),
                    ..Runtime::default()
                };
                // A passada de states em curso não grava por cima.
                runtime.states_epoch = epoch;
                runtime.forget_states();
                drop(runtime);
                result
            }
        }
    }

    async fn refresh_quota(&self) -> Result<(), SyncError> {
        let token = self
            .devices
            .token()
            .map_err(|_| SyncError::Keyring)?
            .ok_or(SyncError::Revoked)?;
        let quota = self.api.quota(&token).await?;
        self.runtime().quota = Some(SyncQuota {
            used_bytes: quota.used_bytes,
            limit_bytes: quota.limit_bytes,
        });
        Ok(())
    }

    /// O estado da página. Decide pelo SQLite (marcador de sessão, espelho,
    /// aparelho) e lê o par do chaveiro uma vez só, quando vai à rede: a lista
    /// de aparelhos (com aparelho guardado) e o `/me` (quando o espelho diz que
    /// o plano não libera o sync, para quem acabou de assinar ver a mudança).
    /// Um `/me` por chamada, nunca em laço.
    async fn status_with(
        &self,
        alva: &Alvanista,
        hw: HardwarePayload,
    ) -> Result<SyncStatus, SyncError> {
        let device = self.devices.device().await.map_err(failed)?;
        if alva.has_session().await {
            let check_tier = alva.mirrored_save_sync().await == Some(false);
            if device.is_some() || check_tier {
                if let Ok(mut tokens) = session_pair(alva).await {
                    let mut ended = false;
                    if device.is_some() {
                        match self.partner_devices_with(alva, &mut tokens).await {
                            Ok(list) => self.absorb_site_state(&list).await,
                            Err(SyncError::NoAccount) => ended = true,
                            Err(_) => {}
                        }
                    }
                    if check_tier && !ended {
                        alva.refresh_profile(tokens).await;
                    }
                }
            }
        }
        // A rede pode ter acabado a sessão, esquecido o aparelho ou mudado o
        // plano: o que vale é o que sobrou depois dela.
        let device = self.devices.device().await.map_err(failed)?;
        let availability = if !alva.has_session().await {
            Availability::NoAccount
        } else if alva.mirrored_save_sync().await == Some(false) {
            Availability::NoTier
        } else {
            Availability::Ready
        };
        let (paused_site, revoked) = self.devices.site_state().await.map_err(failed)?;
        let enabled = device.is_some() && self.devices.enabled().await.map_err(failed)?;
        let paused_local = self.devices.paused().await.map_err(failed)?;
        let saves_enabled = self.devices.saves_enabled().await.map_err(failed)?;
        let states_enabled = self.devices.states_enabled().await.map_err(failed)?;
        // Pelo SQLite: vale logo ao abrir o app e com os game saves desligados
        // (resolver segue valendo, R19).
        let conflicts = match self.conflicts.conflicts_all().await {
            Ok(rows) => rows.len() as u32,
            Err(e) => {
                log::warn!("sync: conflitos ilegíveis: {e}");
                0
            }
        };
        let runtime = self.runtime();
        let state_error = runtime.state_error().filter(|_| states_enabled);
        Ok(SyncStatus {
            availability,
            enabled,
            paused_local,
            paused_site,
            saves_enabled,
            states_enabled,
            revoked,
            label: device
                .as_ref()
                .map(|d| d.label.clone())
                .filter(|l| !l.is_empty())
                .unwrap_or_else(device::default_label),
            device_id: device.map(|d| d.id),
            hardware: hw,
            quota: runtime.quota,
            last_pass_at: runtime.last_pass_at,
            conflicts,
            states_local_only: if states_enabled {
                runtime.states_local.len() as u32
            } else {
                0
            },
            states_wiped: if states_enabled {
                runtime
                    .states_local
                    .values()
                    .filter(|(r, _)| *r == engine::STATE_WIPED)
                    .count() as u32
            } else {
                0
            },
            // O dos game saves primeiro; sem ele, o dos states (que somem com
            // os states desligados).
            last_error: runtime
                .last_error
                .clone()
                .or_else(|| state_error.map(|(e, _)| e.to_string())),
            // Só junto do erro de um arquivo: outro erro depois dele não leva
            // o nome de um arquivo que já não é o motivo.
            last_error_file: match (runtime.last_error.as_deref(), state_error) {
                (Some("too_large" | "rejected"), _) => runtime.last_error_file.clone(),
                (Some(_), _) => None,
                (None, Some((_, file))) => file.clone(),
                (None, None) => None,
            },
        })
    }

    /// Liga ou desliga. Ligar cria o aparelho (e, se a sessão não tem o
    /// escopo `saves`, chama `reconnect` e repete uma vez); desligar revoga
    /// o aparelho e o esquece. A base local nunca é tocada.
    async fn set_enabled_with<F, Fut>(
        &self,
        alva: &Alvanista,
        on: bool,
        hw: HardwarePayload,
        reconnect: F,
    ) -> Result<SyncStatus, SyncError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<AlvanistaStatus, String>>,
    {
        if on {
            // O site revogou ou apagou o aparelho: o guardado não serve mais.
            if alva.has_session().await && self.devices.device().await.map_err(failed)?.is_some() {
                if let Ok(list) = self.partner_devices(alva).await {
                    self.absorb_site_state(&list).await;
                }
            }
            // Ligar de novo depois de revogada: o aparelho novo leva o nome que
            // a pessoa tinha escolhido.
            let mut label = None;
            if self.devices.site_state().await.map_err(failed)?.1 {
                label = self
                    .devices
                    .device()
                    .await
                    .map_err(failed)?
                    .map(|d| d.label);
                self.forget_device(true).await?;
            }
            match self
                .ensure_device_with(alva, label.clone(), hw.clone())
                .await
            {
                Ok(_) => {}
                Err(SyncError::NeedsConsent) => {
                    reconnect().await.map_err(|code| match code.as_str() {
                        "offline" => SyncError::Offline,
                        "keyring" => SyncError::Keyring,
                        "cancelled" | "denied" | "timeout" => SyncError::Connect(code),
                        other => {
                            log::warn!("sync: reconexão terminou com {other}");
                            SyncError::Failed(code)
                        }
                    })?;
                    self.ensure_device_with(alva, label, hw.clone()).await?;
                }
                Err(e) => return Err(e),
            }
            self.devices.set_enabled(true).await.map_err(failed)?;
            self.runtime().last_error = None;
            // A cota é cortesia da tela: sem ela o aparelho continua ligado.
            if let Err(e) = self.refresh_quota().await {
                log::info!("sync: cota não lida ao ligar: {e}");
            }
            self.kick_pass();
        } else {
            let _one_at_a_time = self.mint.lock().await;
            self.forget_device(true).await?;
        }
        self.status_with(alva, hw).await
    }

    async fn set_label_with(
        &self,
        alva: &Alvanista,
        label: &str,
        hw: HardwarePayload,
    ) -> Result<SyncStatus, SyncError> {
        let label = device::clean_text(label);
        if label.is_empty() || label.chars().count() > LABEL_MAX_CHARS {
            return Err(SyncError::InvalidLabel);
        }
        // Sem sessão não se toca o chaveiro nem a rede.
        if !alva.has_session().await {
            return Err(SyncError::NoAccount);
        }
        if self.devices.device().await.map_err(failed)?.is_none() {
            return Err(SyncError::NotEnabled);
        }
        let token = self
            .devices
            .token()
            .map_err(|_| SyncError::Keyring)?
            .ok_or(SyncError::Revoked)?;
        match self.api.patch_own_device(&token, Some(&label), None).await {
            Ok(remote) => self
                .devices
                .set_label(&remote.label)
                .await
                .map_err(failed)?,
            Err(e) => {
                return Err(match e {
                    ApiError::Rejected(_) | ApiError::Invalid(_) => SyncError::InvalidLabel,
                    // Um 401 sozinho não prova revogação (o `SaveAuth` também
                    // o devolve quando falha): a lista de aparelhos decide.
                    ApiError::Unauthorized => self.row_after_401(alva).await,
                    other => SyncError::from(other),
                });
            }
        }
        self.status_with(alva, hw).await
    }

    async fn set_paused_with(
        &self,
        alva: &Alvanista,
        paused: bool,
        hw: HardwarePayload,
    ) -> Result<SyncStatus, SyncError> {
        self.devices.set_paused(paused).await.map_err(failed)?;
        self.status_with(alva, hw).await
    }

    async fn set_saves_enabled_with(
        &self,
        alva: &Alvanista,
        on: bool,
        hw: HardwarePayload,
    ) -> Result<SyncStatus, SyncError> {
        self.devices.set_saves_enabled(on).await.map_err(failed)?;
        if on {
            self.kick_pass();
        }
        self.status_with(alva, hw).await
    }

    /// O interruptor dos save states (`sync.states_enabled`). Desligado, nenhum
    /// state sobe e a base e os arquivos ficam como estão; ligar roda uma
    /// passada já, como o dos game saves.
    async fn set_states_enabled_with(
        &self,
        alva: &Alvanista,
        on: bool,
        hw: HardwarePayload,
    ) -> Result<SyncStatus, SyncError> {
        self.devices.set_states_enabled(on).await.map_err(failed)?;
        if on {
            self.kick_pass();
        } else {
            // A passada de states em curso não grava mais nada (N3).
            self.runtime().forget_states();
        }
        self.status_with(alva, hw).await
    }

    /// Por que cada state desta ROM fica só nesta máquina, pelo id do state,
    /// como a última passada de states contou (a vaga diz o motivo, D116 e
    /// D122). Só a memória do app: nem rede nem disco. Vazia com o sync ou os
    /// states desligados.
    async fn state_notes(&self, rom_id: &str) -> Result<Vec<StateNote>, SyncError> {
        let devices = &self.devices;
        if devices.device().await.map_err(failed)?.is_none()
            || !devices.enabled().await.map_err(failed)?
            || !devices.states_enabled().await.map_err(failed)?
        {
            return Ok(Vec::new());
        }
        let mut notes: Vec<StateNote> = self
            .runtime()
            .states_local
            .iter()
            .filter(|((rom, _), _)| rom == rom_id)
            .map(|(_, (reason, id))| StateNote {
                state_id: id.clone(),
                reason: (*reason).to_owned(),
            })
            .collect();
        notes.sort_by(|a, b| a.state_id.cmp(&b.state_id));
        Ok(notes)
    }

    async fn machines_with(&self, alva: &Alvanista) -> Result<Vec<SyncMachine>, SyncError> {
        let list = self.partner_devices(alva).await?;
        self.absorb_site_state(&list).await;
        let this = self.devices.device().await.map_err(failed)?.map(|d| d.id);
        Ok(list
            .into_iter()
            .filter(|d| d.revoked_at.is_none())
            .map(|d| SyncMachine::from_remote(d, this.as_deref()))
            .collect())
    }

    /// "Sincronizar agora": a passada dos game saves (que lê a linha desta
    /// máquina no site antes de tudo), depois o hardware e a cota. A dos
    /// states não é esperada (M4): `then_states` a dispara, quando a dos game
    /// saves deixa, e ela roda por conta própria. Um state grande subindo não
    /// segura o botão nem a página.
    async fn now_with(
        &self,
        alva: &Alvanista,
        hw: HardwarePayload,
        engine: Option<&Engine>,
        then_states: impl FnOnce(),
    ) -> Result<SyncStatus, SyncError> {
        if !alva.has_session().await {
            return Err(SyncError::NoAccount);
        }
        if self.devices.device().await.map_err(failed)?.is_some()
            && self.devices.enabled().await.map_err(failed)?
        {
            let mut last: Option<SyncError> = None;
            let mut keep = |e: SyncError| {
                if last.as_ref().map_or(true, |l| severity(&e) > severity(l)) {
                    last = Some(e);
                }
            };
            // Pausada só aqui: nem passada (a página já mostra a pausa).
            let paused_here = self.devices.paused().await.map_err(failed)?;
            let mut stop = false;
            if let (Some(engine), false) = (engine, paused_here) {
                let files = engine.saves_pass(alva).await;
                if engine::states_may_follow(&files) {
                    then_states();
                }
                match files {
                    Ok(report) => {
                        if let Some(e) = report.worst() {
                            keep(e);
                        }
                    }
                    Err(e) => {
                        stop = matches!(
                            e,
                            SyncError::Revoked
                                | SyncError::NoAccount
                                | SyncError::NotEnabled
                                | SyncError::Paused
                        );
                        keep(e);
                    }
                }
            }
            if !stop {
                for step in [
                    self.refresh_hardware_with(alva, hw.clone()).await,
                    self.refresh_quota().await,
                ] {
                    match step {
                        // Um 401 sozinho não prova revogação (o `SaveAuth`
                        // também o devolve quando falha): quem decide é a
                        // linha da lista do site, que a passada lê.
                        Err(SyncError::Revoked) if engine.is_some() => keep(SyncError::Offline),
                        Err(e) => keep(e),
                        Ok(()) => {}
                    }
                }
            }
            if last == Some(SyncError::Revoked) && engine.is_none() {
                self.mark_revoked().await;
            }
            self.runtime().last_error = last.map(|e| e.to_string());
        }
        self.status_with(alva, hw).await
    }
}

enum Forgot {
    NoDevice,
    Done {
        result: Result<(), SyncError>,
        /// A revogação no servidor não chegou.
        revoke_failed: bool,
    },
}

/// O corpo de `forget_device`; `Err` só quando nem deu para ler o aparelho.
async fn forget_now(
    api: &SavesApi,
    devices: &DeviceStore,
    revoke: bool,
) -> Result<Forgot, SyncError> {
    if devices.device().await.map_err(failed)?.is_none() {
        return Ok(Forgot::NoDevice);
    }
    let mut revoke_failed = false;
    if revoke {
        match devices.token() {
            Ok(Some(token)) => match api.revoke_own_device(&token).await {
                Ok(()) | Err(ApiError::Unauthorized) | Err(ApiError::NotFound) => {}
                Err(e) => {
                    log::warn!("sync: aparelho não revogado no servidor: {e}");
                    revoke_failed = true;
                }
            },
            Ok(None) => {}
            Err(e) => {
                log::warn!("sync: chaveiro indisponível para revogar: {e}");
                revoke_failed = true;
            }
        }
    }
    let result = devices.forget().await.map_err(|e| {
        log::warn!("sync: aparelho não esquecido: {e}");
        SyncError::Keyring
    });
    Ok(Forgot::Done {
        result,
        revoke_failed,
    })
}

/// Qual erro a tela mostra quando vários passos falham: revoked > paused >
/// quota > offline > um arquivo recusado (a tela diz qual) > o resto.
fn severity(e: &SyncError) -> u8 {
    match e {
        SyncError::Revoked => 6,
        SyncError::Paused => 5,
        SyncError::Quota => 4,
        SyncError::Offline => 3,
        SyncError::TooLarge | SyncError::StateTooLarge | SyncError::Rejected => 2,
        _ => 1,
    }
}

/// O par da sessão, lido do chaveiro (uma leitura).
async fn session_pair(alva: &Alvanista) -> Result<Tokens, SyncError> {
    match alva.session_tokens().await {
        Ok(Some(t)) => Ok(t),
        Ok(None) => Err(SyncError::NoAccount),
        Err(e) => {
            log::warn!("sync: não leu a sessão: {e}");
            Err(SyncError::Keyring)
        }
    }
}

fn renew_error(e: ClientError) -> SyncError {
    match e {
        ClientError::AuthEnded => SyncError::NoAccount,
        ClientError::Transient(_) => SyncError::Offline,
        other => failed(other.to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum Availability {
    NoAccount,
    NoTier,
    Ready,
}

/// O que a página de configurações mostra. Nunca leva token.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SyncStatus {
    pub availability: Availability,
    pub enabled: bool,
    pub paused_local: bool,
    pub paused_site: bool,
    /// O interruptor dos game saves (`sync.saves_enabled`).
    pub saves_enabled: bool,
    /// O interruptor dos save states (`sync.states_enabled`).
    pub states_enabled: bool,
    pub revoked: bool,
    pub device_id: Option<String>,
    pub label: String,
    pub hardware: HardwarePayload,
    pub quota: Option<SyncQuota>,
    /// Segundos Unix.
    pub last_pass_at: Option<i64>,
    pub conflicts: u32,
    /// Os save states que ficam só nesta máquina: os de antes de a versão do
    /// core ser registrada, os apagados no site (D127) e os que a nuvem
    /// recusou. 0 com os states desligados.
    pub states_local_only: u32,
    /// Os que ficam só aqui porque o site os apagou (D127): são os únicos que
    /// "Enviar os states antigos também" manda de novo. Parte de
    /// `states_local_only`.
    pub states_wiped: u32,
    /// O código de i18n do último erro.
    pub last_error: Option<String>,
    /// O nome do arquivo que a nuvem recusou, com `too_large`,
    /// `state_too_large` ou `rejected`.
    pub last_error_file: Option<String>,
}

/// Por que um save state desta máquina fica só nela: a vaga dele diz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct StateNote {
    /// O id do state (`save_states.id`).
    pub state_id: String,
    /// `unversioned`, `too_large`, `rejected`, `wiped`, `duplicate` ou
    /// `deferred`.
    pub reason: String,
}

/// A cota como a tela a recebe (o servidor manda em snake_case: `api::Quota`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "Quota"))]
pub struct SyncQuota {
    pub used_bytes: i64,
    pub limit_bytes: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SyncMachine {
    pub id: String,
    pub label: String,
    pub client_kind: String,
    pub is_this: bool,
    pub hardware: Option<HardwarePayload>,
    pub last_used_at: Option<String>,
    pub revoked: bool,
    pub paused: bool,
}

impl SyncMachine {
    /// A linha da lista de aparelhos do site; `this`: o id desta máquina.
    fn from_remote(d: RemoteDevice, this: Option<&str>) -> Self {
        Self {
            is_this: this == Some(d.id.as_str()),
            revoked: d.revoked_at.is_some(),
            paused: d.paused_at.is_some(),
            id: d.id,
            label: d.label,
            client_kind: d.client_kind,
            hardware: d.hardware,
            last_used_at: d.last_used_at,
        }
    }
}

/// O canal dos eventos do sync.
pub const SYNC_EVENT: &str = "sync";

/// Os eventos do canal `sync` (o tipo `SyncEvent` de `lib/tauri.ts`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncEvent {
    /// A tela relê o estado.
    Status,
    /// Uma passada terminou. `backed_up`: saves apagados no site que foram
    /// para a pasta de resguardo.
    #[serde(rename_all = "camelCase")]
    Pass {
        uploaded: u32,
        downloaded: u32,
        backed_up: u32,
    },
    /// Um conflito novo.
    Conflict { key: String },
    /// Um código de `SyncError` (`revoked`, `quota`...).
    Error { code: String },
    /// A cota acabou: os envios pararam.
    #[serde(rename_all = "camelCase")]
    Quota { used_bytes: i64, limit_bytes: i64 },
    /// A checagem de antes de abrir um jogo terminou: `up_to_date`,
    /// `downloaded`, `conflict` ou `skipped` (o jogo abriu com o save daqui;
    /// `reason` é o código, `timeout` e `offline` inclusive).
    Prelaunch {
        outcome: String,
        reason: Option<String>,
    },
}

fn emit_status(app: &AppHandle) {
    if let Err(e) = app.emit(SYNC_EVENT, SyncEvent::Status) {
        log::warn!("sync: não avisou a interface: {e}");
    }
}

fn engine_of(app: &AppHandle) -> Result<Arc<Engine>, String> {
    app.try_state::<Arc<Engine>>()
        .map(|e| e.inner().clone())
        .ok_or_else(|| "failed".to_owned())
}

fn services(app: &AppHandle) -> Result<(Arc<SyncService>, tauri::State<'_, Alvanista>), String> {
    let sync = app
        .try_state::<Arc<SyncService>>()
        .ok_or_else(|| "failed".to_owned())?
        .inner()
        .clone();
    let alva = app
        .try_state::<Alvanista>()
        .ok_or_else(|| "failed".to_owned())?;
    Ok((sync, alva))
}

#[tauri::command]
pub async fn sync_status(app: AppHandle) -> Result<SyncStatus, String> {
    let (sync, alva) = services(&app)?;
    sync.status_with(&alva, current_hardware(&app))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sync_set_enabled(
    app: AppHandle,
    enabled: bool,
    lang: Option<String>,
) -> Result<SyncStatus, String> {
    let (sync, alva) = services(&app)?;
    let lang = Lang::from_tag(lang.as_deref().unwrap_or("pt-BR"));
    let status = sync
        .set_enabled_with(&alva, enabled, current_hardware(&app), || {
            alva.reconnect_with_saves(lang, crate::alvanista::open_in_browser)
        })
        .await
        .map_err(|e| e.to_string());
    emit_status(&app);
    status
}

#[tauri::command]
pub async fn sync_set_label(app: AppHandle, label: String) -> Result<SyncStatus, String> {
    let (sync, alva) = services(&app)?;
    let status = sync
        .set_label_with(&alva, &label, current_hardware(&app))
        .await
        .map_err(|e| e.to_string());
    emit_status(&app);
    status
}

#[tauri::command]
pub async fn sync_set_paused(app: AppHandle, paused: bool) -> Result<SyncStatus, String> {
    let (sync, alva) = services(&app)?;
    let status = sync
        .set_paused_with(&alva, paused, current_hardware(&app))
        .await
        .map_err(|e| e.to_string());
    emit_status(&app);
    status
}

/// O interruptor dos game saves: desligado, nenhuma passada sobe nem baixa
/// save (a base e os conflitos ficam como estão).
#[tauri::command]
pub async fn sync_set_saves_enabled(app: AppHandle, enabled: bool) -> Result<SyncStatus, String> {
    let (sync, alva) = services(&app)?;
    let status = sync
        .set_saves_enabled_with(&alva, enabled, current_hardware(&app))
        .await
        .map_err(|e| e.to_string());
    emit_status(&app);
    status
}

/// O interruptor dos save states: desligado, nenhum state sobe (a base e os
/// arquivos ficam como estão).
#[tauri::command]
pub async fn sync_set_states_enabled(app: AppHandle, enabled: bool) -> Result<SyncStatus, String> {
    let (sync, alva) = services(&app)?;
    let status = sync
        .set_states_enabled_with(&alva, enabled, current_hardware(&app))
        .await
        .map_err(|e| e.to_string());
    emit_status(&app);
    status
}

/// "Enviar os states antigos também" (D127): os states que um "apagar tudo"
/// no site deixou só nesta máquina sobem de novo.
#[tauri::command]
pub async fn sync_upload_old_states(app: AppHandle) -> Result<SyncStatus, String> {
    let engine = engine_of(&app)?;
    let (sync, alva) = services(&app)?;
    let sent = engine.upload_old_states(&alva).await;
    emit_status(&app);
    sent.map_err(|e| e.to_string())?;
    sync.status_with(&alva, current_hardware(&app))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sync_machines(app: AppHandle) -> Result<Vec<SyncMachine>, String> {
    let (sync, alva) = services(&app)?;
    sync.machines_with(&alva).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sync_now(app: AppHandle) -> Result<SyncStatus, String> {
    let (sync, alva) = services(&app)?;
    let engine = engine_of(&app).ok();
    let status = sync
        .now_with(&alva, current_hardware(&app), engine.as_deref(), || {
            engine::spawn_state_pass(&app)
        })
        .await
        .map_err(|e| e.to_string());
    emit_status(&app);
    status
}

/// Por que cada state de uma ROM fica só nesta máquina (sem rede).
#[tauri::command]
pub async fn sync_state_notes(app: AppHandle, rom_id: String) -> Result<Vec<StateNote>, String> {
    let (sync, _) = services(&app)?;
    sync.state_notes(&rom_id).await.map_err(|e| e.to_string())
}

/// Os conflitos abertos (D118).
#[tauri::command]
pub async fn sync_conflicts(app: AppHandle) -> Result<Vec<SyncConflict>, String> {
    engine_of(&app)?
        .conflicts()
        .await
        .map_err(|e| e.to_string())
}

/// Fecha um conflito: `keep` é `local` ou `remote`.
#[tauri::command]
pub async fn sync_resolve(app: AppHandle, key: String, keep: String) -> Result<(), String> {
    let keep = match keep.as_str() {
        "local" => Keep::Local,
        "remote" => Keep::Remote,
        other => {
            log::warn!("sync: lado desconhecido para resolver: {other}");
            return Err(SyncError::Failed(other.to_owned()).to_string());
        }
    };
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    engine
        .resolve(&alva, &key, keep)
        .await
        .map_err(|e| e.to_string())
}

/// As versões do save de uma chave, da mais nova para a mais velha.
#[tauri::command]
pub async fn sync_versions(app: AppHandle, key: String) -> Result<Vec<SyncVersion>, String> {
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    engine
        .versions(&alva, &key)
        .await
        .map_err(|e| e.to_string())
}

/// Volta uma versão do histórico (D124).
#[tauri::command]
pub async fn sync_restore(app: AppHandle, key: String, version_id: String) -> Result<(), String> {
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    engine
        .restore(&alva, &key, &version_id)
        .await
        .map_err(|e| e.to_string())
}

/// O core que vale para os states de outras máquinas: o que a tela diz
/// (o escolhido na página do jogo) ou, sem isso, o que está aberto (o menu de
/// pausa). E a versão dele instalada aqui, lida do binário (D122).
fn state_core(app: &AppHandle, core_id: Option<String>) -> (Option<String>, Option<String>) {
    let Some(state) = app.try_state::<crate::commands::AppState>() else {
        return (core_id.filter(|c| !c.is_empty()), None);
    };
    let core = core_id
        .filter(|c| !c.is_empty())
        .or_else(|| state.session.loaded_core());
    let installed = core
        .as_deref()
        .and_then(|c| emu_session::core_version(&state.cores_dir, c));
    (core, installed)
}

/// Os save states que as outras máquinas da conta subiram para esta ROM
/// (D116). Só lê: sem sync, com os states desligados ou com a listagem
/// falhando devolve a lista vazia com `offline`.
#[tauri::command]
pub async fn sync_states_for(
    app: AppHandle,
    rom_id: String,
    core_id: Option<String>,
) -> Result<RemoteStates, String> {
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    let (core, installed) = state_core(&app, core_id);
    Ok(engine
        .states_for(&alva, &rom_id, core.as_deref(), installed.as_deref())
        .await)
}

/// Baixa um state de outra máquina para a vaga dele, quando o core e a versão
/// batem. `slot_occupied`: a vaga tem um state, e `overwrite` é falso.
#[tauri::command]
pub async fn sync_download_state(
    app: AppHandle,
    rom_id: String,
    core_id: Option<String>,
    save_id: String,
    overwrite: bool,
) -> Result<crate::commands::SaveStateDto, String> {
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    let (core, installed) = state_core(&app, core_id);
    let meta = engine
        .download_state(
            &alva,
            &rom_id,
            core.as_deref(),
            installed.as_deref(),
            &save_id,
            overwrite,
        )
        .await?;
    // O state virou desta máquina: sobe de novo como dela (D116).
    engine::after_state_saved(&app, rom_id);
    Ok(crate::commands::save_dto(meta, installed.as_deref()))
}

/// O PNG da miniatura de um state de outra máquina (vazio se não tem).
#[tauri::command]
pub async fn sync_state_thumbnail(
    app: AppHandle,
    save_id: String,
) -> Result<tauri::ipc::Response, String> {
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    Ok(tauri::ipc::Response::new(
        engine.state_thumbnail(&alva, &save_id).await,
    ))
}

/// "Enviar as desta máquina" (D114): um documento por grupo sobe como linha
/// `config` deste aparelho, a aparência (`appearance`) vinda da tela. Devolve
/// o resultado de cada grupo e quando (segundos Unix), se algum subiu; rejeita
/// só quando nada pôde ser tentado.
#[tauri::command]
pub async fn sync_settings_upload(
    app: AppHandle,
    appearance: serde_json::Value,
) -> Result<SettingsUpload, String> {
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    let sent = engine.settings_upload(&alva, &appearance).await;
    emit_status(&app);
    sent.map_err(|e| e.to_string())
}

/// O último envio das configurações e os grupos que mudaram desde ele
/// (`appearance`: a aparência de agora, vinda da tela). Sem rede.
#[tauri::command]
pub async fn sync_settings_sent(
    app: AppHandle,
    appearance: serde_json::Value,
) -> Result<SettingsSent, String> {
    engine_of(&app)?
        .settings_sent(&appearance)
        .await
        .map_err(|e| e.to_string())
}

/// O que a cópia das configurações de outra máquina traria: os grupos que ela
/// enviou, o que de cada um não viria e se o vídeo começa desmarcado (GPU ou
/// backend diferentes). Não muda nada.
#[tauri::command]
pub async fn sync_settings_preview(
    app: AppHandle,
    device_id: String,
) -> Result<SettingsPreview, String> {
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    engine
        .settings_preview(&alva, &device_id, &current_hardware(&app))
        .await
        .map_err(|e| e.to_string())
}

/// Copia os grupos marcados de outra máquina, numa transação só, depois de
/// guardar os desta para o "Desfazer". A aparência volta como documento para
/// a tela aplicar. `game_open` com um jogo aberto.
#[tauri::command]
pub async fn sync_settings_copy(
    app: AppHandle,
    device_id: String,
    groups: Vec<String>,
    appearance_wanted: bool,
) -> Result<CopyReport, String> {
    let engine = engine_of(&app)?;
    let (_, alva) = services(&app)?;
    engine
        .settings_copy(&alva, &device_id, &groups, appearance_wanted)
        .await
        .map_err(|e| e.to_string())
}

/// A cópia que o "Desfazer" volta (de qual máquina, quando, quais grupos), ou
/// `None` sem cópia a desfazer. Só o banco: nem chaveiro nem rede.
#[tauri::command]
pub async fn sync_settings_undo_info(app: AppHandle) -> Result<Option<SettingsUndoInfo>, String> {
    engine_of(&app)?
        .settings_undo_info()
        .await
        .map_err(|e| e.to_string())
}

/// Volta as configurações de antes da última cópia (uma vez por cópia); o de
/// agora vai antes para a pasta de resguardo. `false` quando não há cópia a
/// desfazer. Sem rede.
#[tauri::command]
pub async fn sync_settings_undo(app: AppHandle) -> Result<bool, String> {
    engine_of(&app)?
        .settings_undo()
        .await
        .map_err(|e| e.to_string())
}

/// "Esquecer" a última cópia cujo "Desfazer" não volta: só o marcador sai, os
/// arquivos ficam na pasta de resguardo e nenhuma configuração muda. Sem rede.
#[tauri::command]
pub async fn sync_settings_undo_forget(app: AppHandle) -> Result<(), String> {
    engine_of(&app)?
        .settings_undo_forget()
        .await
        .map_err(|e| e.to_string())
}

/// A linha de save da página do jogo (sem rede).
#[tauri::command]
pub async fn sync_save_status(app: AppHandle, rom_id: String) -> Result<SaveLineStatus, String> {
    engine_of(&app)?
        .save_status(&rom_id)
        .await
        .map_err(|e| e.to_string())
}

/// Abre a pasta dos backups do sync (a cria se ainda não existe). Com `key`
/// de um save apagado no site, abre a pasta onde o arquivo dele foi parar.
#[tauri::command]
pub async fn sync_open_backups(app: AppHandle, key: Option<String>) -> Result<(), String> {
    let root = crate::data_dir(&app).join("sync-backups");
    std::fs::create_dir_all(&root).map_err(|e| {
        log::warn!("sync: não criou a pasta de backups: {e}");
        "failed".to_owned()
    })?;
    let removed = match (key, engine_of(&app).ok()) {
        (Some(key), Some(engine)) => engine.removed_folder(&key).await,
        _ => None,
    };
    let dir = removed.unwrap_or(root);
    open::that_detached(&dir).map_err(|e| {
        log::warn!("sync: não abriu a pasta de backups: {e}");
        "failed".to_owned()
    })
}

fn config_scope_saves() -> &'static str {
    crate::alvanista::config::SCOPE_SAVES
}

/// `Failed` leva a mensagem, mas `Display` só mostra o código: o texto
/// (sem segredos) vai para o log onde o erro nasce.
fn failed(e: String) -> SyncError {
    log::warn!("sync: falhou: {e}");
    SyncError::Failed(e)
}

/// O hardware da máquina agora, no formato do servidor.
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

    /// Logger de teste que guarda o texto de cada linha. Um por processo: todo
    /// teste que lê o log passa por `captured_logs`.
    struct Capture(std::sync::Mutex<Vec<String>>);
    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            // O servidor de mentira despeja os pedidos que não esperava (com os
            // cabeçalhos): é ferramenta de teste, não o log do app.
            if record.target().starts_with("wiremock") {
                return;
            }
            self.0
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(format!("{}", record.args()));
        }
        fn flush(&self) {}
    }
    static CAPTURE: Capture = Capture(std::sync::Mutex::new(Vec::new()));
    static INSTALL: std::sync::Once = std::sync::Once::new();

    /// Instala o logger de captura (na primeira chamada) e devolve as linhas
    /// até agora. Se outro logger já estava instalado, falha alto: um teste de
    /// "nada vaza no log" que não vê o log não prova nada.
    pub fn captured_logs() -> Vec<String> {
        INSTALL.call_once(|| {
            log::set_logger(&CAPTURE).expect("não deu pra instalar o logger de captura");
            log::set_max_level(log::LevelFilter::Trace);
        });
        CAPTURE.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::{Client, Entitlements, Me};
    use crate::alvanista::store::test_support::{tokens, MemSecrets, SERIAL};
    use crate::alvanista::store::Store;
    use crate::alvanista::test_api::{browser, FamilyApi};
    use std::sync::atomic::Ordering::SeqCst;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{body_partial_json, header, method, path};
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
            // O corpo da porta do plano, como `writeSaveDenial` o monta.
            ResponseTemplate::new(402).set_body_json(serde_json::json!({
                "error": {"code": "tier_insufficient", "current_tier": "gamer",
                          "required_quota": "save_sync_enabled", "required_value": true,
                          "upgrade_to": "collector"}
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
        // O aparelho novo leva o nome do antigo.
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/save-devices"))
            .and(body_partial_json(serde_json::json!({"label": "Velho"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(minted_body("asv_new")))
            .expect(1)
            .mount(&r.server)
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

    async fn store_device(r: &Rig) {
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
        r.sync.devices().set_enabled(true).await.unwrap();
    }

    fn remote(id: &str, revoked: bool, paused: bool) -> serde_json::Value {
        let mut d = serde_json::json!({
            "id": id, "label": format!("Maquina {id}"), "client_kind": "reemu",
            "created_at": "2026-11-13T10:00:00Z",
            "last_used_at": "2026-11-13T11:00:00Z",
        });
        if revoked {
            d["revoked_at"] = "2026-11-13T12:00:00Z".into();
        }
        if paused {
            d["paused_at"] = "2026-11-13T12:00:00Z".into();
        }
        d
    }

    async fn mount_list(server: &MockServer, devices: Vec<serde_json::Value>) {
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/save-devices"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "devices": devices
            })))
            .mount(server)
            .await;
    }

    async fn never_reconnect() -> Result<AlvanistaStatus, String> {
        panic!("não devia pedir consentimento");
    }

    #[tokio::test]
    async fn set_enabled_on_without_session_is_no_account() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        r.store.unmark_session().await.unwrap();
        let err = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), never_reconnect)
            .await;
        assert_eq!(err, Err(SyncError::NoAccount));
        assert!(r.server.received_requests().await.unwrap().is_empty());
        assert_eq!(r.sync.devices().device().await.unwrap(), None);
    }

    #[tokio::test]
    async fn set_enabled_on_creates_the_device_and_turns_the_flag_on() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        mount_mint(
            &r.server,
            ResponseTemplate::new(201).set_body_json(minted_body("asv_secret")),
            1,
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/quota"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"used_bytes": 10, "limit_bytes": 100})),
            )
            .mount(&r.server)
            .await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        let st = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), never_reconnect)
            .await
            .unwrap();
        assert!(st.enabled);
        assert_eq!(st.availability, Availability::Ready);
        assert_eq!(st.device_id.as_deref(), Some("dev1"));
        assert_eq!(
            st.quota,
            Some(SyncQuota {
                used_bytes: 10,
                limit_bytes: 100
            })
        );
    }

    /// M3: ligar o sync, ou ligar de novo os game saves, roda uma passada já
    /// (desligar não roda nada).
    #[tokio::test]
    async fn turning_sync_or_game_saves_on_runs_a_pass_now() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = passes.clone();
        r.sync.on_enabled(Box::new(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        let count = || passes.load(std::sync::atomic::Ordering::SeqCst);
        mount_mint(
            &r.server,
            ResponseTemplate::new(201).set_body_json(minted_body("asv_secret")),
            1,
        )
        .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/quota"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"used_bytes": 1, "limit_bytes": 9})),
            )
            .mount(&r.server)
            .await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        r.sync
            .set_enabled_with(&r.alva, true, hw(), never_reconnect)
            .await
            .unwrap();
        assert_eq!(count(), 1);
        r.sync
            .set_saves_enabled_with(&r.alva, false, hw())
            .await
            .unwrap();
        assert_eq!(count(), 1);
        r.sync
            .set_saves_enabled_with(&r.alva, true, hw())
            .await
            .unwrap();
        assert_eq!(count(), 2);
    }

    /// O interruptor dos states (R28, mesma forma do dos game saves): vai no
    /// estado da página, desligar não roda passada e zera a conta dos states
    /// só desta máquina, ligar roda uma passada já. Um não mexe no outro.
    #[tokio::test]
    async fn the_states_switch_is_in_the_status_and_turning_it_on_runs_a_pass() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = passes.clone();
        r.sync.on_enabled(Box::new(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        let count = || passes.load(std::sync::atomic::Ordering::SeqCst);
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert!(st.states_enabled && st.saves_enabled);
        r.sync.runtime().states_local.insert(
            ("r1".into(), "state:snes/c/a__slot1.state".into()),
            ("unversioned", "st-a".into()),
        );
        r.sync.runtime().states_local.insert(
            ("r1".into(), "state:snes/c/b__slot1.state".into()),
            ("wiped", "st-b".into()),
        );
        let counted_now = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(counted_now.states_local_only, 2);
        assert_eq!(counted_now.states_wiped, 1, "só o apagado no site");
        r.sync
            .runtime()
            .states_local
            .remove(&("r1".to_owned(), "state:snes/c/b__slot1.state".to_owned()));
        assert_eq!(
            r.sync
                .status_with(&r.alva, hw())
                .await
                .unwrap()
                .states_local_only,
            1
        );

        let st = r
            .sync
            .set_states_enabled_with(&r.alva, false, hw())
            .await
            .unwrap();
        assert!(!st.states_enabled);
        assert!(st.saves_enabled);
        assert_eq!(st.states_local_only, 0);
        assert_eq!(st.states_wiped, 0);
        assert_eq!(count(), 0);
        assert!(r.sync.runtime().states_local.is_empty());

        let st = r
            .sync
            .set_states_enabled_with(&r.alva, true, hw())
            .await
            .unwrap();
        assert!(st.states_enabled);
        assert_eq!(count(), 1);
    }

    /// N3: com os states desligados a página não mostra aviso de state (nem
    /// um guardado); esquecer o aparelho tira os avisos e muda a época, para a
    /// passada de states em curso não gravar mais nada.
    #[tokio::test]
    async fn state_banners_hide_while_states_are_off_and_go_with_the_device() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        r.sync.runtime().state_errors.insert(
            "r1".into(),
            (SyncError::StateTooLarge, Some("a__slot1.state".into())),
        );
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(st.last_error.as_deref(), Some("state_too_large"));
        assert_eq!(st.last_error_file.as_deref(), Some("a__slot1.state"));

        r.sync.devices().set_states_enabled(false).await.unwrap();
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!((st.last_error, st.last_error_file), (None, None));
        r.sync.devices().set_states_enabled(true).await.unwrap();

        let epoch = r.sync.runtime().states_epoch;
        r.sync.forget_device(false).await.unwrap();
        let rt = r.sync.runtime();
        assert!(rt.state_errors.is_empty());
        assert!(rt.states_epoch > epoch);
    }

    /// M2: a contagem de conflitos vem do SQLite: vale logo ao abrir o app
    /// (antes de qualquer passada) e com os game saves desligados.
    #[tokio::test]
    async fn the_conflict_count_comes_from_sqlite_after_a_restart_and_with_saves_off() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        SyncRepo::new(r.db.clone())
            .conflict_put(&domain::sync::ConflictRow {
                key: "file:gba/Zelda.srm".into(),
                local_hash: "l".into(),
                local_mtime: 1,
                local_size: 1,
                remote_save_id: "s".into(),
                remote_version_id: "v".into(),
                remote_hash: "r".into(),
                remote_device_label: None,
                remote_client_mtime: None,
                remote_size: 1,
                remote_deleted: false,
                rom_id: None,
                seen_at: 1,
            })
            .await
            .unwrap();
        // Um serviço novo sobre o mesmo banco: o app acabou de abrir.
        let fresh = SyncService::new(
            SavesApi::new(r.server.uri()),
            DeviceStore::new(r.db.clone(), r.secrets.clone()),
        );
        assert_eq!(fresh.status_with(&r.alva, hw()).await.unwrap().conflicts, 1);
        fresh
            .set_saves_enabled_with(&r.alva, false, hw())
            .await
            .unwrap();
        assert_eq!(fresh.status_with(&r.alva, hw()).await.unwrap().conflicts, 1);
    }

    #[tokio::test]
    async fn set_enabled_on_runs_the_consent_once_and_retries() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/save-devices"))
            .respond_with(
                ResponseTemplate::new(403)
                    .set_body_json(serde_json::json!({"error": "insufficient_scope"})),
            )
            .up_to_n_times(1)
            .with_priority(1)
            .expect(1)
            .mount(&r.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/save-devices"))
            .respond_with(ResponseTemplate::new(201).set_body_json(minted_body("asv_secret")))
            .with_priority(2)
            .expect(1)
            .mount(&r.server)
            .await;
        let asked = std::sync::atomic::AtomicUsize::new(0);
        let st = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), || async {
                asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(AlvanistaStatus::default())
            })
            .await
            .unwrap();
        assert!(st.enabled);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_refused_consent_surfaces_its_code_and_creates_nothing() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        mount_mint(
            &r.server,
            ResponseTemplate::new(403)
                .set_body_json(serde_json::json!({"error": "insufficient_scope"})),
            1,
        )
        .await;
        let err = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), || async { Err("denied".to_owned()) })
            .await;
        assert_eq!(err, Err(SyncError::Connect("denied".into())));
        assert_eq!(err.unwrap_err().to_string(), "denied");
        assert_eq!(r.sync.devices().device().await.unwrap(), None);
    }

    #[tokio::test]
    async fn set_enabled_off_revokes_and_forgets() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .and(header("authorization", "Bearer asv_secret"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&r.server)
            .await;
        mount_list(&r.server, vec![]).await;
        let st = r
            .sync
            .set_enabled_with(&r.alva, false, hw(), never_reconnect)
            .await
            .unwrap();
        assert!(!st.enabled);
        assert_eq!(st.device_id, None);
        assert_eq!(r.sync.devices().token().unwrap(), None, "token no chaveiro");
        assert!(!r.sync.devices().enabled().await.unwrap());
        for v in test_support::all_state_values(&r.db).await {
            assert!(!v.starts_with("sync."), "sobrou estado do sync: {v}");
        }
    }

    #[tokio::test]
    async fn set_enabled_off_ignores_a_401_and_still_forgets() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(serde_json::json!({"error": "invalid_token"})),
            )
            .expect(1)
            .mount(&r.server)
            .await;
        r.sync
            .set_enabled_with(&r.alva, false, hw(), never_reconnect)
            .await
            .unwrap();
        assert_eq!(r.sync.devices().device().await.unwrap(), None);
        assert_eq!(r.sync.devices().token().unwrap(), None);
    }

    #[tokio::test]
    async fn forgetting_without_a_device_touches_neither_keyring_nor_network() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        let before = r.secrets.calls.load(std::sync::atomic::Ordering::SeqCst);
        r.sync.forget_device(true).await.unwrap();
        assert_eq!(
            r.secrets.calls.load(std::sync::atomic::Ordering::SeqCst),
            before
        );
        assert!(r.server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn forget_without_revoke_sends_no_delete() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        r.sync.forget_device(false).await.unwrap();
        assert!(r.server.received_requests().await.unwrap().is_empty());
        assert_eq!(r.sync.devices().token().unwrap(), None);
    }

    #[tokio::test]
    async fn status_reports_site_pause_from_the_device_list() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(
            &r.server,
            vec![remote("dev1", false, true), remote("dev2", false, false)],
        )
        .await;
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert!(st.paused_site);
        assert!(!st.revoked);
        assert!(st.enabled && !st.paused_local);
        assert_eq!(st.label, "Sala");
        // O último valor conhecido fica guardado para quando a lista não responder.
        assert_eq!(r.sync.devices().site_state().await.unwrap(), (true, false));
    }

    #[tokio::test]
    async fn status_falls_back_to_the_last_known_site_state_when_the_list_fails() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        r.sync.devices().set_site_state(true, true).await.unwrap();
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/save-devices"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&r.server)
            .await;
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert!(st.paused_site);
        assert!(st.revoked);
    }

    #[tokio::test]
    async fn status_reports_a_revoked_device_from_the_list() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", true, false)]).await;
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert!(st.revoked);
    }

    #[tokio::test]
    async fn status_without_a_session_is_no_account_and_asks_nothing() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        r.store.unmark_session().await.unwrap();
        let calls = || r.secrets.calls.load(std::sync::atomic::Ordering::SeqCst);
        let before = calls();
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(st.availability, Availability::NoAccount);
        assert_eq!(calls(), before, "tocou o chaveiro");
        assert!(r.server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn status_with_a_session_and_no_device_is_ready_and_off() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(st.availability, Availability::Ready);
        assert!(!st.enabled);
        assert_eq!(st.device_id, None);
        assert!(r.server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn set_label_patches_and_stores_what_the_server_returns() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "dev1", "label": "Quarto", "client_kind": "reemu",
                "created_at": "2026-11-13T10:00:00Z"
            })))
            .expect(1)
            .mount(&r.server)
            .await;
        let st = r
            .sync
            .set_label_with(&r.alva, " Quarto ", hw())
            .await
            .unwrap();
        assert_eq!(st.label, "Quarto");
        assert!(r.sync.set_label_with(&r.alva, "  ", hw()).await.is_err());
        let long = "x".repeat(81);
        assert!(r.sync.set_label_with(&r.alva, &long, hw()).await.is_err());
    }

    #[tokio::test]
    async fn pausing_is_local_only() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        let st = r.sync.set_paused_with(&r.alva, true, hw()).await.unwrap();
        assert!(st.paused_local);
        let reqs = r.server.received_requests().await.unwrap();
        assert!(reqs.iter().all(|q| q.method.as_str() == "GET"));
    }

    #[tokio::test]
    async fn machines_lists_active_devices_and_marks_this_one() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(
            &r.server,
            vec![
                remote("dev1", false, false),
                remote("dev2", true, false),
                remote("dev3", false, true),
            ],
        )
        .await;
        let list = r.sync.machines_with(&r.alva).await.unwrap();
        let ids: Vec<_> = list
            .iter()
            .map(|m| (m.id.as_str(), m.is_this, m.paused))
            .collect();
        assert_eq!(ids, vec![("dev1", true, false), ("dev3", false, true)]);
    }

    #[tokio::test]
    async fn sync_now_refreshes_quota_and_the_list() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/quota"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"used_bytes": 5, "limit_bytes": 50})),
            )
            .expect(1)
            .mount(&r.server)
            .await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .respond_with(ResponseTemplate::new(200).set_body_json(remote("dev1", false, false)))
            .mount(&r.server)
            .await;
        let st = r.sync.now_with(&r.alva, hw(), None, || {}).await.unwrap();
        assert_eq!(st.quota.map(|q| q.used_bytes), Some(5));
        assert_eq!(st.last_error, None);
    }

    #[tokio::test]
    async fn sync_now_with_a_revoked_token_marks_the_device_revoked() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![]).await;
        for (m, p) in [
            ("GET", "/api/v1/saves/quota"),
            ("PATCH", "/api/v1/saves/device"),
        ] {
            Mock::given(method(m))
                .and(path(p))
                .respond_with(
                    ResponseTemplate::new(401)
                        .set_body_json(serde_json::json!({"error": "invalid_token"})),
                )
                .mount(&r.server)
                .await;
        }
        let st = r.sync.now_with(&r.alva, hw(), None, || {}).await.unwrap();
        assert!(st.revoked);
        assert_eq!(st.last_error.as_deref(), Some("revoked"));
    }

    #[tokio::test]
    async fn status_reports_a_device_missing_from_the_list_as_revoked() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        r.sync.devices().set_site_state(true, false).await.unwrap();
        mount_list(&r.server, vec![remote("other", false, false)]).await;
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert!(st.revoked);
        assert!(st.paused_site, "o pause guardado se mantém");
    }

    #[tokio::test]
    async fn set_enabled_on_mints_a_new_device_when_the_stored_one_is_missing_from_the_list() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        // O nome que a pessoa escolheu antes de a máquina sair do site.
        r.sync.devices().set_label("Quarto do Ed").await.unwrap();
        mount_list(&r.server, vec![remote("other", false, false)]).await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&r.server)
            .await;
        // "Ligar de novo" mantém o nome, em vez de voltar ao da máquina.
        Mock::given(method("POST"))
            .and(path("/api/partner/v1/save-devices"))
            .and(body_partial_json(
                serde_json::json!({"label": "Quarto do Ed"}),
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(minted_body("asv_new")))
            .expect(1)
            .mount(&r.server)
            .await;
        let st = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), never_reconnect)
            .await
            .unwrap();
        assert_eq!(st.device_id.as_deref(), Some("dev1"));
        assert_eq!(
            r.sync.devices().token().unwrap().as_deref(),
            Some("asv_new")
        );
    }

    #[tokio::test]
    async fn partner_devices_renews_once_and_retries_the_list() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        r.store
            .start_session(&tokens("7"), &["profile:read", "saves"])
            .await
            .unwrap();
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/save-devices"))
            .and(header("authorization", "Bearer alp_7"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&r.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "alp_8", "refresh_token": "alr_8", "token_type": "Bearer",
                "expires_in": 3600
            })))
            .expect(1)
            .mount(&r.server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/save-devices"))
            .and(header("authorization", "Bearer alp_8"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "devices": [remote("dev9", false, false)]
            })))
            .expect(1)
            .mount(&r.server)
            .await;
        let list = r.sync.machines_with(&r.alva).await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn partner_devices_forgets_and_revokes_the_device_when_the_session_ended() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        r.store
            .start_session(&tokens("9"), &["profile:read", "saves"])
            .await
            .unwrap();
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/save-devices"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&r.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/token"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"error": "invalid_grant"})),
            )
            .mount(&r.server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .and(header("authorization", "Bearer asv_secret"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&r.server)
            .await;
        // O `mint` preso por outra chamada não pode travar o esquecer.
        let _minting = r.sync.hold_mint_for_test().await;
        let got = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            r.sync.status_with(&r.alva, hw()),
        )
        .await
        .expect("travou")
        .unwrap();
        assert_eq!(got.availability, Availability::NoAccount);
        assert_eq!(got.device_id, None);
        assert_eq!(r.sync.devices().token().unwrap(), None);
    }

    #[tokio::test]
    async fn a_failed_revoke_leaves_offline_for_the_page() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        Mock::given(method("DELETE"))
            .and(path("/api/v1/saves/device"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&r.server)
            .await;
        mount_list(&r.server, vec![]).await;
        let st = r
            .sync
            .set_enabled_with(&r.alva, false, hw(), never_reconnect)
            .await
            .unwrap();
        assert_eq!(st.device_id, None);
        assert_eq!(st.last_error.as_deref(), Some("offline"));
    }

    #[tokio::test]
    async fn label_and_rename_errors_have_their_own_codes() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        // Sem aparelho: not_enabled.
        assert_eq!(
            r.sync.set_label_with(&r.alva, "Sala", hw()).await,
            Err(SyncError::NotEnabled)
        );
        assert_eq!(
            r.sync.set_label_with(&r.alva, "", hw()).await,
            Err(SyncError::InvalidLabel)
        );
        // O servidor recusa o nome: invalid_label.
        store_device(&r).await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({"error": {"code": "invalid_label"}})),
            )
            .mount(&r.server)
            .await;
        assert_eq!(
            r.sync.set_label_with(&r.alva, "Sala", hw()).await,
            Err(SyncError::InvalidLabel)
        );
        // Sem sessão: antes de qualquer chaveiro ou rede.
        r.store.unmark_session().await.unwrap();
        let before = r.secrets.calls.load(std::sync::atomic::Ordering::SeqCst);
        let n = r.server.received_requests().await.unwrap().len();
        assert_eq!(
            r.sync.set_label_with(&r.alva, "Sala", hw()).await,
            Err(SyncError::NoAccount)
        );
        assert_eq!(
            r.secrets.calls.load(std::sync::atomic::Ordering::SeqCst),
            before
        );
        assert_eq!(r.server.received_requests().await.unwrap().len(), n);
    }

    /// M12: renomear com um 401 do `SaveAuth` não basta para dizer "removida
    /// no site": com a linha viva na lista é `offline` e nada vira revogado;
    /// com a linha revogada, aí sim.
    #[tokio::test]
    async fn a_rename_401_with_the_row_alive_is_offline_not_revoked() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(serde_json::json!({"error": {"code": "save_token_invalid"}})),
            )
            .mount(&r.server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/save-devices"))
            .respond_with(seq_list(vec![
                vec![remote("dev1", false, false)],
                vec![remote("dev1", true, false)],
            ]))
            .mount(&r.server)
            .await;
        assert_eq!(
            r.sync.set_label_with(&r.alva, "Quarto", hw()).await,
            Err(SyncError::Offline)
        );
        assert_eq!(r.sync.devices().site_state().await.unwrap(), (false, false));
        assert_eq!(
            r.sync.devices().token().unwrap().as_deref(),
            Some("asv_secret")
        );
        // A lista confirma a remoção: revogada.
        assert_eq!(
            r.sync.set_label_with(&r.alva, "Quarto", hw()).await,
            Err(SyncError::Revoked)
        );
        assert_eq!(r.sync.devices().site_state().await.unwrap(), (false, true));
    }

    /// Respostas da lista de aparelhos em sequência; a última se repete.
    fn seq_list(lists: Vec<Vec<serde_json::Value>>) -> impl wiremock::Respond {
        let n = std::sync::atomic::AtomicUsize::new(0);
        move |_: &wiremock::Request| {
            let i = n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"devices": lists[i.min(lists.len() - 1)]}))
        }
    }

    #[tokio::test]
    async fn an_unknown_reconnect_code_becomes_failed() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        r.store
            .start_session(&tokens("5"), &["profile:read"])
            .await
            .unwrap();
        let err = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), || async { Err("weird".to_owned()) })
            .await;
        assert_eq!(err.unwrap_err().to_string(), "failed");
    }

    #[tokio::test]
    async fn without_the_saves_scope_nothing_is_sent_to_the_mint() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        r.store
            .start_session(&tokens("5"), &["profile:read"])
            .await
            .unwrap();
        mount_mint(&r.server, ResponseTemplate::new(500), 0).await;
        let err = r.sync.ensure_device_with(&r.alva, None, hw()).await;
        assert_eq!(err, Err(SyncError::NeedsConsent));
        assert!(r.server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn sync_now_keeps_the_most_severe_error_and_clears_a_stale_revoked() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        // O hardware volta 423 (paused) e a cota 503 (offline): vale o paused.
        Mock::given(method("PATCH"))
            .and(path("/api/v1/saves/device"))
            .respond_with(ResponseTemplate::new(423))
            .mount(&r.server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v1/saves/quota"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&r.server)
            .await;
        let st = r.sync.now_with(&r.alva, hw(), None, || {}).await.unwrap();
        assert_eq!(st.last_error.as_deref(), Some("paused"));
        // Um "revoked" velho some quando a lista mostra o aparelho vivo.
        r.sync.runtime().last_error = Some("revoked".into());
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(st.last_error, None);
    }

    /// M1: o estado decide pelo SQLite e lê o par do chaveiro uma vez, só
    /// quando vai à rede.
    #[tokio::test]
    async fn the_status_reads_the_keyring_at_most_once() {
        let _serial = SERIAL.lock().await;
        let r = rig().await;
        let gets = || r.secrets.gets.load(SeqCst);
        // Conectado e sem aparelho: nada a perguntar ao site, nem ao chaveiro.
        let before = gets();
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(st.availability, Availability::Ready);
        assert_eq!(gets() - before, 0, "sem aparelho");
        // Com aparelho: a lista do site, com uma leitura do par.
        store_device(&r).await;
        mount_list(&r.server, vec![remote("dev1", false, false)]).await;
        let before = gets();
        r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(gets() - before, 1, "com aparelho");
        // Sem plano: a lista e o `/me` dividem a mesma leitura.
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
        Mock::given(method("GET"))
            .and(path("/api/partner/v1/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "u1", "username": "toy",
                "profile_url": "https://www.alvanista.com/u/toy",
                "entitlements": {"save_sync": false}
            })))
            .expect(1)
            .mount(&r.server)
            .await;
        let before = gets();
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(st.availability, Availability::NoTier);
        assert_eq!(gets() - before, 1, "sem plano");
    }

    /// Alvanista e sync contra o servidor que revoga por família, com uma
    /// sessão (`old`, com `scopes`) que o servidor conhece e o perfil já lido
    /// uma vez (o app abriu conectado).
    struct FamilyRig {
        api: FamilyApi,
        alva: Alvanista,
        sync: SyncService,
        store: Store,
    }

    async fn family_rig(scopes: &[&str]) -> FamilyRig {
        let api = FamilyApi::start(Arc::new(Mutex::new(Vec::new()))).await;
        let secrets = Arc::new(MemSecrets::default());
        let db = db::connect_in_memory().await.unwrap();
        let store = Store::new(db.clone(), secrets.clone());
        let device = store.device_id().await.unwrap();
        api.seed(&tokens("old"), &device, scopes);
        store.start_session(&tokens("old"), scopes).await.unwrap();
        let alva = Alvanista::new(Client::with_api(
            api.uri(),
            Store::new(db.clone(), secrets.clone()),
        ));
        let sync = SyncService::new(SavesApi::new(api.uri()), DeviceStore::new(db, secrets));
        assert!(alva.status().await.connected);
        FamilyRig {
            api,
            alva,
            sync,
            store,
        }
    }

    /// C1: ligar o sync numa sessão só de perfil pede o consentimento,
    /// reconecta e cria o aparelho, e a sessão segue viva num servidor que
    /// revoga a família inteira de um par.
    #[tokio::test]
    async fn turning_sync_on_through_the_consent_keeps_the_session_and_mints_the_device() {
        let _serial = SERIAL.lock().await;
        let r = family_rig(&["profile:read"]).await;
        let st = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), || {
                r.alva.reconnect_with_saves(Lang::PtBr, browser(true))
            })
            .await
            .unwrap();
        assert!(st.enabled, "{st:?}");
        assert_eq!(st.availability, Availability::Ready);
        assert_eq!(st.device_id.as_deref(), Some("dev1"));
        assert_eq!(r.api.minted(), 1);
        assert_eq!(
            r.sync.devices().token().unwrap().as_deref(),
            Some("asv_dev1")
        );
        // Nada foi revogado, e o par guardado é o novo, vivo no servidor.
        let events = r.api.events();
        assert!(
            !events.iter().any(|e| e.starts_with("revoke:")),
            "{events:?}"
        );
        let session = r.store.load_tokens().await.unwrap().unwrap();
        assert_ne!(session, tokens("old"));
        assert!(r.api.is_live(&session.access_token) && r.api.is_live(&session.refresh_token));
        // A conexão segue de pé, agora com `saves`.
        let alva = r.alva.status().await;
        assert!(alva.connected && !alva.ended && !alva.offline, "{alva:?}");
        assert!(alva.scopes.iter().any(|s| s == "saves"), "{alva:?}");
        let again = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert!(again.enabled && again.availability == Availability::Ready);
    }

    /// I2: um 402 do mint vira `no_tier` na página, e assinar no site faz a
    /// página voltar a oferecer o toggle sem reiniciar o app. Um `/me` por
    /// leitura do estado, e nenhum quando o plano já libera.
    #[tokio::test]
    async fn a_402_at_mint_shows_no_tier_and_a_new_plan_turns_the_page_ready() {
        let _serial = SERIAL.lock().await;
        let r = family_rig(&["profile:read", "saves"]).await;
        // O espelho ainda diz que o plano libera; no servidor ele venceu.
        r.api.set_save_sync(false);
        let err = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), never_reconnect)
            .await;
        assert_eq!(err, Err(SyncError::NoTier));
        assert_eq!(
            r.store.mirror_get().await.unwrap().unwrap().save_sync,
            Some(false)
        );
        let me = r.api.me_calls();
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(st.availability, Availability::NoTier);
        assert_eq!(r.api.me_calls(), me + 1, "um /me por leitura");
        // A pessoa assina no site e volta para a página.
        r.api.set_save_sync(true);
        let st = r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(st.availability, Availability::Ready);
        assert_eq!(r.api.me_calls(), me + 2);
        r.sync.status_with(&r.alva, hw()).await.unwrap();
        assert_eq!(
            r.api.me_calls(),
            me + 2,
            "com plano, o estado não vai ao /me"
        );
        let on = r
            .sync
            .set_enabled_with(&r.alva, true, hw(), never_reconnect)
            .await
            .unwrap();
        assert!(on.enabled && on.device_id.is_some(), "{on:?}");
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
            (SyncError::TooLarge, "too_large"),
            (SyncError::Rejected, "rejected"),
            (SyncError::Offline, "offline"),
            (SyncError::Keyring, "keyring"),
            (SyncError::InvalidLabel, "invalid_label"),
            (SyncError::NotEnabled, "not_enabled"),
            (SyncError::Connect("cancelled".into()), "cancelled"),
            (SyncError::GameOpen, "game_open"),
            (SyncError::SettingsVersion, "settings_version"),
            (SyncError::SettingsInvalid, "settings_invalid"),
            (SyncError::SettingsMissing, "settings_missing"),
            (SyncError::BackupFailed, "backup_failed"),
            (SyncError::Failed("x".into()), "failed"),
        ];
        for (err, code) in codes {
            assert_eq!(err.to_string(), code);
        }
        assert_eq!(SyncError::from(ApiError::Unauthorized), SyncError::Revoked);
        assert_eq!(
            SyncError::from(ApiError::TooLarge { max_bytes: None }),
            SyncError::TooLarge
        );
        assert_eq!(
            SyncError::from(ApiError::Transient("x".into())),
            SyncError::Offline
        );
    }

    /// O formato que `lib/tauri.ts` espera em `SyncEvent`.
    #[test]
    fn sync_events_serialize_as_the_frontend_reads_them() {
        let json = |e: SyncEvent| serde_json::to_value(e).unwrap();
        assert_eq!(
            json(SyncEvent::Status),
            serde_json::json!({"type": "status"})
        );
        assert_eq!(
            json(SyncEvent::Pass {
                uploaded: 1,
                downloaded: 2,
                backed_up: 3
            }),
            serde_json::json!({"type": "pass", "uploaded": 1, "downloaded": 2, "backedUp": 3})
        );
        assert_eq!(
            json(SyncEvent::Conflict { key: "k".into() }),
            serde_json::json!({"type": "conflict", "key": "k"})
        );
        assert_eq!(
            json(SyncEvent::Error {
                code: "revoked".into()
            }),
            serde_json::json!({"type": "error", "code": "revoked"})
        );
        assert_eq!(
            json(SyncEvent::Quota {
                used_bytes: 5,
                limit_bytes: 9
            }),
            serde_json::json!({"type": "quota", "usedBytes": 5, "limitBytes": 9})
        );
        assert_eq!(
            json(SyncEvent::Prelaunch {
                outcome: "skipped".into(),
                reason: Some("timeout".into())
            }),
            serde_json::json!({"type": "prelaunch", "outcome": "skipped", "reason": "timeout"})
        );
    }

    #[tokio::test]
    async fn no_log_line_and_no_state_row_ever_holds_the_device_token() {
        let _serial = SERIAL.lock().await;
        test_support::captured_logs();
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
        let lines = test_support::captured_logs();
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
