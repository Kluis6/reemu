//! O motor do cloud save de game saves: as passadas (ao abrir o app, de 10
//! em 10 minutos sem jogo aberto, antes de abrir um jogo e depois de fechá-lo),
//! o conflito guardado para a pessoa decidir e a volta pelo histórico. Os save
//! states desta máquina sobem na passada completa e logo depois de gravados
//! (`states`).
//!
//! Regras de todo caminho:
//! - nada em disco é apagado: o que é substituído é copiado antes para
//!   `<dados>/sync-backups/<carimbo>/`, e o que a nuvem apagou se MUDA para lá;
//! - com um jogo aberto (ou abrindo) nenhum save é escrito, e os arquivos
//!   desse jogo (e os cartões e VMUs) não sobem;
//! - a base só anda depois do 2xx daquele arquivo;
//! - arquivo vazio nunca sobe nem desce.
//!
//! Cada passada começa pela linha desta máquina na lista de aparelhos do site
//! (D128): pausada, nada mais vai à rede; revogada ou fora da lista, o token
//! sai do chaveiro e o sync desliga (o id e o nome ficam, para "Ligar de novo"
//! criar o aparelho novo com o mesmo nome). Um 401 de `/api/v1/saves` sozinho
//! não prova revogação: o `SaveAuth` do Alvanista também responde 401 quando o
//! armazenamento dele falha. O motor relê a lista e só desliga se ela
//! confirmar; com a linha viva, a passada termina `offline`.

use super::api::{
    ApiError, FileFilter, RemoteDevice, RemoteFile, RemoteVersion, UploadMeta, Uploaded,
};
use super::device::{default_label, rfc3339, Device};
#[cfg(test)]
use super::local::account_saves;
use super::local::{self, log_tag, parse_key, LocalSaves, NameCase, SaveFile};
use super::platform::platform_for_system;
use super::reconcile::{decide, Action, Kind, Local, Remote};
use super::{failed, severity, SyncError, SyncEvent, SyncService, SYNC_EVENT};
use crate::alvanista::store::now_secs;
use crate::alvanista::Alvanista;
use db::{RomsRepo, SyncRepo};
use domain::library::RomRepository;
use domain::sync::{BaseRow, ConflictRow};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

mod remote_states;
pub use remote_states::RemoteStates;
mod settings;
pub use settings::{CopyReport, SettingsPreview, SettingsSent, SettingsUndoInfo, SettingsUpload};
mod app;
mod states;
pub use app::{
    after_credentials_changed, after_state_saved, after_unload, before_launch, install,
    run_on_start, spawn_state_pass,
};
mod backups;
mod requests;
mod steps;
pub(in crate::sync) use states::WIPED as STATE_WIPED;

/// Quanto um jogo espera pela nuvem antes de abrir (D117).
const PRELAUNCH_BUDGET: Duration = Duration::from_secs(5);
/// A passada sem jogo aberto.
const IDLE_EVERY: Duration = Duration::from_secs(600);
/// Uma rajada de saves de state sobe 3 s depois do último.
const STATE_DEBOUNCE: Duration = Duration::from_secs(3);

/// O que a sessão do emulador está fazendo agora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Playing {
    Idle,
    /// `load_game` começou e o core ainda não abriu.
    Loading,
    /// Um core está aberto: o id da ROM quando se sabe qual.
    Running(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassScope {
    /// Os game saves e os save states desta máquina.
    All,
    /// Os saves de uma ROM, mais os cartões e VMUs da plataforma dela.
    Rom(String),
    /// Os save states desta máquina de uma ROM (depois de gravar um).
    State(String),
    /// Todos os save states desta máquina, sem os game saves.
    States,
}

impl PassScope {
    /// A passada olha os game saves?
    fn files(&self) -> bool {
        matches!(self, Self::All | Self::Rom(_))
    }

    /// A passada olha os save states?
    fn states(&self) -> bool {
        matches!(self, Self::All | Self::State(_) | Self::States)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrelaunchOutcome {
    UpToDate,
    Downloaded,
    Conflict,
    /// O jogo abre com o save local; o texto é o código do motivo.
    Skipped(String),
}

impl PrelaunchOutcome {
    /// O aviso para a tela (D117: sem a nuvem a tempo, um toast diz que ela
    /// não pôde ser conferida).
    fn event(&self) -> SyncEvent {
        let (outcome, reason) = match self {
            Self::UpToDate => ("up_to_date", None),
            Self::Downloaded => ("downloaded", None),
            Self::Conflict => ("conflict", None),
            Self::Skipped(reason) => ("skipped", Some(reason.clone())),
        };
        SyncEvent::Prelaunch {
            outcome: outcome.into(),
            reason,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    Local,
    Remote,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PassReport {
    /// Game saves enviados (os states têm a conta deles).
    pub uploaded: u32,
    pub downloaded: u32,
    /// Chaves em conflito nesta passada (novas ou não).
    pub conflicts: u32,
    /// Arquivos que a nuvem apagou e foram para a pasta de backup.
    pub backed_up: u32,
    /// Ações adiadas: jogo aberto, plataforma que esta máquina não recebe,
    /// envios parados pela cota ou pelo plano.
    pub deferred: u32,
    /// Erros de um arquivo só; a passada seguiu.
    pub errors: Vec<(String, SyncError)>,
    /// Save states enviados (sem contar as miniaturas).
    pub states_uploaded: u32,
}

impl PassReport {
    /// O erro que a tela mostra (o mais grave).
    pub fn worst(&self) -> Option<SyncError> {
        self.worst_entry().map(|(_, e)| e.clone())
    }

    /// O erro mais grave e a chave do arquivo dele.
    fn worst_entry(&self) -> Option<&(String, SyncError)> {
        self.errors.iter().max_by_key(|(_, e)| severity(e))
    }
}

/// A nuvem recusou estes bytes de vez (413, 400, 422): a mesma chave com os
/// mesmos bytes não sobe de novo; a passada só repete o erro.
fn is_permanent(e: &SyncError) -> bool {
    matches!(
        e,
        SyncError::TooLarge | SyncError::StateTooLarge | SyncError::Rejected
    )
}

/// A passada de states pode vir depois desta dos game saves? Não quando ela
/// disse que a máquina não sincroniza agora (revogada, sem conta, desligada,
/// pausada, sem chaveiro).
pub fn states_may_follow(files: &Result<PassReport, SyncError>) -> bool {
    !matches!(
        files,
        Err(SyncError::Revoked
            | SyncError::NoAccount
            | SyncError::NotEnabled
            | SyncError::Paused
            | SyncError::Keyring)
    )
}

/// O nome do arquivo de uma chave, para a página dizer qual a nuvem recusou:
/// o do save, ou o do state na nuvem (nunca o caminho).
fn key_file_name(key: &str) -> Option<&str> {
    parse_key(key).map(|(_, _, name)| name).or_else(|| {
        key.strip_prefix("state:")
            .and_then(|rest| rest.rsplit_once('/'))
            .map(|(_, name)| name)
    })
}

/// Um lado do conflito, como o diálogo mostra (D118).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Side {
    pub machine: String,
    /// O id do aparelho: o desta máquina no lado local, o de quem escreveu a
    /// versão da nuvem no remoto (`None` quando a última leitura não o trouxe).
    pub device_id: Option<String>,
    /// Segundos Unix.
    pub saved_at: i64,
    pub size: u64,
    /// Só no lado da nuvem: o save foi apagado no site (não é outra versão,
    /// e não há máquina, data nem tamanho a mostrar).
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SyncConflict {
    pub key: String,
    pub rom_id: Option<String>,
    pub title: String,
    pub local: Side,
    pub remote: Side,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SyncVersion {
    pub version_id: String,
    pub machine: String,
    /// O id do aparelho que escreveu a versão (o da listagem).
    pub device_id: Option<String>,
    /// Segundos Unix.
    pub created_at: i64,
    pub size: u64,
    pub is_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum SaveLineState {
    /// Sem save deste jogo nesta máquina.
    None,
    Synced,
    /// Mudou aqui (ou nunca subiu) e o sync está ligado.
    Pending,
    Conflict,
    /// O save foi apagado no site e o arquivo daqui foi para a pasta de
    /// resguardo (S13).
    Removed,
}

/// A linha de save da página do jogo. Só disco e SQLite: nada de rede.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SaveLineStatus {
    pub state: SaveLineState,
    pub synced_at: Option<i64>,
    /// As versões que a última leitura da nuvem contou (0 sem leitura).
    pub versions: u32,
    pub key: Option<String>,
}

/// As pastas do app que o motor usa.
#[derive(Debug, Clone)]
pub struct Dirs {
    /// `<dados>`: os backups ficam em `<dados>/sync-backups`.
    pub data: PathBuf,
    pub saves: PathBuf,
    pub system: PathBuf,
    /// `<dados>/shaders` e `<dados>/decorations`: os catálogos que o ReEmu
    /// baixa, por onde as configurações acham presets e molduras (D126).
    pub shaders: PathBuf,
    pub decorations: PathBuf,
    /// `<dados>/cores`: os cores instalados (o core preferido de uma
    /// plataforma só vem se ele estiver aqui).
    pub cores: PathBuf,
}

type PlayingFn = Box<dyn Fn() -> Playing + Send + Sync>;
type EmitFn = Box<dyn Fn(SyncEvent) + Send + Sync>;
/// Relê do banco o que o app guarda em memória das configurações (atalhos,
/// teclado, vídeo, áudio), depois de uma cópia ou de desfazê-la.
pub type ReloadFn =
    Box<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pass,
    /// Antes de abrir um jogo: só o que desce, e a escrita vale enquanto o
    /// jogo ainda está abrindo (é ele quem espera).
    BeforeLaunch,
}

/// O que vale durante uma passada.
struct Ctx<'a> {
    alva: &'a Alvanista,
    mode: Mode,
    token: String,
    /// Os aparelhos da conta: o nome da máquina que escreveu cada versão.
    devices: Vec<RemoteDevice>,
    /// As plataformas que esta máquina recebe (`None`: todas).
    receive: Option<Vec<String>>,
    /// A cota ou o plano parou os envios desta passada.
    uploads_stopped: bool,
    /// Envios de state seguidos que falharam na rede (M1): um só fica para a
    /// próxima e a passada segue; três seguidos acabam a passada.
    network_failures: u32,
    /// A época dos game saves quando a passada começou
    /// (`Runtime::saves_epoch`): desligá-los no meio a muda.
    saves_epoch: u64,
}

/// Uma chave: o arquivo neste disco (exista ou não), a base e a linha da
/// nuvem escolhida para ela.
struct Entry {
    file: SaveFile,
    base: Option<BaseRow>,
    row: Option<RemoteFile>,
}

enum Done {
    Nothing,
    Uploaded,
    Downloaded,
    Conflict,
    BackedUp,
    Deferred,
}

enum Fail {
    /// Fica naquele arquivo; a passada segue.
    File(SyncError),
    /// Cota ou plano: param os envios, as descidas seguem (D51).
    StopUploads(SyncError),
    /// Acaba a passada.
    End(SyncError),
}

fn fail_error(f: Fail) -> SyncError {
    match f {
        Fail::File(e) | Fail::StopUploads(e) | Fail::End(e) => e,
    }
}

fn repo_err(e: domain::error::RepoError) -> SyncError {
    failed(format!("base do sync: {e}"))
}

pub struct Engine {
    sync: Arc<SyncService>,
    /// O banco das configurações (enviar, copiar, desfazer).
    db: db::Db,
    repo: SyncRepo,
    roms: RomsRepo,
    dirs: Dirs,
    playing: PlayingFn,
    emit: EmitFn,
    reload: ReloadFn,
    /// Um envio, cópia ou "Desfazer" de configurações por vez.
    settings_lock: tokio::sync::Mutex<()>,
    /// Uma passada (ou resolução, ou restauração) por vez.
    one_at_a_time: tokio::sync::Mutex<()>,
    /// A pasta de backup da passada em andamento.
    stamp: Mutex<String>,
    /// As linhas da última leitura, só para contar versões na página do jogo.
    known_files: Mutex<Vec<RemoteFile>>,
    /// Chave -> (hash dos bytes recusados, o erro): a nuvem recusou estes
    /// bytes de vez, e eles só sobem de novo quando mudarem. Vale enquanto o
    /// app roda: abrir o app tenta uma vez mais (o limite do servidor pode ter
    /// subido).
    rejected: Mutex<HashMap<String, (String, SyncError)>>,
    prelaunch_budget: Duration,
    /// Os save states desta máquina (`save_states`).
    states: db::SaveStateRepo,
    /// O hash do último state lido de cada caminho, com o tamanho e o mtime
    /// de então: um state que não mudou não é relido a cada passada.
    state_hashes: Mutex<HashMap<PathBuf, states::Seen>>,
    /// A senha da última gravação de state de cada ROM: só a última de uma
    /// rajada roda a passada. As senhas vêm de um contador e nunca se repetem.
    state_bursts: Mutex<HashMap<String, u64>>,
    state_tickets: std::sync::atomic::AtomicU64,
    /// Quanto uma rajada de saves de state espera, quieta, antes de subir.
    state_debounce: Duration,
    /// Uma passada de states por vez. Não é o trinco dos game saves: um state
    /// grande subindo devagar nunca segura a checagem de antes de abrir um
    /// jogo, e a passada de states não escreve em disco nem faz backup.
    states_lock: tokio::sync::Mutex<()>,
    /// O maior objeto que a nuvem aceita, pelo `max_bytes` do último 413 de
    /// um state: um state maior não sobe (nem é enviado para ouvir o 413).
    /// Vale até outro 413 dizer outro número ou o app abrir de novo.
    state_ceiling: Mutex<Option<i64>>,
    /// Os states de outras máquinas da última listagem, por `save_id`: o
    /// download e a miniatura acham ali a versão e o nome.
    remote_found: Mutex<HashMap<String, remote_states::Found>>,
    /// As miniaturas já baixadas, por (`save_id`, versão): cada uma vem uma
    /// vez só.
    remote_thumbs: Mutex<HashMap<(String, String), Vec<u8>>>,
    /// Como o disco compara os nomes dos saves (a do sistema; os testes
    /// trocam).
    name_case: NameCase,
    /// Chaveiro das credenciais de emuladores sincronizadas (`OsKeyring` em
    /// produção; isolado em memória nos testes).
    secrets: Arc<dyn crate::credentials::SecretStore + Send + Sync>,
    /// Aviso disparado quando o sync atualiza a sessão local do RetroAchievements.
    on_ra_changed: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

#[cfg(test)]
#[derive(Default)]
struct MemCredStore(Mutex<HashMap<String, String>>);

#[cfg(test)]
impl crate::credentials::SecretStore for MemCredStore {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(account)
            .cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(account.to_owned(), secret.to_owned());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(account);
        Ok(())
    }
}

impl Engine {
    pub fn new(
        sync: Arc<SyncService>,
        db: db::Db,
        dirs: Dirs,
        playing: PlayingFn,
        emit: EmitFn,
        reload: ReloadFn,
    ) -> Self {
        #[cfg(not(test))]
        let secrets: Arc<dyn crate::credentials::SecretStore + Send + Sync> =
            Arc::new(crate::credentials::OsKeyring);
        #[cfg(test)]
        let secrets: Arc<dyn crate::credentials::SecretStore + Send + Sync> =
            Arc::new(MemCredStore::default());
        Self {
            sync,
            repo: SyncRepo::new(db.clone()),
            roms: RomsRepo::new(db.clone()),
            db: db.clone(),
            dirs,
            playing,
            emit,
            reload,
            settings_lock: tokio::sync::Mutex::new(()),
            one_at_a_time: tokio::sync::Mutex::new(()),
            stamp: Mutex::new(String::new()),
            known_files: Mutex::new(Vec::new()),
            rejected: Mutex::new(HashMap::new()),
            prelaunch_budget: PRELAUNCH_BUDGET,
            states: db::SaveStateRepo::new(db),
            state_hashes: Mutex::new(HashMap::new()),
            state_bursts: Mutex::new(HashMap::new()),
            state_tickets: std::sync::atomic::AtomicU64::new(0),
            state_debounce: STATE_DEBOUNCE,
            states_lock: tokio::sync::Mutex::new(()),
            state_ceiling: Mutex::new(None),
            remote_found: Mutex::new(HashMap::new()),
            remote_thumbs: Mutex::new(HashMap::new()),
            name_case: NameCase::HERE,
            secrets,
            on_ra_changed: Mutex::new(None),
        }
    }

    pub fn set_on_ra_changed(&self, f: Box<dyn Fn() + Send + Sync>) {
        *self.on_ra_changed.lock().unwrap_or_else(|p| p.into_inner()) = Some(f);
    }

    #[cfg(test)]
    pub fn set_secret_store(
        &mut self,
        store: Arc<dyn crate::credentials::SecretStore + Send + Sync>,
    ) {
        self.secrets = store;
    }

    pub fn playing(&self) -> Playing {
        (self.playing)()
    }

    fn emit(&self, event: SyncEvent) {
        (self.emit)(event)
    }

    // --- as passadas -------------------------------------------------------

    /// Uma passada: a dos game saves (completa ou de uma ROM) e, depois, a
    /// dos save states, cada uma com o seu trinco e o seu relatório. Uma falha
    /// dos states nunca apaga nem troca o relatório dos game saves: com os
    /// dois, o que volta é o dos game saves com a conta de states enviados.
    pub async fn pass(&self, alva: &Alvanista, scope: PassScope) -> Result<PassReport, SyncError> {
        let files = if scope.files() {
            Some(self.file_pass(alva, &scope).await)
        } else {
            None
        };
        // A máquina não sincroniza agora: a passada de states nem tenta.
        let halted = files.as_ref().is_some_and(|f| !states_may_follow(f));
        let states = if scope.states() && !halted {
            Some(self.state_pass(alva, &scope).await)
        } else {
            None
        };
        match (files, states) {
            (Some(files), states) => files.map(|mut report| {
                if let Some(Ok(states)) = states {
                    report.states_uploaded = states.states_uploaded;
                }
                report
            }),
            (None, Some(states)) => states,
            (None, None) => Ok(PassReport::default()),
        }
    }

    /// Só a metade dos game saves de uma passada completa ("Sincronizar
    /// agora", M4): a dos states roda depois, sem a página esperar por ela.
    pub async fn saves_pass(&self, alva: &Alvanista) -> Result<PassReport, SyncError> {
        self.file_pass(alva, &PassScope::All).await
    }

    /// A passada dos game saves: lê a linha desta máquina, a nuvem e o disco,
    /// decide cada chave e age.
    async fn file_pass(
        &self,
        alva: &Alvanista,
        scope: &PassScope,
    ) -> Result<PassReport, SyncError> {
        let _one = self.one_at_a_time.lock().await;
        let device = self.ready(alva).await?;
        if !self.saves_on().await? {
            return Ok(PassReport::default());
        }
        let result = self.run(alva, &device, scope, Mode::Pass).await;
        self.finish(&result, Mode::Pass).await;
        result
    }

    /// Antes de abrir um jogo: só o save daquele jogo, no máximo 5 s. Passado
    /// o prazo o jogo abre com o save local.
    pub async fn prelaunch(&self, alva: &Alvanista, rom_id: &str) -> PrelaunchOutcome {
        if !matches!(self.saves_on().await, Ok(true)) {
            return PrelaunchOutcome::Skipped("saves_off".into());
        }
        let work = async {
            let _one = self.one_at_a_time.lock().await;
            let device = self.ready(alva).await?;
            let scope = PassScope::Rom(rom_id.to_owned());
            let result = self.run(alva, &device, &scope, Mode::BeforeLaunch).await;
            self.finish(&result, Mode::BeforeLaunch).await;
            result
        };
        let outcome = match tokio::time::timeout(self.prelaunch_budget, work).await {
            Err(_) => PrelaunchOutcome::Skipped("timeout".into()),
            Ok(Err(e)) => PrelaunchOutcome::Skipped(e.to_string()),
            Ok(Ok(r)) if r.conflicts > 0 => PrelaunchOutcome::Conflict,
            Ok(Ok(r)) if r.downloaded > 0 => PrelaunchOutcome::Downloaded,
            Ok(Ok(r)) => match r.worst() {
                Some(e) => PrelaunchOutcome::Skipped(e.to_string()),
                None => PrelaunchOutcome::UpToDate,
            },
        };
        self.emit(outcome.event());
        outcome
    }

    /// A linha de save existe: aparelho guardado, sync ligado, não revogado e
    /// game saves ligados. Só o SQLite.
    async fn saves_line_on(&self) -> Result<bool, SyncError> {
        let devices = &self.sync.devices;
        if devices.device().await.map_err(failed)?.is_none()
            || !devices.enabled().await.map_err(failed)?
        {
            return Ok(false);
        }
        let (_, revoked) = devices.site_state().await.map_err(failed)?;
        Ok(!revoked && self.saves_on().await?)
    }

    /// O interruptor dos game saves: desligado, a passada não faz nada.
    async fn saves_on(&self) -> Result<bool, SyncError> {
        self.sync.devices.saves_enabled().await.map_err(failed)
    }

    /// A passada dos game saves ainda vale: os game saves e o sync ligados,
    /// sem pausa aqui, na mesma época de quando ela começou (desligar os game
    /// saves ou esquecer o aparelho muda a época). Só a memória e o SQLite.
    async fn saves_still_on(&self, epoch: u64) -> bool {
        let now = self.sync.runtime().saves_epoch;
        if now != epoch {
            return false;
        }
        let devices = &self.sync.devices;
        matches!(devices.enabled().await, Ok(true))
            && matches!(devices.paused().await, Ok(false))
            && matches!(devices.saves_enabled().await, Ok(true))
    }

    /// O interruptor dos save states: desligado, nenhum state sobe e a base
    /// deles fica como está.
    async fn states_on(&self) -> Result<bool, SyncError> {
        self.sync.devices.states_enabled().await.map_err(failed)
    }

    /// Sessão, aparelho, ligado e sem pausa local, e a base é desta conta (a
    /// de outra conta sai aqui). Só o SQLite: nem chaveiro nem rede. Só o lado
    /// dos game saves chama, com o trinco deles: é lá que a base é apagada.
    async fn ready(&self, alva: &Alvanista) -> Result<Device, SyncError> {
        let device = self.ready_device(alva).await?;
        self.own_account(alva).await?;
        Ok(device)
    }

    /// Como `ready`, para a passada de states: ela nunca apaga a base. Se a
    /// base ainda não é da conta de agora (ou de conta nenhuma), a passada
    /// termina `offline` sem tocar em nada; a dos game saves, com o trinco
    /// dela, faz a troca, e a de states seguinte anda.
    async fn ready_for_states(&self, alva: &Alvanista) -> Result<Device, SyncError> {
        let device = self.ready_device(alva).await?;
        let current = alva.account_id().await;
        let owner = self.sync.devices.base_account().await.map_err(failed)?;
        match (current, owner) {
            (Some(current), Some(owner)) if current == owner => Ok(device),
            _ => {
                log::info!("sync: a base ainda não é desta conta; os states esperam os game saves");
                Err(SyncError::Offline)
            }
        }
    }

    /// Sessão, aparelho, ligado e sem pausa local. Só o SQLite.
    async fn ready_device(&self, alva: &Alvanista) -> Result<Device, SyncError> {
        if !alva.has_session().await {
            return Err(SyncError::NoAccount);
        }
        let devices = &self.sync.devices;
        let device = devices
            .device()
            .await
            .map_err(failed)?
            .ok_or(SyncError::NotEnabled)?;
        if !devices.enabled().await.map_err(failed)? {
            return Err(SyncError::NotEnabled);
        }
        if devices.paused().await.map_err(failed)? {
            return Err(SyncError::Paused);
        }
        Ok(device)
    }

    /// A base e os conflitos são da conta que os gravou (M1). Outra conta
    /// nesta máquina não herda o "já sincronizado" da anterior: a base e os
    /// conflitos saem, e o primeiro sync dela segue a regra de duas máquinas
    /// configuradas à parte (bytes diferentes viram conflito, nada desce por
    /// cima). Só o SQLite.
    async fn own_account(&self, alva: &Alvanista) -> Result<(), SyncError> {
        let devices = &self.sync.devices;
        let Some(current) = alva.account_id().await else {
            // Sem o perfil não se sabe de quem é a conta (N1): nada roda e nada
            // se grava, nem com dono registrado nem sem. Uma base escrita agora
            // seria adotada pela primeira conta que aparecesse.
            return Err(SyncError::Offline);
        };
        let owner = devices.base_account().await.map_err(failed)?;
        if owner.as_deref() == Some(current.as_str()) {
            return Ok(());
        }
        if owner.is_some() {
            log::info!("sync: outra conta nesta máquina; a base e os conflitos da anterior saem");
            self.repo.clear_all().await.map_err(repo_err)?;
        }
        devices.set_base_account(&current).await.map_err(failed)
    }

    async fn run(
        &self,
        alva: &Alvanista,
        device: &Device,
        scope: &PassScope,
        mode: Mode,
    ) -> Result<PassReport, SyncError> {
        // Desligar os game saves daqui em diante muda a época.
        let saves_epoch = self.sync.runtime().saves_epoch;
        self.new_stamp();
        self.sweep_temps();
        // Toda passada lê a linha desta máquina de novo (sem leitura guardada):
        // uma pausa ou revogação feita há segundos já vale antes de abrir.
        let devices = self.sync.partner_devices(alva).await?;
        let receive = self.check_row(&devices, &device.id, true).await?;
        let token = self.token()?;
        let mut cx = Ctx {
            alva,
            mode,
            token,
            devices,
            receive,
            uploads_stopped: false,
            network_failures: 0,
            saves_epoch,
        };
        let mut report = PassReport::default();
        self.run_files(&mut cx, &device.id, scope, &mut report)
            .await?;
        if mode == Mode::Pass
            && matches!(scope, PassScope::All)
            && self.saves_still_on(cx.saves_epoch).await
        {
            self.reconcile_credentials(&mut cx).await;
        }
        Ok(report)
    }

    /// Sincroniza credenciais de emuladores (RetroAchievements, ScreenScraper e
    /// TheGamesDB) com o cofre criptografado do Cloud Save (D155, D157).
    async fn reconcile_credentials(&self, cx: &mut Ctx<'_>) {
        let remote = match self.sync.api.get_credentials(&cx.token).await {
            Ok(resp) => resp,
            Err(ApiError::NotFound) => return,
            Err(e) => {
                log::warn!("sync: credenciais não puderam ser lidas: {e}");
                return;
            }
        };
        let (to_upload, ra_changed) = match crate::credentials::reconcile_with_remote(
            &self.db,
            self.secrets.as_ref(),
            remote.credentials.as_ref(),
            remote.linked_ra_username,
        )
        .await
        {
            Ok(res) => res,
            Err(e) => {
                log::warn!("sync: reconciliação de credenciais falhou: {e}");
                return;
            }
        };
        if ra_changed {
            if let Some(cb) = self
                .on_ra_changed
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
            {
                cb();
            }
        }
        if !to_upload.is_empty() && !cx.uploads_stopped {
            match self.sync.api.put_credentials(&cx.token, &to_upload).await {
                Ok(saved) => {
                    if let Err(e) =
                        crate::credentials::mark_sync_uploaded(&self.db, &to_upload).await
                    {
                        log::warn!("sync: marcação local de credenciais enviadas falhou: {e}");
                    }
                    if saved.linked_ra_username.is_some() {
                        let _ = crate::credentials::set_linked_alvanista_ra_username(
                            &self.db,
                            saved.linked_ra_username,
                        )
                        .await;
                    }
                }
                Err(ApiError::TierInsufficient { .. } | ApiError::Quota) => {
                    cx.uploads_stopped = true;
                }
                Err(e) => {
                    log::warn!("sync: envio de credenciais adiado: {e}");
                }
            }
        }
    }

    /// Dispara a reconciliação imediata das credenciais quando o usuário faz
    /// login, logout ou altera preferências locais com o sync ligado.
    pub async fn push_credentials_now(&self, alva: &Alvanista) -> Result<(), SyncError> {
        let _one = self.one_at_a_time.lock().await;
        let device = self.ready(alva).await?;
        let devices = self.sync.partner_devices(alva).await?;
        let receive = self.check_row(&devices, &device.id, true).await?;
        let token = self.token()?;
        let mut cx = Ctx {
            alva,
            mode: Mode::Pass,
            token,
            devices,
            receive,
            uploads_stopped: false,
            network_failures: 0,
            saves_epoch: self.sync.runtime().saves_epoch,
        };
        self.reconcile_credentials(&mut cx).await;
        Ok(())
    }

    /// Os game saves da passada: a nuvem e o disco, e cada chave decidida.
    async fn run_files(
        &self,
        cx: &mut Ctx<'_>,
        me: &str,
        scope: &PassScope,
        report: &mut PassReport,
    ) -> Result<(), SyncError> {
        let alva = cx.alva;
        let token = cx.token.clone();
        // Erro ao ler o disco ou a biblioteca acaba a passada: uma lista curta
        // demais faria save sincronizado parecer apagado.
        let local_err = |e: String| failed(format!("saves locais: {e}"));
        let (files, local) = match scope {
            PassScope::All => {
                let files = self.list(alva, &token).await?;
                let local = local::account_saves_with_db(
                    &self.db,
                    &self.roms,
                    &self.dirs.saves,
                    &self.dirs.system,
                )
                .await
                .map_err(local_err)?;
                (files, local)
            }
            // Uma ROM (antes de abrir, depois de fechar): só as vagas dela no
            // disco e só os nomes dela na nuvem, lidos agora.
            PassScope::Rom(id) => {
                let Some(rom) = self
                    .roms
                    .get(id)
                    .await
                    .map_err(|e| failed(format!("ROM {id}: {e}")))?
                else {
                    return Ok(());
                };
                let (names, cores) = local::rom_listing(&rom, &self.dirs.saves);
                let mut files = self.list_for(alva, &token, &names, &cores).await?;
                // Saiu da biblioteca no meio: nada a fazer (um mapa vazio faria
                // todo conflito parecer de jogo fora da biblioteca).
                let Some(local) = local::rom_saves_with_db(
                    &self.db,
                    &self.roms,
                    &self.dirs.saves,
                    &self.dirs.system,
                    id,
                )
                .await
                .map_err(local_err)?
                else {
                    return Ok(());
                };
                if !has_rom_save_row(&files)
                    && self.rom_may_fallback(&rom, local.rom_game_uuid(id)).await?
                {
                    let extra = self
                        .list_platform_rom_files(alva, &token, platform_for_system(&rom.system_id))
                        .await?;
                    merge_remote_files(&mut files, extra);
                }
                (files, local)
            }
            // Os escopos de states não olham os game saves.
            PassScope::State(_) | PassScope::States => return Ok(()),
        };
        let entries = self.entries(&local, &files, scope).await?;
        let mut known: HashSet<String> = HashSet::new();
        for c in self.repo.conflicts_all().await.map_err(repo_err)? {
            // O jogo saiu da biblioteca: o conflito não tem mais onde ser
            // resolvido e não pode ficar contando na página.
            if local.slot_for_key(&c.key).is_none() {
                log::info!("sync: conflito de um save fora da biblioteca sai da lista");
                if let Err(e) = self.repo.conflict_delete(&c.key).await {
                    log::warn!("sync: conflito órfão não saiu da lista: {e}");
                }
                continue;
            }
            known.insert(c.key);
        }
        for entry in &entries {
            // Os game saves (ou o sync) desligaram no meio: o envio que já
            // corria terminou, e nenhum arquivo mais sobe, desce ou muda aqui.
            if !self.saves_still_on(cx.saves_epoch).await {
                log::info!("sync: os game saves desligaram no meio da passada; o resto fica");
                break;
            }
            self.step(cx, me, entry, &known, report).await?;
        }
        Ok(())
    }

    /// O que a página lê depois de uma passada, e os avisos para ela.
    async fn finish(&self, result: &Result<PassReport, SyncError>, mode: Mode) {
        {
            let mut rt = self.sync.runtime();
            if mode == Mode::Pass {
                match result {
                    Ok(report) => {
                        rt.last_pass_at = Some(now_secs());
                        let worst = report.worst_entry();
                        rt.last_error = worst.map(|(_, e)| e.to_string());
                        // O nome do arquivo (nunca o caminho): a tela diz qual.
                        rt.last_error_file = worst
                            .filter(|(_, e)| is_permanent(e))
                            .and_then(|(key, _)| key_file_name(key))
                            .map(str::to_owned);
                    }
                    // O aviso de pausada no site já diz isso.
                    Err(SyncError::Paused) => rt.last_error = None,
                    // `revoked_by_site` já escreveu; sem conta ou desligado não é erro da passada.
                    Err(SyncError::Revoked | SyncError::NoAccount | SyncError::NotEnabled) => {}
                    Err(e) => rt.last_error = Some(e.to_string()),
                }
            }
        }
        // Também antes de abrir: uma remoção no site aceita ali é avisada igual.
        if let Ok(report) = result {
            self.emit(SyncEvent::Pass {
                uploaded: report.uploaded,
                downloaded: report.downloaded,
                backed_up: report.backed_up,
            });
        }
        self.emit(SyncEvent::Status);
    }

    /// A linha desta máquina na lista do site (D128); devolve as plataformas
    /// que ela recebe. `absorb`: a lista acabou de ser lida, e o que ela diz
    /// fica guardado para a página.
    async fn check_row(
        &self,
        list: &[RemoteDevice],
        id: &str,
        absorb: bool,
    ) -> Result<Option<Vec<String>>, SyncError> {
        if absorb {
            self.sync.absorb_site_state(list).await;
        }
        match list.iter().find(|d| d.id == id) {
            Some(row) if row.revoked_at.is_none() => {
                if row.paused_at.is_some() {
                    return Err(SyncError::Paused);
                }
                Ok(row.receive_platforms.clone())
            }
            _ => {
                self.revoked_by_site().await;
                Err(SyncError::Revoked)
            }
        }
    }

    /// O site removeu esta máquina: o token sai do chaveiro e o sync desliga.
    /// O id e o nome ficam: "Ligar de novo" cria o aparelho novo com o nome
    /// que a pessoa escolheu (`set_enabled_with` esquece o resto).
    async fn revoked_by_site(&self) {
        log::warn!("sync: o site removeu esta máquina; o sync desliga");
        if let Err(e) = self.sync.devices.drop_token() {
            log::warn!("sync: token do aparelho não saiu do chaveiro: {e}");
        }
        if let Err(e) = self.sync.devices.set_enabled(false).await {
            log::warn!("sync: o sync não desligou: {e}");
        }
        self.sync.mark_revoked().await;
        self.sync.runtime().last_error = Some(SyncError::Revoked.to_string());
        self.emit(SyncEvent::Error {
            code: SyncError::Revoked.to_string(),
        });
        self.emit(SyncEvent::Status);
    }

    /// Um 401 de `/api/v1/saves`: a lista de aparelhos decide.
    async fn after_401(&self, alva: &Alvanista) -> SyncError {
        let id = match self.sync.devices.device().await {
            Ok(Some(d)) => d.id,
            _ => return SyncError::NotEnabled,
        };
        match self.sync.partner_devices(alva).await {
            Ok(list) => match self.check_row(&list, &id, true).await {
                Ok(_) => {
                    log::info!("sync: 401 com esta máquina viva na lista; tenta de novo depois");
                    SyncError::Offline
                }
                Err(e) => e,
            },
            Err(SyncError::NoAccount) => SyncError::NoAccount,
            Err(e) => {
                log::info!("sync: 401 e a lista de aparelhos não respondeu ({e})");
                SyncError::Offline
            }
        }
    }

    async fn note_site_paused(&self) {
        let revoked = self.sync.devices.site_state().await.is_ok_and(|s| s.1);
        if let Err(e) = self.sync.devices.set_site_state(true, revoked).await {
            log::warn!("sync: pausa do site não guardada: {e}");
        }
    }

    /// Erro que acaba a passada.
    async fn ending(&self, alva: &Alvanista, err: ApiError) -> SyncError {
        match err {
            ApiError::Unauthorized => self.after_401(alva).await,
            ApiError::Paused => {
                self.note_site_paused().await;
                SyncError::Paused
            }
            ApiError::TierInsufficient { .. } => {
                alva.note_no_save_sync().await;
                SyncError::NoTier
            }
            other => SyncError::from(other),
        }
    }

    /// Erro de uma chamada por arquivo.
    async fn file_error(&self, alva: &Alvanista, err: ApiError, upload: bool) -> Fail {
        match err {
            ApiError::Unauthorized | ApiError::Paused | ApiError::Transient(_) => {
                Fail::End(self.ending(alva, err).await)
            }
            // A cota cheia não é falta de plano: o espelho do plano fica como
            // está e a tela mostra os números da cota.
            ApiError::Quota if upload => Fail::StopUploads(SyncError::Quota),
            ApiError::TierInsufficient { .. } if upload => {
                // A tela passa a oferecer o plano; as descidas seguem (D51).
                alva.note_no_save_sync().await;
                Fail::StopUploads(SyncError::NoTier)
            }
            ApiError::TooLarge { .. } => Fail::File(SyncError::TooLarge),
            ApiError::Invalid(_) if upload => Fail::File(SyncError::Rejected),
            other => Fail::File(SyncError::from(other)),
        }
    }

    fn token(&self) -> Result<String, SyncError> {
        match self.sync.devices.token() {
            Ok(Some(t)) => Ok(t),
            Ok(None) => {
                log::warn!("sync: aparelho ligado sem token no chaveiro");
                Err(SyncError::Keyring)
            }
            Err(e) => {
                log::warn!("sync: chaveiro indisponível: {e}");
                Err(SyncError::Keyring)
            }
        }
    }

    /// Todos os arquivos da conta, os apagados no site inclusive: sem eles,
    /// uma remoção pareceria "não existe" e o arquivo subiria de novo.
    async fn list(&self, alva: &Alvanista, token: &str) -> Result<Vec<RemoteFile>, SyncError> {
        let filter = FileFilter {
            kind: "file",
            include_deleted: true,
            ..FileFilter::default()
        };
        match self.sync.api.list_files(token, &filter).await {
            Ok(files) => {
                let files: Vec<RemoteFile> =
                    files.into_iter().filter(|f| f.kind == "file").collect();
                *lock(&self.known_files) = files.clone();
                Ok(files)
            }
            Err(err) => Err(self.ending(alva, err).await),
        }
    }

    /// Só as linhas de alguns nomes (`filename` exato, como o servidor
    /// compara) e de alguns `core` (os cartões e VMUs de uma plataforma), os
    /// apagados inclusive. Os pedidos vão juntos; um que falha acaba a passada
    /// como a listagem completa.
    async fn list_for(
        &self,
        alva: &Alvanista,
        token: &str,
        names: &[String],
        cores: &[&'static str],
    ) -> Result<Vec<RemoteFile>, SyncError> {
        let mut set = tokio::task::JoinSet::new();
        let queries = names
            .iter()
            .map(|n| (Some(n.clone()), None))
            .chain(cores.iter().map(|c| (None, Some(*c))));
        for (filename, core) in queries {
            let (api, token) = (self.sync.api.clone(), token.to_owned());
            set.spawn(async move {
                let filter = FileFilter {
                    kind: "file",
                    filename: filename.as_deref(),
                    core,
                    include_deleted: true,
                    ..FileFilter::default()
                };
                api.list_files(&token, &filter).await
            });
        }
        let mut rows: Vec<RemoteFile> = Vec::new();
        let mut worst: Option<ApiError> = None;
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(found)) => rows.extend(found),
                Ok(Err(e)) => {
                    // O 401 decide pela lista de aparelhos; a pausa vem antes
                    // de um erro qualquer.
                    let weight = |e: &ApiError| match e {
                        ApiError::Unauthorized => 3,
                        ApiError::Paused => 2,
                        ApiError::TierInsufficient { .. } => 1,
                        _ => 0,
                    };
                    if worst.as_ref().map_or(true, |w| weight(&e) > weight(w)) {
                        worst = Some(e);
                    }
                }
                Err(e) => {
                    worst.get_or_insert(ApiError::Transient(format!("listagem interrompida: {e}")));
                }
            }
        }
        if let Some(err) = worst {
            return Err(self.ending(alva, err).await);
        }
        let mut ids = HashSet::new();
        rows.retain(|f| f.kind == "file" && ids.insert(f.save_id.clone()));
        let mut known = lock(&self.known_files);
        known.retain(|f| {
            !names.contains(&f.filename) && !f.core.as_deref().is_some_and(|c| cores.contains(&c))
        });
        known.extend(rows.iter().cloned());
        Ok(rows)
    }

    /// As linhas `kind=file` sem `core` de uma plataforma, incluindo apagadas:
    /// fallback quando o nome exato do arquivo local não existe na nuvem e a
    /// ROM tem `game_uuid`, `crc32` de 8 hex ou base registrada.
    async fn list_platform_rom_files(
        &self,
        alva: &Alvanista,
        token: &str,
        platform: &str,
    ) -> Result<Vec<RemoteFile>, SyncError> {
        let filter = FileFilter {
            kind: "file",
            platform: Some(platform),
            core: Some(""),
            include_deleted: true,
            ..FileFilter::default()
        };
        match self.sync.api.list_files(token, &filter).await {
            Ok(found) => {
                let mut ids = HashSet::new();
                let rows: Vec<RemoteFile> = found
                    .into_iter()
                    .filter(|f| {
                        f.kind == "file"
                            && f.platform == platform
                            && is_coreless(f)
                            && ids.insert(f.save_id.clone())
                    })
                    .collect();
                let mut known = lock(&self.known_files);
                known.retain(|f| !(f.platform == platform && is_coreless(f)));
                known.extend(rows.iter().cloned());
                Ok(rows)
            }
            Err(err) => Err(self.ending(alva, err).await),
        }
    }

    async fn rom_may_fallback(
        &self,
        rom: &domain::library::Rom,
        game_uuid: Option<&str>,
    ) -> Result<bool, SyncError> {
        if game_uuid.is_some() || local::clean_crc32_hex(&rom.crc32).is_some() {
            return Ok(true);
        }
        for slot in local::rom_slot_files(rom, &self.dirs.saves) {
            if self
                .repo
                .base_get(&slot.key)
                .await
                .map_err(repo_err)?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// As linhas da nuvem de um arquivo: o nome dele, ou a pasta (`core`) de
    /// um cartão ou VMU. Para saves unitários de ROM sem linha exata na nuvem,
    /// cai na listagem da plataforma quando há `game_uuid`, `crc32` ou base.
    async fn list_file(
        &self,
        alva: &Alvanista,
        token: &str,
        file: &SaveFile,
    ) -> Result<Vec<RemoteFile>, SyncError> {
        match file.core {
            Some(core) => self.list_for(alva, token, &[], &[core]).await,
            None => {
                let mut rows = self
                    .list_for(alva, token, std::slice::from_ref(&file.filename), &[])
                    .await?;
                if !has_rom_save_row(&rows) && !file.is_folder {
                    let can_fallback = file.game_uuid.is_some()
                        || file
                            .rom_crc32
                            .as_deref()
                            .and_then(local::clean_crc32_hex)
                            .is_some()
                        || self
                            .repo
                            .base_get(&file.key)
                            .await
                            .map_err(repo_err)?
                            .is_some();
                    if can_fallback {
                        let extra = self
                            .list_platform_rom_files(alva, token, file.platform)
                            .await?;
                        merge_remote_files(&mut rows, extra);
                    }
                }
                Ok(rows)
            }
        }
    }

    /// As chaves da passada: os arquivos em disco; os que a base conhece e
    /// sumiram daqui; os que só a nuvem tem e têm lugar neste disco (uma ROM
    /// da biblioteca, um cartão ou VMU). Um arquivo, uma chave.
    async fn entries(
        &self,
        local: &LocalSaves,
        files: &[RemoteFile],
        scope: &PassScope,
    ) -> Result<Vec<Entry>, SyncError> {
        let by_name = by_name(files, self.name_case);
        let bases: HashMap<String, BaseRow> = self
            .repo
            .base_all("file")
            .await
            .map_err(repo_err)?
            .into_iter()
            .map(|b| (b.key.clone(), b))
            .collect();
        let mut keys = Keys::new(self.name_case);
        for file in &local.files {
            keys.add_present(file.clone());
        }
        let mut base_keys: Vec<&String> = bases.keys().collect();
        base_keys.sort();
        // A linha que cada base desta biblioteca segue, e de que chaves.
        let mut claimed: HashMap<&str, HashSet<&str>> = HashMap::new();
        for key in base_keys {
            if let Some(slot) = local.slot_for_key(key) {
                claimed
                    .entry(bases[key].remote_save_id.as_str())
                    .or_default()
                    .insert(key.as_str());
                keys.add(slot);
            }
        }
        for row in files {
            let slots: Vec<SaveFile> = match row.core.as_deref() {
                Some(core) if core.starts_with("system/") => {
                    local.system_slot(core, &row.filename).into_iter().collect()
                }
                Some(core) if core.starts_with("saves/") => {
                    local.folder_slot(core, &row.filename).into_iter().collect()
                }
                _ => {
                    let exact = local.rom_slots(&row.filename);
                    if exact
                        .iter()
                        .any(|s| row.platform == "other" || s.platform == row.platform)
                    {
                        exact
                    } else if is_coreless(row) {
                        local
                            .fallback_rom_slot(
                                &row.platform,
                                &row.filename,
                                row.game_uuid.as_deref(),
                                row.slug.as_deref(),
                            )
                            .into_iter()
                            .collect()
                    } else {
                        Vec::new()
                    }
                }
            };
            for slot in slots {
                // Uma linha que a base de outra chave segue não ganha vaga só
                // da nuvem: no Linux, o `MCD001.PS2` que o `Mcd001.ps2` daqui
                // seguia desceria como um segundo cartão.
                let followed = claimed
                    .get(row.save_id.as_str())
                    .is_some_and(|by| !by.contains(slot.key.as_str()));
                if !followed
                    && pick_remote(&by_name, files, local, bases.get(&slot.key), &slot).is_some()
                {
                    keys.add(slot);
                }
            }
        }
        // Duas ROMs de mesmo stem dividem o arquivo: a passada de uma também
        // leva o arquivo que a outra listou primeiro.
        let scope_paths: HashSet<String> = match scope {
            PassScope::All | PassScope::State(_) | PassScope::States => HashSet::new(),
            PassScope::Rom(id) => match self.roms.get(id).await {
                Ok(Some(rom)) => {
                    local::rom_slot_files_with_uuid(&rom, local.rom_game_uuid(id), &self.dirs.saves)
                        .iter()
                        .map(|f| path_key(self.name_case, &f.path))
                        .collect()
                }
                Ok(None) => HashSet::new(),
                Err(e) => return Err(failed(format!("ROM {id}: {e}"))),
            },
        };
        let in_scope = |f: &SaveFile| match scope {
            PassScope::All | PassScope::State(_) | PassScope::States => true,
            PassScope::Rom(id) => {
                f.rom_id.as_deref() == Some(id.as_str())
                    || scope_paths.contains(&path_key(self.name_case, &f.path))
                    || (f.rom_id.is_none() && local.rom_matches_unowned_save(id, f))
            }
        };
        Ok(keys
            .files
            .into_iter()
            .filter(in_scope)
            .map(|file| {
                let base = bases.get(&file.key);
                let row = pick_remote(&by_name, files, local, base, &file);
                Entry {
                    row: row.cloned(),
                    base: base.cloned(),
                    file,
                }
            })
            .collect())
    }
}

fn is_coreless(r: &RemoteFile) -> bool {
    r.core.as_deref().map_or(true, str::is_empty)
}

fn has_rom_save_row(files: &[RemoteFile]) -> bool {
    files.iter().any(|r| {
        let c = r.core.as_deref().unwrap_or_default();
        !c.starts_with("system/") && !c.starts_with("saves/")
    })
}

fn merge_remote_files(into: &mut Vec<RemoteFile>, extra: Vec<RemoteFile>) {
    let mut seen: HashSet<String> = into.iter().map(|r| r.save_id.clone()).collect();
    for r in extra {
        if seen.insert(r.save_id.clone()) {
            into.push(r);
        }
    }
}

/// Escolhe a linha remota para `file`:
/// 1. Regra 1: stem exato (`pick`) vence sempre;
/// 2. Cartões/VMUs com maiúsculas legadas na base (`base_row`);
/// 3. Regra 2: fallback de ROM (`core == None`) por `crc32` ou `game_uuid` na
///    mesma plataforma, ou pela base já vinculada a esse `save_id`.
fn pick_remote<'a>(
    by_name: &ByName<'a>,
    files: &'a [RemoteFile],
    local: &LocalSaves,
    base: Option<&BaseRow>,
    file: &SaveFile,
) -> Option<&'a RemoteFile> {
    if let Some(exact) = pick(by_name, file) {
        return Some(exact);
    }
    if let Some(card_base) = base_row(files, base, file) {
        return Some(card_base);
    }
    if file.core.is_some() || file.is_folder {
        return None;
    }
    let file_crc = file.rom_crc32.as_deref().and_then(local::clean_crc32_hex);
    if let Some(matched) = files
        .iter()
        .filter(|r| {
            let live_or_tracked = r.deleted_at.is_none()
                || base
                    .filter(|b| b.wiped_at.is_none())
                    .is_some_and(|b| b.remote_save_id == r.save_id);
            live_or_tracked
                && is_coreless(r)
                && r.platform == file.platform
                && local
                    .fallback_rom_slot(
                        &r.platform,
                        &r.filename,
                        r.game_uuid.as_deref(),
                        r.slug.as_deref(),
                    )
                    .is_some_and(|slot| slot.key == file.key)
        })
        .max_by(|a, b| cmp_fallback(a, b, file_crc.as_deref()))
    {
        return Some(matched);
    }
    base_rom_row(files, local, base, file)
}

fn cmp_fallback(a: &RemoteFile, b: &RemoteFile, file_crc: Option<&str>) -> std::cmp::Ordering {
    let crc_match = |r: &RemoteFile| {
        file_crc.is_some()
            && r.slug
                .as_deref()
                .and_then(local::parse_crc32_slug)
                .as_deref()
                == file_crc
    };
    (a.deleted_at.is_none(), crc_match(a), rank(a)).cmp(&(
        b.deleted_at.is_none(),
        crc_match(b),
        rank(b),
    ))
}

fn base_rom_row<'a>(
    files: &'a [RemoteFile],
    local: &LocalSaves,
    base: Option<&BaseRow>,
    file: &SaveFile,
) -> Option<&'a RemoteFile> {
    let base = base.filter(|b| b.wiped_at.is_none())?;
    if file.core.is_some() || file.is_folder {
        return None;
    }
    files.iter().find(|r| {
        if r.save_id != base.remote_save_id || !is_coreless(r) {
            return false;
        }
        let Some((remote_stem, _)) = r.filename.rsplit_once('.') else {
            return false;
        };
        !local.has_exact_rom_stem(&r.platform, remote_stem)
    })
}

/// A linha que a base de um cartão ou VMU segue, quando o nome não acha
/// nenhuma e ela é o mesmo arquivo com outras maiúsculas. Até a 0.1.16 o
/// Linux pareava `Mcd001.ps2` com a linha `MCD001.PS2`; o par continua, e a
/// primeira passada depois da atualização não sobe uma segunda linha. Só
/// quando o nome não acha nada: entre linhas que o nome acha vale `rank`.
fn base_row<'a>(
    files: &'a [RemoteFile],
    base: Option<&BaseRow>,
    file: &SaveFile,
) -> Option<&'a RemoteFile> {
    let base = base?;
    file.core?;
    files.iter().find(|r| {
        r.save_id == base.remote_save_id
            && r.core.as_deref() == file.core
            && r.filename.to_lowercase() == file.filename.to_lowercase()
    })
}

/// O caminho como o disco compara: no Windows `VMU_SAVE_A1.BIN` e
/// `vmu_save_a1.bin` são o mesmo arquivo, no Linux são dois.
fn path_key(case: NameCase, p: &Path) -> String {
    case.of(&p.to_string_lossy())
}

/// Uma chave por arquivo e um arquivo por chave: o primeiro que chega fica.
/// O caminho compara como o disco (`path_key`).
struct Keys {
    case: NameCase,
    files: Vec<SaveFile>,
    keys: HashSet<String>,
    paths: HashSet<String>,
}

impl Keys {
    fn new(case: NameCase) -> Self {
        Self {
            case,
            files: Vec::new(),
            keys: HashSet::new(),
            paths: HashSet::new(),
        }
    }

    /// Os arquivos em disco já chegam sem repetição de `account_saves`.
    fn add_present(&mut self, f: SaveFile) {
        self.paths.insert(path_key(self.case, &f.path));
        if self.keys.insert(f.key.clone()) {
            self.files.push(f);
        }
    }

    fn add(&mut self, f: SaveFile) {
        let path = path_key(self.case, &f.path);
        if self.keys.contains(&f.key) || self.paths.contains(&path) {
            return;
        }
        self.keys.insert(f.key.clone());
        self.paths.insert(path);
        self.files.push(f);
    }
}

/// As linhas da nuvem pelo nome, como o disco compara.
struct ByName<'a> {
    case: NameCase,
    rows: HashMap<String, Vec<&'a RemoteFile>>,
}

fn by_name(files: &[RemoteFile], case: NameCase) -> ByName<'_> {
    let mut rows: HashMap<String, Vec<&RemoteFile>> = HashMap::new();
    for f in files {
        rows.entry(case.of(&f.filename)).or_default().push(f);
    }
    ByName { case, rows }
}

/// A ordem entre linhas do mesmo arquivo: viva antes de apagada, da conta
/// antes de aparelho, a escrita por último primeiro.
fn rank(r: &RemoteFile) -> (bool, bool, (i64, u32), &str) {
    (
        r.deleted_at.is_none(),
        r.device_token_id.is_none(),
        r.current
            .as_ref()
            .and_then(|c| parse_time(&c.server_created_at))
            .unwrap_or((i64::MIN, 0)),
        r.save_id.as_str(),
    )
}

/// A linha da nuvem de um arquivo (D113, D125): o mesmo nome (e, nos cartões
/// e VMUs, a mesma pasta) na plataforma do ReEmu, senão em `other`, as vivas
/// antes das apagadas; entre várias, a primeira de `rank`. O nome de um
/// cartão ou VMU compara como o disco: no Linux `VMU_SAVE_A1.BIN` não é a
/// linha de `vmu_save_a1.bin`, que é outro arquivo lá.
fn pick<'a>(by_name: &ByName<'a>, file: &SaveFile) -> Option<&'a RemoteFile> {
    let case = by_name.case;
    let rows = by_name.rows.get(&case.of(&file.filename))?;
    let same: Vec<&'a RemoteFile> = rows
        .iter()
        .copied()
        .filter(|r| match file.core {
            Some(core) => {
                r.core.as_deref() == Some(core) && case.of(&r.filename) == case.of(&file.filename)
            }
            None => {
                let c = r.core.as_deref().unwrap_or_default();
                r.filename == file.filename && !c.starts_with("system/") && !c.starts_with("saves/")
            }
        })
        .collect();
    let on = |platform: &str, live: bool| {
        same.iter()
            .copied()
            .filter(|r| r.platform == platform && r.deleted_at.is_none() == live)
            .max_by(|a, b| rank(a).cmp(&rank(b)))
    };
    // Uma linha viva, em qualquer das duas plataformas, vem antes de uma
    // apagada: a remoção da linha do ReEmu não esconde a `other` que um
    // RetroArch plano segue escrevendo.
    on(file.platform, true)
        .or_else(|| on("other", true))
        .or_else(|| on(file.platform, false))
        .or_else(|| on("other", false))
}

/// A linha como a regra de três lados a vê. O dono (`device_token_id` do
/// arquivo) nunca é trocado por quem escreveu a última versão.
fn remote_of(f: &RemoteFile) -> Option<Remote> {
    let deleted = f.deleted_at.is_some();
    match &f.current {
        Some(c) => Some(Remote {
            save_id: f.save_id.clone(),
            version_id: c.version_id.clone(),
            hash: c.content_hash.to_ascii_lowercase(),
            size: c.size_bytes,
            device_token_id: f.device_token_id.clone(),
            client_mtime: c.client_mtime.as_deref().and_then(parse_time).map(|t| t.0),
            deleted,
        }),
        None if deleted => Some(Remote {
            save_id: f.save_id.clone(),
            version_id: String::new(),
            hash: String::new(),
            size: 0,
            device_token_id: f.device_token_id.clone(),
            client_mtime: None,
            deleted,
        }),
        // Linha viva sem versão: nada a baixar.
        None => None,
    }
}

/// O nome da máquina que escreveu uma versão: pela lista de aparelhos da
/// conta, senão o que o Alvanista mandou, senão o cliente.
fn machine_label(v: &RemoteVersion, devices: &[RemoteDevice]) -> Option<String> {
    v.device_token_id
        .as_deref()
        .and_then(|id| devices.iter().find(|d| d.id == id))
        .map(|d| d.label.clone())
        .or_else(|| v.device_label.clone())
        .or_else(|| v.source_client.clone())
        .filter(|s| !s.is_empty())
}

fn receives(cx: &Ctx<'_>, platform: &str) -> bool {
    cx.receive
        .as_ref()
        .map_or(true, |list| list.iter().any(|p| p == platform))
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// O arquivo existe? Erro que não é "não existe" é erro.
fn present(path: &Path) -> io::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// O sufixo dos temporários de `write_atomic` (e só deles).
const TMP_SUFFIX: &str = ".sync-tmp";

/// Copia e grava a cópia no disco (`fsync`).
fn copy_synced(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::copy(from, to)?;
    std::fs::OpenOptions::new().write(true).open(to)?.sync_all()
}

/// Grava num `.sync-tmp` ao lado, `fsync`, e troca pelo `rename`: um corte no
/// meio deixa o arquivo de antes inteiro.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(TMP_SUFFIX);
    let tmp = path.with_file_name(name);
    let written = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() {
        // Só o temporário que esta função criou.
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

/// `yyyymmdd-hhmmss` no relógio da pessoa: é o nome que ela vê na pasta.
fn local_stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Dias desde 1970-01-01 de uma data civil (Howard Hinnant, `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// RFC 3339 em (segundos Unix, nanossegundos). `Z` ou `±hh:mm`.
fn parse_time(s: &str) -> Option<(i64, u32)> {
    let s = s.trim();
    let b = s.as_bytes();
    let num = |from: usize, to: usize| -> Option<i64> {
        let part = s.get(from..to)?;
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || !matches!(b[10], b'T' | b't' | b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (y, mo, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (h, mi, se) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut i = 19;
    let mut nanos = 0u32;
    if b.get(i) == Some(&b'.') {
        let start = i + 1;
        i = start;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let frac = s.get(start..i)?;
        if frac.is_empty() {
            return None;
        }
        let kept = &frac[..frac.len().min(9)];
        nanos = kept.parse::<u32>().ok()? * 10u32.pow(9 - kept.len() as u32);
    }
    let offset = match s.get(i..)? {
        "Z" | "z" => 0,
        tz if tz.len() == 6 && tz.as_bytes()[3] == b':' => {
            let sign = match tz.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let hh: i64 = tz.get(1..3)?.parse().ok()?;
            let mm: i64 = tz.get(4..6)?.parse().ok()?;
            sign * (hh * 3_600 + mm * 60)
        }
        _ => return None,
    };
    let secs = days_from_civil(y, mo, d) * 86_400 + h * 3_600 + mi * 60 + se - offset;
    Some((secs, nanos))
}

#[cfg(test)]
mod tests;
