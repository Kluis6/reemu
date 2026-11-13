//! O motor do cloud save de game saves: as passadas (ao abrir o app, de 10
//! em 10 minutos sem jogo aberto, antes de abrir um jogo e depois de fechá-lo),
//! o conflito guardado para a pessoa decidir e a volta pelo histórico.
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

use super::api::{ApiError, FileFilter, RemoteDevice, RemoteFile, RemoteVersion, UploadMeta};
use super::device::{default_label, rfc3339, Device};
use super::local::{
    self, account_saves, log_tag, parse_key, rom_saves, sha256_file, LocalSaves, SaveFile,
};
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

/// Quanto um jogo espera pela nuvem antes de abrir (D117).
const PRELAUNCH_BUDGET: Duration = Duration::from_secs(5);
/// A passada sem jogo aberto.
const IDLE_EVERY: Duration = Duration::from_secs(600);

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
    All,
    /// Os saves de uma ROM, mais os cartões e VMUs da plataforma dela.
    Rom(String),
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
    matches!(e, SyncError::TooLarge | SyncError::Rejected)
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
}

type PlayingFn = Box<dyn Fn() -> Playing + Send + Sync>;
type EmitFn = Box<dyn Fn(SyncEvent) + Send + Sync>;

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
    repo: SyncRepo,
    roms: RomsRepo,
    dirs: Dirs,
    playing: PlayingFn,
    emit: EmitFn,
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
}

impl Engine {
    pub fn new(
        sync: Arc<SyncService>,
        db: db::Db,
        dirs: Dirs,
        playing: PlayingFn,
        emit: EmitFn,
    ) -> Self {
        Self {
            sync,
            repo: SyncRepo::new(db.clone()),
            roms: RomsRepo::new(db),
            dirs,
            playing,
            emit,
            one_at_a_time: tokio::sync::Mutex::new(()),
            stamp: Mutex::new(String::new()),
            known_files: Mutex::new(Vec::new()),
            rejected: Mutex::new(HashMap::new()),
            prelaunch_budget: PRELAUNCH_BUDGET,
        }
    }

    pub fn playing(&self) -> Playing {
        (self.playing)()
    }

    fn emit(&self, event: SyncEvent) {
        (self.emit)(event)
    }

    // --- as passadas -------------------------------------------------------

    /// Uma passada completa (ou de uma ROM): lê a linha desta máquina, a
    /// nuvem e o disco, decide cada chave e age.
    pub async fn pass(&self, alva: &Alvanista, scope: PassScope) -> Result<PassReport, SyncError> {
        let _one = self.one_at_a_time.lock().await;
        let device = self.ready(alva).await?;
        if !self.saves_on().await? {
            return Ok(PassReport::default());
        }
        let result = self.run(alva, &device, &scope, Mode::Pass).await;
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

    /// Sessão, aparelho, ligado e sem pausa local. Só o SQLite: nem chaveiro
    /// nem rede.
    async fn ready(&self, alva: &Alvanista) -> Result<Device, SyncError> {
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
        self.own_account(alva).await?;
        Ok(device)
    }

    /// A base e os conflitos são da conta que os gravou (M1). Outra conta
    /// nesta máquina não herda o "já sincronizado" da anterior: a base e os
    /// conflitos saem, e o primeiro sync dela segue a regra de duas máquinas
    /// configuradas à parte (bytes diferentes viram conflito, nada desce por
    /// cima). Só o SQLite.
    async fn own_account(&self, alva: &Alvanista) -> Result<(), SyncError> {
        let devices = &self.sync.devices;
        let owner = devices.base_account().await.map_err(failed)?;
        let Some(current) = alva.account_id().await else {
            // Sem o perfil não se sabe de quem é a conta: uma base que é de
            // alguém não serve até o `/me` responder.
            return match owner {
                Some(_) => Err(SyncError::Offline),
                None => Ok(()),
            };
        };
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
        self.new_stamp();
        self.sweep_temps();
        // Toda passada lê a linha desta máquina de novo (sem leitura guardada):
        // uma pausa ou revogação feita há segundos já vale antes de abrir.
        let devices = self.sync.partner_devices(alva).await?;
        let receive = self.check_row(&devices, &device.id, true).await?;
        let token = self.token()?;
        // Erro ao ler o disco ou a biblioteca acaba a passada: uma lista curta
        // demais faria save sincronizado parecer apagado.
        let local_err = |e: String| failed(format!("saves locais: {e}"));
        let (files, local) = match scope {
            PassScope::All => {
                let files = self.list(alva, &token).await?;
                let local = account_saves(&self.roms, &self.dirs.saves, &self.dirs.system)
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
                    return Ok(PassReport::default());
                };
                let (names, cores) = local::rom_listing(&rom, &self.dirs.saves);
                let files = self.list_for(alva, &token, &names, &cores).await?;
                // Saiu da biblioteca no meio: nada a fazer (um mapa vazio faria
                // todo conflito parecer de jogo fora da biblioteca).
                let Some(local) = rom_saves(&self.roms, &self.dirs.saves, &self.dirs.system, id)
                    .await
                    .map_err(local_err)?
                else {
                    return Ok(PassReport::default());
                };
                (files, local)
            }
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
        let mut cx = Ctx {
            alva,
            mode,
            token,
            devices,
            receive,
            uploads_stopped: false,
        };
        let mut report = PassReport::default();
        for entry in &entries {
            self.step(&mut cx, &device.id, entry, &known, &mut report)
                .await?;
        }
        Ok(report)
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
                            .and_then(|(key, _)| parse_key(key))
                            .map(|(_, _, name)| name.to_owned());
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
            ApiError::TooLarge => Fail::File(SyncError::TooLarge),
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

    /// As linhas da nuvem de um arquivo: o nome dele, ou a pasta (`core`) de
    /// um cartão ou VMU.
    async fn list_file(
        &self,
        alva: &Alvanista,
        token: &str,
        file: &SaveFile,
    ) -> Result<Vec<RemoteFile>, SyncError> {
        match file.core {
            Some(core) => self.list_for(alva, token, &[], &[core]).await,
            None => {
                self.list_for(alva, token, std::slice::from_ref(&file.filename), &[])
                    .await
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
        let by_name = by_name(files);
        let bases: HashMap<String, BaseRow> = self
            .repo
            .base_all("file")
            .await
            .map_err(repo_err)?
            .into_iter()
            .map(|b| (b.key.clone(), b))
            .collect();
        let mut keys = Keys::default();
        for file in &local.files {
            keys.add_present(file.clone());
        }
        let mut base_keys: Vec<&String> = bases.keys().collect();
        base_keys.sort();
        for key in base_keys {
            if let Some(slot) = local.slot_for_key(key) {
                keys.add(slot);
            }
        }
        for row in files {
            let slots = match row.core.as_deref() {
                Some(core) if core.starts_with("system/") => {
                    local.system_slot(core, &row.filename).into_iter().collect()
                }
                _ => local.rom_slots(&row.filename),
            };
            for slot in slots {
                if pick(&by_name, &slot).is_some() {
                    keys.add(slot);
                }
            }
        }
        // Duas ROMs de mesmo stem dividem o arquivo: a passada de uma também
        // leva o arquivo que a outra listou primeiro.
        let scope_paths: HashSet<String> = match scope {
            PassScope::All => HashSet::new(),
            PassScope::Rom(id) => match self.roms.get(id).await {
                Ok(Some(rom)) => local::rom_slot_files(&rom, &self.dirs.saves)
                    .iter()
                    .map(|f| Keys::path_key(&f.path))
                    .collect(),
                Ok(None) => HashSet::new(),
                Err(e) => return Err(failed(format!("ROM {id}: {e}"))),
            },
        };
        let in_scope = |f: &SaveFile| match scope {
            PassScope::All => true,
            PassScope::Rom(id) => {
                f.rom_id.as_deref() == Some(id.as_str())
                    || scope_paths.contains(&Keys::path_key(&f.path))
                    || (f.rom_id.is_none() && local.rom_platform(id) == Some(f.platform))
            }
        };
        Ok(keys
            .files
            .into_iter()
            .filter(in_scope)
            .map(|file| Entry {
                base: bases.get(&file.key).cloned(),
                row: pick(&by_name, &file).cloned(),
                file,
            })
            .collect())
    }

    async fn step(
        &self,
        cx: &mut Ctx<'_>,
        me: &str,
        e: &Entry,
        known: &HashSet<String>,
        report: &mut PassReport,
    ) -> Result<(), SyncError> {
        let key = &e.file.key;
        // O hash fora das tarefas do Tokio: o prazo de antes de abrir vale aqui.
        let path = e.file.path.clone();
        let hashed = tokio::task::spawn_blocking(move || sha256_file(&path))
            .await
            .unwrap_or_else(|j| Err(io::Error::other(j.to_string())));
        let local = match hashed {
            Ok((hash, size, mtime)) => Some(Local { hash, size, mtime }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => None,
            Err(err) => {
                // Arquivo que não se lê não é arquivo ausente: nada desce por cima.
                report
                    .errors
                    .push((key.clone(), failed(format!("{}: {err}", log_tag(key)))));
                return Ok(());
            }
        };
        let remote = e.row.as_ref().and_then(remote_of);
        let kind = Kind::of_key(key).unwrap_or(Kind::File);
        let action = decide(kind, local.as_ref(), e.base.as_ref(), remote.as_ref(), me);
        let done = match action {
            Action::Nothing => Ok(Done::Nothing),
            Action::Adopt => self.adopt(e, local.as_ref(), remote.as_ref()).await,
            Action::Upload | Action::ReUpload => self.upload_step(cx, e, local.as_ref()).await,
            Action::Download => self.download_step(cx, e, remote.as_ref()).await,
            Action::Conflict => {
                self.conflict_step(cx, e, local.as_ref(), remote.as_ref(), known)
                    .await
            }
            Action::LocalToBackupAndClearBase => self.accept_deletion(cx, e).await,
            Action::MarkBaseWipedKeepLocal | Action::DropBase => self
                .base_only(key, action)
                .await
                .map(|()| Done::Nothing)
                .map_err(Fail::File),
        };
        match done {
            Ok(Done::Deferred) => report.deferred += 1,
            Ok(done) => {
                match done {
                    Done::Uploaded => report.uploaded += 1,
                    Done::Downloaded => report.downloaded += 1,
                    Done::Conflict => report.conflicts += 1,
                    Done::BackedUp => report.backed_up += 1,
                    Done::Nothing | Done::Deferred => {}
                }
                // A chave que deixou de estar em conflito sai da lista.
                if !matches!(done, Done::Conflict) && known.contains(key) {
                    if let Err(err) = self.repo.conflict_delete(key).await {
                        log::warn!("sync: conflito resolvido não saiu da lista: {err}");
                    }
                }
            }
            Err(Fail::File(err)) => report.errors.push((key.clone(), err)),
            Err(Fail::StopUploads(err)) => {
                cx.uploads_stopped = true;
                if err == SyncError::Quota {
                    self.quota_event().await;
                }
                report.errors.push((key.clone(), err));
            }
            Err(Fail::End(err)) => return Err(err),
        }
        Ok(())
    }

    /// A cota acabou: a tela recebe os números (ou só o código, se a cota não
    /// responder).
    async fn quota_event(&self) {
        let quota = match self.sync.refresh_quota().await {
            Ok(()) => self.sync.runtime().quota,
            Err(_) => None,
        };
        self.emit(match quota {
            Some(q) => SyncEvent::Quota {
                used_bytes: q.used_bytes,
                limit_bytes: q.limit_bytes,
            },
            None => SyncEvent::Error {
                code: SyncError::Quota.to_string(),
            },
        });
    }

    /// Os dois lados já são iguais: só a base anda.
    async fn adopt(
        &self,
        e: &Entry,
        local: Option<&Local>,
        remote: Option<&Remote>,
    ) -> Result<Done, Fail> {
        let (Some(l), Some(r)) = (local, remote) else {
            return Ok(Done::Nothing);
        };
        self.put_base(&e.file, &l.hash, &r.save_id, &r.version_id, &r.hash)
            .await
            .map_err(Fail::File)?;
        Ok(Done::Nothing)
    }

    async fn upload_step(
        &self,
        cx: &Ctx<'_>,
        e: &Entry,
        local: Option<&Local>,
    ) -> Result<Done, Fail> {
        if cx.mode == Mode::BeforeLaunch || cx.uploads_stopped || !self.may_upload(&e.file) {
            return Ok(Done::Deferred);
        }
        // Os mesmos bytes que a nuvem já recusou de vez não sobem de novo; a
        // passada repete o erro para a tela seguir dizendo qual arquivo.
        if let Some(err) = local.and_then(|l| self.still_rejected(&e.file.key, &l.hash)) {
            return Err(Fail::File(err));
        }
        self.send(cx.alva, &cx.token, &e.file, e.row.as_ref()).await
    }

    /// O erro guardado quando a nuvem já recusou estes bytes desta chave.
    fn still_rejected(&self, key: &str, hash: &str) -> Option<SyncError> {
        lock(&self.rejected)
            .get(key)
            .filter(|(seen, _)| seen == hash)
            .map(|(_, err)| err.clone())
    }

    /// Lê o arquivo, manda, e só com o 201 grava a base.
    async fn send(
        &self,
        alva: &Alvanista,
        token: &str,
        file: &SaveFile,
        row: Option<&RemoteFile>,
    ) -> Result<Done, Fail> {
        let bytes = std::fs::read(&file.path)
            .map_err(|err| Fail::File(failed(format!("{}: {err}", log_tag(&file.key)))))?;
        // Arquivo vazio é "sem save": nunca sobe.
        if bytes.is_empty() {
            return Ok(Done::Deferred);
        }
        let hash = sha256_hex(&bytes);
        let client_mtime = std::fs::metadata(&file.path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| rfc3339(d.as_secs() as i64));
        // Linha que já existe: o envio vai para ela, com a plataforma e o core
        // dela (D113, D125). Linha nova: a plataforma do ReEmu, e core só nos
        // cartões e VMUs (a pasta, como a porta WebDAV arquiva).
        let (platform, core, core_version, filename) = match row {
            Some(r) => (
                r.platform.as_str(),
                r.core.as_deref(),
                r.core_version.as_deref(),
                r.filename.as_str(),
            ),
            None => (file.platform, file.core, None, file.filename.as_str()),
        };
        let meta = UploadMeta {
            platform,
            kind: "file",
            filename,
            core,
            core_version,
            client_mtime,
        };
        let up = match self.sync.api.upload(token, &meta, bytes).await {
            Ok(up) => up,
            Err(err) => {
                let fail = self.file_error(alva, err, true).await;
                if let Fail::File(e) = &fail {
                    if is_permanent(e) {
                        lock(&self.rejected).insert(file.key.clone(), (hash, e.clone()));
                    }
                }
                return Err(fail);
            }
        };
        lock(&self.rejected).remove(&file.key);
        if up.deduplicated {
            log::debug!("sync: {} já estava na nuvem", log_tag(&file.key));
        }
        self.put_base(file, &hash, &up.save_id, &up.version_id, &hash)
            .await
            .map_err(Fail::File)?;
        Ok(Done::Uploaded)
    }

    async fn download_step(
        &self,
        cx: &Ctx<'_>,
        e: &Entry,
        remote: Option<&Remote>,
    ) -> Result<Done, Fail> {
        let Some(r) = remote else {
            return Ok(Done::Nothing);
        };
        // O filtro é pela plataforma do jogo daqui: uma linha `other` de um
        // RetroArch plano desce para quem recebe `snes`.
        if !receives(cx, e.file.platform) || !self.may_write(cx.mode) {
            return Ok(Done::Deferred);
        }
        let bytes = match self
            .sync
            .api
            .download(&cx.token, &r.save_id, &r.version_id, r.size)
            .await
        {
            Ok(b) => b,
            Err(err) => return Err(self.file_error(cx.alva, err, false).await),
        };
        if !self.place(&e.file, &bytes, &r.hash, cx.mode)? {
            return Ok(Done::Deferred);
        }
        self.put_base(&e.file, &r.hash, &r.save_id, &r.version_id, &r.hash)
            .await
            .map_err(Fail::File)?;
        Ok(Done::Downloaded)
    }

    /// Confere os bytes, copia o arquivo de agora para o backup e grava por
    /// cima (tmp, fsync, rename). `false`: um jogo abriu no meio e nada foi
    /// escrito.
    fn place(
        &self,
        file: &SaveFile,
        bytes: &[u8],
        expected: &str,
        mode: Mode,
    ) -> Result<bool, Fail> {
        if bytes.is_empty() {
            return Err(Fail::File(failed(format!(
                "{}: a nuvem mandou um arquivo vazio",
                log_tag(&file.key)
            ))));
        }
        if !sha256_hex(bytes).eq_ignore_ascii_case(expected) {
            return Err(Fail::File(failed(format!(
                "{}: os bytes não batem com o hash da nuvem",
                log_tag(&file.key)
            ))));
        }
        // De novo, logo antes de escrever: o jogo pode ter aberto no meio.
        if !self.may_write(mode) {
            return Ok(false);
        }
        let io_fail = |what: &str, err: io::Error| {
            Fail::File(failed(format!("{what} {}: {err}", log_tag(&file.key))))
        };
        if present(&file.path).map_err(|err| io_fail("lendo", err))? {
            self.backup(&file.path)
                .map_err(|err| io_fail("backup de", err))?;
        }
        write_atomic(&file.path, bytes).map_err(|err| io_fail("gravando", err))?;
        Ok(true)
    }

    /// A nuvem apagou um save da conta: o arquivo daqui se muda para o backup
    /// e a base fica marcada (S13: a página do jogo diz que foi apagado no
    /// site e abre a pasta).
    async fn accept_deletion(&self, cx: &Ctx<'_>, e: &Entry) -> Result<Done, Fail> {
        if !receives(cx, e.file.platform) || !self.may_write(cx.mode) {
            return Ok(Done::Deferred);
        }
        self.removed_on_site(&e.file).await.map_err(Fail::File)?;
        Ok(Done::BackedUp)
    }

    /// Muda o arquivo daqui para o backup e deixa a base como marca de
    /// "apagado no site", com o lugar para onde ele foi.
    async fn removed_on_site(&self, file: &SaveFile) -> Result<(), SyncError> {
        let moved = self.clear_away(file)?;
        let moved = moved.map(|p| p.to_string_lossy().into_owned());
        self.repo
            .base_mark_wiped(&file.key, now_secs(), moved.as_deref())
            .await
            .map_err(repo_err)
    }

    /// Muda o arquivo (se existir) para a pasta de backup; devolve para onde.
    fn clear_away(&self, file: &SaveFile) -> Result<Option<PathBuf>, SyncError> {
        let fail = |err: io::Error| failed(format!("backup de {}: {err}", log_tag(&file.key)));
        if present(&file.path).map_err(fail)? {
            return self.move_to_backup(&file.path).map(Some).map_err(fail);
        }
        Ok(None)
    }

    async fn conflict_step(
        &self,
        cx: &Ctx<'_>,
        e: &Entry,
        local: Option<&Local>,
        remote: Option<&Remote>,
        known: &HashSet<String>,
    ) -> Result<Done, Fail> {
        let (Some(l), Some(r)) = (local, remote) else {
            return Ok(Done::Nothing);
        };
        // O nome é de quem ESCREVEU a versão da nuvem, não do dono da linha.
        let writer = e
            .row
            .as_ref()
            .and_then(|row| row.current.as_ref())
            .and_then(|v| machine_label(v, &cx.devices));
        let row = ConflictRow {
            key: e.file.key.clone(),
            local_hash: l.hash.clone(),
            local_mtime: l.mtime,
            local_size: l.size as i64,
            remote_save_id: r.save_id.clone(),
            remote_version_id: r.version_id.clone(),
            remote_hash: r.hash.clone(),
            remote_device_label: writer,
            remote_client_mtime: r.client_mtime,
            remote_size: r.size,
            // A linha da nuvem foi apagada no site: o diálogo pergunta outra
            // coisa (subir de novo ou aceitar a remoção).
            remote_deleted: r.deleted,
            rom_id: e.file.rom_id.clone(),
            seen_at: now_secs(),
        };
        self.repo
            .conflict_put(&row)
            .await
            .map_err(|err| Fail::File(repo_err(err)))?;
        if !known.contains(&e.file.key) {
            self.emit(SyncEvent::Conflict {
                key: e.file.key.clone(),
            });
        }
        Ok(Done::Conflict)
    }

    /// As ações que só mexem na base: nada em disco nem na rede (D127).
    async fn base_only(&self, key: &str, action: Action) -> Result<(), SyncError> {
        let done = match action {
            Action::MarkBaseWipedKeepLocal => {
                self.repo.base_mark_wiped(key, now_secs(), None).await
            }
            Action::DropBase => self.repo.base_delete(key).await,
            _ => Ok(()),
        };
        done.map_err(repo_err)
    }

    async fn put_base(
        &self,
        file: &SaveFile,
        local_hash: &str,
        save_id: &str,
        version_id: &str,
        remote_hash: &str,
    ) -> Result<(), SyncError> {
        self.repo
            .base_put(&BaseRow {
                key: file.key.clone(),
                kind: "file".into(),
                remote_save_id: save_id.into(),
                remote_version_id: version_id.into(),
                remote_hash: remote_hash.to_ascii_lowercase(),
                local_hash: local_hash.into(),
                local_path: file.path.to_string_lossy().into_owned(),
                rom_crc32: file.rom_crc32.clone(),
                synced_at: now_secs(),
                wiped_at: None,
                backup_path: None,
            })
            .await
            .map_err(repo_err)
    }

    /// Escrever save só sem jogo aberto. Antes de abrir, quem espera é o
    /// próprio jogo que está abrindo: aí vale.
    fn may_write(&self, mode: Mode) -> bool {
        match self.playing() {
            Playing::Idle => true,
            Playing::Loading => mode == Mode::BeforeLaunch,
            Playing::Running(_) => false,
        }
    }

    /// Ler para subir vale com jogo aberto, menos os arquivos dele e os
    /// cartões e VMUs (que o core pode estar gravando).
    fn may_upload(&self, file: &SaveFile) -> bool {
        match self.playing() {
            Playing::Idle => true,
            Playing::Loading | Playing::Running(None) => false,
            Playing::Running(Some(rom)) => file.rom_id.as_deref().is_some_and(|r| r != rom),
        }
    }

    // --- backups -----------------------------------------------------------

    fn new_stamp(&self) {
        *lock(&self.stamp) = local_stamp();
    }

    /// Apaga os `.sync-tmp` que uma escrita interrompida deixou (o app fechou
    /// entre o temporário e o `rename`). Só esses: nenhum outro arquivo.
    fn sweep_temps(&self) {
        let mut dirs = vec![self.dirs.saves.clone()];
        dirs.extend(local::system_dirs(&self.dirs.system));
        for dir in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let is_ours = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|n| n.ends_with(TMP_SUFFIX));
                if is_ours && entry.file_type().is_ok_and(|t| t.is_file()) {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        log::warn!("sync: temporário velho não saiu: {e}");
                    }
                }
            }
        }
    }

    /// Onde o backup de `path` fica: `<dados>/sync-backups/<carimbo>/<caminho
    /// relativo aos dados>`, sem nunca sobrescrever um backup.
    fn backup_target(&self, path: &Path) -> io::Result<PathBuf> {
        let stamp = {
            let mut s = lock(&self.stamp);
            if s.is_empty() {
                *s = local_stamp();
            }
            s.clone()
        };
        let rel = match path.strip_prefix(&self.dirs.data) {
            Ok(rel) if !rel.as_os_str().is_empty() => rel.to_path_buf(),
            _ => PathBuf::from(path.file_name().unwrap_or(path.as_os_str())),
        };
        let first = self.dirs.data.join("sync-backups").join(stamp).join(rel);
        if let Some(parent) = first.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut target = first.clone();
        let mut n = 2;
        while present(&target)? {
            let name = first
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            target = first.with_file_name(format!("{name} ({n})"));
            n += 1;
        }
        Ok(target)
    }

    /// Copia o arquivo para a pasta de backup da passada (o original fica). A
    /// cópia vai ao disco (`fsync`) antes de o original ser substituído.
    pub fn backup(&self, path: &Path) -> io::Result<PathBuf> {
        let target = self.backup_target(path)?;
        copy_synced(path, &target)?;
        Ok(target)
    }

    /// Muda o arquivo para a pasta de backup da passada.
    fn move_to_backup(&self, path: &Path) -> io::Result<PathBuf> {
        let target = self.backup_target(path)?;
        if std::fs::rename(path, &target).is_err() {
            // Outro volume: copia, grava no disco e só então tira o original.
            copy_synced(path, &target)?;
            std::fs::remove_file(path)?;
        }
        Ok(target)
    }

    // --- o que a pessoa pede ------------------------------------------------

    /// Fecha um conflito (D118). `Local`: o arquivo daqui sobe como versão
    /// nova. `Remote`: o daqui vai para o backup e o da nuvem desce; se a
    /// nuvem apagou o save, aceitar é só mandar o daqui para o backup.
    pub async fn resolve(&self, alva: &Alvanista, key: &str, keep: Keep) -> Result<(), SyncError> {
        let _one = self.one_at_a_time.lock().await;
        self.ready(alva).await?;
        self.new_stamp();
        let conflict = self
            .repo
            .conflicts_all()
            .await
            .map_err(repo_err)?
            .into_iter()
            .find(|c| c.key == key)
            .ok_or_else(|| failed(format!("{}: sem conflito aberto", log_tag(key))))?;
        let token = self.token()?;
        let file = self.file_for(key).await?;
        let files = self.list_file(alva, &token, &file).await?;
        let names = by_name(&files);
        let row = files
            .iter()
            .find(|f| f.save_id == conflict.remote_save_id)
            .or_else(|| pick(&names, &file));
        match keep {
            Keep::Local => {
                if !self.may_upload(&file) {
                    return Err(failed(format!("{}: jogo aberto", log_tag(key))));
                }
                match self.send(alva, &token, &file, row).await {
                    Ok(Done::Uploaded) => {}
                    Ok(_) => return Err(failed(format!("{}: nada para enviar", log_tag(key)))),
                    Err(f) => return Err(fail_error(f)),
                }
            }
            Keep::Remote => {
                self.keep_remote(alva, &token, &file, row, &conflict)
                    .await?
            }
        }
        self.repo.conflict_delete(key).await.map_err(repo_err)?;
        self.after_change().await;
        Ok(())
    }

    async fn keep_remote(
        &self,
        alva: &Alvanista,
        token: &str,
        file: &SaveFile,
        row: Option<&RemoteFile>,
        conflict: &ConflictRow,
    ) -> Result<(), SyncError> {
        if !self.may_write(Mode::Pass) {
            return Err(failed(format!("{}: jogo aberto", log_tag(&file.key))));
        }
        let live = row.filter(|r| r.deleted_at.is_none());
        let Some((row, current)) = live.and_then(|r| r.current.as_ref().map(|c| (r, c))) else {
            // A nuvem apagou o save: aceitar é mandar o daqui para o backup, e
            // a página do jogo passa a dizer que ele foi apagado no site.
            return self.removed_on_site(file).await;
        };
        // A versão que o diálogo mostrou; se a linha mudou, a atual.
        let (version_id, hash, size) =
            if row.save_id == conflict.remote_save_id && !conflict.remote_version_id.is_empty() {
                (
                    conflict.remote_version_id.clone(),
                    conflict.remote_hash.clone(),
                    conflict.remote_size,
                )
            } else {
                (
                    current.version_id.clone(),
                    current.content_hash.to_ascii_lowercase(),
                    current.size_bytes,
                )
            };
        let bytes = match self
            .sync
            .api
            .download(token, &row.save_id, &version_id, size)
            .await
        {
            Ok(b) => b,
            Err(err) => return Err(fail_error(self.file_error(alva, err, false).await)),
        };
        if !self
            .place(file, &bytes, &hash, Mode::Pass)
            .map_err(fail_error)?
        {
            return Err(failed(format!("{}: jogo aberto", log_tag(&file.key))));
        }
        self.put_base(file, &hash, &row.save_id, &version_id, &hash)
            .await
    }

    /// Volta uma versão do histórico (D124): o arquivo de agora vai para o
    /// backup, a versão desce e sobe como a atual, para as outras máquinas
    /// seguirem.
    pub async fn restore(
        &self,
        alva: &Alvanista,
        key: &str,
        version_id: &str,
    ) -> Result<(), SyncError> {
        let _one = self.one_at_a_time.lock().await;
        self.ready(alva).await?;
        self.new_stamp();
        let token = self.token()?;
        let file = self.file_for(key).await?;
        if !self.may_write(Mode::Pass) || !self.may_upload(&file) {
            return Err(failed(format!("{}: jogo aberto", log_tag(key))));
        }
        let files = self.list_file(alva, &token, &file).await?;
        let row = pick(&by_name(&files), &file)
            .cloned()
            .ok_or_else(|| failed(format!("{}: sem linha na nuvem", log_tag(key))))?;
        let versions = match self.sync.api.versions(&token, &row.save_id).await {
            Ok(v) => v,
            Err(err) => return Err(fail_error(self.file_error(alva, err, false).await)),
        };
        let version = versions
            .iter()
            .find(|v| v.version_id == version_id)
            .ok_or_else(|| failed(format!("{}: versão {version_id} não existe", log_tag(key))))?;
        if version.size_bytes <= 0 {
            return Err(failed(format!("{}: versão vazia", log_tag(key))));
        }
        let bytes = match self
            .sync
            .api
            .download(&token, &row.save_id, version_id, version.size_bytes)
            .await
        {
            Ok(b) => b,
            Err(err) => return Err(fail_error(self.file_error(alva, err, false).await)),
        };
        if !self
            .place(&file, &bytes, &version.content_hash, Mode::Pass)
            .map_err(fail_error)?
        {
            return Err(failed(format!("{}: jogo aberto", log_tag(key))));
        }
        // Sobe como a versão atual. Se o envio falhar, o arquivo daqui já é a
        // versão antiga e a base não andou: a próxima passada sobe de novo.
        match self.send(alva, &token, &file, Some(&row)).await {
            Ok(Done::Uploaded) => {}
            Ok(_) => return Err(failed(format!("{}: nada para enviar", log_tag(key)))),
            Err(f) => return Err(fail_error(f)),
        }
        if let Err(e) = self.repo.conflict_delete(key).await {
            log::warn!("sync: conflito não saiu depois de restaurar: {e}");
        }
        self.after_change().await;
        Ok(())
    }

    /// As versões do arquivo de uma chave, da mais nova para a mais velha.
    pub async fn versions(
        &self,
        alva: &Alvanista,
        key: &str,
    ) -> Result<Vec<SyncVersion>, SyncError> {
        self.ready(alva).await?;
        let token = self.token()?;
        let file = self.file_for(key).await?;
        let files = self.list_file(alva, &token, &file).await?;
        let Some(row) = pick(&by_name(&files), &file).cloned() else {
            return Ok(Vec::new());
        };
        let versions = match self.sync.api.versions(&token, &row.save_id).await {
            Ok(v) => v,
            Err(err) => return Err(fail_error(self.file_error(alva, err, false).await)),
        };
        // Os nomes das máquinas são cortesia: sem a lista, ficam os do servidor.
        let devices = self.sync.partner_devices(alva).await.unwrap_or_default();
        let current = row
            .current
            .as_ref()
            .filter(|_| row.deleted_at.is_none())
            .map(|c| c.version_id.as_str());
        Ok(versions
            .iter()
            .map(|v| SyncVersion {
                version_id: v.version_id.clone(),
                machine: machine_label(v, &devices).unwrap_or_default(),
                device_id: v.device_token_id.clone(),
                created_at: parse_time(&v.server_created_at).map_or(0, |t| t.0),
                size: v.size_bytes.max(0) as u64,
                is_current: current == Some(v.version_id.as_str()),
            })
            .collect())
    }

    /// Os conflitos abertos, como o diálogo os mostra.
    pub async fn conflicts(&self) -> Result<Vec<SyncConflict>, SyncError> {
        let rows = self.repo.conflicts_all().await.map_err(repo_err)?;
        let this = self.sync.devices.device().await.map_err(failed)?;
        let here = this
            .as_ref()
            .map(|d| d.label.clone())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(default_label);
        let here_id = this.map(|d| d.id);
        let known = lock(&self.known_files).clone();
        let mut out = Vec::with_capacity(rows.len());
        for c in rows {
            let rom = match &c.rom_id {
                Some(id) => self.roms.get(id).await.ok().flatten(),
                None => None,
            };
            let title = match rom {
                Some(rom) => crate::commands::rom_title(&rom),
                None => parse_key(&c.key).map_or_else(|| c.key.clone(), |(_, _, n)| n.to_owned()),
            };
            out.push(SyncConflict {
                title,
                local: Side {
                    machine: here.clone(),
                    device_id: here_id.clone(),
                    saved_at: c.local_mtime,
                    size: c.local_size.max(0) as u64,
                    deleted: false,
                },
                remote: Side {
                    deleted: c.remote_deleted,
                    machine: c.remote_device_label.unwrap_or_default(),
                    device_id: known
                        .iter()
                        .find(|f| f.save_id == c.remote_save_id)
                        .and_then(|f| f.current.as_ref())
                        .and_then(|v| v.device_token_id.clone()),
                    saved_at: c.remote_client_mtime.unwrap_or(c.seen_at),
                    size: c.remote_size.max(0) as u64,
                },
                key: c.key,
                rom_id: c.rom_id,
            });
        }
        Ok(out)
    }

    /// A linha de save da página do jogo, pelo disco e pelo SQLite.
    pub async fn save_status(&self, rom_id: &str) -> Result<SaveLineStatus, SyncError> {
        // Sync desligado, aparelho revogado ou game saves desligados: a página
        // do jogo não mostra nada nem pergunta nada. Só SQLite, sem rede.
        if !self.saves_line_on().await? {
            return Ok(SaveLineStatus {
                state: SaveLineState::None,
                synced_at: None,
                versions: 0,
                key: None,
            });
        }
        let rom = self
            .roms
            .get(rom_id)
            .await
            .map_err(|e| failed(format!("ROM {rom_id}: {e}")))?
            .ok_or_else(|| failed(format!("ROM {rom_id} não existe")))?;
        let mut files = local::rom_slot_files(&rom, &self.dirs.saves);
        let platform = platform_for_system(&rom.system_id);
        files.extend(local::system_saves(&self.dirs.system, platform).map_err(failed)?);
        let versions_of = |key: &str| -> u32 {
            let known = lock(&self.known_files);
            files
                .iter()
                .find(|f| f.key == key)
                .and_then(|f| pick(&by_name(&known), f).map(|r| r.version_count.max(0) as u32))
                .unwrap_or(0)
        };
        let conflicts = self.repo.conflicts_all().await.map_err(repo_err)?;
        if let Some(c) = conflicts
            .iter()
            .find(|c| c.rom_id.as_deref() == Some(rom_id) || files.iter().any(|f| f.key == c.key))
        {
            return Ok(SaveLineStatus {
                state: SaveLineState::Conflict,
                synced_at: None,
                versions: versions_of(&c.key),
                key: Some(c.key.clone()),
            });
        }
        let mut main: Option<String> = None;
        let mut pending = false;
        let mut synced_at: Option<i64> = None;
        // Um save apagado no site cujo arquivo daqui foi para o resguardo.
        let mut removed: Option<String> = None;
        for f in &files {
            let base = self.repo.base_get(&f.key).await.map_err(repo_err)?;
            let local = match sha256_file(&f.path) {
                Ok((hash, size, _)) if size > 0 => Some(hash),
                Ok(_) => None,
                Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                Err(e) => return Err(failed(format!("{}: {e}", log_tag(&f.key)))),
            };
            // A base marcada pela remoção aceita vale por ausente; sem arquivo
            // aqui, ela é o que a página diz.
            let wiped = base.as_ref().is_some_and(|b| b.wiped_at.is_some());
            let base = base.filter(|b| b.wiped_at.is_none());
            if wiped && local.is_none() {
                removed.get_or_insert_with(|| f.key.clone());
            }
            if local.is_none() && base.is_none() {
                continue;
            }
            main.get_or_insert_with(|| f.key.clone());
            match (local, base) {
                (Some(hash), Some(b)) if hash == b.local_hash => {
                    synced_at = synced_at.max(Some(b.synced_at));
                }
                _ => pending = true,
            }
        }
        let state = match (&main, pending, &removed) {
            (None, _, None) => SaveLineState::None,
            (None, _, Some(_)) => SaveLineState::Removed,
            (Some(_), true, _) => SaveLineState::Pending,
            (Some(_), false, _) => SaveLineState::Synced,
        };
        Ok(SaveLineStatus {
            state,
            synced_at: if state == SaveLineState::Synced {
                synced_at
            } else {
                None
            },
            versions: main.as_deref().map_or(0, versions_of),
            key: main.or(removed),
        })
    }

    /// A pasta onde foi parar o arquivo de um save apagado no site (para o
    /// "Abrir pasta" da página do jogo). Só dentro de `sync-backups`, e só se
    /// ela ainda existe; senão `None` e quem chama abre a pasta de backups.
    pub async fn removed_folder(&self, key: &str) -> Option<PathBuf> {
        let base = self.repo.base_get(key).await.ok().flatten()?;
        base.wiped_at?;
        let moved = PathBuf::from(base.backup_path?);
        let dir = moved.parent()?.to_path_buf();
        let root = self.dirs.data.join("sync-backups");
        let plain = dir
            .components()
            .all(|c| !matches!(c, std::path::Component::ParentDir));
        (plain && dir.starts_with(&root) && dir.is_dir()).then_some(dir)
    }

    /// O arquivo de uma chave, exista ou não: o mapa da biblioteca diz onde
    /// ele fica, sem ler o disco (a chave já é a que a passada deu a ele).
    async fn file_for(&self, key: &str) -> Result<SaveFile, SyncError> {
        let library = local::library(&self.roms, &self.dirs.saves, &self.dirs.system)
            .await
            .map_err(|e| failed(format!("saves locais: {e}")))?;
        library
            .slot_for_key(key)
            .ok_or_else(|| failed(format!("{}: arquivo fora da biblioteca", log_tag(key))))
    }

    /// Depois de resolver ou restaurar: a página relê (a contagem de
    /// conflitos vem do SQLite).
    async fn after_change(&self) {
        self.emit(SyncEvent::Status);
    }
}

/// Uma chave por arquivo e um arquivo por chave: o primeiro que chega fica.
/// O caminho compara sem maiúsculas (no Windows `VMU_SAVE_A1.BIN` e
/// `vmu_save_a1.bin` são o mesmo arquivo).
#[derive(Default)]
struct Keys {
    files: Vec<SaveFile>,
    keys: HashSet<String>,
    paths: HashSet<String>,
}

impl Keys {
    fn path_key(p: &Path) -> String {
        p.to_string_lossy().to_lowercase()
    }

    /// Os arquivos em disco já chegam sem repetição de `account_saves`.
    fn add_present(&mut self, f: SaveFile) {
        self.paths.insert(Self::path_key(&f.path));
        if self.keys.insert(f.key.clone()) {
            self.files.push(f);
        }
    }

    fn add(&mut self, f: SaveFile) {
        let path = Self::path_key(&f.path);
        if self.keys.contains(&f.key) || self.paths.contains(&path) {
            return;
        }
        self.keys.insert(f.key.clone());
        self.paths.insert(path);
        self.files.push(f);
    }
}

fn by_name(files: &[RemoteFile]) -> HashMap<String, Vec<&RemoteFile>> {
    let mut map: HashMap<String, Vec<&RemoteFile>> = HashMap::new();
    for f in files {
        map.entry(f.filename.to_lowercase()).or_default().push(f);
    }
    map
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
/// antes das apagadas; entre várias, a primeira de `rank`.
fn pick<'a>(
    by_name: &HashMap<String, Vec<&'a RemoteFile>>,
    file: &SaveFile,
) -> Option<&'a RemoteFile> {
    let rows = by_name.get(&file.filename.to_lowercase())?;
    let same: Vec<&'a RemoteFile> = rows
        .iter()
        .copied()
        .filter(|r| match file.core {
            Some(core) => {
                r.core.as_deref() == Some(core) && r.filename.eq_ignore_ascii_case(&file.filename)
            }
            None => {
                r.filename == file.filename
                    && !r.core.as_deref().unwrap_or_default().starts_with("system/")
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

// --- o app ---------------------------------------------------------------

/// O motor no estado do Tauri: o que está rodando vem do `AppState`, e os
/// eventos vão para a tela no canal `sync`.
pub fn install(app: &AppHandle, sync: Arc<SyncService>, db: db::Db, dirs: Dirs) {
    let probe = app.clone();
    let playing: PlayingFn = Box::new(move || {
        let Some(state) = probe.try_state::<crate::commands::AppState>() else {
            return Playing::Idle;
        };
        let current_rom = lock(&state.current_rom).clone();
        playing_from(
            state.session.loaded_core().is_some(),
            state
                .loading_game
                .load(std::sync::atomic::Ordering::Relaxed),
            current_rom,
        )
    });
    let sink = app.clone();
    let emit: EmitFn = Box::new(move |event| {
        if let Err(e) = sink.emit(SYNC_EVENT, &event) {
            log::warn!("sync: não avisou a interface: {e}");
        }
    });
    // Ligar o sync (ou os game saves) roda uma passada já, sem esperar a de
    // 10 minutos nem um jogo (M3).
    let kicker = app.clone();
    sync.on_enabled(Box::new(move || {
        let app = kicker.clone();
        tauri::async_runtime::spawn(async move {
            let Some((engine, alva)) = parts(&app) else {
                return;
            };
            log_pass("passada ao ligar", engine.pass(&alva, PassScope::All).await);
        });
    }));
    app.manage(Arc::new(Engine::new(sync, db, dirs, playing, emit)));
}

/// O que está rodando, pelo estado da sessão. Com um load em andamento e um
/// core já aberto, o `current_rom` ainda é o do jogo anterior: vale
/// `Running(None)`. Sem core aberto, o load é `Loading` (a passada de antes de
/// abrir ainda escreve).
fn playing_from(core_open: bool, loading: bool, current_rom: Option<String>) -> Playing {
    match (core_open, loading) {
        (true, true) => Playing::Running(None),
        (true, false) => Playing::Running(current_rom),
        (false, true) => Playing::Loading,
        (false, false) => Playing::Idle,
    }
}

fn parts(app: &AppHandle) -> Option<(Arc<Engine>, tauri::State<'_, Alvanista>)> {
    let engine = app.try_state::<Arc<Engine>>()?.inner().clone();
    Some((engine, app.try_state::<Alvanista>()?))
}

/// A primeira passada com o app aberto e, dali em diante, uma a cada 10
/// minutos sem jogo aberto.
pub async fn run_on_start(app: AppHandle) {
    scheduled_pass(&app).await;
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(IDLE_EVERY).await;
            scheduled_pass(&app).await;
        }
    });
}

async fn scheduled_pass(app: &AppHandle) {
    let Some((engine, alva)) = parts(app) else {
        return;
    };
    if engine.playing() != Playing::Idle {
        return;
    }
    log_pass("passada", engine.pass(&alva, PassScope::All).await);
}

/// Antes de abrir um jogo: espera a nuvem só daquele jogo (no máximo 5 s).
pub async fn before_launch(app: &AppHandle, rom_id: &str) {
    let Some((engine, alva)) = parts(app) else {
        return;
    };
    let outcome = engine.prelaunch(&alva, rom_id).await;
    log::info!("sync: antes de abrir: {outcome:?}");
}

/// Depois de fechar um jogo: a passada daquela ROM, sem esperar por ela.
pub fn after_unload(app: &AppHandle, rom_id: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some((engine, alva)) = parts(&app) else {
            return;
        };
        log_pass(
            "passada depois do jogo",
            engine.pass(&alva, PassScope::Rom(rom_id)).await,
        );
    });
}

fn log_pass(what: &str, result: Result<PassReport, SyncError>) {
    match result {
        Ok(r) => log::info!(
            "sync: {what}: {} enviado(s), {} baixado(s), {} em conflito, {} no backup, {} adiado(s), {} erro(s)",
            r.uploaded,
            r.downloaded,
            r.conflicts,
            r.backed_up,
            r.deferred,
            r.errors.len()
        ),
        Err(SyncError::NoAccount | SyncError::NotEnabled) => {}
        Err(e) => log::info!("sync: {what} terminou em {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alvanista::client::{Client, Entitlements, Me};
    use crate::alvanista::store::test_support::{tokens, MemSecrets, SERIAL};
    use crate::alvanista::store::Store;
    use crate::alvanista::AlvanistaStatus;
    use crate::sync::api::SavesApi;
    use crate::sync::device::{DeviceStore, HardwarePayload};
    use domain::library::Rom;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use wiremock::matchers::{body_partial_json, method, path, query_param};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    const SAVES: &str = "/api/v1/saves";
    const DEVICES: &str = "/api/partner/v1/save-devices";
    const ZELDA: &str = "file:gba/Zelda.srm";

    struct Rig {
        engine: Engine,
        sync: Arc<SyncService>,
        alva: Alvanista,
        secrets: Arc<MemSecrets>,
        store: Store,
        server: MockServer,
        root: PathBuf,
        playing: Arc<Mutex<Playing>>,
        events: Arc<Mutex<Vec<SyncEvent>>>,
        repo: SyncRepo,
        roms: RomsRepo,
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("reemu-sync-engine-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Sessão com `saves`, o aparelho `dev1` ("Sala") ligado, chaveiro em
    /// memória, o Alvanista de mentira e uma pasta de dados vazia.
    async fn rig(name: &str) -> Rig {
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
        let sync = Arc::new(SyncService::new(
            SavesApi::new(server.uri()),
            DeviceStore::new(db.clone(), secrets.clone()),
        ));
        sync.devices
            .save(
                &Device {
                    id: "dev1".into(),
                    label: "Sala".into(),
                },
                "asv_secret",
            )
            .await
            .unwrap();
        sync.devices.set_enabled(true).await.unwrap();
        // Como no app: a conta que acaba no site leva o aparelho junto.
        alva.attach_sync(sync.clone());
        let root = temp(name);
        let playing = Arc::new(Mutex::new(Playing::Idle));
        let events = Arc::new(Mutex::new(Vec::new()));
        let (p, ev) = (playing.clone(), events.clone());
        let engine = Engine::new(
            sync.clone(),
            db.clone(),
            Dirs {
                data: root.clone(),
                saves: root.join("saves"),
                system: root.join("system"),
            },
            Box::new(move || p.lock().unwrap().clone()),
            Box::new(move |e| ev.lock().unwrap().push(e)),
        );
        Rig {
            engine,
            sync,
            alva,
            secrets,
            store,
            server,
            root,
            playing,
            events,
            repo: SyncRepo::new(db.clone()),
            roms: RomsRepo::new(db),
        }
    }

    impl Rig {
        fn path(&self, rel: &str) -> PathBuf {
            self.root.join(rel)
        }

        fn write(&self, rel: &str, bytes: &[u8]) {
            let p = self.path(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        }

        fn read(&self, rel: &str) -> Option<Vec<u8>> {
            std::fs::read(self.path(rel)).ok()
        }

        async fn rom(&self, id: &str, file: &str, system: &str) {
            self.roms
                .add(&Rom {
                    id: id.into(),
                    file_path: format!("C:/roms/{file}"),
                    crc32: "CRC".into(),
                    md5: "MD5".into(),
                    system_id: system.into(),
                    added_at: 0,
                    last_played_at: None,
                    is_favorite: false,
                    user_title: None,
                })
                .await
                .unwrap();
        }

        async fn base(&self, key: &str, rel: &str, local: &[u8], remote: &[u8], ids: (&str, &str)) {
            self.repo
                .base_put(&BaseRow {
                    key: key.into(),
                    kind: "file".into(),
                    remote_save_id: ids.0.into(),
                    remote_version_id: ids.1.into(),
                    remote_hash: sha(remote),
                    local_hash: sha(local),
                    local_path: self.path(rel).to_string_lossy().into_owned(),
                    rom_crc32: Some("CRC".into()),
                    synced_at: 1,
                    wiped_at: None,
                    backup_path: None,
                })
                .await
                .unwrap();
        }

        async fn base_of(&self, key: &str) -> Option<BaseRow> {
            self.repo.base_get(key).await.unwrap()
        }

        /// Cada arquivo de `sync-backups`: (caminho depois do carimbo, bytes).
        fn backups(&self) -> Vec<(String, Vec<u8>)> {
            fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
                for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else {
                        out.push(p);
                    }
                }
            }
            let root = self.path("sync-backups");
            let mut files = Vec::new();
            walk(&root, &mut files);
            files.sort();
            files
                .into_iter()
                .map(|p| {
                    let rel = p.strip_prefix(&root).unwrap();
                    let after_stamp: PathBuf = rel.components().skip(1).collect();
                    let name = after_stamp.to_string_lossy().replace('\\', "/");
                    (name, std::fs::read(&p).unwrap())
                })
                .collect()
        }

        async fn reqs(&self, m: &str, path_prefix: &str) -> Vec<Request> {
            self.server
                .received_requests()
                .await
                .unwrap()
                .into_iter()
                .filter(|r| r.method.as_str() == m && r.url.path().starts_with(path_prefix))
                .collect()
        }

        async fn content_gets(&self) -> usize {
            self.reqs("GET", SAVES)
                .await
                .iter()
                .filter(|r| r.url.path().ends_with("/content"))
                .count()
        }

        async fn listings(&self) -> usize {
            self.reqs("GET", SAVES)
                .await
                .iter()
                .filter(|r| r.url.path() == SAVES)
                .count()
        }

        fn events(&self) -> Vec<SyncEvent> {
            self.events.lock().unwrap().clone()
        }

        fn set_playing(&self, p: Playing) {
            *self.playing.lock().unwrap() = p;
        }

        async fn pass(&self) -> Result<PassReport, SyncError> {
            self.engine.pass(&self.alva, PassScope::All).await
        }

        /// O estado da página, como o comando o devolve.
        async fn status(&self) -> crate::sync::SyncStatus {
            self.sync
                .status_with(&self.alva, HardwarePayload::default())
                .await
                .unwrap()
        }
    }

    fn sha(b: &[u8]) -> String {
        sha256_hex(b)
    }

    fn device(id: &str, label: &str) -> Value {
        json!({"id": id, "label": label, "client_kind": "reemu",
               "created_at": "2026-11-01T00:00:00Z"})
    }

    fn devices_body(rows: Vec<Value>) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({ "devices": rows }))
    }

    async fn mount_devices(server: &MockServer, rows: Vec<Value>) {
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(devices_body(rows))
            .mount(server)
            .await;
    }

    /// Respostas em sequência; a última se repete.
    struct Seq(AtomicUsize, Vec<ResponseTemplate>);

    impl Respond for Seq {
        fn respond(&self, _: &Request) -> ResponseTemplate {
            let n = self.0.fetch_add(1, SeqCst);
            self.1[n.min(self.1.len() - 1)].clone()
        }
    }

    fn seq(responses: Vec<ResponseTemplate>) -> Seq {
        Seq(AtomicUsize::new(0), responses)
    }

    /// Uma linha `kind=file` da conta (sem dono), cuja versão atual foi
    /// escrita por `writer`.
    fn row(
        save_id: &str,
        filename: &str,
        platform: &str,
        core: Option<&str>,
        bytes: &[u8],
        version: &str,
        writer: Option<&str>,
    ) -> Value {
        json!({
            "save_id": save_id, "filename": filename, "slug": null, "kind": "file",
            "platform": platform, "game_uuid": null, "core": core, "core_version": null,
            "visibility": "private",
            "current": {
                "version_id": version, "content_hash": sha(bytes), "size_bytes": bytes.len(),
                "server_created_at": "2026-11-13T10:00:00.250Z",
                "client_mtime": "2026-11-13T09:59:00Z",
                "last_seen_at": "2026-11-13T10:00:00Z", "source_client": "retroarch",
                "origin_media": "upload", "device_token_id": writer
            },
            "version_count": 3, "deleted_at": null, "device_token_id": null
        })
    }

    fn tombstone(mut r: Value) -> Value {
        r["deleted_at"] = json!("2026-11-13T11:00:00Z");
        r
    }

    fn files_body(rows: Vec<Value>) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(Value::Array(rows))
    }

    /// A listagem só responde com `kind=file` e `include_deleted=true`: sem
    /// eles o motor recebe 404 e a passada falha.
    fn files_mock() -> wiremock::MockBuilder {
        Mock::given(method("GET"))
            .and(path(SAVES))
            .and(query_param("kind", "file"))
            .and(query_param("include_deleted", "true"))
    }

    /// A listagem filtrada como o GameKave filtra (`list_files`): `filename`
    /// exato, `core` contra `COALESCE(core, '')` e `platform` exato. As linhas
    /// podem mudar no meio do teste (outra máquina enviou).
    #[derive(Clone)]
    struct Listing(Arc<Mutex<Vec<Value>>>);

    impl Listing {
        fn set(&self, rows: Vec<Value>) {
            *self.0.lock().unwrap() = rows;
        }
    }

    impl Respond for Listing {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            let q: HashMap<String, String> = req.url.query_pairs().into_owned().collect();
            let rows = self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|r| {
                    q.get("filename")
                        .is_none_or(|f| r["filename"].as_str() == Some(f.as_str()))
                        && q.get("core")
                            .is_none_or(|c| r["core"].as_str().unwrap_or("") == c)
                        && q.get("platform")
                            .is_none_or(|p| r["platform"].as_str() == Some(p.as_str()))
                })
                .cloned()
                .collect();
            files_body(rows)
        }
    }

    async fn mount_listing(server: &MockServer, rows: Vec<Value>) -> Listing {
        let listing = Listing(Arc::new(Mutex::new(rows)));
        files_mock()
            .respond_with(listing.clone())
            .mount(server)
            .await;
        listing
    }

    async fn mount_files(server: &MockServer, rows: Vec<Value>) {
        mount_listing(server, rows).await;
    }

    /// Os pedidos de listagem: os `filename` e `core` de cada um (`-` quando
    /// o pedido não filtra por ele).
    async fn listing_filters(r: &Rig) -> Vec<(String, String)> {
        r.reqs("GET", SAVES)
            .await
            .iter()
            .filter(|q| q.url.path() == SAVES)
            .map(|q| {
                let get = |k: &str| {
                    q.url
                        .query_pairs()
                        .find(|(n, _)| n == k)
                        .map_or("-".to_owned(), |(_, v)| v.into_owned())
                };
                (get("filename"), get("core"))
            })
            .collect()
    }

    fn uploaded(save_id: &str, version: &str) -> ResponseTemplate {
        ResponseTemplate::new(201).set_body_json(json!({
            "save_id": save_id, "version_id": version, "deduplicated": false
        }))
    }

    async fn mount_upload(server: &MockServer, save_id: &str, version: &str) {
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(uploaded(save_id, version))
            .mount(server)
            .await;
    }

    async fn mount_content(server: &MockServer, save_id: &str, version: &str, bytes: &[u8]) {
        Mock::given(method("GET"))
            .and(path(format!(
                "{SAVES}/{save_id}/versions/{version}/content"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.to_vec()))
            .mount(server)
            .await;
    }

    fn unauthorized() -> ResponseTemplate {
        ResponseTemplate::new(401).set_body_json(json!({"error": "invalid_token"}))
    }

    /// O valor de um campo do multipart.
    fn field(body: &[u8], name: &str) -> Option<String> {
        let text = String::from_utf8_lossy(body);
        let at = text.find(&format!("name=\"{name}\""))?;
        let rest = &text[at..];
        let start = rest.find("\r\n\r\n")? + 4;
        let end = rest[start..].find("\r\n--")? + start;
        Some(rest[start..end].to_owned())
    }

    async fn never_reconnect() -> Result<AlvanistaStatus, String> {
        panic!("não devia pedir consentimento");
    }

    #[tokio::test]
    async fn first_pass_uploads_local_only_and_writes_base_after_201() {
        let _serial = SERIAL.lock().await;
        let r = rig("first").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"zelda-1");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        mount_upload(&r.server, "s1", "v1").await;

        let report = r.pass().await.unwrap();
        assert_eq!((report.uploaded, report.downloaded), (1, 0));
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!(base.local_hash, sha(b"zelda-1"));
        assert_eq!(base.remote_hash, sha(b"zelda-1"));
        assert_eq!(
            (
                base.remote_save_id.as_str(),
                base.remote_version_id.as_str()
            ),
            ("s1", "v1")
        );
        assert_eq!(PathBuf::from(&base.local_path), r.path("saves/Zelda.srm"));
        assert_eq!(base.rom_crc32.as_deref(), Some("CRC"));

        let posts = r.reqs("POST", SAVES).await;
        assert_eq!(posts.len(), 1);
        let auth = posts[0].headers.get("authorization").unwrap();
        assert_eq!(auth.to_str().unwrap(), "Bearer asv_secret");
        let body = &posts[0].body;
        assert_eq!(field(body, "platform").as_deref(), Some("gba"));
        assert_eq!(field(body, "kind").as_deref(), Some("file"));
        assert_eq!(field(body, "filename").as_deref(), Some("Zelda.srm"));
        assert_eq!(field(body, "core"), None, "save de ROM novo vai sem core");
        assert_eq!(field(body, "core_version"), None);
        assert!(field(body, "client_mtime")
            .and_then(|t| parse_time(&t))
            .is_some());
        assert_eq!(field(body, "file").as_deref(), Some("zelda-1"));

        assert!(r.events().contains(&SyncEvent::Pass {
            uploaded: 1,
            downloaded: 0,
            backed_up: 0
        }));
        let rt = r.sync.runtime();
        assert!(rt.last_pass_at.is_some());
        assert_eq!(rt.last_error, None);
    }

    #[tokio::test]
    async fn upload_failure_leaves_base_untouched() {
        let _serial = SERIAL.lock().await;
        let r = rig("upfail").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"progress");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(seq(vec![ResponseTemplate::new(500), uploaded("s1", "v1")]))
            .mount(&r.server)
            .await;

        assert_eq!(r.pass().await, Err(SyncError::Offline));
        assert!(r.base_of(ZELDA).await.is_none(), "base sem 2xx");
        assert_eq!(r.sync.runtime().last_error.as_deref(), Some("offline"));

        let report = r.pass().await.unwrap();
        assert_eq!(report.uploaded, 1);
        assert_eq!(r.reqs("POST", SAVES).await.len(), 2);
        assert_eq!(r.base_of(ZELDA).await.unwrap().local_hash, sha(b"progress"));
    }

    /// Review Focus 2: o envio falha depois de o jogo gravar; a base não anda,
    /// a próxima passada sobe de novo e a cópia velha da nuvem nunca desce
    /// por cima da mais nova daqui.
    #[tokio::test]
    async fn a_failed_upload_never_brings_the_older_cloud_copy_down() {
        let _serial = SERIAL.lock().await;
        let r = rig("rf2").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"newer");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"old",
                "v1",
                Some("dev1"),
            )],
        )
        .await;
        mount_content(&r.server, "s1", "v1", b"old").await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(seq(vec![ResponseTemplate::new(503), uploaded("s1", "v2")]))
            .mount(&r.server)
            .await;

        assert_eq!(r.pass().await, Err(SyncError::Offline));
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"newer");
        assert_eq!(r.base_of(ZELDA).await.unwrap().local_hash, sha(b"old"));

        let report = r.pass().await.unwrap();
        assert_eq!((report.uploaded, report.downloaded), (1, 0));
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"newer");
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!(
            (base.local_hash, base.remote_version_id),
            (sha(b"newer"), "v2".into())
        );
        assert_eq!(r.content_gets().await, 0);
        assert!(r.backups().is_empty());
    }

    #[tokio::test]
    async fn download_moves_previous_bytes_to_backup() {
        let _serial = SERIAL.lock().await;
        let r = rig("download").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"old");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"new",
                "v2",
                Some("dev2"),
            )],
        )
        .await;
        mount_content(&r.server, "s1", "v2", b"new").await;

        let report = r.pass().await.unwrap();
        assert_eq!((report.uploaded, report.downloaded), (0, 1));
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"new");
        assert_eq!(
            r.backups(),
            vec![("saves/Zelda.srm".to_owned(), b"old".to_vec())]
        );
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!(
            (base.local_hash, base.remote_version_id),
            (sha(b"new"), "v2".into())
        );
        assert!(r.reqs("POST", SAVES).await.is_empty());
        let leftovers: Vec<_> = std::fs::read_dir(r.path("saves"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(leftovers, vec!["Zelda.srm".to_owned()], "sobrou temporário");
    }

    #[tokio::test]
    async fn a_cloud_copy_that_does_not_match_its_hash_is_not_written() {
        let _serial = SERIAL.lock().await;
        let r = rig("badhash").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"old");
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"new",
                "v2",
                Some("dev2"),
            )],
        )
        .await;
        mount_content(&r.server, "s1", "v2", b"garbled").await;
        let report = r.pass().await.unwrap();
        assert_eq!(report.downloaded, 0);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"old");
        assert!(r.backups().is_empty());
        assert_eq!(r.base_of(ZELDA).await.unwrap().remote_version_id, "v1");
    }

    async fn conflict_rig(name: &str) -> Rig {
        let r = rig(name).await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"mine");
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device("dev2", "Notebook")],
        )
        .await;
        mount_files(
            &r.server,
            vec![row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"theirs",
                "v2",
                Some("dev2"),
            )],
        )
        .await;
        mount_content(&r.server, "s1", "v2", b"theirs").await;
        r
    }

    #[tokio::test]
    async fn both_changed_is_a_conflict_and_nothing_is_written() {
        let _serial = SERIAL.lock().await;
        let r = conflict_rig("conflict").await;

        let report = r.pass().await.unwrap();
        assert_eq!(report.conflicts, 1);
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"mine");
        assert!(r.backups().is_empty());
        assert!(r.reqs("POST", SAVES).await.is_empty());
        assert_eq!(r.content_gets().await, 0);
        assert_eq!(r.base_of(ZELDA).await.unwrap().local_hash, sha(b"old"));

        let rows = r.repo.conflicts_all().await.unwrap();
        assert_eq!(rows.len(), 1);
        let c = &rows[0];
        assert_eq!(c.key, ZELDA);
        assert_eq!(c.rom_id.as_deref(), Some("r1"));
        assert_eq!(c.local_hash, sha(b"mine"));
        assert_eq!(c.remote_hash, sha(b"theirs"));
        assert_eq!(c.remote_version_id, "v2");
        // O nome é de quem escreveu a versão da nuvem.
        assert_eq!(c.remote_device_label.as_deref(), Some("Notebook"));
        assert_eq!(r.status().await.conflicts, 1);
        let conflict_events = |r: &Rig| {
            r.events()
                .iter()
                .filter(|e| matches!(e, SyncEvent::Conflict { .. }))
                .count()
        };
        assert_eq!(conflict_events(&r), 1);

        // A próxima passada não avisa de novo nem escreve nada.
        r.pass().await.unwrap();
        assert_eq!(conflict_events(&r), 1);
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"mine");

        let view = r.engine.conflicts().await.unwrap();
        assert_eq!(view.len(), 1);
        assert_eq!(view[0].title, "Zelda");
        assert_eq!(view[0].local.machine, "Sala");
        assert_eq!(view[0].remote.machine, "Notebook");
        assert_eq!(view[0].local.device_id.as_deref(), Some("dev1"));
        assert_eq!(view[0].remote.device_id.as_deref(), Some("dev2"));
        assert_eq!((view[0].local.size, view[0].remote.size), (4, 6));
        assert_eq!(
            view[0].remote.saved_at,
            parse_time("2026-11-13T09:59:00Z").unwrap().0
        );
    }

    /// Review Focus 1: duas máquinas configuradas separadamente; cada arquivo
    /// com bytes diferentes nos dois lados é um conflito, nada é
    /// sobrescrito, e a página conta todos.
    #[tokio::test]
    async fn first_sync_with_diverging_files_is_all_conflicts_and_the_page_counts_them() {
        let _serial = SERIAL.lock().await;
        let r = rig("rf1").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Mario.gba", "gba").await;
        r.write("saves/Zelda.srm", b"zelda-here");
        r.write("saves/Mario.srm", b"mario-here");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![
                row("s1", "Zelda.srm", "gba", None, b"zelda-there", "v1", None),
                row("s2", "Mario.srm", "gba", None, b"mario-there", "v1", None),
            ],
        )
        .await;
        mount_content(&r.server, "s1", "v1", b"zelda-there").await;
        mount_content(&r.server, "s2", "v1", b"mario-there").await;

        let report = r.pass().await.unwrap();
        assert_eq!(report.conflicts, 2);
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"zelda-here");
        assert_eq!(r.read("saves/Mario.srm").unwrap(), b"mario-here");
        assert!(r.reqs("POST", SAVES).await.is_empty());
        assert_eq!(r.content_gets().await, 0);
        assert!(r.repo.base_all("file").await.unwrap().is_empty());
        let st = r
            .sync
            .status_with(&r.alva, HardwarePayload::default())
            .await
            .unwrap();
        assert_eq!(st.conflicts, 2);
    }

    #[tokio::test]
    async fn resolve_remote_backs_up_then_downloads() {
        let _serial = SERIAL.lock().await;
        let r = conflict_rig("resolve-remote").await;
        r.pass().await.unwrap();

        r.engine
            .resolve(&r.alva, ZELDA, Keep::Remote)
            .await
            .unwrap();
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"theirs");
        assert_eq!(
            r.backups(),
            vec![("saves/Zelda.srm".to_owned(), b"mine".to_vec())]
        );
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!(
            (base.local_hash, base.remote_hash, base.remote_version_id),
            (sha(b"theirs"), sha(b"theirs"), "v2".into())
        );
        assert!(r.repo.conflicts_all().await.unwrap().is_empty());
        assert_eq!(r.status().await.conflicts, 0);
        assert!(r.reqs("POST", SAVES).await.is_empty());
    }

    #[tokio::test]
    async fn resolve_local_uploads_into_the_row_and_clears_the_conflict() {
        let _serial = SERIAL.lock().await;
        let r = conflict_rig("resolve-local").await;
        mount_upload(&r.server, "s1", "v3").await;
        r.pass().await.unwrap();

        r.engine.resolve(&r.alva, ZELDA, Keep::Local).await.unwrap();
        let posts = r.reqs("POST", SAVES).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(field(&posts[0].body, "file").as_deref(), Some("mine"));
        assert_eq!(field(&posts[0].body, "platform").as_deref(), Some("gba"));
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!(
            (base.local_hash, base.remote_version_id),
            (sha(b"mine"), "v3".into())
        );
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"mine");
        assert!(r.backups().is_empty());
        assert!(r.repo.conflicts_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_remote_against_a_tombstone_accepts_the_deletion() {
        let _serial = SERIAL.lock().await;
        let r = rig("resolve-tomb").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"a", b"a", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"a2");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![tombstone(row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"a",
                "v1",
                None,
            ))],
        )
        .await;
        mount_content(&r.server, "s1", "v1", b"a").await;
        assert_eq!(r.pass().await.unwrap().conflicts, 1);
        // I-4: o diálogo sabe que o outro lado é uma remoção no site.
        let view = r.engine.conflicts().await.unwrap();
        assert!(view[0].remote.deleted && !view[0].local.deleted);
        assert!(r.repo.conflicts_all().await.unwrap()[0].remote_deleted);

        r.engine
            .resolve(&r.alva, ZELDA, Keep::Remote)
            .await
            .unwrap();
        assert!(r.read("saves/Zelda.srm").is_none());
        assert_eq!(
            r.backups(),
            vec![("saves/Zelda.srm".to_owned(), b"a2".to_vec())]
        );
        // A base fica como marca de "apagado no site", com o lugar do arquivo.
        let base = r.base_of(ZELDA).await.unwrap();
        assert!(base.wiped_at.is_some());
        assert!(PathBuf::from(base.backup_path.unwrap()).ends_with("saves/Zelda.srm"));
        assert_eq!(
            r.engine.save_status("r1").await.unwrap().state,
            SaveLineState::Removed
        );
        assert!(r.repo.conflicts_all().await.unwrap().is_empty());
        assert_eq!(r.content_gets().await, 0, "nada desce de uma linha apagada");
    }

    /// I-4: "Manter esta (sobe de novo)" contra uma remoção no site: o save
    /// daqui sobe com os campos da linha apagada (o servidor abre uma linha
    /// viva nova e a apagada fica), nada vai para o resguardo.
    #[tokio::test]
    async fn keeping_this_save_against_a_site_deletion_uploads_it_again() {
        let _serial = SERIAL.lock().await;
        let r = rig("keep-against-tomb").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"a", b"a", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"a2");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        // A última versão foi escrita por esta mesma máquina e apagada.
        let mut gone = tombstone(row(
            "s1",
            "Zelda.srm",
            "gba",
            None,
            b"a",
            "v1",
            Some("dev1"),
        ));
        gone["current"] = Value::Null;
        mount_files(&r.server, vec![gone]).await;
        mount_upload(&r.server, "s2", "v1").await;
        assert_eq!(r.pass().await.unwrap().conflicts, 1);
        let view = r.engine.conflicts().await.unwrap();
        assert!(view[0].remote.deleted);

        r.engine.resolve(&r.alva, ZELDA, Keep::Local).await.unwrap();
        let posts = r.reqs("POST", SAVES).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(field(&posts[0].body, "file").as_deref(), Some("a2"));
        assert_eq!(field(&posts[0].body, "platform").as_deref(), Some("gba"));
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!((base.remote_save_id.as_str(), base.wiped_at), ("s2", None));
        assert!(r.backups().is_empty());
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"a2");
    }

    /// Review Focus 5: o 401 no meio da passada, confirmado pela lista do
    /// site. A passada para, o sync desliga, o token sai do chaveiro, a
    /// página mostra a faixa, e nada foi escrito depois do 401.
    #[tokio::test]
    async fn revoked_mid_pass_stops_and_forgets() {
        let _serial = SERIAL.lock().await;
        let r = rig("revoked").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Mario.gba", "gba").await;
        r.rom("r3", "Chrono.sfc", "snes").await;
        r.write("saves/Zelda.srm", b"zelda");
        r.write("saves/Mario.srm", b"mario");
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(seq(vec![
                devices_body(vec![device("dev1", "Sala")]),
                // O site removeu a máquina: ela some da lista.
                devices_body(vec![device("dev2", "Notebook")]),
            ]))
            .mount(&r.server)
            .await;
        mount_files(
            &r.server,
            vec![row(
                "s9",
                "Chrono.srm",
                "snes",
                None,
                b"chrono",
                "v1",
                Some("dev2"),
            )],
        )
        .await;
        mount_content(&r.server, "s9", "v1", b"chrono").await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(unauthorized())
            .mount(&r.server)
            .await;

        assert_eq!(r.pass().await, Err(SyncError::Revoked));
        let to_saves: Vec<String> = r
            .server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|q| q.url.path().starts_with(SAVES))
            .map(|q| format!("{} {}", q.method, q.url.path()))
            .collect();
        assert_eq!(to_saves, vec!["GET /api/v1/saves", "POST /api/v1/saves"]);
        assert_eq!(r.reqs("GET", DEVICES).await.len(), 2);
        assert!(
            !r.secrets
                .secrets
                .lock()
                .unwrap()
                .contains_key("alvanista-saves"),
            "token no chaveiro"
        );
        assert!(!r.sync.devices.enabled().await.unwrap());
        assert_eq!(r.sync.devices.site_state().await.unwrap(), (false, true));
        assert!(r.repo.base_all("file").await.unwrap().is_empty());
        assert!(r.read("saves/Chrono.srm").is_none());
        assert!(r.backups().is_empty());
        assert!(r.events().contains(&SyncEvent::Error {
            code: "revoked".into()
        }));
        assert_eq!(r.sync.runtime().last_error.as_deref(), Some("revoked"));

        // A página: a faixa de revogada com "Ligar de novo".
        let st = r
            .sync
            .status_with(&r.alva, HardwarePayload::default())
            .await
            .unwrap();
        assert!(st.revoked && !st.enabled, "{st:?}");

        // Desligado: a passada seguinte nem vai à rede.
        let before = r.server.received_requests().await.unwrap().len();
        assert_eq!(r.pass().await, Err(SyncError::NotEnabled));
        assert_eq!(r.server.received_requests().await.unwrap().len(), before);
    }

    #[tokio::test]
    async fn ligar_de_novo_after_a_revoked_pass_keeps_the_name() {
        let _serial = SERIAL.lock().await;
        let r = rig("relink").await;
        r.sync.devices.set_label("Quarto do Ed").await.unwrap();
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"zelda");
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(seq(vec![
                devices_body(vec![device("dev1", "Quarto do Ed")]),
                devices_body(vec![]),
                devices_body(vec![]),
                devices_body(vec![device("dev9", "Quarto do Ed")]),
            ]))
            .mount(&r.server)
            .await;
        mount_files(&r.server, vec![]).await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(unauthorized())
            .mount(&r.server)
            .await;
        assert_eq!(r.pass().await, Err(SyncError::Revoked));

        Mock::given(method("POST"))
            .and(path(DEVICES))
            .and(body_partial_json(json!({"label": "Quarto do Ed"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": "dev9", "label": "Quarto do Ed", "client_kind": "reemu",
                "token": "asv_new", "token_prefix": "asv_ne",
                "created_at": "2026-11-13T12:00:00Z"
            })))
            .expect(1)
            .mount(&r.server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{SAVES}/quota")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"used_bytes": 1, "limit_bytes": 9})),
            )
            .mount(&r.server)
            .await;
        let st = r
            .sync
            .set_enabled_with(&r.alva, true, HardwarePayload::default(), never_reconnect)
            .await
            .unwrap();
        assert!(st.enabled && !st.revoked, "{st:?}");
        assert_eq!(st.device_id.as_deref(), Some("dev9"));
        assert_eq!(st.label, "Quarto do Ed");
        assert_eq!(r.sync.devices.token().unwrap().as_deref(), Some("asv_new"));
        // Sem token não há o que revogar: nenhum DELETE.
        assert!(r.reqs("DELETE", SAVES).await.is_empty());
    }

    #[tokio::test]
    async fn transient_401_with_live_row_keeps_the_token() {
        let _serial = SERIAL.lock().await;
        let r = rig("transient401").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"zelda");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(unauthorized())
            .mount(&r.server)
            .await;

        assert_eq!(r.pass().await, Err(SyncError::Offline));
        assert_eq!(
            r.sync.devices.token().unwrap().as_deref(),
            Some("asv_secret")
        );
        assert!(r.sync.devices.enabled().await.unwrap());
        assert_eq!(r.sync.devices.site_state().await.unwrap(), (false, false));
        assert!(r.base_of(ZELDA).await.is_none());
        assert_eq!(r.reqs("GET", DEVICES).await.len(), 2, "relê a lista");
        assert_eq!(r.sync.runtime().last_error.as_deref(), Some("offline"));
        assert!(!r
            .events()
            .iter()
            .any(|e| matches!(e, SyncEvent::Error { .. })));
    }

    #[tokio::test]
    async fn a_401_on_the_listing_with_the_row_revoked_also_turns_sync_off() {
        let _serial = SERIAL.lock().await;
        let r = rig("listing401").await;
        let mut gone = device("dev1", "Sala");
        gone["revoked_at"] = json!("2026-11-13T12:00:00Z");
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(seq(vec![
                devices_body(vec![device("dev1", "Sala")]),
                devices_body(vec![gone]),
            ]))
            .mount(&r.server)
            .await;
        files_mock()
            .respond_with(unauthorized())
            .mount(&r.server)
            .await;
        assert_eq!(r.pass().await, Err(SyncError::Revoked));
        assert_eq!(r.sync.devices.token().unwrap(), None);
        assert!(!r.sync.devices.enabled().await.unwrap());
    }

    #[tokio::test]
    async fn saves_off_makes_no_save_request_and_writes_nothing() {
        let _serial = SERIAL.lock().await;
        let r = rig("saves-off").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Chrono.sfc", "snes").await;
        r.write("saves/Zelda.srm", b"local");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row("s2", "Chrono.srm", "snes", None, b"cloud", "v1", None)],
        )
        .await;
        mount_content(&r.server, "s2", "v1", b"cloud").await;
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v0"))
            .await;
        r.sync.devices.set_saves_enabled(false).await.unwrap();
        assert!(!r.sync.devices.saves_enabled().await.unwrap());

        let report = r.pass().await.unwrap();
        assert_eq!((report.uploaded, report.downloaded), (0, 0));
        assert_eq!(
            r.engine.prelaunch(&r.alva, "r2").await,
            PrelaunchOutcome::Skipped("saves_off".into())
        );
        r.engine
            .pass(&r.alva, PassScope::Rom("r1".into()))
            .await
            .unwrap();

        assert!(
            r.server.received_requests().await.unwrap().is_empty(),
            "nenhum pedido, nem a lista de aparelhos"
        );
        assert_eq!(r.read("saves/Zelda.srm").as_deref(), Some(&b"local"[..]));
        assert!(r.read("saves/Chrono.srm").is_none());
        assert_eq!(r.base_of(ZELDA).await.unwrap().local_hash, sha(b"old"));
        assert!(r.events().is_empty());

        // Ligar de novo (ausente vale ligado) volta a sincronizar.
        r.sync.devices.set_saves_enabled(true).await.unwrap();
        assert!(r.sync.devices.saves_enabled().await.unwrap());
        r.pass().await.unwrap();
        assert!(!r.server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn paused_row_makes_no_traffic() {
        let _serial = SERIAL.lock().await;
        let r = rig("paused").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"zelda");
        let mut paused = device("dev1", "Sala");
        paused["paused_at"] = json!("2026-11-13T12:00:00Z");
        mount_devices(&r.server, vec![paused]).await;

        assert_eq!(r.pass().await, Err(SyncError::Paused));
        let all = r.server.received_requests().await.unwrap();
        assert_eq!(all.len(), 1, "só a leitura da lista");
        assert_eq!(all[0].url.path(), DEVICES);
        assert_eq!(r.sync.devices.site_state().await.unwrap(), (true, false));
        assert_eq!(r.sync.runtime().last_error, None);
        assert!(r.base_of(ZELDA).await.is_none());

        // Pausada só aqui: nem a lista.
        r.sync.devices.set_paused(true).await.unwrap();
        assert_eq!(r.pass().await, Err(SyncError::Paused));
        assert_eq!(r.server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn receive_platforms_filters_downloads() {
        let _serial = SERIAL.lock().await;
        let r = rig("receive").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Chrono.sfc", "snes").await;
        r.rom("r3", "Mario.gba", "gba").await;
        let mut me = device("dev1", "Sala");
        me["receive_platforms"] = json!(["gba"]);
        mount_devices(&r.server, vec![me]).await;
        mount_files(
            &r.server,
            vec![
                row("s1", "Zelda.srm", "gba", None, b"zelda", "v1", None),
                row("s2", "Chrono.srm", "snes", None, b"chrono", "v1", None),
                // RetroArch plano arquiva em `other`; o jogo daqui é de GBA.
                row("s3", "Mario.srm", "other", None, b"mario", "v1", None),
            ],
        )
        .await;
        mount_content(&r.server, "s1", "v1", b"zelda").await;
        mount_content(&r.server, "s2", "v1", b"chrono").await;
        mount_content(&r.server, "s3", "v1", b"mario").await;

        let report = r.pass().await.unwrap();
        assert_eq!((report.downloaded, report.deferred), (2, 1));
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"zelda");
        assert_eq!(r.read("saves/Mario.srm").unwrap(), b"mario");
        assert!(r.read("saves/Chrono.srm").is_none());
        let mut gets: Vec<String> = r
            .reqs("GET", SAVES)
            .await
            .iter()
            .filter(|q| q.url.path().ends_with("/content"))
            .map(|q| q.url.path().to_owned())
            .collect();
        gets.sort();
        assert_eq!(
            gets,
            vec![
                "/api/v1/saves/s1/versions/v1/content",
                "/api/v1/saves/s3/versions/v1/content"
            ]
        );
    }

    /// Save da conta apagado no site, e a última versão foi escrita POR ESTA
    /// máquina: o dono da linha (ninguém) decide, não quem escreveu. O
    /// arquivo vai para o backup e não sobe de novo.
    #[tokio::test]
    async fn tombstone_of_account_save_backs_up_local() {
        let _serial = SERIAL.lock().await;
        let r = rig("tomb").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"old");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![tombstone(row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"old",
                "v1",
                Some("dev1"),
            ))],
        )
        .await;

        let report = r.pass().await.unwrap();
        assert_eq!(report.backed_up, 1);
        assert!(r.read("saves/Zelda.srm").is_none());
        assert_eq!(
            r.backups(),
            vec![("saves/Zelda.srm".to_owned(), b"old".to_vec())]
        );
        // S13: a base fica como marca, e a página do jogo diz que o save foi
        // apagado no site e abre a pasta onde o arquivo foi parar.
        let base = r.base_of(ZELDA).await.unwrap();
        assert!(base.wiped_at.is_some());
        let line = r.engine.save_status("r1").await.unwrap();
        assert_eq!(
            (line.state, line.key.as_deref(), line.versions),
            (SaveLineState::Removed, Some(ZELDA), 0)
        );
        let folder = r.engine.removed_folder(ZELDA).await.unwrap();
        assert!(folder.starts_with(r.path("sync-backups")));
        assert_eq!(
            std::fs::read(folder.join("Zelda.srm")).unwrap(),
            b"old",
            "a pasta é a do arquivo"
        );
        // O aviso da passada conta o que foi para o resguardo.
        assert!(r.events().contains(&SyncEvent::Pass {
            uploaded: 0,
            downloaded: 0,
            backed_up: 1
        }));

        let again = r.pass().await.unwrap();
        assert_eq!((again.uploaded, again.downloaded), (0, 0));
        assert!(
            r.reqs("POST", SAVES).await.is_empty(),
            "a remoção não se desfaz"
        );
        assert!(r.read("saves/Zelda.srm").is_none());
        assert_eq!(
            r.engine.save_status("r1").await.unwrap().state,
            SaveLineState::Removed,
            "a marca fica"
        );

        // Jogou de novo: o save novo é novo, sobe como linha viva e a marca sai.
        r.write("saves/Zelda.srm", b"new run");
        assert_eq!(
            r.engine.save_status("r1").await.unwrap().state,
            SaveLineState::Pending
        );
        mount_upload(&r.server, "s9", "v1").await;
        assert_eq!(r.pass().await.unwrap().uploaded, 1);
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!((base.wiped_at, base.backup_path), (None, None));
        assert_eq!(r.engine.removed_folder(ZELDA).await, None);
    }

    /// D127: a linha própria apagada (states e configurações; arquivo nunca
    /// tem dono, então isto passa pelo executor direto) marca a base e deixa
    /// o arquivo. Sem arquivo local, uma linha apagada só tira a base.
    #[tokio::test]
    async fn tombstone_of_own_row_keeps_local_and_clears_base() {
        let _serial = SERIAL.lock().await;
        let r = rig("ownrow").await;
        let key = "state:snes/snes9x/Chrono__quick.state";
        r.write("states/Chrono__quick.state", b"state");
        r.repo
            .base_put(&BaseRow {
                key: key.into(),
                kind: "state".into(),
                remote_save_id: "s1".into(),
                remote_version_id: "v1".into(),
                remote_hash: sha(b"state"),
                local_hash: sha(b"state"),
                local_path: r
                    .path("states/Chrono__quick.state")
                    .to_string_lossy()
                    .into(),
                rom_crc32: None,
                synced_at: 1,
                wiped_at: None,
                backup_path: None,
            })
            .await
            .unwrap();
        r.engine
            .base_only(key, Action::MarkBaseWipedKeepLocal)
            .await
            .unwrap();
        assert!(r.base_of(key).await.unwrap().wiped_at.is_some());
        assert_eq!(r.read("states/Chrono__quick.state").unwrap(), b"state");
        assert!(r.server.received_requests().await.unwrap().is_empty());

        // DropBase: arquivo local sumido e linha apagada.
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![tombstone(row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"old",
                "v1",
                None,
            ))],
        )
        .await;
        r.pass().await.unwrap();
        assert!(r.base_of(ZELDA).await.is_none());
        assert!(r.read("saves/Zelda.srm").is_none());
        assert!(r.backups().is_empty());
        assert_eq!(r.content_gets().await, 0);
    }

    #[tokio::test]
    async fn adopt_writes_the_base_and_nothing_on_disk() {
        let _serial = SERIAL.lock().await;
        let r = rig("adopt").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"same");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"same",
                "v4",
                Some("dev2"),
            )],
        )
        .await;
        mount_content(&r.server, "s1", "v4", b"same").await;

        let report = r.pass().await.unwrap();
        assert_eq!(report, PassReport::default());
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!(base.local_hash, sha(b"same"));
        assert_eq!(base.remote_hash, sha(b"same"));
        assert_eq!(
            (
                base.remote_save_id.as_str(),
                base.remote_version_id.as_str()
            ),
            ("s1", "v4")
        );
        assert!(r.reqs("POST", SAVES).await.is_empty());
        assert_eq!(r.content_gets().await, 0);
        assert!(r.backups().is_empty());
    }

    #[tokio::test]
    async fn restore_uploads_the_old_version_as_current() {
        let _serial = SERIAL.lock().await;
        let r = rig("restore").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"two", b"two", ("s1", "v2"))
            .await;
        r.write("saves/Zelda.srm", b"two");
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device("dev2", "Notebook")],
        )
        .await;
        mount_files(
            &r.server,
            vec![row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"two",
                "v2",
                Some("dev1"),
            )],
        )
        .await;
        Mock::given(method("GET"))
            .and(path(format!("{SAVES}/s1/versions")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {"version_id": "v2", "content_hash": sha(b"two"), "size_bytes": 3,
                 "server_created_at": "2026-11-13T10:00:00Z", "client_mtime": null,
                 "last_seen_at": "2026-11-13T10:00:00Z", "source_client": "reemu",
                 "origin_media": "upload", "device_token_id": "dev1"},
                {"version_id": "v1", "content_hash": sha(b"one"), "size_bytes": 3,
                 "server_created_at": "2026-11-12T10:00:00Z", "client_mtime": null,
                 "last_seen_at": "2026-11-12T10:00:00Z", "source_client": "reemu",
                 "origin_media": "upload", "device_token_id": "dev2"}
            ])))
            .mount(&r.server)
            .await;
        mount_content(&r.server, "s1", "v1", b"one").await;
        mount_upload(&r.server, "s1", "v3").await;

        let versions = r.engine.versions(&r.alva, ZELDA).await.unwrap();
        let seen: Vec<(&str, &str, bool)> = versions
            .iter()
            .map(|v| (v.version_id.as_str(), v.machine.as_str(), v.is_current))
            .collect();
        assert_eq!(seen, vec![("v2", "Sala", true), ("v1", "Notebook", false)]);
        assert_eq!(versions[0].device_id.as_deref(), Some("dev1"));
        assert_eq!(versions[1].device_id.as_deref(), Some("dev2"));
        assert_eq!(
            versions[1].created_at,
            parse_time("2026-11-12T10:00:00Z").unwrap().0
        );

        r.engine.restore(&r.alva, ZELDA, "v1").await.unwrap();
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"one");
        assert_eq!(
            r.backups(),
            vec![("saves/Zelda.srm".to_owned(), b"two".to_vec())]
        );
        let posts = r.reqs("POST", SAVES).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(field(&posts[0].body, "file").as_deref(), Some("one"));
        assert_eq!(field(&posts[0].body, "platform").as_deref(), Some("gba"));
        let base = r.base_of(ZELDA).await.unwrap();
        assert_eq!(
            (base.local_hash, base.remote_hash, base.remote_version_id),
            (sha(b"one"), sha(b"one"), "v3".into())
        );
    }

    #[tokio::test]
    async fn prelaunch_times_out_to_skipped() {
        let _serial = SERIAL.lock().await;
        let mut r = rig("timeout").await;
        r.engine.prelaunch_budget = Duration::from_millis(200);
        r.rom("r1", "Zelda.gba", "gba").await;
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(
                devices_body(vec![device("dev1", "Sala")]).set_delay(Duration::from_secs(3)),
            )
            .mount(&r.server)
            .await;
        mount_files(
            &r.server,
            vec![row("s1", "Zelda.srm", "gba", None, b"cloud", "v1", None)],
        )
        .await;
        mount_content(&r.server, "s1", "v1", b"cloud").await;
        r.set_playing(Playing::Loading);

        assert_eq!(
            r.engine.prelaunch(&r.alva, "r1").await,
            PrelaunchOutcome::Skipped("timeout".into())
        );
        assert!(r.read("saves/Zelda.srm").is_none());
        assert!(r.repo.base_all("file").await.unwrap().is_empty());
        // M4: a tela fica sabendo, para o toast de "a nuvem não pôde ser
        // conferida".
        assert!(r.events().contains(&SyncEvent::Prelaunch {
            outcome: "skipped".into(),
            reason: Some("timeout".into())
        }));
    }

    /// Os 12 nomes de save de um stem, como a listagem de uma ROM os pede.
    fn slot_names(stem: &str) -> Vec<(String, String)> {
        let mut names: Vec<(String, String)> = [
            "srm", "sav", "rtc", "fs", "eep", "fla", "mcr", "mcd", "bsv", "mpk", "ldci", "sram",
        ]
        .iter()
        .map(|ext| (format!("{stem}.{ext}"), "-".to_owned()))
        .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn prelaunch_downloads_only_the_launched_game_with_a_fresh_read() {
        let _serial = SERIAL.lock().await;
        let r = rig("prelaunch").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Mario.gba", "gba").await;
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![
                row("s1", "Zelda.srm", "gba", None, b"zelda", "v1", None),
                row("s2", "Mario.srm", "gba", None, b"mario", "v1", None),
            ],
        )
        .await;
        mount_content(&r.server, "s1", "v1", b"zelda").await;
        mount_content(&r.server, "s2", "v1", b"mario").await;
        r.set_playing(Playing::Loading);

        assert_eq!(
            r.engine.prelaunch(&r.alva, "r1").await,
            PrelaunchOutcome::Downloaded
        );
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"zelda");
        assert!(r.read("saves/Mario.srm").is_none(), "só o jogo que abre");
        let mut asked = listing_filters(&r).await;
        asked.sort();
        assert_eq!(asked, slot_names("Zelda"), "só os nomes do jogo que abre");
        assert_eq!(r.reqs("GET", DEVICES).await.len(), 1);

        // O próximo jogo lê a linha da máquina e a nuvem de novo.
        assert_eq!(
            r.engine.prelaunch(&r.alva, "r2").await,
            PrelaunchOutcome::Downloaded
        );
        assert_eq!(r.read("saves/Mario.srm").unwrap(), b"mario");
        assert_eq!(r.reqs("GET", DEVICES).await.len(), 2);
        assert_eq!(
            r.engine.prelaunch(&r.alva, "r1").await,
            PrelaunchOutcome::UpToDate
        );
    }

    /// I-3: numa biblioteca grande, a checagem de antes de abrir consulta no
    /// disco só as 12 vagas do jogo e pede à nuvem só os nomes dele: o resto
    /// da biblioteca não entra no prazo de 5 s.
    #[tokio::test]
    async fn prelaunch_in_a_big_library_touches_only_the_launched_rom() {
        let _serial = SERIAL.lock().await;
        let r = rig("big-library").await;
        for i in 0..300 {
            r.rom(&format!("r{i:03}"), &format!("Game{i:03}.gba"), "gba")
                .await;
            r.write(&format!("saves/Game{i:03}.srm"), b"local only");
        }
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![
                row("s7", "Game007.srm", "gba", None, b"cloud", "v1", None),
                row("s8", "Game008.srm", "gba", None, b"other", "v1", None),
            ],
        )
        .await;
        mount_content(&r.server, "s7", "v1", b"cloud").await;
        r.set_playing(Playing::Loading);

        let saves = r.path("saves");
        let before = local::probe::count(&saves);
        assert_eq!(
            r.engine.prelaunch(&r.alva, "r007").await,
            PrelaunchOutcome::Conflict
        );
        assert_eq!(
            local::probe::count(&saves) - before,
            12,
            "o disco foi consultado fora das vagas do jogo"
        );
        let mut asked = listing_filters(&r).await;
        asked.sort();
        assert_eq!(asked, slot_names("Game007"));
        // Só o jogo que abre virou conflito; nada do resto foi tocado.
        let conflicts = r.repo.conflicts_all().await.unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, "file:gba/Game007.srm");
        assert!(r.repo.base_all("file").await.unwrap().is_empty());
        assert_eq!(r.content_gets().await, 0);

        // A passada completa lê a pasta de saves uma vez, não 12 por ROM (com
        // um jogo abrindo, nada sobe nem desce).
        r.set_playing(Playing::Running(None));
        let before = local::probe::count(&saves);
        r.pass().await.unwrap();
        assert_eq!(local::probe::count(&saves) - before, 1);
    }

    /// M5: outra máquina enviou depois da última passada daqui; a checagem de
    /// antes de abrir lê a nuvem agora e traz o save novo (a leitura de até
    /// 60 s atrás abria o jogo com o save velho).
    #[tokio::test]
    async fn prelaunch_sees_what_another_machine_uploaded_after_the_last_pass() {
        let _serial = SERIAL.lock().await;
        let r = rig("fresh-listing").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"old");
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device("dev2", "Notebook")],
        )
        .await;
        let listing = mount_listing(
            &r.server,
            vec![row(
                "s1",
                "Zelda.srm",
                "gba",
                None,
                b"old",
                "v1",
                Some("dev1"),
            )],
        )
        .await;
        mount_content(&r.server, "s1", "v2", b"newer").await;
        assert_eq!(r.pass().await.unwrap(), PassReport::default());

        // 30 s depois o Notebook envia.
        listing.set(vec![row(
            "s1",
            "Zelda.srm",
            "gba",
            None,
            b"newer",
            "v2",
            Some("dev2"),
        )]);
        r.set_playing(Playing::Loading);
        assert_eq!(
            r.engine.prelaunch(&r.alva, "r1").await,
            PrelaunchOutcome::Downloaded
        );
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"newer");
        assert_eq!(
            r.backups(),
            vec![("saves/Zelda.srm".to_owned(), b"old".to_vec())]
        );
    }

    /// M5: a pausa feita no site depois da última passada vale antes de abrir:
    /// só a lista de aparelhos é lida, nada da nuvem.
    #[tokio::test]
    async fn prelaunch_honors_a_pause_made_after_the_last_pass() {
        let _serial = SERIAL.lock().await;
        let r = rig("fresh-pause").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        let mut paused = device("dev1", "Sala");
        paused["paused_at"] = json!("2026-11-13T12:00:00Z");
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(seq(vec![
                devices_body(vec![device("dev1", "Sala")]),
                devices_body(vec![paused]),
            ]))
            .mount(&r.server)
            .await;
        mount_files(
            &r.server,
            vec![row("s1", "Zelda.srm", "gba", None, b"cloud", "v1", None)],
        )
        .await;
        mount_content(&r.server, "s1", "v1", b"cloud").await;
        // Uma passada com um jogo aberto: lê a linha e a nuvem, não escreve.
        r.set_playing(Playing::Running(None));
        assert_eq!(r.pass().await.unwrap().deferred, 1);
        let listings = r.listings().await;

        r.set_playing(Playing::Loading);
        assert_eq!(
            r.engine.prelaunch(&r.alva, "r1").await,
            PrelaunchOutcome::Skipped("paused".into())
        );
        assert_eq!(r.reqs("GET", DEVICES).await.len(), 2);
        assert_eq!(r.listings().await, listings, "pausada não lista a nuvem");
        assert!(r.read("saves/Zelda.srm").is_none());
        assert_eq!(r.sync.devices.site_state().await.unwrap(), (true, false));
    }

    /// Depois de um envio a nuvem já é outra: a checagem de antes de abrir lê
    /// de novo e nunca traz a versão anterior por cima do save que subiu.
    #[tokio::test]
    async fn prelaunch_after_an_upload_lists_again_and_keeps_the_new_save() {
        let _serial = SERIAL.lock().await;
        let r = rig("relist").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"newer");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        files_mock()
            .respond_with(seq(vec![
                files_body(vec![row(
                    "s1",
                    "Zelda.srm",
                    "gba",
                    None,
                    b"old",
                    "v1",
                    None,
                )]),
                files_body(vec![row(
                    "s1",
                    "Zelda.srm",
                    "gba",
                    None,
                    b"newer",
                    "v2",
                    Some("dev1"),
                )]),
            ]))
            .mount(&r.server)
            .await;
        mount_content(&r.server, "s1", "v1", b"old").await;
        mount_upload(&r.server, "s1", "v2").await;

        assert_eq!(r.pass().await.unwrap().uploaded, 1);
        r.set_playing(Playing::Loading);
        assert_eq!(
            r.engine.prelaunch(&r.alva, "r1").await,
            PrelaunchOutcome::UpToDate
        );
        // A completa e as 12 do jogo.
        assert_eq!(r.listings().await, 1 + 12);
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"newer");
        assert_eq!(r.content_gets().await, 0);
    }

    #[tokio::test]
    async fn zero_byte_srm_is_not_uploaded() {
        let _serial = SERIAL.lock().await;
        let r = rig("zero").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        mount_upload(&r.server, "s1", "v1").await;

        let report = r.pass().await.unwrap();
        assert_eq!(report.uploaded, 0);
        assert!(r.reqs("POST", SAVES).await.is_empty());
        assert!(r.base_of(ZELDA).await.is_none());
    }

    #[tokio::test]
    async fn no_srm_write_while_the_game_is_loaded() {
        let _serial = SERIAL.lock().await;
        let r = rig("playing").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Mario.gba", "gba").await;
        // O save do jogo aberto mudou na nuvem; o relógio dele mudou aqui.
        r.base(ZELDA, "saves/Zelda.srm", b"old", b"old", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"old");
        r.write("saves/Zelda.rtc", b"clock");
        // O save de outro jogo e um cartão de memória, só daqui.
        r.write("saves/Mario.srm", b"mario");
        r.write("system/pcsx2/memcards/Mcd001.ps2", b"card");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        let zelda = row("s1", "Zelda.srm", "gba", None, b"new", "v2", Some("dev2"));
        // Depois da primeira passada a nuvem já tem o save do Mario.
        let mario = row("s2", "Mario.srm", "gba", None, b"mario", "v1", Some("dev1"));
        files_mock()
            .respond_with(seq(vec![
                files_body(vec![zelda.clone()]),
                files_body(vec![zelda, mario]),
            ]))
            .mount(&r.server)
            .await;
        mount_content(&r.server, "s1", "v2", b"new").await;
        mount_upload(&r.server, "s2", "v1").await;

        r.set_playing(Playing::Running(Some("r1".into())));
        let report = r.pass().await.unwrap();
        assert_eq!(
            (report.downloaded, report.uploaded, report.deferred),
            (0, 1, 3)
        );
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"old");
        assert_eq!(r.content_gets().await, 0);
        assert!(r.backups().is_empty());
        let posts = r.reqs("POST", SAVES).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(
            field(&posts[0].body, "filename").as_deref(),
            Some("Mario.srm")
        );

        // Abrindo um jogo, uma passada comum também não escreve nem sobe.
        r.set_playing(Playing::Loading);
        let report = r.pass().await.unwrap();
        assert_eq!((report.downloaded, report.uploaded), (0, 0));
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"old");

        // Jogo fechado: desce o save, sobem o relógio e o cartão.
        r.set_playing(Playing::Idle);
        let report = r.pass().await.unwrap();
        assert_eq!((report.downloaded, report.uploaded), (1, 2));
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"new");
        assert_eq!(
            r.backups(),
            vec![("saves/Zelda.srm".to_owned(), b"old".to_vec())]
        );
    }

    #[tokio::test]
    async fn matches_a_retroarch_row_by_filename_and_uploads_into_it() {
        let _serial = SERIAL.lock().await;
        let r = rig("retroarch").await;
        r.rom("r1", "Chrono.sfc", "snes").await;
        let mut retroarch = row(
            "s9",
            "Chrono.srm",
            "snes",
            Some("Snes9x"),
            b"ra-1",
            "v1",
            Some("dev2"),
        );
        retroarch["core_version"] = json!("1.10");
        r.base(
            "file:snes/Chrono.srm",
            "saves/Chrono.srm",
            b"mine-1",
            b"ra-1",
            ("s9", "v1"),
        )
        .await;
        r.write("saves/Chrono.srm", b"mine-2");
        // Um RetroArch "plano" arquiva em `other`: a linha serve ao Zelda daqui.
        r.rom("r2", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"zelda");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![
                // A da plataforma do ReEmu ganha da de `other`.
                row("s8", "Chrono.srm", "other", None, b"stray", "v1", None),
                retroarch,
                row(
                    "s5",
                    "Zelda.srm",
                    "other",
                    None,
                    b"zelda",
                    "v1",
                    Some("dev3"),
                ),
            ],
        )
        .await;
        mount_upload(&r.server, "s9", "v2").await;

        let report = r.pass().await.unwrap();
        assert_eq!(report.uploaded, 1);
        let posts = r.reqs("POST", SAVES).await;
        assert_eq!(posts.len(), 1);
        let body = &posts[0].body;
        assert_eq!(field(body, "filename").as_deref(), Some("Chrono.srm"));
        assert_eq!(field(body, "platform").as_deref(), Some("snes"));
        assert_eq!(field(body, "core").as_deref(), Some("Snes9x"));
        // A versão do core também é chave da linha no servidor.
        assert_eq!(field(body, "core_version").as_deref(), Some("1.10"));
        let chrono = r.base_of("file:snes/Chrono.srm").await.unwrap();
        assert_eq!(
            (
                chrono.remote_save_id.as_str(),
                chrono.remote_version_id.as_str()
            ),
            ("s9", "v2")
        );
        // Mesmos bytes na linha `other`: adotada, sem tráfego de bytes.
        assert_eq!(r.base_of(ZELDA).await.unwrap().remote_save_id, "s5");
    }

    #[tokio::test]
    async fn a_new_card_goes_up_with_its_core() {
        let _serial = SERIAL.lock().await;
        let r = rig("card").await;
        r.write("system/pcsx2/memcards/Mcd001.ps2", b"card");
        r.write("system/dc/vmu_save_A1.bin", b"vmu");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        mount_upload(&r.server, "s3", "v1").await;

        assert_eq!(r.pass().await.unwrap().uploaded, 2);
        let posts = r.reqs("POST", SAVES).await;
        let sent: Vec<(Option<String>, Option<String>, Option<String>)> = posts
            .iter()
            .map(|p| {
                (
                    field(&p.body, "platform"),
                    field(&p.body, "core"),
                    field(&p.body, "filename"),
                )
            })
            .collect();
        assert!(sent.contains(&(
            Some("ps2".into()),
            Some("system/pcsx2/memcards".into()),
            Some("Mcd001.ps2".into())
        )));
        assert!(sent.contains(&(
            Some("dreamcast".into()),
            Some("system/dc".into()),
            Some("vmu_save_A1.bin".into())
        )));
        assert!(r
            .base_of("file:ps2/system/pcsx2/memcards/Mcd001.ps2")
            .await
            .is_some());
    }

    #[tokio::test]
    async fn a_card_only_in_the_cloud_comes_down_to_the_system_folder() {
        let _serial = SERIAL.lock().await;
        let r = rig("card-down").await;
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        let cards = Some("system/pcsx2/memcards");
        mount_files(
            &r.server,
            vec![
                row("s3", "Mcd001.ps2", "ps2", cards, b"card", "v1", None),
                // Nome com pasta nunca escolhe onde o arquivo cai.
                row("s4", "../evil.ps2", "ps2", cards, b"x", "v1", None),
            ],
        )
        .await;
        mount_content(&r.server, "s3", "v1", b"card").await;
        mount_content(&r.server, "s4", "v1", b"x").await;
        assert_eq!(r.pass().await.unwrap().downloaded, 1);
        assert_eq!(r.read("system/pcsx2/memcards/Mcd001.ps2").unwrap(), b"card");
        assert!(r.read("system/pcsx2/evil.ps2").is_none());
        assert_eq!(r.content_gets().await, 1);
    }

    /// Erro ao ler o disco acaba a passada: nada desce por cima de um save
    /// que uma lista curta faria parecer apagado.
    #[tokio::test]
    async fn a_failed_local_listing_aborts_the_pass() {
        let _serial = SERIAL.lock().await;
        let r = rig("abort").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.base(ZELDA, "saves/Zelda.srm", b"a", b"a", ("s1", "v1"))
            .await;
        r.write("saves/Zelda.srm", b"unsynced");
        // `memcards` é um arquivo: ler a pasta falha sem ser NotFound.
        r.write("system/pcsx2/memcards", b"not a dir");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row("s1", "Zelda.srm", "gba", None, b"cloud", "v2", None)],
        )
        .await;
        mount_content(&r.server, "s1", "v2", b"cloud").await;
        mount_upload(&r.server, "s1", "v3").await;

        assert!(matches!(r.pass().await, Err(SyncError::Failed(_))));
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"unsynced");
        assert_eq!(r.content_gets().await, 0);
        assert!(r.reqs("POST", SAVES).await.is_empty());
        assert_eq!(r.base_of(ZELDA).await.unwrap().local_hash, sha(b"a"));
        assert_eq!(r.sync.runtime().last_error.as_deref(), Some("failed"));
    }

    /// Os dois 402 que o Alvanista manda, com o corpo exato de
    /// `billing.TierInsufficient` (internal/billing/errors.go).
    fn gate_402() -> ResponseTemplate {
        ResponseTemplate::new(402).set_body_json(json!({"error": {
            "code": "tier_insufficient", "current_tier": "gamer",
            "required_quota": "save_sync_enabled", "required_value": true,
            "upgrade_to": "collector"}}))
    }

    /// A cota cheia: `saves.go` reescreve o 507 `quota_exceeded` do GameKave.
    fn quota_402() -> ResponseTemplate {
        ResponseTemplate::new(402).set_body_json(json!({"error": {
            "code": "tier_insufficient", "current_tier": "collector",
            "required_quota": "emu_saves_quota_mb", "required_value": 1024,
            "upgrade_to": "collector"}}))
    }

    async fn mirror_with_save_sync(r: &Rig) {
        mirror_as(r, "u1").await;
    }

    /// O espelho do `/me` de uma conta (o AlvaID em `id`), plano com sync.
    async fn mirror_as(r: &Rig, alva_id: &str) {
        r.store
            .mirror_put(&Me {
                id: alva_id.into(),
                username: "toy".into(),
                avatar_url: None,
                hero_url: None,
                bio: None,
                profile_url: "https://www.alvanista.com/u/toy".into(),
                card: None,
                entitlements: Some(Entitlements { save_sync: true }),
            })
            .await
            .unwrap();
    }

    /// I-1: a cota cheia chega como o 402 de cota do Alvanista. Os envios
    /// param, as descidas seguem, a tela recebe os números, e o espelho do
    /// plano não muda: quem é Collector não vê "assine o Collector".
    #[tokio::test]
    async fn the_real_quota_402_stops_uploads_shows_the_numbers_and_keeps_the_plan() {
        let _serial = SERIAL.lock().await;
        let r = rig("quota").await;
        mirror_with_save_sync(&r).await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Mario.gba", "gba").await;
        r.rom("r3", "Chrono.sfc", "snes").await;
        r.write("saves/Zelda.srm", b"zelda");
        r.write("saves/Mario.srm", b"mario");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row("s9", "Chrono.srm", "snes", None, b"chrono", "v1", None)],
        )
        .await;
        mount_content(&r.server, "s9", "v1", b"chrono").await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(quota_402())
            .mount(&r.server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{SAVES}/quota")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"used_bytes": 100, "limit_bytes": 100})),
            )
            .mount(&r.server)
            .await;

        let report = r.pass().await.unwrap();
        assert_eq!(
            (report.uploaded, report.downloaded, report.deferred),
            (0, 1, 1)
        );
        assert_eq!(
            r.reqs("POST", SAVES).await.len(),
            1,
            "parou no primeiro 402 de cota"
        );
        assert_eq!(report.worst(), Some(SyncError::Quota));
        assert_eq!(r.read("saves/Chrono.srm").unwrap(), b"chrono");
        assert!(r.events().contains(&SyncEvent::Quota {
            used_bytes: 100,
            limit_bytes: 100
        }));
        assert_eq!(r.sync.runtime().last_error.as_deref(), Some("quota"));
        assert_eq!(r.alva.mirrored_save_sync().await, Some(true));
        let st = r.status().await;
        assert_eq!(st.availability, crate::sync::Availability::Ready);
        assert_eq!(st.last_error.as_deref(), Some("quota"));
        assert_eq!(st.quota.map(|q| q.used_bytes), Some(100));
    }

    #[tokio::test]
    async fn the_gate_402_stops_uploads_keeps_downloads_and_shows_no_tier() {
        let _serial = SERIAL.lock().await;
        let r = rig("tier").await;
        mirror_with_save_sync(&r).await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r3", "Chrono.sfc", "snes").await;
        r.write("saves/Zelda.srm", b"zelda");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row("s9", "Chrono.srm", "snes", None, b"chrono", "v1", None)],
        )
        .await;
        mount_content(&r.server, "s9", "v1", b"chrono").await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(gate_402())
            .mount(&r.server)
            .await;

        let report = r.pass().await.unwrap();
        assert_eq!((report.uploaded, report.downloaded), (0, 1));
        assert_eq!(report.worst(), Some(SyncError::NoTier));
        assert_eq!(r.alva.mirrored_save_sync().await, Some(false));
        assert_eq!(r.read("saves/Chrono.srm").unwrap(), b"chrono");
        assert!(!r
            .events()
            .iter()
            .any(|e| matches!(e, SyncEvent::Quota { .. })));
    }

    /// M10 (NFR-4): o log (que vai para o diagnóstico) nunca leva o nome de um
    /// save, que é o nome da ROM: as falhas de um arquivo levam um rótulo.
    #[tokio::test]
    async fn failure_logs_never_name_the_save() {
        let _serial = SERIAL.lock().await;
        crate::sync::test_support::captured_logs();
        let r = rig("log-names").await;
        r.rom("r1", "Quuxomatic.gba", "gba").await;
        r.rom("r2", "Quuxomatic Two.gba", "gba").await;
        // Um "save" que não se lê (é uma pasta) e uma descida com bytes que
        // não batem com o hash.
        let unreadable = "file:gba/Quuxomatic.srm";
        r.base(unreadable, "saves/Quuxomatic.srm", b"a", b"a", ("s1", "v1"))
            .await;
        std::fs::create_dir_all(r.path("saves/Quuxomatic.srm")).unwrap();
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![row(
                "s2",
                "Quuxomatic Two.srm",
                "gba",
                None,
                b"good",
                "v1",
                None,
            )],
        )
        .await;
        mount_content(&r.server, "s2", "v1", b"garbled").await;

        let report = r.pass().await.unwrap();
        assert_eq!(report.errors.len(), 2, "{report:?}");
        assert!(r
            .engine
            .resolve(&r.alva, "file:gba/Quuxomatic Two.srm", Keep::Local)
            .await
            .is_err());

        let lines = crate::sync::test_support::captured_logs();
        let failures: Vec<&String> = lines
            .iter()
            .filter(|l| l.contains(&local::log_tag(unreadable)))
            .collect();
        assert!(!failures.is_empty(), "as falhas não chegaram ao log");
        for l in &lines {
            assert!(!l.contains("Quuxomatic"), "nome de save no log: {l}");
        }
    }

    /// M1: a base é da conta que a gravou. Outra conta nesta máquina, com um
    /// save de mesmo nome e outros bytes, não desce por cima do local: sem a
    /// base da anterior é conflito, como duas máquinas configuradas à parte.
    /// E uma remoção na conta nova não manda o arquivo daqui para o resguardo.
    #[tokio::test]
    async fn another_account_does_not_inherit_the_base_of_the_previous_one() {
        let _serial = SERIAL.lock().await;
        let r = rig("account").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Mario.gba", "gba").await;
        // A máquina sincronizou com a conta A.
        r.sync.devices.set_base_account("alva_A").await.unwrap();
        r.base(
            ZELDA,
            "saves/Zelda.srm",
            b"zelda-a",
            b"zelda-a",
            ("sA", "v1"),
        )
        .await;
        r.write("saves/Zelda.srm", b"zelda-a");
        let mario = "file:gba/Mario.srm";
        r.base(
            mario,
            "saves/Mario.srm",
            b"mario-a",
            b"mario-a",
            ("sM", "v1"),
        )
        .await;
        r.write("saves/Mario.srm", b"mario-a");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(
            &r.server,
            vec![
                row("sB", "Zelda.srm", "gba", None, b"zelda-b", "v1", None),
                tombstone(row("sN", "Mario.srm", "gba", None, b"mario-b", "v1", None)),
            ],
        )
        .await;
        mount_content(&r.server, "sB", "v1", b"zelda-b").await;
        mount_upload(&r.server, "sX", "v1").await;

        // Sem o perfil não se sabe de quem é a conta: nada anda.
        assert_eq!(r.pass().await, Err(SyncError::Offline));
        assert!(r.server.received_requests().await.unwrap().is_empty());

        // A pessoa conectou a conta B.
        mirror_as(&r, "alva_B").await;
        let report = r.pass().await.unwrap();
        assert_eq!(
            (report.conflicts, report.downloaded, report.backed_up),
            (1, 0, 0)
        );
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"zelda-a");
        assert_eq!(r.read("saves/Mario.srm").unwrap(), b"mario-a");
        assert!(r.backups().is_empty());
        assert_eq!(r.content_gets().await, 0);
        assert_eq!(
            r.sync.devices.base_account().await.unwrap().as_deref(),
            Some("alva_B")
        );
        // Sem base, a remoção de B é ausência: o Mario daqui sobe para B.
        assert_eq!(report.uploaded, 1);
        assert_eq!(r.base_of(mario).await.unwrap().remote_save_id, "sX");
        assert!(r.base_of(ZELDA).await.is_none());

        // A mesma conta de novo: a base fica.
        r.pass().await.unwrap();
        assert!(r.base_of(mario).await.is_some());
    }

    /// I-2 e M9: a nuvem recusa um save de N64 maior que o limite da
    /// plataforma (413 `save_file_too_large`, corpo do GameKave). A página diz
    /// `too_large` e qual arquivo; a passada seguinte não manda os mesmos
    /// bytes de novo, mas segue dizendo; bytes novos sobem.
    #[tokio::test]
    async fn a_too_large_save_is_named_and_not_resent_until_its_bytes_change() {
        let _serial = SERIAL.lock().await;
        let r = rig("too-large").await;
        r.rom("r1", "Mario.n64", "n64").await;
        r.write("saves/Mario.srm", &vec![1u8; 4096]);
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(seq(vec![
                ResponseTemplate::new(413).set_body_json(json!({"error": {
                    "code": "save_file_too_large",
                    "message": "object exceeds the allowed size", "max_bytes": 65536}})),
                uploaded("s1", "v1"),
            ]))
            .mount(&r.server)
            .await;
        let key = "file:n64/Mario.srm";

        let report = r.pass().await.unwrap();
        assert_eq!(report.errors, vec![(key.to_owned(), SyncError::TooLarge)]);
        let st = r.status().await;
        assert_eq!(st.last_error.as_deref(), Some("too_large"));
        assert_eq!(st.last_error_file.as_deref(), Some("Mario.srm"));
        assert!(r.base_of(key).await.is_none());

        // Os mesmos bytes: nada sobe, a tela segue dizendo qual.
        let again = r.pass().await.unwrap();
        assert_eq!(again.errors, vec![(key.to_owned(), SyncError::TooLarge)]);
        assert_eq!(
            r.reqs("POST", SAVES).await.len(),
            1,
            "reenviou os mesmos bytes"
        );
        assert_eq!(
            r.status().await.last_error_file.as_deref(),
            Some("Mario.srm")
        );

        // O jogo gravou de novo: os bytes novos sobem.
        r.write("saves/Mario.srm", &vec![2u8; 4096]);
        let after = r.pass().await.unwrap();
        assert_eq!((after.uploaded, after.errors.len()), (1, 0));
        assert_eq!(r.reqs("POST", SAVES).await.len(), 2);
        let st = r.status().await;
        assert_eq!((st.last_error, st.last_error_file), (None, None));
    }

    /// M9: 422 `save_bios_rejected` e 400 `validation_failed` também são
    /// recusa de vez: código `rejected`, sem reenvio dos mesmos bytes.
    #[tokio::test]
    async fn a_rejected_save_is_rejected_not_failed_and_is_not_resent() {
        let _serial = SERIAL.lock().await;
        let r = rig("rejected").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.rom("r2", "Chrono.sfc", "snes").await;
        r.write("saves/Zelda.srm", b"zelda");
        r.write("saves/Chrono.srm", b"chrono");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(ResponseTemplate::new(422).set_body_json(json!({"error": {
                "code": "save_bios_rejected", "message": "BIOS images are never stored"}})))
            .mount(&r.server)
            .await;

        let report = r.pass().await.unwrap();
        assert_eq!(report.errors.len(), 2);
        assert!(report.errors.iter().all(|(_, e)| *e == SyncError::Rejected));
        assert_eq!(r.sync.runtime().last_error.as_deref(), Some("rejected"));
        r.pass().await.unwrap();
        assert_eq!(r.reqs("POST", SAVES).await.len(), 2, "um envio por arquivo");
    }

    #[tokio::test]
    async fn save_status_reports_what_the_game_page_shows() {
        let _serial = SERIAL.lock().await;
        let r = rig("saveline").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        let state = |s: SaveLineStatus| (s.state, s.key);

        let none = r.engine.save_status("r1").await.unwrap();
        assert_eq!(state(none), (SaveLineState::None, None));
        r.write("saves/Zelda.srm", b"a");
        let pending = r.engine.save_status("r1").await.unwrap();
        assert_eq!(state(pending), (SaveLineState::Pending, Some(ZELDA.into())));
        r.base(ZELDA, "saves/Zelda.srm", b"a", b"a", ("s1", "v1"))
            .await;
        let synced = r.engine.save_status("r1").await.unwrap();
        assert_eq!(synced.state, SaveLineState::Synced);
        assert_eq!(synced.synced_at, Some(1));
        r.repo
            .conflict_put(&ConflictRow {
                key: ZELDA.into(),
                local_hash: sha(b"a"),
                local_mtime: 1,
                local_size: 1,
                remote_save_id: "s1".into(),
                remote_version_id: "v2".into(),
                remote_hash: sha(b"b"),
                remote_device_label: None,
                remote_client_mtime: None,
                remote_size: 1,
                remote_deleted: false,
                rom_id: Some("r1".into()),
                seen_at: 1,
            })
            .await
            .unwrap();
        let conflict = r.engine.save_status("r1").await.unwrap();
        assert_eq!(
            state(conflict),
            (SaveLineState::Conflict, Some(ZELDA.into()))
        );
        r.repo.conflict_delete(ZELDA).await.unwrap();
        r.sync.devices.set_enabled(false).await.unwrap();
        let off = r.engine.save_status("r1").await.unwrap();
        assert_eq!((off.state, off.key), (SaveLineState::None, None));
        assert!(r.server.received_requests().await.unwrap().is_empty());
    }

    /// Os três portões da linha do jogo: sem rede, e `none` mesmo com save e
    /// com conflito guardado.
    #[tokio::test]
    async fn save_status_is_none_without_network_when_sync_is_off_revoked_or_saves_off() {
        let _serial = SERIAL.lock().await;
        let r = rig("saveline-gates").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"a");
        r.repo
            .conflict_put(&ConflictRow {
                key: ZELDA.into(),
                local_hash: sha(b"a"),
                local_mtime: 1,
                local_size: 1,
                remote_save_id: "s1".into(),
                remote_version_id: "v2".into(),
                remote_hash: sha(b"b"),
                remote_device_label: None,
                remote_client_mtime: None,
                remote_size: 1,
                remote_deleted: false,
                rom_id: Some("r1".into()),
                seen_at: 1,
            })
            .await
            .unwrap();
        let state = |s: SaveLineStatus| (s.state, s.key, s.versions);
        assert_eq!(
            r.engine.save_status("r1").await.unwrap().state,
            SaveLineState::Conflict
        );

        // 1. sync desligado
        r.sync.devices.set_enabled(false).await.unwrap();
        assert_eq!(
            state(r.engine.save_status("r1").await.unwrap()),
            (SaveLineState::None, None, 0)
        );
        r.sync.devices.set_enabled(true).await.unwrap();

        // 2. aparelho revogado
        r.sync.devices.set_site_state(false, true).await.unwrap();
        assert_eq!(
            state(r.engine.save_status("r1").await.unwrap()),
            (SaveLineState::None, None, 0)
        );
        r.sync.devices.set_site_state(false, false).await.unwrap();

        // 3. game saves desligados
        r.sync.devices.set_saves_enabled(false).await.unwrap();
        assert_eq!(
            state(r.engine.save_status("r1").await.unwrap()),
            (SaveLineState::None, None, 0)
        );
        r.sync.devices.set_saves_enabled(true).await.unwrap();

        assert_eq!(
            r.engine.save_status("r1").await.unwrap().state,
            SaveLineState::Conflict
        );
        assert!(r.server.received_requests().await.unwrap().is_empty());
    }

    /// Ação manual: com os game saves desligados, resolver um conflito que já
    /// existe continua valendo.
    #[tokio::test]
    async fn resolve_still_works_with_saves_off() {
        let _serial = SERIAL.lock().await;
        let r = conflict_rig("resolve-saves-off").await;
        r.pass().await.unwrap();
        r.sync.devices.set_saves_enabled(false).await.unwrap();

        r.engine
            .resolve(&r.alva, ZELDA, Keep::Remote)
            .await
            .unwrap();
        assert_eq!(r.read("saves/Zelda.srm").unwrap(), b"theirs");
        assert!(r.repo.conflicts_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn sync_now_runs_the_pass_before_hardware_and_quota() {
        let _serial = SERIAL.lock().await;
        let r = rig("now").await;
        r.rom("r1", "Zelda.gba", "gba").await;
        r.write("saves/Zelda.srm", b"zelda");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        mount_upload(&r.server, "s1", "v1").await;
        Mock::given(method("PATCH"))
            .and(path(format!("{SAVES}/device")))
            .respond_with(ResponseTemplate::new(200).set_body_json(device("dev1", "Sala")))
            .mount(&r.server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!("{SAVES}/quota")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"used_bytes": 5, "limit_bytes": 50})),
            )
            .mount(&r.server)
            .await;

        let st = r
            .sync
            .now_with(&r.alva, HardwarePayload::default(), Some(&r.engine))
            .await
            .unwrap();
        assert!(r.base_of(ZELDA).await.is_some());
        assert_eq!(st.last_error, None);
        assert!(st.last_pass_at.is_some());
        assert_eq!(st.quota.map(|q| q.used_bytes), Some(5));
        let order: Vec<String> = r
            .server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|q| format!("{} {}", q.method, q.url.path()))
            .collect();
        let at = |what: &str| order.iter().position(|o| o == what).unwrap();
        assert!(at("POST /api/v1/saves") < at("PATCH /api/v1/saves/device"));
    }

    #[tokio::test]
    async fn sync_now_with_a_live_row_does_not_read_a_bare_401_as_revoked() {
        let _serial = SERIAL.lock().await;
        let r = rig("now401").await;
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        for (m, p) in [("PATCH", "/device"), ("GET", "/quota")] {
            Mock::given(method(m))
                .and(path(format!("{SAVES}{p}")))
                .respond_with(unauthorized())
                .mount(&r.server)
                .await;
        }
        let st = r
            .sync
            .now_with(&r.alva, HardwarePayload::default(), Some(&r.engine))
            .await
            .unwrap();
        assert!(!st.revoked && st.enabled, "{st:?}");
        assert_eq!(st.last_error.as_deref(), Some("offline"));
        assert_eq!(
            r.sync.devices.token().unwrap().as_deref(),
            Some("asv_secret")
        );
    }

    #[test]
    fn the_owner_is_the_row_owner_never_the_writer() {
        let mut v = row("s1", "Zelda.srm", "gba", None, b"x", "v1", Some("dev1"));
        let account: RemoteFile = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(remote_of(&account).unwrap().device_token_id, None);
        v["device_token_id"] = json!("devX");
        let owned: RemoteFile = serde_json::from_value(v).unwrap();
        assert_eq!(
            remote_of(&owned).unwrap().device_token_id.as_deref(),
            Some("devX")
        );
        // Linha apagada sem versão ainda é uma marca de apagado.
        let mut gone = tombstone(row("s2", "Zelda.srm", "gba", None, b"x", "v1", None));
        gone["current"] = Value::Null;
        let gone: RemoteFile = serde_json::from_value(gone).unwrap();
        assert!(remote_of(&gone).unwrap().deleted);
        // Linha viva sem versão: nada a baixar.
        let mut empty = row("s3", "Zelda.srm", "gba", None, b"x", "v1", None);
        empty["current"] = Value::Null;
        let empty: RemoteFile = serde_json::from_value(empty).unwrap();
        assert_eq!(remote_of(&empty), None);
    }

    fn save_file(platform: &'static str, core: Option<&'static str>, filename: &str) -> SaveFile {
        SaveFile {
            key: format!("file:{platform}/{filename}"),
            rom_id: None,
            rom_crc32: None,
            platform,
            core,
            filename: filename.into(),
            path: PathBuf::from(filename),
        }
    }

    fn file_row(v: Value) -> RemoteFile {
        serde_json::from_value(v).unwrap()
    }

    fn written_at(mut v: Value, when: &str) -> Value {
        v["current"]["server_created_at"] = json!(when);
        v
    }

    #[test]
    fn pick_follows_platform_then_other_and_prefers_live_account_newest() {
        let snes = save_file("snes", None, "Chrono.srm");
        let rows = [
            file_row(row("o1", "Chrono.srm", "other", None, b"x", "v1", None)),
            file_row(row(
                "s1",
                "Chrono.srm",
                "snes",
                Some("Snes9x"),
                b"x",
                "v1",
                None,
            )),
        ];
        assert_eq!(pick(&by_name(&rows), &snes).unwrap().save_id, "s1");
        let only_other = [file_row(row(
            "o1",
            "Chrono.srm",
            "other",
            None,
            b"x",
            "v1",
            None,
        ))];
        assert_eq!(pick(&by_name(&only_other), &snes).unwrap().save_id, "o1");
        let zelda = save_file("gba", None, "Zelda.srm");
        assert!(pick(&by_name(&only_other), &zelda).is_none());

        // Entre várias da mesma plataforma: a viva, a da conta, a mais nova.
        let snes_row = |id: &str, when: &str| {
            written_at(row(id, "Chrono.srm", "snes", None, b"x", "v1", None), when)
        };
        let older = snes_row("a", "2026-11-13T10:00:00Z");
        let newer = snes_row("b", "2026-11-13T10:00:00.5Z");
        let dead_newest = tombstone(snes_row("c", "2026-11-14T00:00:00Z"));
        let mut device_newest = snes_row("d", "2026-11-15T00:00:00Z");
        device_newest["device_token_id"] = json!("devX");
        let rows: Vec<RemoteFile> = [older, newer, dead_newest, device_newest]
            .into_iter()
            .map(file_row)
            .collect();
        assert_eq!(pick(&by_name(&rows), &snes).unwrap().save_id, "b");

        // Linha de cartão ou VMU nunca serve a save de ROM.
        let card_row = [file_row(row(
            "k",
            "Chrono.srm",
            "snes",
            Some("system/dc"),
            b"x",
            "v1",
            None,
        ))];
        assert!(pick(&by_name(&card_row), &snes).is_none());
        // Cartão: mesma pasta, nome sem diferença de maiúsculas.
        let card = save_file("ps2", Some("system/pcsx2/memcards"), "Mcd001.ps2");
        let cards = [
            file_row(row(
                "m1",
                "MCD001.PS2",
                "ps2",
                Some("system/pcsx2/memcards"),
                b"x",
                "v1",
                None,
            )),
            file_row(row(
                "m2",
                "Mcd001.ps2",
                "ps2",
                Some("other/folder"),
                b"x",
                "v1",
                None,
            )),
        ];
        assert_eq!(pick(&by_name(&cards), &card).unwrap().save_id, "m1");

        // M6: a linha do ReEmu apagada no site não esconde a `other` viva
        // que um RetroArch plano segue escrevendo; sem viva, a apagada vale.
        let dead_snes = file_row(tombstone(row(
            "t1",
            "Chrono.srm",
            "snes",
            None,
            b"x",
            "v1",
            None,
        )));
        let live_other = file_row(row("o1", "Chrono.srm", "other", None, b"x", "v1", None));
        let rows = [dead_snes.clone(), live_other];
        assert_eq!(pick(&by_name(&rows), &snes).unwrap().save_id, "o1");
        let dead_other = file_row(tombstone(row(
            "t2",
            "Chrono.srm",
            "other",
            None,
            b"x",
            "v1",
            None,
        )));
        let rows = [dead_other, dead_snes];
        assert_eq!(pick(&by_name(&rows), &snes).unwrap().save_id, "t1");
    }

    #[test]
    fn parse_time_reads_rfc3339_with_fractions_and_offsets() {
        assert_eq!(parse_time("2026-11-13T10:00:00Z"), Some((1_794_564_000, 0)));
        assert_eq!(
            parse_time("2026-11-13T10:00:00.250Z"),
            Some((1_794_564_000, 250_000_000))
        );
        assert_eq!(
            parse_time("2026-11-13T07:00:00-03:00"),
            Some((1_794_564_000, 0))
        );
        assert_eq!(
            parse_time("2026-11-13T10:00:00.123456789123Z"),
            Some((1_794_564_000, 123_456_789))
        );
        for bad in [
            "2026-13-01T00:00:00Z",
            "yesterday",
            "2026-11-13T10:00:00",
            "2026-11-13T10:00:00.Z",
            "+026-11-13T10:00:00Z",
        ] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
        assert!(parse_time("2026-11-13T10:00:00.5Z") > parse_time("2026-11-13T10:00:00Z"));
    }

    #[test]
    fn backup_stamps_use_the_local_clock() {
        let stamp = local_stamp();
        let at = chrono::NaiveDateTime::parse_from_str(&stamp, "%Y%m%d-%H%M%S").unwrap();
        let now = chrono::Local::now().naive_local();
        assert!((now - at).num_seconds().abs() <= 5, "{stamp}");
    }

    #[tokio::test]
    async fn backups_keep_the_relative_path_and_never_overwrite() {
        let _serial = SERIAL.lock().await;
        let r = rig("backup").await;
        r.engine.new_stamp();
        r.write("saves/Zelda.srm", b"one");
        let first = r.engine.backup(&r.path("saves/Zelda.srm")).unwrap();
        r.write("saves/Zelda.srm", b"two");
        let second = r.engine.backup(&r.path("saves/Zelda.srm")).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            second.file_name().unwrap().to_string_lossy(),
            "Zelda.srm (2)"
        );
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(&second).unwrap(), b"two");
        assert_eq!(
            r.read("saves/Zelda.srm").unwrap(),
            b"two",
            "o original fica"
        );
        // Fora da pasta de dados: só o nome.
        let outside = temp("backup-outside").join("Card.ps2");
        std::fs::write(&outside, b"c").unwrap();
        let third = r.engine.backup(&outside).unwrap();
        assert_eq!(
            third.parent().unwrap().parent().unwrap(),
            r.path("sync-backups")
        );
    }

    /// Fix R18 item 1: o prazo de antes de abrir corta a passada no meio de
    /// uma renovação do par do parceiro. O servidor já trocou o par: o novo é
    /// gravado assim mesmo e a sessão segue.
    #[tokio::test]
    async fn prelaunch_timeout_during_a_renewal_keeps_the_session() {
        let _serial = SERIAL.lock().await;
        let mut r = rig("renewal").await;
        r.engine.prelaunch_budget = Duration::from_millis(100);
        r.rom("r1", "Zelda.gba", "gba").await;
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(ResponseTemplate::new(401))
            .mount(&r.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "access_token": "alp_2", "refresh_token": "alr_2",
                        "token_type": "Bearer", "expires_in": 3600
                    }))
                    .set_delay(Duration::from_millis(400)),
            )
            .expect(1)
            .mount(&r.server)
            .await;
        r.set_playing(Playing::Loading);

        assert_eq!(
            r.engine.prelaunch(&r.alva, "r1").await,
            PrelaunchOutcome::Skipped("timeout".into())
        );
        let mut stored = None;
        for _ in 0..100 {
            stored = r.store.load_tokens().await.unwrap();
            if stored == Some(tokens("2")) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(stored, Some(tokens("2")), "o par novo se perdeu");
        assert!(r.store.has_session().await.unwrap());
        assert_eq!(
            r.sync.devices.token().unwrap().as_deref(),
            Some("asv_secret")
        );
    }

    /// Fix R18 item 1: a renovação que o prazo cortou descobre que a conta
    /// acabou no site. O aparelho é revogado e esquecido mesmo sem ninguém
    /// esperando: o `asv_` não fica no chaveiro.
    #[tokio::test]
    async fn a_dropped_prelaunch_that_finds_the_account_ended_still_forgets_the_device() {
        let _serial = SERIAL.lock().await;
        let mut r = rig("ended").await;
        r.engine.prelaunch_budget = Duration::from_millis(100);
        r.rom("r1", "Zelda.gba", "gba").await;
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(ResponseTemplate::new(401))
            .mount(&r.server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/partner/oauth/token"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!({"error": "invalid_grant"}))
                    .set_delay(Duration::from_millis(400)),
            )
            .mount(&r.server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(format!("{SAVES}/device")))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&r.server)
            .await;
        r.set_playing(Playing::Loading);

        assert_eq!(
            r.engine.prelaunch(&r.alva, "r1").await,
            PrelaunchOutcome::Skipped("timeout".into())
        );
        // O esquecer apaga o SQLite e depois o chaveiro: espera os dois.
        for _ in 0..100 {
            if r.sync.devices.device().await.unwrap().is_none()
                && r.sync.devices.token().unwrap().is_none()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(r.sync.devices.device().await.unwrap(), None);
        assert_eq!(r.sync.devices.token().unwrap(), None, "asv_ no chaveiro");
        assert!(!r.store.has_session().await.unwrap());
    }

    /// M4: com um load em andamento e um core aberto, o `current_rom` ainda é
    /// o do jogo anterior; sem core, o load ainda deixa a passada de antes de
    /// abrir escrever.
    #[test]
    fn the_probe_never_names_the_previous_game_while_loading() {
        let prev = || Some("r-old".to_owned());
        assert_eq!(playing_from(true, true, prev()), Playing::Running(None));
        assert_eq!(playing_from(true, false, prev()), Playing::Running(prev()));
        assert_eq!(playing_from(false, true, prev()), Playing::Loading);
        assert_eq!(playing_from(false, false, None), Playing::Idle);
    }

    /// M5: duas ROMs de mesmo stem dividem o arquivo; fechar a segunda também
    /// sobe o save.
    #[tokio::test]
    async fn a_rom_pass_takes_the_file_its_stem_shares_with_another_rom() {
        let _serial = SERIAL.lock().await;
        let r = rig("shared-stem").await;
        r.rom("r1", "Game.gba", "gba").await;
        r.rom("r2", "Game.nes", "nes").await;
        r.write("saves/Game.srm", b"played on nes");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;
        mount_upload(&r.server, "s1", "v1").await;

        let report = r
            .engine
            .pass(&r.alva, PassScope::Rom("r2".into()))
            .await
            .unwrap();
        assert_eq!(report.uploaded, 1);
        assert!(r.base_of("file:gba/Game.srm").await.is_some());
    }

    /// M6: o jogo saiu da biblioteca; o conflito dele sai da lista na
    /// passada seguinte, em vez de contar na página para sempre.
    #[tokio::test]
    async fn a_conflict_whose_rom_left_the_library_is_dropped() {
        let _serial = SERIAL.lock().await;
        let r = rig("orphan-conflict").await;
        r.repo
            .conflict_put(&ConflictRow {
                key: ZELDA.into(),
                local_hash: sha(b"a"),
                local_mtime: 1,
                local_size: 1,
                remote_save_id: "s1".into(),
                remote_version_id: "v2".into(),
                remote_hash: sha(b"b"),
                remote_device_label: None,
                remote_client_mtime: None,
                remote_size: 1,
                remote_deleted: false,
                rom_id: Some("r1".into()),
                seen_at: 1,
            })
            .await
            .unwrap();
        assert_eq!(r.status().await.conflicts, 1);
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;

        r.pass().await.unwrap();
        assert!(r.repo.conflicts_all().await.unwrap().is_empty());
        assert_eq!(r.status().await.conflicts, 0);
    }

    /// M7: um 200 sem `devices` é resposta ilegível: a passada acaba offline e
    /// a máquina não vira revogada.
    #[tokio::test]
    async fn an_unexpected_device_list_is_offline_not_revoked() {
        let _serial = SERIAL.lock().await;
        let r = rig("odd-list").await;
        Mock::given(method("GET"))
            .and(path(DEVICES))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"items": []})))
            .mount(&r.server)
            .await;
        assert_eq!(r.pass().await, Err(SyncError::Offline));
        assert_eq!(
            r.sync.devices.token().unwrap().as_deref(),
            Some("asv_secret")
        );
        assert!(r.sync.devices.enabled().await.unwrap());
        assert_eq!(r.sync.devices.site_state().await.unwrap(), (false, false));
    }

    /// M8: o app fechou entre o temporário e o `rename`. A próxima passada
    /// tira os `.sync-tmp` e só eles.
    #[tokio::test]
    async fn stale_temporaries_are_swept_at_pass_start() {
        let _serial = SERIAL.lock().await;
        let r = rig("sweep").await;
        r.write("saves/Zelda.srm.sync-tmp", b"half");
        r.write("system/pcsx2/memcards/Mcd001.ps2.sync-tmp", b"half");
        r.write("system/dc/vmu_save_A1.bin.sync-tmp", b"half");
        // O temporário da própria sessão e outros arquivos ficam.
        r.write("saves/Zelda.srm.tmp", b"session");
        r.write("saves/notes.txt", b"mine");
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_files(&r.server, vec![]).await;

        r.pass().await.unwrap();
        assert!(r.read("saves/Zelda.srm.sync-tmp").is_none());
        assert!(r
            .read("system/pcsx2/memcards/Mcd001.ps2.sync-tmp")
            .is_none());
        assert!(r.read("system/dc/vmu_save_A1.bin.sync-tmp").is_none());
        assert_eq!(r.read("saves/Zelda.srm.tmp").unwrap(), b"session");
        assert_eq!(r.read("saves/notes.txt").unwrap(), b"mine");
    }
}
