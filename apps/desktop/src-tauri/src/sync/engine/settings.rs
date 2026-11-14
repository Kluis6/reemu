//! As configurações desta máquina na nuvem e as de outra aqui (D114). Nada
//! disso roda sozinho, é a pessoa quem pede, e por isso não há interruptor
//! como o dos saves e o dos states:
//!
//! - "Enviar as desta máquina" sobe um documento por grupo como linha `config`
//!   deste aparelho: `platform=other`, sem `core` (o GameKave guarda um `core`
//!   vazio como nenhum, e a linha é a mesma), `filename=reemu/<grupo>.json`. A
//!   aparência mora na tela e chega pronta. Cada grupo volta com o resultado
//!   dele, e o hash de cada documento enviado fica guardado: é o que diz se um
//!   grupo mudou desde o envio;
//! - a prévia lê as linhas de outra máquina, aplica os documentos numa
//!   transação que não fica (o relatório é o que a cópia diria) e compara a
//!   GPU e o backend das duas, para o vídeo começar desmarcado quando diferem.
//!   Um documento que não serve marca só o grupo dele;
//! - copiar baixa os grupos marcados e confere todos; na transação de escrita
//!   grava antes, no disco, o backup do que a cópia troca, aplica os grupos e
//!   o marcador do "Desfazer" (de qual máquina, quando, quais grupos), e só
//!   então confirma. Um documento ruim ou um backup que não grava, e nada muda;
//! - o backup é desta máquina e nunca sai dela: os jogos vão pela ROM (R38),
//!   em `<dados>/sync-backups/<carimbo>/settings/<grupo>.json`, e o marcador
//!   guarda o caminho relativo a `<dados>`. O "Desfazer" grava antes o estado
//!   de agora dos mesmos grupos num carimbo novo (o que mudou depois da cópia
//!   não se perde, R39), volta o backup e apaga o marcador na mesma transação:
//!   vale uma vez por cópia, e não há "desfazer o desfazer";
//! - copiar e desfazer não rodam com um jogo aberto ou abrindo (o core lê
//!   opções, atalhos e áudio enquanto roda); depois, o que o app guarda em
//!   memória (atalhos, teclado, vídeo, áudio) é relido do banco;
//! - a rede segue as regras das passadas: a linha desta máquina na lista do
//!   site primeiro (pausada ou removida, nada mais sai, D128), um 401 sozinho
//!   relê a lista, o 402 da porta não é o da cota, uma recusa de vez (413,
//!   400, 422) não sobe de novo os mesmos bytes, e sem o AlvaID nada sai;
//! - nenhum nome de opção, preset, jogo ou arquivo vai para o log: só o grupo
//!   e contagens.

use super::*;
use crate::sync::api::FileFilter;
use crate::sync::device::{HardwarePayload, LAST_COPY_BACKUP_KEY, LAST_UNDO_BACKUP_KEY};
use crate::sync::settings_groups::{
    apply_in, check, check_all, dry_run, export, export_backup_in, log_applied, ApplyMode,
    ApplyReport, Catalog, Checked, Group, MissingFile, ThisMachine, DOC_VERSION, ERR_INVALID,
    ERR_VERSION,
};
use crate::sync::SyncMachine;
use db::{SettingsRepo, SettingsTx};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// O grupo que mora na tela (`lib/appearanceGroup.ts`).
const APPEARANCE: &str = "appearance";

/// Os grupos na ordem do diálogo de cópia.
const ORDER: [&str; 6] = [
    "core-options",
    "shaders-decorations",
    "hotkeys-keyboard",
    "audio",
    APPEARANCE,
    "video",
];

/// O nome da linha de um grupo na nuvem.
fn filename_of(group: &str) -> String {
    format!("reemu/{group}.json")
}

/// O grupo de uma linha `reemu/<grupo>.json` que esta versão conhece.
fn group_of(filename: &str) -> Option<&'static str> {
    let name = filename.strip_prefix("reemu/")?.strip_suffix(".json")?;
    ORDER.into_iter().find(|g| *g == name)
}

fn group_by_name(name: &str) -> Option<Group> {
    Group::ALL.into_iter().find(|g| g.as_str() == name)
}

/// Um grupo do diálogo e o que dele vem.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SettingsGroupPreview {
    #[cfg_attr(
        test,
        ts(
            type = "'core-options' | 'shaders-decorations' | 'hotkeys-keyboard' | 'audio' | 'appearance' | 'video'"
        )
    )]
    pub group: String,
    /// A outra máquina enviou este grupo.
    pub present: bool,
    /// O documento não serve aqui e o grupo não pode ser copiado:
    /// `settings_version` (veio de um ReEmu mais novo), `settings_invalid`
    /// (outro formato), `settings_missing` (sumiu da nuvem entre a listagem e
    /// a descida) ou `failed` (chegou diferente do hash da nuvem; vale tentar
    /// de novo).
    pub error: Option<String>,
    /// Presets e molduras que não viriam, com o motivo (D126).
    pub missing_files: Vec<MissingFile>,
}

/// O que `sync_settings_preview` devolve.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SettingsPreview {
    pub machine: SyncMachine,
    /// Segundos Unix do envio mais novo dela; 0 quando nunca enviou.
    pub sent_at: i64,
    /// Os seis grupos, na ordem do diálogo.
    pub groups: Vec<SettingsGroupPreview>,
    /// A GPU ou o backend dela diferem dos desta máquina (ou um dos dois não
    /// é conhecido): o vídeo começa desmarcado.
    pub video_default_off: bool,
    /// A GPU desta máquina que a comparação usou (a de agora; sem ela, a da
    /// lista do site). Vazia quando não se sabe.
    pub here_gpu: String,
    /// O backend desta máquina que a comparação usou. Vazio quando não se sabe.
    pub here_backend: String,
}

/// Um grupo copiado e o que a cópia fez nele.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SettingsGroupReport {
    #[cfg_attr(
        test,
        ts(
            type = "'core-options' | 'shaders-decorations' | 'hotkeys-keyboard' | 'audio' | 'video'"
        )
    )]
    pub group: String,
    pub report: ApplyReport,
}

/// O que `sync_settings_copy` devolve.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct CopyReport {
    /// Os grupos do banco, na ordem em que entraram.
    pub per_group: Vec<SettingsGroupReport>,
    /// A pasta do backup desta máquina; `None` quando só a aparência veio.
    pub backup_dir: Option<String>,
    /// O documento de aparência da outra máquina, quando pedido: a tela o
    /// aplica e guarda o de antes (`reemu.sync.appearanceUndo`).
    #[cfg_attr(test, ts(type = "unknown"))]
    pub appearance_doc: Option<Value>,
    /// Segundos Unix desta cópia, os mesmos do marcador do "Desfazer"
    /// (`SettingsUndoInfo::copied_at`): a aparência de antes guardada pela
    /// tela leva este valor e só volta no "Desfazer" desta cópia.
    pub copied_at: i64,
}

/// O que `sync_settings_sent` devolve.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SettingsSent {
    /// Segundos Unix do último "Enviar as desta máquina"; `None` antes do
    /// primeiro (ou depois de desligar o sync).
    pub sent_at: Option<i64>,
    /// Os grupos cujo documento de agora não é o que subiu por último, na
    /// ordem do diálogo.
    #[cfg_attr(
        test,
        ts(
            type = "Array<'core-options' | 'shaders-decorations' | 'hotkeys-keyboard' | 'audio' | 'appearance' | 'video'>"
        )
    )]
    pub changed: Vec<String>,
}

/// Como um grupo terminou o envio.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SettingsGroupSent {
    #[cfg_attr(
        test,
        ts(
            type = "'core-options' | 'shaders-decorations' | 'hotkeys-keyboard' | 'audio' | 'appearance' | 'video'"
        )
    )]
    pub group: String,
    /// Subiu neste envio.
    pub sent: bool,
    /// Por que não subiu, um código de `sync.err`: `too_large` ou `rejected`
    /// (a nuvem recusou estes bytes de vez; a tela diz o grupo), ou o que
    /// parou o envio antes dele (`no_tier`, `quota`, `offline`, `revoked`,
    /// `paused`).
    pub error: Option<String>,
}

/// O que `sync_settings_upload` devolve: o resultado de cada grupo é o que
/// vale.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SettingsUpload {
    /// Segundos Unix do envio; `None` quando nenhum grupo subiu (a "Última
    /// enviada" fica a de antes).
    pub sent_at: Option<i64>,
    /// Os seis grupos, na ordem do diálogo.
    pub groups: Vec<SettingsGroupSent>,
    /// O erro que a tela mostra quando algum grupo ficou: o plano vem antes de
    /// uma recusa lembrada, para a oferta do plano aparecer.
    pub error: Option<String>,
}

/// O que `sync_settings_undo_info` devolve: a cópia que o "Desfazer" volta.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SettingsUndoInfo {
    /// O nome da máquina de onde as configurações vieram.
    pub machine: String,
    /// O aparelho dela (a cor da máquina na tela), quando se sabe.
    pub machine_id: Option<String>,
    /// Segundos Unix da cópia.
    pub copied_at: i64,
    /// Os grupos copiados, na ordem do diálogo (a aparência inclusive).
    pub groups: Vec<String>,
}

/// O marcador do "Desfazer". Os caminhos são relativos a `<dados>`, com `/`:
/// a pasta de dados pode mudar de lugar.
#[derive(Debug, Default, Serialize, Deserialize)]
struct CopyBackup {
    #[serde(default)]
    machine: String,
    #[serde(default)]
    machine_id: Option<String>,
    #[serde(default)]
    copied_at: i64,
    #[serde(default)]
    groups: Vec<String>,
    /// O backup de cada grupo do banco copiado.
    files: BTreeMap<String, String>,
}

/// O registro do estado de antes de um "Desfazer" (R39): fica para a pessoa
/// achar os arquivos, e nada o oferece de volta.
#[derive(Debug, Serialize)]
struct UndoBackup {
    undone_at: i64,
    files: BTreeMap<String, String>,
}

/// O que esta máquina tem e o banco não diz, para aplicar.
struct Here {
    roms: Vec<(String, String)>,
    cores: Vec<String>,
    catalog: Catalog,
}

impl Here {
    fn view(&self) -> ThisMachine<'_> {
        ThisMachine {
            roms: &self.roms,
            cores: &self.cores,
            catalog: &self.catalog,
        }
    }
}

/// Os erros de `settings_groups` como códigos da tela.
fn settings_err(e: String) -> SyncError {
    match e.as_str() {
        ERR_VERSION => SyncError::SettingsVersion,
        ERR_INVALID => SyncError::SettingsInvalid,
        _ => failed(format!("configurações: {e}")),
    }
}

fn db_err(e: domain::error::RepoError) -> SyncError {
    failed(format!("configurações: {e}"))
}

fn backup_err(e: io::Error) -> SyncError {
    log::warn!("sync: o backup das configurações falhou: {e}");
    SyncError::BackupFailed
}

/// O documento de aparência: um objeto da versão que esta conhece. O resto
/// do formato a tela confere ao aplicar.
fn appearance_doc(doc: &Value) -> Result<Value, SyncError> {
    if !doc.is_object() {
        return Err(SyncError::SettingsInvalid);
    }
    if doc.get("v").and_then(Value::as_u64) != Some(DOC_VERSION) {
        return Err(SyncError::SettingsVersion);
    }
    Ok(doc.clone())
}

/// O mesmo documento dá sempre os mesmos bytes: as chaves em ordem, em todo
/// nível. É o que sobe e o que o hash do "mudou desde o envio" compara.
fn canonical_bytes(doc: &Value) -> Vec<u8> {
    fn sorted(v: &Value) -> Value {
        match v {
            Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort();
                let mut out = serde_json::Map::new();
                for k in keys {
                    out.insert(k.clone(), sorted(&map[k]));
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_vec(&sorted(doc)).unwrap_or_default()
}

/// As duas máquinas têm a mesma GPU no mesmo backend. Um lado sem GPU ou sem
/// backend conhecidos não é "a mesma": o vídeo começa desmarcado.
fn same_gpu(a: Option<&HardwarePayload>, b: Option<&HardwarePayload>) -> bool {
    let norm = |s: &str| s.trim().to_lowercase();
    match (a, b) {
        (Some(a), Some(b)) => {
            let known =
                |h: &HardwarePayload| !h.gpu.trim().is_empty() && !h.gpu_backend.trim().is_empty();
            known(a)
                && known(b)
                && norm(&a.gpu) == norm(&b.gpu)
                && norm(&a.gpu_backend) == norm(&b.gpu_backend)
        }
        _ => false,
    }
}

/// Os grupos do banco pedidos pela tela; um nome desconhecido ou repetido
/// não serve.
fn ticked(names: &[String]) -> Result<Vec<Group>, SyncError> {
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let group = group_by_name(name).ok_or(SyncError::SettingsInvalid)?;
        if out.contains(&group) {
            return Err(SyncError::SettingsInvalid);
        }
        out.push(group);
    }
    Ok(out)
}

/// A ordem das linhas de um mesmo grupo: a versão escrita por último.
fn written(r: &RemoteFile) -> (i64, u32) {
    r.current
        .as_ref()
        .and_then(|v| parse_time(&v.server_created_at))
        .unwrap_or((i64::MIN, 0))
}

/// Qual erro de um envio a tela mostra: o plano antes da cota e de uma
/// recusa lembrada (a oferta do plano é o que a pessoa pode fazer).
fn upload_rank(e: &SyncError) -> u8 {
    match e {
        SyncError::Revoked => 7,
        SyncError::Paused => 6,
        SyncError::NoTier => 5,
        SyncError::Quota => 4,
        SyncError::Offline => 3,
        SyncError::TooLarge | SyncError::StateTooLarge | SyncError::Rejected => 2,
        _ => 1,
    }
}

/// Os pedaços de um caminho relativo do marcador; `None` para o que sairia de
/// `<dados>` ou seria absoluto.
fn data_parts(rel: &str) -> Option<Vec<&str>> {
    let parts: Vec<&str> = rel.split('/').collect();
    parts
        .iter()
        .all(|p| !p.is_empty() && *p != "." && *p != ".." && !p.contains(['\\', ':']))
        .then_some(parts)
}

impl Engine {
    fn catalog(&self) -> Catalog {
        Catalog::new(&self.dirs.shaders, &self.dirs.decorations)
    }

    /// Copiar e desfazer só sem jogo aberto nem abrindo.
    fn settings_idle(&self) -> Result<(), SyncError> {
        match self.playing() {
            Playing::Idle => Ok(()),
            Playing::Loading | Playing::Running(_) => Err(SyncError::GameOpen),
        }
    }

    async fn installed_cores(&self) -> Vec<String> {
        let dir = self.dirs.cores.clone();
        tokio::task::spawn_blocking(move || emu_session::installed_core_ids(&dir))
            .await
            .unwrap_or_default()
    }

    /// O que esta máquina tem agora, lido fora de uma transação (a prévia).
    async fn here(&self) -> Result<Here, SyncError> {
        let mut tx = SettingsRepo::new(self.db.clone())
            .read()
            .await
            .map_err(db_err)?;
        let roms = tx.rom_crc32s().await.map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(Here {
            roms,
            cores: self.installed_cores().await,
            catalog: self.catalog(),
        })
    }

    /// O caminho de um arquivo de `<dados>` como o marcador guarda.
    fn data_relative(&self, path: &Path) -> Result<String, SyncError> {
        let rel = path
            .strip_prefix(&self.dirs.data)
            .map_err(|_| backup_err(io::Error::other("backup fora da pasta de dados")))?;
        let parts = rel
            .components()
            .map(|c| match c {
                std::path::Component::Normal(p) => p.to_str(),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| backup_err(io::Error::other("caminho de backup ilegível")))?;
        Ok(parts.join("/"))
    }

    /// O arquivo de um caminho relativo do marcador, dentro de `<dados>`.
    fn data_path(&self, rel: &str) -> Result<PathBuf, SyncError> {
        let parts = data_parts(rel).ok_or_else(|| {
            log::warn!("sync: o marcador do backup das configurações tem um caminho que não serve");
            SyncError::BackupFailed
        })?;
        let mut path = self.dirs.data.clone();
        path.extend(parts);
        Ok(path)
    }

    /// Antes de qualquer pedido de configurações à nuvem: sessão, aparelho,
    /// sync ligado e sem pausa aqui, a conta conhecida (sem o AlvaID nada sai
    /// da máquina), e então a linha desta máquina na lista do site, lida
    /// agora (D128). Devolve o aparelho, a lista e o token.
    async fn settings_online(
        &self,
        alva: &Alvanista,
    ) -> Result<(Device, Vec<RemoteDevice>, String), SyncError> {
        let device = self.ready_device(alva).await?;
        if alva.account_id().await.is_none() {
            return Err(SyncError::Offline);
        }
        let devices = self.sync.partner_devices(alva).await?;
        self.check_row(&devices, &device.id, true).await?;
        let token = self.token()?;
        Ok((device, devices, token))
    }

    // --- enviar --------------------------------------------------------------

    /// "Enviar as desta máquina": um documento por grupo, a aparência vinda
    /// da tela, e o resultado de cada um. Um grupo que a nuvem recusa de vez
    /// fica de fora e os outros sobem; a cota ou o plano param o resto; o
    /// aparelho removido, pausado ou a rede caída acabam o envio, e os grupos
    /// que não chegaram a ir levam esse motivo. O que subiu fica guardado com
    /// o hash (o "mudou desde o envio"), e a data só anda quando algum subiu.
    /// `Err` só quando nada pôde ser tentado.
    pub async fn settings_upload(
        &self,
        alva: &Alvanista,
        appearance: &Value,
    ) -> Result<SettingsUpload, SyncError> {
        let _one = self.settings_lock.lock().await;
        let appearance = appearance_doc(appearance)?;
        let (_, _, token) = self.settings_online(alva).await?;
        let catalog = self.catalog();
        let mut docs: Vec<(&str, Value)> = Vec::with_capacity(ORDER.len());
        for group in ORDER {
            let doc = match group_by_name(group) {
                Some(g) => export(g, &self.db, &catalog).await.map_err(settings_err)?,
                None => appearance.clone(),
            };
            docs.push((group, doc));
        }
        let (_, mut hashes) = self.sync.devices.settings_sent().await.map_err(failed)?;
        let now = now_secs();
        let mut outcomes: Vec<(&str, Option<SyncError>)> = Vec::with_capacity(docs.len());
        // o que parou o envio: os grupos que faltam levam este motivo
        let mut stopped: Option<SyncError> = None;
        for (group, doc) in &docs {
            let outcome = match &stopped {
                Some(stop) => Err((stop.clone(), true)),
                None => self.send_settings(alva, &token, group, doc).await,
            };
            match outcome {
                Ok(hash) => {
                    hashes.insert((*group).to_owned(), hash);
                    outcomes.push((group, None));
                }
                Err((err, stops)) => {
                    if stops {
                        stopped = Some(err.clone());
                    }
                    outcomes.push((group, Some(err)));
                }
            }
        }
        let uploaded = outcomes.iter().filter(|(_, e)| e.is_none()).count();
        let sent_at = (uploaded > 0).then_some(now);
        if uploaded > 0 {
            self.sync
                .devices
                .set_settings_sent(now, &hashes)
                .await
                .map_err(failed)?;
        }
        log::info!(
            "sync: configurações enviadas: {uploaded} de {} grupos",
            docs.len()
        );
        let error = outcomes
            .iter()
            .filter_map(|(_, e)| e.as_ref())
            .max_by_key(|e| upload_rank(e))
            .map(SyncError::to_string);
        let groups = outcomes
            .into_iter()
            .map(|(group, e)| SettingsGroupSent {
                group: group.to_owned(),
                sent: e.is_none(),
                error: e.map(|e| e.to_string()),
            })
            .collect();
        Ok(SettingsUpload {
            sent_at,
            groups,
            error,
        })
    }

    /// Manda o documento de um grupo; devolve o hash dos bytes que subiram.
    /// No erro, o `bool` diz se ele para o envio dos grupos seguintes.
    async fn send_settings(
        &self,
        alva: &Alvanista,
        token: &str,
        group: &str,
        doc: &Value,
    ) -> Result<String, (SyncError, bool)> {
        let bytes = canonical_bytes(doc);
        let hash = sha256_hex(&bytes);
        let filename = filename_of(group);
        let key = format!("config:{filename}");
        if let Some(err) = self.still_rejected(&key, &hash) {
            return Err((err, false));
        }
        let meta = UploadMeta {
            platform: "other",
            kind: "config",
            filename: &filename,
            core: None,
            core_version: None,
            client_mtime: Some(rfc3339(now_secs())),
        };
        match self
            .upload_bytes(alva, token, &key, &meta, bytes, &hash)
            .await
        {
            Ok(_) => Ok(hash),
            Err(Fail::File(e)) => Err((e, false)),
            Err(Fail::StopUploads(e)) => {
                if e == SyncError::Quota {
                    self.quota_event().await;
                }
                Err((e, true))
            }
            Err(Fail::End(e)) => Err((e, true)),
        }
    }

    /// O último envio e os grupos cujo documento de agora não é o que subiu.
    /// Só o banco desta máquina: nem chaveiro nem rede.
    pub async fn settings_sent(&self, appearance: &Value) -> Result<SettingsSent, SyncError> {
        let (sent_at, hashes) = self.sync.devices.settings_sent().await.map_err(failed)?;
        let catalog = self.catalog();
        let mut changed = Vec::new();
        for group in ORDER {
            let doc = match group_by_name(group) {
                Some(g) => export(g, &self.db, &catalog).await.map_err(settings_err)?,
                None => appearance.clone(),
            };
            if hashes.get(group) != Some(&sha256_hex(&canonical_bytes(&doc))) {
                changed.push(group.to_owned());
            }
        }
        Ok(SettingsSent { sent_at, changed })
    }

    // --- ler as de outra máquina ---------------------------------------------

    /// As linhas de configuração vivas de outra máquina, pelo grupo. O filtro
    /// do GameKave traz também as da conta: ficam só as dela, e de cada nome
    /// a escrita por último.
    async fn config_rows(
        &self,
        alva: &Alvanista,
        token: &str,
        device_id: &str,
    ) -> Result<HashMap<&'static str, RemoteFile>, SyncError> {
        let filter = FileFilter {
            kind: "config",
            platform: Some("other"),
            device_token_id: Some(device_id),
            ..FileFilter::default()
        };
        let files = match self.sync.api.list_files(token, &filter).await {
            Ok(files) => files,
            Err(err) => return Err(self.ending(alva, err).await),
        };
        let mut rows: HashMap<&'static str, RemoteFile> = HashMap::new();
        for f in files {
            let theirs = f.device_token_id.as_deref() == Some(device_id);
            let live = f.deleted_at.is_none() && f.current.is_some();
            let no_core = f.core.as_deref().map_or(true, str::is_empty);
            if f.kind != "config" || f.platform != "other" || !theirs || !live || !no_core {
                continue;
            }
            let Some(group) = group_of(&f.filename) else {
                continue;
            };
            match rows.get(group) {
                Some(old) if written(old) >= written(&f) => {}
                _ => {
                    rows.insert(group, f);
                }
            }
        }
        Ok(rows)
    }

    /// O documento da versão atual de uma linha, conferido pelo hash. Um
    /// documento que chegou diferente do hash é `failed` (vale tentar de
    /// novo); um que não é JSON, `settings_invalid`.
    async fn config_doc(
        &self,
        alva: &Alvanista,
        token: &str,
        row: &RemoteFile,
    ) -> Result<Value, SyncError> {
        let version = row.current.as_ref().ok_or(SyncError::SettingsMissing)?;
        let bytes = match self
            .sync
            .api
            .download(token, &row.save_id, &version.version_id, version.size_bytes)
            .await
        {
            Ok(bytes) => bytes,
            // apagada no site entre a listagem e agora
            Err(ApiError::NotFound) => return Err(SyncError::SettingsMissing),
            Err(err) => return Err(self.ending(alva, err).await),
        };
        if !sha256_hex(&bytes).eq_ignore_ascii_case(&version.content_hash) {
            return Err(failed(
                "um documento de configurações não confere com o hash da nuvem".into(),
            ));
        }
        serde_json::from_slice(&bytes).map_err(|_| SyncError::SettingsInvalid)
    }

    /// O que a cópia das configurações de `device_id` traria. Nada muda: os
    /// documentos entram numa transação desfeita no fim. Um documento que não
    /// serve marca só o grupo dele (`error`). `here_hw`: o hardware desta
    /// máquina agora (sem GPU conhecida, vale o que a lista do site tem dela).
    pub async fn settings_preview(
        &self,
        alva: &Alvanista,
        device_id: &str,
        here_hw: &HardwarePayload,
    ) -> Result<SettingsPreview, SyncError> {
        let _one = self.settings_lock.lock().await;
        let (me, devices, token) = self.settings_online(alva).await?;
        let other = devices
            .iter()
            .find(|d| d.id == device_id)
            .cloned()
            .ok_or(SyncError::SettingsMissing)?;
        let rows = self.config_rows(alva, &token, device_id).await?;
        let mut errors: HashMap<&str, String> = HashMap::new();
        let mut checked: Vec<Checked> = Vec::new();
        for group in ORDER {
            let Some(row) = rows.get(group) else {
                continue;
            };
            let doc = match self.config_doc(alva, &token, row).await {
                Ok(doc) => doc,
                // do documento, não da máquina: só este grupo fica de fora
                Err(
                    e @ (SyncError::SettingsInvalid
                    | SyncError::SettingsMissing
                    | SyncError::Failed(_)),
                ) => {
                    errors.insert(group, e.to_string());
                    continue;
                }
                Err(e) => return Err(e),
            };
            let verdict = match group_by_name(group) {
                Some(g) => check(g, &doc, ApplyMode::Merge).map(|c| checked.push(c)),
                None => appearance_doc(&doc).map(|_| ()).map_err(|e| e.to_string()),
            };
            if let Err(e) = verdict {
                errors.insert(group, settings_err(e).to_string());
            }
        }
        let here = self.here().await?;
        let reports = dry_run(&self.db, checked, &here.view())
            .await
            .map_err(settings_err)?;
        let mine = if here_hw.gpu.trim().is_empty() || here_hw.gpu_backend.trim().is_empty() {
            devices
                .iter()
                .find(|d| d.id == me.id)
                .and_then(|d| d.hardware.clone())
        } else {
            Some(here_hw.clone())
        };
        let video_default_off = !same_gpu(other.hardware.as_ref(), mine.as_ref());
        let here_gpu = mine.as_ref().map_or_else(String::new, |h| h.gpu.clone());
        let here_backend = mine
            .as_ref()
            .map_or_else(String::new, |h| h.gpu_backend.clone());
        let sent_at = rows
            .values()
            .map(|r| written(r).0)
            .filter(|t| *t != i64::MIN)
            .max()
            .unwrap_or(0);
        let groups = ORDER
            .iter()
            .map(|group| SettingsGroupPreview {
                group: (*group).to_owned(),
                present: rows.contains_key(*group),
                error: errors.get(*group).cloned(),
                missing_files: reports
                    .iter()
                    .find(|(g, _)| g.as_str() == *group)
                    .map(|(_, r)| r.missing_files.clone())
                    .unwrap_or_default(),
            })
            .collect();
        Ok(SettingsPreview {
            machine: SyncMachine::from_remote(other, Some(&me.id)),
            sent_at,
            groups,
            video_default_off,
            here_gpu,
            here_backend,
        })
    }

    // --- copiar e desfazer ---------------------------------------------------

    /// Copia os grupos marcados de `device_id` (D114). Recusa com um jogo
    /// aberto ou abrindo. Todos os documentos são baixados e conferidos antes
    /// de qualquer escrita; a aparência volta para a tela aplicar.
    pub async fn settings_copy(
        &self,
        alva: &Alvanista,
        device_id: &str,
        groups: &[String],
        appearance: bool,
    ) -> Result<CopyReport, SyncError> {
        let ticked = ticked(groups)?;
        if ticked.is_empty() && !appearance {
            return Err(SyncError::SettingsInvalid);
        }
        let _one = self.settings_lock.lock().await;
        self.settings_idle()?;
        let (_, devices, token) = self.settings_online(alva).await?;
        let rows = self.config_rows(alva, &token, device_id).await?;
        let mut docs: Vec<(Group, Value)> = Vec::with_capacity(ticked.len());
        for group in &ticked {
            let row = rows.get(group.as_str()).ok_or(SyncError::SettingsMissing)?;
            docs.push((*group, self.config_doc(alva, &token, row).await?));
        }
        let appearance_doc = match appearance {
            true => {
                let row = rows.get(APPEARANCE).ok_or(SyncError::SettingsMissing)?;
                Some(appearance_doc(&self.config_doc(alva, &token, row).await?)?)
            }
            false => None,
        };
        let refs: Vec<(Group, &Value)> = docs.iter().map(|(g, d)| (*g, d)).collect();
        let checked = check_all(&refs, ApplyMode::Merge).map_err(settings_err)?;
        let source = devices.iter().find(|d| d.id == device_id);
        let marker = CopyBackup {
            machine: source.map(|d| d.label.clone()).unwrap_or_default(),
            machine_id: Some(device_id.to_owned()),
            copied_at: 0,
            groups: ORDER
                .into_iter()
                .filter(|g| match group_by_name(g) {
                    Some(group) => ticked.contains(&group),
                    None => appearance,
                })
                .map(str::to_owned)
                .collect(),
            files: BTreeMap::new(),
        };
        let (per_group, backup_dir, copied_at) = self.copy_here(checked, marker).await?;
        (self.reload)().await;
        Ok(CopyReport {
            per_group,
            backup_dir,
            appearance_doc,
            copied_at,
        })
    }

    /// A metade local da cópia, numa transação de escrita: o backup do que
    /// muda vai ao disco primeiro, os grupos entram, o marcador do "Desfazer"
    /// entra junto, e só então a transação fica. Um jogo que começou a abrir
    /// no meio, um backup que não grava ou um grupo que falha, e nada fica.
    /// Devolve os relatórios, a pasta do backup e o instante do marcador.
    async fn copy_here(
        &self,
        checked: Vec<Checked>,
        mut marker: CopyBackup,
    ) -> Result<(Vec<SettingsGroupReport>, Option<String>, i64), SyncError> {
        self.settings_idle()?;
        let (mut tx, here) = self.open_write().await?;
        // o backup é o que esta transação vai trocar
        let ticked: Vec<Group> = checked.iter().map(Checked::group).collect();
        let stamp = local_stamp();
        let files = self
            .back_up_groups(&mut tx, &here.catalog, &stamp, &ticked)
            .await?;
        let mut reports = Vec::with_capacity(checked.len());
        for c in checked {
            let group = c.group();
            let report = apply_in(&mut tx, c, &here.view())
                .await
                .map_err(settings_err)?;
            reports.push((group, report));
        }
        let backup_dir = (!files.is_empty()).then(|| {
            self.dirs
                .data
                .join("sync-backups")
                .join(&stamp)
                .to_string_lossy()
                .into_owned()
        });
        marker.files = files;
        marker.copied_at = now_secs();
        let copied_at = marker.copied_at;
        let record = serde_json::to_string(&marker)
            .map_err(|e| failed(format!("marcador da cópia: {e}")))?;
        tx.set_state(LAST_COPY_BACKUP_KEY, &record)
            .await
            .map_err(db_err)?;
        self.settings_idle()?;
        tx.commit().await.map_err(db_err)?;
        log_applied(&reports, ApplyMode::Merge);
        let per_group = reports
            .into_iter()
            .map(|(group, report)| SettingsGroupReport {
                group: group.as_str().to_owned(),
                report,
            })
            .collect();
        Ok((per_group, backup_dir, copied_at))
    }

    /// A transação de escrita das configurações e o que esta máquina tem,
    /// lido nela.
    async fn open_write(&self) -> Result<(SettingsTx, Here), SyncError> {
        let catalog = self.catalog();
        let cores = self.installed_cores().await;
        let mut tx = SettingsRepo::new(self.db.clone())
            .write()
            .await
            .map_err(db_err)?;
        let here = Here {
            roms: tx.rom_crc32s().await.map_err(db_err)?,
            cores,
            catalog,
        };
        Ok((tx, here))
    }

    /// O estado de agora dos grupos, na forma do backup (os jogos pela ROM),
    /// lido na transação de quem chama e gravado em
    /// `<dados>/sync-backups/<carimbo>/settings/<grupo>.json` no disco
    /// (`fsync`). Devolve onde ficou cada um, relativo a `<dados>`.
    async fn back_up_groups(
        &self,
        tx: &mut SettingsTx,
        catalog: &Catalog,
        stamp: &str,
        groups: &[Group],
    ) -> Result<BTreeMap<String, String>, SyncError> {
        let mut jobs = Vec::with_capacity(groups.len());
        for group in groups {
            let doc = export_backup_in(tx, *group, catalog)
                .await
                .map_err(settings_err)?;
            let original = self
                .dirs
                .data
                .join("settings")
                .join(format!("{}.json", group.as_str()));
            let target = self
                .backup_target_in(stamp, &original)
                .map_err(backup_err)?;
            let bytes =
                serde_json::to_vec_pretty(&doc).map_err(|e| backup_err(io::Error::other(e)))?;
            let rel = self.data_relative(&target)?;
            jobs.push((group.as_str().to_owned(), target, rel, bytes));
        }
        let written = tokio::task::spawn_blocking(move || {
            let mut files = BTreeMap::new();
            for (group, target, rel, bytes) in jobs {
                write_atomic(&target, &bytes)?;
                files.insert(group, rel);
            }
            Ok::<_, io::Error>(files)
        })
        .await
        .unwrap_or_else(|j| Err(io::Error::other(j.to_string())));
        written.map_err(backup_err)
    }

    /// O marcador da última cópia; `None` sem cópia a desfazer. Um marcador
    /// ilegível é erro (o "Desfazer" não teria o que voltar).
    async fn copy_marker(&self) -> Result<Option<CopyBackup>, SyncError> {
        let Some(raw) = self.sync.devices.last_copy_backup().await.map_err(failed)? else {
            return Ok(None);
        };
        serde_json::from_str(&raw).map(Some).map_err(|_| {
            log::warn!("sync: o marcador do backup das configurações está ilegível");
            SyncError::BackupFailed
        })
    }

    /// A cópia que o "Desfazer" volta, para a tela oferecê-lo também depois
    /// de abrir o app de novo: de qual máquina, quando e quais grupos. Só o
    /// marcador no banco: nem chaveiro nem rede.
    pub async fn settings_undo_info(&self) -> Result<Option<SettingsUndoInfo>, SyncError> {
        Ok(self.copy_marker().await?.map(|m| SettingsUndoInfo {
            machine: m.machine,
            machine_id: m.machine_id,
            copied_at: m.copied_at,
            groups: m.groups,
        }))
    }

    /// "Desfazer": volta o backup da última cópia (`Restore`, os jogos pela
    /// ROM). Antes, o estado de agora dos mesmos grupos vai para um carimbo
    /// novo, na mesma transação (o que a pessoa mudou depois da cópia fica
    /// guardado, R39); o marcador sai e o registro desse backup entra. `false`
    /// quando não há cópia a desfazer. Recusa com um jogo aberto ou abrindo.
    /// Um backup que sumiu ou não confere é `backup_failed`, e nada muda. Sem
    /// rede: o backup é desta máquina e vale com o sync desligado.
    pub async fn settings_undo(&self) -> Result<bool, SyncError> {
        let _one = self.settings_lock.lock().await;
        self.settings_idle()?;
        let Some(marker) = self.copy_marker().await? else {
            return Ok(false);
        };
        let mut paths = Vec::with_capacity(marker.files.len());
        for (name, rel) in &marker.files {
            let group = group_by_name(name).ok_or(SyncError::BackupFailed)?;
            paths.push((group, self.data_path(rel)?));
        }
        let read = tokio::task::spawn_blocking(move || {
            paths
                .into_iter()
                .map(|(group, path)| {
                    let bytes = std::fs::read(&path)?;
                    let doc: Value = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
                    Ok((group, doc))
                })
                .collect::<io::Result<Vec<(Group, Value)>>>()
        })
        .await
        .unwrap_or_else(|j| Err(io::Error::other(j.to_string())));
        let docs = read.map_err(backup_err)?;
        let refs: Vec<(Group, &Value)> = docs.iter().map(|(g, d)| (*g, d)).collect();
        let checked = check_all(&refs, ApplyMode::Restore).map_err(|e| {
            log::warn!("sync: o backup das configurações não confere ({e})");
            SyncError::BackupFailed
        })?;
        let (mut tx, here) = self.open_write().await?;
        // o de agora, antes de voltar o de antes da cópia
        let groups: Vec<Group> = checked.iter().map(Checked::group).collect();
        let saved = self
            .back_up_groups(&mut tx, &here.catalog, &local_stamp(), &groups)
            .await?;
        let mut reports = Vec::with_capacity(checked.len());
        for c in checked {
            let group = c.group();
            let report = apply_in(&mut tx, c, &here.view())
                .await
                .map_err(settings_err)?;
            reports.push((group, report));
        }
        let record = serde_json::to_string(&UndoBackup {
            undone_at: now_secs(),
            files: saved,
        })
        .map_err(|e| failed(format!("registro do desfazer: {e}")))?;
        tx.set_state(LAST_UNDO_BACKUP_KEY, &record)
            .await
            .map_err(db_err)?;
        tx.delete_state(LAST_COPY_BACKUP_KEY)
            .await
            .map_err(db_err)?;
        self.settings_idle()?;
        tx.commit().await.map_err(db_err)?;
        log_applied(&reports, ApplyMode::Restore);
        (self.reload)().await;
        Ok(true)
    }

    /// "Esquecer" a última cópia quando o "Desfazer" dela não volta (o backup
    /// sumiu ou não confere): só o marcador sai. Os arquivos ficam na pasta de
    /// resguardo, nenhuma configuração muda e nada é relido. Sem rede.
    pub async fn settings_undo_forget(&self) -> Result<(), SyncError> {
        let _one = self.settings_lock.lock().await;
        let mut tx = SettingsRepo::new(self.db.clone())
            .write()
            .await
            .map_err(db_err)?;
        tx.delete_state(LAST_COPY_BACKUP_KEY)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        counting_reload, device, dirs_in, field, files_body, mount_content, mount_devices,
        mount_upload, rig, rig_bare, sha, uploaded, Rig, SAVES,
    };
    use super::*;
    use crate::alvanista::store::test_support::SERIAL;
    use crate::sync::device::LAST_UNDO_BACKUP_KEY;
    use domain::library::Rom;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};
    // --- configurações entre máquinas (D114, R36, R38) -----------------------

    use crate::sync::settings_groups::{self as groups, Catalog, Group};

    /// O aparelho da outra máquina da conta.
    const OTHER: &str = "dev2";
    /// Uma chave de opção que não pode aparecer em log nenhum.
    const SECRET_KEY: &str = "snes9x_chave_de_teste_4_2";
    /// Um valor de lá que também não pode aparecer em log.
    const THEIR_VALUE: &str = "valor-de-la-4-2";
    const MAC: &str = "ppsspp_change_mac_address01";

    fn appearance() -> Value {
        json!({
            "v": 1,
            "themeSelection": "{\"selection\":{\"id\":\"dark\"}}",
            "uiScale": 1.0,
            "overscan": 0,
            "uiSounds": true,
            "reduceMotion": false
        })
    }

    /// Uma linha da lista de aparelhos com o hardware.
    fn machine_row(id: &str, label: &str, gpu: &str, backend: &str) -> Value {
        let mut d = device(id, label);
        d["hardware"] = json!({"os": "Windows 11", "gpu": gpu, "gpu_backend": backend});
        d
    }

    /// Uma linha `config` de outra máquina, com os bytes do documento.
    fn config_row(
        save_id: &str,
        group: &str,
        doc: &Value,
        owner: Option<&str>,
        at: &str,
    ) -> (Value, Vec<u8>) {
        let bytes = serde_json::to_vec(doc).unwrap();
        let row = json!({
            "save_id": save_id, "filename": format!("reemu/{group}.json"), "slug": null,
            "kind": "config", "platform": "other", "game_uuid": null, "core": null,
            "core_version": null, "visibility": "private",
            "current": {
                "version_id": format!("v-{save_id}"), "content_hash": sha(&bytes),
                "size_bytes": bytes.len(), "server_created_at": at, "client_mtime": at,
                "source_client": "reemu", "origin_media": "upload", "device_token_id": owner
            },
            "version_count": 1, "deleted_at": null, "device_token_id": owner
        });
        (row, bytes)
    }

    /// As linhas `config` que a listagem com `device_token_id=OTHER` devolve
    /// (o GameKave junta as da conta) e o conteúdo de cada uma.
    async fn mount_configs(server: &MockServer, rows: Vec<(Value, Vec<u8>)>) {
        let mut listed = Vec::new();
        for (row, bytes) in rows {
            let id = row["save_id"].as_str().unwrap().to_owned();
            mount_content(server, &id, &format!("v-{id}"), &bytes).await;
            listed.push(row);
        }
        Mock::given(method("GET"))
            .and(path(SAVES))
            .and(query_param("kind", "config"))
            .and(query_param("platform", "other"))
            .and(query_param("device_token_id", OTHER))
            .respond_with(files_body(listed))
            .mount(server)
            .await;
    }

    /// A outra máquina: um banco à parte, com o que cada teste põe nele.
    struct There {
        db: db::Db,
        catalog: Catalog,
    }

    impl There {
        async fn new(r: &Rig) -> Self {
            There {
                db: db::connect_in_memory().await.unwrap(),
                catalog: Catalog::new(&r.path("ali/shaders"), &r.path("ali/decorations")),
            }
        }

        async fn doc(&self, group: Group) -> Value {
            groups::export(group, &self.db, &self.catalog)
                .await
                .unwrap()
        }

        /// As linhas dos grupos que ela enviou.
        async fn rows(&self, list: &[Group]) -> Vec<(Value, Vec<u8>)> {
            let mut out = Vec::new();
            for (n, g) in list.iter().enumerate() {
                let doc = self.doc(*g).await;
                out.push(config_row(
                    &format!("c{n}"),
                    g.as_str(),
                    &doc,
                    Some(OTHER),
                    "2026-11-13T10:00:00Z",
                ));
            }
            out
        }
    }

    async fn rom_crc(db: &db::Db, id: &str, crc32: &str) {
        RomsRepo::new(db.clone())
            .add(&Rom {
                id: id.into(),
                file_path: format!("C:/roms/{id}.sfc"),
                crc32: crc32.into(),
                md5: "MD5".into(),
                system_id: "snes".into(),
                added_at: 0,
                last_played_at: None,
                is_favorite: false,
                user_title: None,
            })
            .await
            .unwrap();
    }

    async fn opt(db: &db::Db, core: &str, rom: Option<&str>, key: &str, value: &str) {
        use domain::core_options::CoreOptionsStore;
        db::CoreOptionsRepo::new(db.clone())
            .set_scoped_value(core, rom, key, Some(value))
            .await
            .unwrap();
    }

    async fn aspect(db: &db::Db, scope: &str, target: &str, value: &str) {
        db::AspectRepo::new(db.clone())
            .set(scope, target, Some(value))
            .await
            .unwrap();
    }

    async fn audio(db: &db::Db, rate_control: bool, device: &str) {
        use domain::audio::{AudioConfig, AudioConfigRepository};
        db::AudioConfigRepo::new(db.clone())
            .update(&AudioConfig {
                output_device_id: Some(device.into()),
                output_device_name: Some(format!("Saída {device}")),
                rate_control_enabled: rate_control,
                rate_control_delta: 0.25,
                sample_rate_preference: Some(48000),
            })
            .await
            .unwrap();
    }

    async fn video(db: &db::Db, integer: bool) {
        use domain::video::{VideoConfig, VideoConfigRepository};
        db::VideoConfigRepo::new(db.clone())
            .update(&VideoConfig {
                integer_scaling: integer,
                hdr: false,
                hdr_paper_white_nits: 120,
                hdr_highlights: false,
                hdr_peak_nits: 600,
            })
            .await
            .unwrap();
    }

    /// O valor de uma opção global (o MAC não sai em documento nenhum).
    async fn option_value(db: &db::Db, key: &str) -> Option<String> {
        sqlx::query_scalar("SELECT value FROM core_options_values WHERE option_key = ?1")
            .bind(key)
            .fetch_optional(db)
            .await
            .unwrap()
    }

    /// Cada linha das tabelas que os grupos escrevem, como texto.
    async fn settings_rows(db: &db::Db) -> Vec<String> {
        let mut out = Vec::new();
        for table in [
            "core_options_values",
            "core_option_overrides",
            "system_core_prefs",
            "shader_chain_assignments",
            "shader_parameter_overrides",
            "decoration_assignments",
            "system_hotkeys",
            "keyboard_bindings",
            "audio_config",
            "video_config",
            "aspect_assignments",
        ] {
            let cols: Vec<String> = sqlx::query_scalar(&format!(
                "SELECT name FROM pragma_table_info('{table}') ORDER BY cid"
            ))
            .fetch_all(db)
            .await
            .unwrap();
            let expr = cols
                .iter()
                .map(|c| format!("quote({c})"))
                .collect::<Vec<_>>()
                .join(" || '|' || ");
            let rows: Vec<String> =
                sqlx::query_scalar(&format!("SELECT {expr} FROM {table} ORDER BY 1"))
                    .fetch_all(db)
                    .await
                    .unwrap();
            out.extend(rows.into_iter().map(|row| format!("{table}: {row}")));
        }
        out
    }

    async fn copy_marker(r: &Rig) -> Option<String> {
        r.sync.devices.last_copy_backup().await.unwrap()
    }

    fn ticks(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    impl Rig {
        fn reload_count(&self) -> usize {
            self.reloads.load(SeqCst)
        }

        async fn copy(&self, list: &[&str], appearance: bool) -> Result<CopyReport, SyncError> {
            self.engine
                .settings_copy(&self.alva, OTHER, &ticks(list), appearance)
                .await
        }
    }

    /// D114: um documento por grupo, cada um numa linha `config` deste
    /// aparelho, `platform=other`, sem core, `reemu/<grupo>.json`; a aparência
    /// é a que a tela mandou. O envio fica guardado com o hash de cada grupo,
    /// e um grupo mudado depois aparece como mudado.
    #[tokio::test]
    async fn settings_upload_writes_one_row_per_group() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-upload").await;
        opt(&r.db, "snes9x", None, "snes9x_region", "pal").await;
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        mount_upload(&r.server, "c1", "v1").await;

        let upload = r
            .engine
            .settings_upload(&r.alva, &appearance())
            .await
            .unwrap();
        let at = upload.sent_at.unwrap();
        assert_eq!(upload.error, None);
        let results: Vec<(&str, bool)> = upload
            .groups
            .iter()
            .map(|g| (g.group.as_str(), g.sent && g.error.is_none()))
            .collect();
        assert_eq!(
            results,
            [
                ("core-options", true),
                ("shaders-decorations", true),
                ("hotkeys-keyboard", true),
                ("audio", true),
                ("appearance", true),
                ("video", true),
            ]
        );

        let posts = r.reqs("POST", SAVES).await;
        let mut names: Vec<String> = posts
            .iter()
            .map(|p| field(&p.body, "filename").unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "reemu/appearance.json",
                "reemu/audio.json",
                "reemu/core-options.json",
                "reemu/hotkeys-keyboard.json",
                "reemu/shaders-decorations.json",
                "reemu/video.json",
            ]
        );
        let catalog = Catalog::new(&r.path("shaders"), &r.path("decorations"));
        for p in &posts {
            let auth = p.headers.get("authorization").unwrap();
            assert_eq!(auth.to_str().unwrap(), "Bearer asv_secret");
            assert_eq!(field(&p.body, "kind").as_deref(), Some("config"));
            assert_eq!(field(&p.body, "platform").as_deref(), Some("other"));
            assert_eq!(
                field(&p.body, "core"),
                None,
                "sem core: o GameKave guarda vazio como nenhum"
            );
            assert_eq!(field(&p.body, "core_version"), None);
            let name = field(&p.body, "filename").unwrap();
            let sent: Value = serde_json::from_str(&field(&p.body, "file").unwrap()).unwrap();
            let group = name.trim_start_matches("reemu/").trim_end_matches(".json");
            let want = match Group::ALL.into_iter().find(|g| g.as_str() == group) {
                Some(g) => groups::export(g, &r.db, &catalog).await.unwrap(),
                None => appearance(),
            };
            assert_eq!(sent, want, "{name}");
        }

        let (sent_at, hashes) = r.sync.devices.settings_sent().await.unwrap();
        assert_eq!(sent_at, Some(at));
        assert_eq!(hashes.len(), 6);
        let state = r.engine.settings_sent(&appearance()).await.unwrap();
        assert_eq!((state.sent_at, state.changed.len()), (Some(at), 0));
        // mudou uma opção e a aparência depois do envio
        opt(&r.db, "snes9x", None, "snes9x_region", "ntsc").await;
        let mut look = appearance();
        look["uiScale"] = json!(1.3);
        let state = r.engine.settings_sent(&look).await.unwrap();
        assert_eq!(state.changed, ["core-options", "appearance"]);
    }

    /// O resultado de um grupo no envio: subiu, ou o código do que o deixou.
    fn sent_of(upload: &SettingsUpload) -> Vec<(&str, bool, Option<&str>)> {
        upload
            .groups
            .iter()
            .map(|g| (g.group.as_str(), g.sent, g.error.as_deref()))
            .collect()
    }

    /// As regras de envio das passadas valem aqui, com o resultado de cada
    /// grupo: uma recusa de vez (413) fica lembrada para aqueles bytes e os
    /// outros grupos sobem; o 402 da cota não é o do plano, e o plano vem
    /// antes de uma recusa lembrada no erro que a tela mostra.
    #[tokio::test]
    async fn settings_upload_follows_the_pass_rules() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-upload-rules").await;
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        struct TooBigVideo;
        impl Respond for TooBigVideo {
            fn respond(&self, req: &Request) -> ResponseTemplate {
                if field(&req.body, "filename").as_deref() == Some("reemu/video.json") {
                    ResponseTemplate::new(413)
                        .set_body_json(json!({"error": {"code": "save_file_too_large"}}))
                } else {
                    uploaded("c1", "v1")
                }
            }
        }
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(TooBigVideo)
            .mount(&r.server)
            .await;
        let video_posts = |posts: &[Request]| {
            posts
                .iter()
                .filter(|p| field(&p.body, "filename").as_deref() == Some("reemu/video.json"))
                .count()
        };

        let first = r
            .engine
            .settings_upload(&r.alva, &appearance())
            .await
            .unwrap();
        assert_eq!(first.error.as_deref(), Some("too_large"));
        assert!(first.sent_at.is_some(), "os outros grupos subiram");
        assert_eq!(
            sent_of(&first),
            [
                ("core-options", true, None),
                ("shaders-decorations", true, None),
                ("hotkeys-keyboard", true, None),
                ("audio", true, None),
                ("appearance", true, None),
                ("video", false, Some("too_large")),
            ]
        );
        assert_eq!(r.reqs("POST", SAVES).await.len(), 6);
        let (_, hashes) = r.sync.devices.settings_sent().await.unwrap();
        assert!(hashes.contains_key("core-options") && !hashes.contains_key("video"));
        // os mesmos bytes do vídeo não sobem de novo
        let again = r
            .engine
            .settings_upload(&r.alva, &appearance())
            .await
            .unwrap();
        assert_eq!(again.groups[5].error.as_deref(), Some("too_large"));
        assert_eq!(video_posts(&r.reqs("POST", SAVES).await), 1);

        // 402 de cota: para no primeiro; 402 da porta: é o plano
        for (quota, code) in [
            ("emu_saves_quota_mb", SyncError::Quota),
            ("save_sync_enabled", SyncError::NoTier),
        ] {
            let r = rig(&format!("settings-402-{quota}")).await;
            mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
            Mock::given(method("POST"))
                .and(path(SAVES))
                .respond_with(ResponseTemplate::new(402).set_body_json(json!({"error": {
                    "code": "tier_insufficient", "required_quota": quota, "upgrade_to": "collector"
                }})))
                .mount(&r.server)
                .await;
            let got = r
                .engine
                .settings_upload(&r.alva, &appearance())
                .await
                .unwrap();
            let code = code.to_string();
            assert_eq!(got.error.as_deref(), Some(code.as_str()), "{quota}");
            assert_eq!(got.sent_at, None, "{quota}");
            // parou no primeiro: os outros levam o mesmo motivo
            assert!(
                got.groups
                    .iter()
                    .all(|g| !g.sent && g.error.as_deref() == Some(code.as_str())),
                "{quota}: {got:?}"
            );
            assert_eq!(r.reqs("POST", SAVES).await.len(), 1, "{quota}");
            assert_eq!(r.sync.devices.settings_sent().await.unwrap().0, None);
        }
    }

    /// R39: um grupo recusado de vez antes (lembrado, nem vai à rede) e depois
    /// o 402 da porta nos outros: o erro que a tela mostra é o do plano, para
    /// a oferta do plano aparecer, e cada grupo diz o seu.
    #[tokio::test]
    async fn the_plan_gate_wins_over_a_remembered_rejection() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-upload-gate").await;
        mount_devices(&r.server, vec![device("dev1", "Sala")]).await;
        let phase = Arc::new(AtomicUsize::new(0));
        struct Gate(Arc<AtomicUsize>);
        impl Respond for Gate {
            fn respond(&self, req: &Request) -> ResponseTemplate {
                let name = field(&req.body, "filename");
                match self.0.load(SeqCst) {
                    0 if name.as_deref() == Some("reemu/core-options.json") => {
                        ResponseTemplate::new(413)
                            .set_body_json(json!({"error": {"code": "save_file_too_large"}}))
                    }
                    0 => uploaded("c1", "v1"),
                    _ => ResponseTemplate::new(402).set_body_json(json!({"error": {
                        "code": "tier_insufficient", "required_quota": "save_sync_enabled",
                        "upgrade_to": "collector"
                    }})),
                }
            }
        }
        Mock::given(method("POST"))
            .and(path(SAVES))
            .respond_with(Gate(phase.clone()))
            .mount(&r.server)
            .await;
        let first = r
            .engine
            .settings_upload(&r.alva, &appearance())
            .await
            .unwrap();
        assert_eq!(first.error.as_deref(), Some("too_large"));

        // o plano acabou: a recusa lembrada das opções nem vai à rede
        phase.store(1, SeqCst);
        let gated = r
            .engine
            .settings_upload(&r.alva, &appearance())
            .await
            .unwrap();
        assert_eq!(gated.error.as_deref(), Some("no_tier"));
        assert_eq!(
            sent_of(&gated),
            [
                ("core-options", false, Some("too_large")),
                ("shaders-decorations", false, Some("no_tier")),
                ("hotkeys-keyboard", false, Some("no_tier")),
                ("audio", false, Some("no_tier")),
                ("appearance", false, Some("no_tier")),
                ("video", false, Some("no_tier")),
            ]
        );
        assert_eq!(gated.sent_at, None);
        assert_eq!(
            r.sync.devices.settings_sent().await.unwrap().0,
            first.sent_at,
            "a data do envio fica a do que subiu"
        );
    }

    /// Pausada no site ou com a conta desconhecida, nada vai à nuvem (D128,
    /// N1): a pausa é lida na linha desta máquina, e sem o AlvaID nem a lista
    /// de aparelhos é pedida.
    #[tokio::test]
    async fn settings_traffic_needs_the_site_row_and_the_account() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-paused").await;
        let mut row = device("dev1", "Sala");
        row["paused_at"] = json!("2026-11-13T09:00:00Z");
        mount_devices(&r.server, vec![row, device(OTHER, "Notebook")]).await;
        assert_eq!(
            r.engine.settings_upload(&r.alva, &appearance()).await,
            Err(SyncError::Paused)
        );
        assert_eq!(r.copy(&["audio"], false).await, Err(SyncError::Paused));
        assert!(r.reqs("POST", SAVES).await.is_empty());
        assert!(r.reqs("GET", SAVES).await.is_empty());

        let r = rig_bare("settings-no-account").await;
        assert_eq!(
            r.engine.settings_upload(&r.alva, &appearance()).await,
            Err(SyncError::Offline)
        );
        assert_eq!(r.copy(&["audio"], false).await, Err(SyncError::Offline));
        assert!(r.server.received_requests().await.unwrap().is_empty());
    }

    /// A prévia lê só as linhas da outra máquina (as da conta ficam de fora),
    /// diz o que veio e o que não viria, e desmarca o vídeo quando a GPU ou o
    /// backend dela diferem dos daqui. Nada muda.
    #[tokio::test]
    async fn preview_flags_video_when_gpu_differs() {
        let _serial = SERIAL.lock().await;
        use domain::shader_chain::ShaderChainStore;
        let r = rig("settings-preview").await;
        mount_devices(
            &r.server,
            vec![
                // o que esta máquina mandou ao site: a mesma GPU da outra
                machine_row("dev1", "Sala", "Intel Iris Xe", "Vulkan"),
                machine_row(OTHER, "Notebook", "Intel Iris Xe", "Vulkan"),
            ],
        )
        .await;
        let there = There::new(&r).await;
        opt(&there.db, "snes9x", None, "snes9x_region", "ntsc").await;
        // um preset importado do disco lá, na plataforma do GBA
        let mine = r.path("ali/Meus Shaders/crt-do-ed.slangp");
        let mine = mine.to_string_lossy().into_owned();
        db::ShaderChainRepo::new(there.db.clone())
            .upsert_preset(&domain::shader_chain::ShaderPreset {
                id: mine.clone(),
                name: "crt-do-ed".into(),
                source_path: mine.clone(),
                format: domain::shader_chain::ShaderFormat::Slang,
                is_builtin: false,
                includes_bezel: false,
            })
            .await
            .unwrap();
        db::ShaderChainRepo::new(there.db.clone())
            .set_assignment(
                domain::shader_chain::AssignmentScope::System,
                Some("gba"),
                None,
                &mine,
            )
            .await
            .unwrap();
        let mut rows = there
            .rows(&[Group::CoreOptions, Group::ShadersDecorations, Group::Video])
            .await;
        let (newest, _) = &mut rows[2];
        newest["current"]["server_created_at"] = json!("2026-11-13T12:00:00Z");
        rows.push(config_row(
            "c-app",
            "appearance",
            &appearance(),
            Some(OTHER),
            "2026-11-13T10:00:00Z",
        ));
        // da conta (sem dono): o filtro do GameKave a traz, a prévia não
        rows.push(config_row(
            "c-acct",
            "audio",
            &json!({"v": 1}),
            None,
            "2026-11-13T13:00:00Z",
        ));
        mount_configs(&r.server, rows).await;
        let before = settings_rows(&r.db).await;

        let rtx = |gpu: &str, backend: &str| HardwarePayload {
            gpu: gpu.into(),
            gpu_backend: backend.into(),
            ..HardwarePayload::default()
        };
        let p = r
            .engine
            .settings_preview(&r.alva, OTHER, &rtx("NVIDIA GeForce RTX 4070", "Vulkan"))
            .await
            .unwrap();
        assert!(p.video_default_off);
        // a comparação mostra a GPU e o backend que decidiram
        assert_eq!(
            (p.here_gpu.as_str(), p.here_backend.as_str()),
            ("NVIDIA GeForce RTX 4070", "Vulkan")
        );
        assert_eq!(
            (p.machine.id.as_str(), p.machine.label.as_str()),
            (OTHER, "Notebook")
        );
        assert!(!p.machine.is_this);
        assert_eq!(p.sent_at, parse_time("2026-11-13T12:00:00Z").unwrap().0);
        let present: Vec<(&str, bool)> = p
            .groups
            .iter()
            .map(|g| (g.group.as_str(), g.present))
            .collect();
        assert_eq!(
            present,
            [
                ("core-options", true),
                ("shaders-decorations", true),
                ("hotkeys-keyboard", false),
                ("audio", false),
                ("appearance", true),
                ("video", true),
            ]
        );
        let shaders = &p.groups[1];
        assert_eq!(shaders.error, None);
        assert_eq!(shaders.missing_files.len(), 1);
        let missing = &shaders.missing_files[0];
        assert_eq!(
            (
                missing.kind,
                missing.name.as_str(),
                missing.system_id.as_deref()
            ),
            (
                groups::MissingKind::ImportedElsewhere,
                "crt-do-ed",
                Some("gba")
            )
        );
        assert_eq!(settings_rows(&r.db).await, before, "a prévia não muda nada");

        // mesma GPU no mesmo backend: o vídeo vem marcado
        let same = r
            .engine
            .settings_preview(&r.alva, OTHER, &rtx("Intel Iris Xe", "vulkan"))
            .await
            .unwrap();
        assert!(!same.video_default_off);
        // mesma GPU em outro backend: desmarcado
        let other_backend = r
            .engine
            .settings_preview(&r.alva, OTHER, &rtx("Intel Iris Xe", "Direct3D 12"))
            .await
            .unwrap();
        assert!(other_backend.video_default_off);
        // sem a GPU daqui agora, vale a da lista do site (a mesma Iris Xe
        // em Vulkan): marcado
        let unknown = r
            .engine
            .settings_preview(&r.alva, OTHER, &HardwarePayload::default())
            .await
            .unwrap();
        assert!(
            !unknown.video_default_off,
            "a linha do site vale sem a GPU daqui"
        );
        assert_eq!(
            (unknown.here_gpu.as_str(), unknown.here_backend.as_str()),
            ("Intel Iris Xe", "Vulkan")
        );
    }

    /// Uma cópia típica: opções e áudio daqui, vídeo de lá não marcado.
    async fn copy_rig(name: &str) -> (Rig, There) {
        let r = rig(name).await;
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device(OTHER, "Notebook")],
        )
        .await;
        opt(&r.db, "snes9x", None, "snes9x_region", "pal").await;
        opt(&r.db, "snes9x", None, SECRET_KEY, "daqui").await;
        audio(&r.db, false, "fone-daqui").await;
        video(&r.db, false).await;
        let there = There::new(&r).await;
        opt(&there.db, "snes9x", None, "snes9x_region", "ntsc").await;
        opt(&there.db, "snes9x", None, SECRET_KEY, THEIR_VALUE).await;
        audio(&there.db, true, "caixa-de-la").await;
        video(&there.db, true).await;
        let mut rows = there
            .rows(&[Group::CoreOptions, Group::Audio, Group::Video])
            .await;
        rows.push(config_row(
            "c-app",
            "appearance",
            &appearance(),
            Some(OTHER),
            "2026-11-13T10:00:00Z",
        ));
        mount_configs(&r.server, rows).await;
        (r, there)
    }

    /// D114: o backup do que a cópia troca vai para
    /// `<dados>/sync-backups/<carimbo>/settings/<grupo>.json` com os valores de
    /// antes, e só os grupos marcados entram. A aparência volta para a tela.
    /// O que o app guarda em memória é relido uma vez.
    #[tokio::test]
    async fn copy_backs_up_then_applies_only_ticked_groups() {
        let _serial = SERIAL.lock().await;
        crate::sync::test_support::captured_logs();
        let (r, there) = copy_rig("settings-copy").await;
        let catalog = Catalog::new(&r.path("shaders"), &r.path("decorations"));
        let video_before = groups::export(Group::Video, &r.db, &catalog).await.unwrap();

        let report = r.copy(&["core-options", "audio"], true).await.unwrap();
        let copied: Vec<&str> = report.per_group.iter().map(|g| g.group.as_str()).collect();
        assert_eq!(copied, ["core-options", "audio"]);
        assert_eq!(report.appearance_doc, Some(appearance()));
        assert_eq!(r.reload_count(), 1);

        // os marcados vieram; o vídeo não
        let options = groups::export(Group::CoreOptions, &r.db, &catalog)
            .await
            .unwrap();
        assert_eq!(options["values"]["snes9x"]["snes9x_region"], json!("ntsc"));
        let sound = groups::export(Group::Audio, &r.db, &catalog).await.unwrap();
        assert_eq!(sound, there.doc(Group::Audio).await);
        let device: Option<String> =
            sqlx::query_scalar("SELECT output_device_id FROM audio_config WHERE id = 1")
                .fetch_one(&r.db)
                .await
                .unwrap();
        assert_eq!(
            device.as_deref(),
            Some("fone-daqui"),
            "o aparelho de áudio é daqui"
        );
        assert_eq!(
            groups::export(Group::Video, &r.db, &catalog).await.unwrap(),
            video_before
        );

        // o backup tem o de antes, um arquivo por grupo marcado
        let backups = r.backups();
        let names: Vec<&str> = backups.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["settings/audio.json", "settings/core-options.json"]);
        let saved: Value = serde_json::from_slice(&backups[1].1).unwrap();
        assert_eq!(saved["backup"], json!(true));
        assert_eq!(saved["values"]["snes9x"]["snes9x_region"], json!("pal"));
        let dir = PathBuf::from(report.backup_dir.unwrap());
        assert!(dir.starts_with(r.path("sync-backups")));
        assert!(dir.join("settings").join("core-options.json").is_file());
        assert!(copy_marker(&r).await.is_some());

        // nenhum nome de opção nem de arquivo no log
        let leaked: Vec<String> = crate::sync::test_support::captured_logs()
            .into_iter()
            .filter(|l| l.contains(SECRET_KEY) || l.contains("reemu/") || l.contains(THEIR_VALUE))
            .collect();
        assert!(leaked.is_empty(), "{leaked:?}");
    }

    /// Sem backup, sem cópia: um backup que não grava não deixa nada mudar.
    #[tokio::test]
    async fn copy_applies_nothing_when_the_backup_fails() {
        let _serial = SERIAL.lock().await;
        let (r, _there) = copy_rig("settings-backup-fails").await;
        // a pasta de backups é um arquivo: nada se grava dentro dela
        r.write("sync-backups", b"nao e pasta");
        let before = settings_rows(&r.db).await;
        assert_eq!(
            r.copy(&["core-options", "audio"], false).await,
            Err(SyncError::BackupFailed)
        );
        assert_eq!(settings_rows(&r.db).await, before);
        assert_eq!(copy_marker(&r).await, None);
        assert_eq!(r.reload_count(), 0);
    }

    /// "Desfazer" volta o backup da última cópia (`Restore`) e apaga o
    /// marcador junto: a segunda vez não há o que desfazer.
    #[tokio::test]
    async fn undo_restores_the_backup_set_once() {
        let _serial = SERIAL.lock().await;
        let (r, _there) = copy_rig("settings-undo").await;
        assert!(
            !r.engine.settings_undo().await.unwrap(),
            "nada copiado ainda"
        );
        let before = settings_rows(&r.db).await;

        r.copy(&["core-options", "audio", "video"], false)
            .await
            .unwrap();
        assert_ne!(settings_rows(&r.db).await, before);
        assert!(r.engine.settings_undo().await.unwrap());
        assert_eq!(settings_rows(&r.db).await, before);
        assert_eq!(copy_marker(&r).await, None);
        assert_eq!(r.reload_count(), 2, "relido depois de copiar e de desfazer");

        // uma vez por cópia
        assert!(!r.engine.settings_undo().await.unwrap());
        assert_eq!(r.reload_count(), 2);

        // a cópia seguinte troca o backup: o "Desfazer" volta o de antes dela
        r.copy(&["audio"], false).await.unwrap();
        opt(&r.db, "snes9x", None, "snes9x_region", "mudou-depois").await;
        let after_second = {
            let mut rows = settings_rows(&r.db).await;
            rows.retain(|l| !l.starts_with("audio_config"));
            rows
        };
        assert!(r.engine.settings_undo().await.unwrap());
        let mut rows = settings_rows(&r.db).await;
        rows.retain(|l| !l.starts_with("audio_config"));
        assert_eq!(rows, after_second, "só o grupo marcado volta");
        assert_eq!(
            settings_rows(&r.db)
                .await
                .iter()
                .filter(|l| l.starts_with("audio_config"))
                .collect::<Vec<_>>(),
            before
                .iter()
                .filter(|l| l.starts_with("audio_config"))
                .collect::<Vec<_>>()
        );
    }

    /// Copiar e desfazer com um jogo aberto ou abrindo: recusado antes de
    /// qualquer pedido à nuvem, nada muda e nada é relido.
    #[tokio::test]
    async fn copy_refuses_while_a_game_is_loaded() {
        let _serial = SERIAL.lock().await;
        let (r, _there) = copy_rig("settings-playing").await;
        let before = settings_rows(&r.db).await;
        for playing in [
            Playing::Running(Some("r1".into())),
            Playing::Running(None),
            Playing::Loading,
        ] {
            r.set_playing(playing.clone());
            assert_eq!(
                r.copy(&["core-options"], true).await,
                Err(SyncError::GameOpen),
                "{playing:?}"
            );
        }
        assert!(r.server.received_requests().await.unwrap().is_empty());
        assert_eq!(settings_rows(&r.db).await, before);
        assert!(r.backups().is_empty());

        // com uma cópia feita, o "Desfazer" também espera o jogo fechar
        r.set_playing(Playing::Idle);
        r.copy(&["core-options"], false).await.unwrap();
        let copied = settings_rows(&r.db).await;
        r.set_playing(Playing::Running(Some("r1".into())));
        assert_eq!(r.engine.settings_undo().await, Err(SyncError::GameOpen));
        assert_eq!(settings_rows(&r.db).await, copied);
        assert!(
            copy_marker(&r).await.is_some(),
            "o desfazer segue disponível"
        );
        assert_eq!(r.reload_count(), 1);
    }

    /// D114: os grupos marcados entram juntos ou nenhum. Um documento de uma
    /// versão mais nova para tudo antes do backup; uma falha do banco no
    /// segundo grupo desfaz o primeiro.
    #[tokio::test]
    async fn copy_applies_nothing_when_one_ticked_document_is_bad() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-bad-doc").await;
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device(OTHER, "Notebook")],
        )
        .await;
        opt(&r.db, "snes9x", None, "snes9x_region", "pal").await;
        audio(&r.db, false, "fone-daqui").await;
        let there = There::new(&r).await;
        opt(&there.db, "snes9x", None, "snes9x_region", "ntsc").await;
        audio(&there.db, true, "caixa-de-la").await;
        let mut rows = there.rows(&[Group::CoreOptions, Group::Audio]).await;
        let mut newer = there.doc(Group::Video).await;
        newer["v"] = json!(2);
        rows.push(config_row(
            "c-new",
            "video",
            &newer,
            Some(OTHER),
            "2026-11-13T10:00:00Z",
        ));
        rows.push(config_row(
            "c-bad",
            "hotkeys-keyboard",
            &json!({"v": 1, "hotkeys": "x"}),
            Some(OTHER),
            "2026-11-13T10:00:00Z",
        ));
        mount_configs(&r.server, rows).await;
        let before = settings_rows(&r.db).await;

        assert_eq!(
            r.copy(&["core-options", "video"], false).await,
            Err(SyncError::SettingsVersion)
        );
        assert_eq!(
            r.copy(&["core-options", "hotkeys-keyboard"], false).await,
            Err(SyncError::SettingsInvalid)
        );
        assert_eq!(settings_rows(&r.db).await, before);
        assert!(r.backups().is_empty(), "conferido antes do backup");
        assert_eq!(copy_marker(&r).await, None);

        // o banco recusa o áudio, que entra depois das opções
        sqlx::query(
            "CREATE TRIGGER no_audio BEFORE UPDATE ON audio_config \
             BEGIN SELECT RAISE(ABORT, 'audio recusado'); END",
        )
        .execute(&r.db)
        .await
        .unwrap();
        assert!(matches!(
            r.copy(&["core-options", "audio"], false).await,
            Err(SyncError::Failed(_))
        ));
        assert_eq!(settings_rows(&r.db).await, before, "nem as opções ficaram");
        assert_eq!(copy_marker(&r).await, None);
        assert_eq!(r.reload_count(), 0);
        assert!(!r.engine.settings_undo().await.unwrap());
    }

    /// R38: duas ROMs da biblioteca com o mesmo CRC32 e escolhas diferentes.
    /// A cópia leva a escolha de lá para as duas; o "Desfazer" devolve cada
    /// uma com a sua.
    #[tokio::test]
    async fn undo_is_exact_with_two_roms_sharing_a_crc() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-dup-crc").await;
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device(OTHER, "Notebook")],
        )
        .await;
        rom_crc(&r.db, "d-1", "AAAA0001").await;
        rom_crc(&r.db, "d-2", "AAAA0001").await;
        opt(&r.db, "snes9x", Some("d-1"), "snes9x_overclock", "on").await;
        opt(&r.db, "snes9x", Some("d-2"), "snes9x_overclock", "off").await;
        aspect(&r.db, "rom", "d-1", "4:3").await;
        aspect(&r.db, "rom", "d-2", "fill").await;
        let there = There::new(&r).await;
        rom_crc(&there.db, "t-1", "AAAA0001").await;
        opt(&there.db, "snes9x", Some("t-1"), "snes9x_overclock", "max").await;
        aspect(&there.db, "rom", "t-1", "16:9").await;
        mount_configs(
            &r.server,
            there.rows(&[Group::CoreOptions, Group::Video]).await,
        )
        .await;
        let before = settings_rows(&r.db).await;

        r.copy(&["core-options", "video"], false).await.unwrap();
        let copied = settings_rows(&r.db).await;
        for rom in ["d-1", "d-2"] {
            assert!(
                copied
                    .iter()
                    .any(|l| l.contains(rom) && l.contains("'max'")),
                "{rom}"
            );
            assert!(
                copied
                    .iter()
                    .any(|l| l.contains(rom) && l.contains("'16:9'")),
                "{rom}"
            );
        }
        assert!(r.engine.settings_undo().await.unwrap());
        assert_eq!(settings_rows(&r.db).await, before);
    }

    /// O MAC do PSP emulado é identidade desta máquina: não sobe no envio, uma
    /// cópia (mesmo de um documento forjado com ele) não o troca, e o
    /// "Desfazer" não o apaga.
    #[tokio::test]
    async fn copy_never_touches_the_ppsspp_mac() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-mac").await;
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device(OTHER, "Notebook")],
        )
        .await;
        mount_upload(&r.server, "c1", "v1").await;
        opt(&r.db, "ppsspp_libretro", None, MAC, "12").await;
        opt(&r.db, "ppsspp_libretro", None, "ppsspp_cpu_core", "jit").await;

        r.engine
            .settings_upload(&r.alva, &appearance())
            .await
            .unwrap();
        for p in r.reqs("POST", SAVES).await {
            assert!(!field(&p.body, "file")
                .unwrap()
                .contains("ppsspp_change_mac_address"));
        }

        let there = There::new(&r).await;
        opt(
            &there.db,
            "ppsspp_libretro",
            None,
            "ppsspp_cpu_core",
            "interpreter",
        )
        .await;
        let mut doc = there.doc(Group::CoreOptions).await;
        doc["values"]["ppsspp_libretro"][MAC] = json!("99");
        mount_configs(
            &r.server,
            vec![config_row(
                "c-co",
                "core-options",
                &doc,
                Some(OTHER),
                "2026-11-13T10:00:00Z",
            )],
        )
        .await;
        r.copy(&["core-options"], false).await.unwrap();
        assert_eq!(option_value(&r.db, MAC).await.as_deref(), Some("12"));
        assert_eq!(
            option_value(&r.db, "ppsspp_cpu_core").await.as_deref(),
            Some("interpreter")
        );
        let (_, backup) = r
            .backups()
            .into_iter()
            .find(|(n, _)| n == "settings/core-options.json")
            .unwrap();
        assert!(!String::from_utf8_lossy(&backup).contains("ppsspp_change_mac_address"));

        assert!(r.engine.settings_undo().await.unwrap());
        assert_eq!(option_value(&r.db, MAC).await.as_deref(), Some("12"));
        assert_eq!(
            option_value(&r.db, "ppsspp_cpu_core").await.as_deref(),
            Some("jit")
        );
    }

    /// Um marcador gravado no banco, como texto (o do "Desfazer" ou o do
    /// backup de antes dele).
    async fn state_value(r: &Rig, key: &str) -> Option<String> {
        sqlx::query_scalar("SELECT value FROM alvanista_state WHERE key = ?1")
            .bind(key)
            .fetch_optional(&r.db)
            .await
            .unwrap()
    }

    /// O backup de um grupo pelo marcador (`files`, relativo a `<dados>`).
    fn file_of(r: &Rig, marker: &str, group: &str) -> (String, PathBuf) {
        let v: Value = serde_json::from_str(marker).unwrap();
        let rel = v["files"][group].as_str().unwrap().to_owned();
        let path = r.root.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
        (rel, path)
    }

    /// R39: o "Desfazer" grava antes, num carimbo novo, o estado de agora dos
    /// grupos que volta: uma mudança feita depois da cópia fica guardada. O
    /// registro desse backup fica, e não há "desfazer o desfazer".
    #[tokio::test]
    async fn undo_backs_up_what_it_replaces() {
        let _serial = SERIAL.lock().await;
        let (r, _there) = copy_rig("settings-undo-backup").await;
        let before = settings_rows(&r.db).await;
        r.copy(&["core-options", "audio"], false).await.unwrap();
        // a pessoa mudou uma opção depois da cópia
        opt(&r.db, "snes9x", None, "snes9x_region", "mudei-depois").await;

        assert!(r.engine.settings_undo().await.unwrap());
        assert_eq!(settings_rows(&r.db).await, before);
        let record = state_value(&r, LAST_UNDO_BACKUP_KEY)
            .await
            .expect("o backup de antes do desfazer fica registrado");
        let (rel, path) = file_of(&r, &record, "core-options");
        assert!(rel.starts_with("sync-backups/"), "{rel}");
        let saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["backup"], json!(true));
        assert_eq!(
            saved["values"]["snes9x"]["snes9x_region"],
            json!("mudei-depois")
        );
        let (_, audio) = file_of(&r, &record, "audio");
        assert!(audio.is_file());
        // o backup da cópia continua lá, e nada oferece voltar o desfazer
        assert_eq!(r.backups().len(), 4);
        assert_eq!(r.engine.settings_undo_info().await.unwrap(), None);
        assert!(!r.engine.settings_undo().await.unwrap());
    }

    /// R39: a tela sabe, sem rede nem chaveiro, se há uma cópia a desfazer, de
    /// qual máquina, quando e quais grupos (a aparência inclusive). O
    /// marcador guarda os caminhos relativos a `<dados>`.
    #[tokio::test]
    async fn undo_info_names_the_copy_from_the_marker_alone() {
        let _serial = SERIAL.lock().await;
        let (r, _there) = copy_rig("settings-undo-info").await;
        assert_eq!(r.engine.settings_undo_info().await.unwrap(), None);
        let started = now_secs();
        let report = r.copy(&["audio", "core-options"], true).await.unwrap();

        let requests = r.server.received_requests().await.unwrap().len();
        let info = r.engine.settings_undo_info().await.unwrap().unwrap();
        assert_eq!(r.server.received_requests().await.unwrap().len(), requests);
        assert_eq!(info.machine, "Notebook");
        assert_eq!(info.machine_id.as_deref(), Some(OTHER));
        assert!(info.copied_at >= started);
        assert_eq!(
            report.copied_at, info.copied_at,
            "a tela casa a aparência de antes com esta cópia"
        );
        assert_eq!(info.groups, ["core-options", "audio", "appearance"]);
        let marker = copy_marker(&r).await.unwrap();
        for group in ["core-options", "audio"] {
            let (rel, path) = file_of(&r, &marker, group);
            let tail = format!("/settings/{group}.json");
            assert!(
                rel.starts_with("sync-backups/") && rel.ends_with(&tail) && !rel.contains('\\'),
                "{rel}"
            );
            assert!(path.is_file(), "{rel}");
        }

        // com o sync desligado ainda se sabe (e se desfaz)
        r.sync.devices.set_enabled(false).await.unwrap();
        assert!(r.engine.settings_undo_info().await.unwrap().is_some());
        assert!(r.engine.settings_undo().await.unwrap());
        assert_eq!(r.engine.settings_undo_info().await.unwrap(), None);
    }

    /// Um backup que sumiu, que não é JSON ou um marcador que aponta para fora
    /// de `<dados>`: o "Desfazer" diz `backup_failed`, nada muda e o marcador
    /// fica.
    #[tokio::test]
    async fn undo_with_a_missing_or_corrupt_backup_changes_nothing() {
        let _serial = SERIAL.lock().await;
        let (r, _there) = copy_rig("settings-undo-broken").await;
        r.copy(&["core-options", "audio"], false).await.unwrap();
        let copied = settings_rows(&r.db).await;
        let marker = copy_marker(&r).await.unwrap();
        let (_, path) = file_of(&r, &marker, "core-options");
        let good = std::fs::read(&path).unwrap();
        let files_before = r.backups().len();

        std::fs::remove_file(&path).unwrap();
        assert_eq!(r.engine.settings_undo().await, Err(SyncError::BackupFailed));
        std::fs::write(&path, b"{ isto nao e json").unwrap();
        assert_eq!(r.engine.settings_undo().await, Err(SyncError::BackupFailed));
        std::fs::write(&path, br#"{"v": 1, "values": {}, "overrides": []}"#).unwrap();
        assert_eq!(
            r.engine.settings_undo().await,
            Err(SyncError::BackupFailed),
            "um documento sem a marca de backup não volta"
        );
        // um marcador mexido que sai da pasta de dados
        let mut forged: Value = serde_json::from_str(&marker).unwrap();
        forged["files"]["core-options"] = json!("../fora/core-options.json");
        sqlx::query("UPDATE alvanista_state SET value = ?1 WHERE key = 'sync.last_copy_backup'")
            .bind(forged.to_string())
            .execute(&r.db)
            .await
            .unwrap();
        assert_eq!(r.engine.settings_undo().await, Err(SyncError::BackupFailed));

        assert_eq!(settings_rows(&r.db).await, copied);
        assert!(
            copy_marker(&r).await.is_some(),
            "o desfazer segue disponível"
        );
        assert_eq!(r.backups().len(), files_before, "nada novo gravado");
        assert_eq!(r.reload_count(), 1, "só a da cópia");

        // com o marcador e o arquivo de volta, ele desfaz
        sqlx::query("UPDATE alvanista_state SET value = ?1 WHERE key = 'sync.last_copy_backup'")
            .bind(&marker)
            .execute(&r.db)
            .await
            .unwrap();
        std::fs::write(&path, &good).unwrap();
        assert!(r.engine.settings_undo().await.unwrap());
    }

    /// "Esquecer" a cópia cujo backup sumiu: só o marcador sai. Os arquivos
    /// que restam ficam na pasta de resguardo, nenhuma configuração muda,
    /// nada é relido e nada vai à rede.
    #[tokio::test]
    async fn forgetting_an_undo_removes_only_the_marker() {
        let _serial = SERIAL.lock().await;
        let (r, _there) = copy_rig("settings-undo-forget").await;
        r.copy(&["core-options", "audio"], false).await.unwrap();
        let copied = settings_rows(&r.db).await;
        let marker = copy_marker(&r).await.unwrap();
        let (_, path) = file_of(&r, &marker, "core-options");
        std::fs::remove_file(&path).unwrap();
        assert_eq!(r.engine.settings_undo().await, Err(SyncError::BackupFailed));
        let files = r.backups();
        let requests = r.server.received_requests().await.unwrap().len();

        r.engine.settings_undo_forget().await.unwrap();
        assert_eq!(copy_marker(&r).await, None);
        assert_eq!(r.engine.settings_undo_info().await.unwrap(), None);
        assert!(!r.engine.settings_undo().await.unwrap());
        assert_eq!(settings_rows(&r.db).await, copied);
        assert_eq!(r.backups(), files, "nenhum arquivo apagado");
        assert_eq!(r.reload_count(), 1, "só a da cópia");
        assert_eq!(r.server.received_requests().await.unwrap().len(), requests);
        // sem cópia a esquecer, nada acontece
        r.engine.settings_undo_forget().await.unwrap();
    }

    /// O jogo que começa a abrir enquanto a cópia anda: ela confere de novo ao
    /// abrir a transação (antes do backup) e logo antes de confirmar. O mesmo
    /// no "Desfazer".
    #[tokio::test]
    async fn copy_rechecks_the_game_inside_its_transaction() {
        let _serial = SERIAL.lock().await;
        let (r, _there) = copy_rig("settings-recheck").await;
        let before = settings_rows(&r.db).await;
        // a `n`-ésima pergunta "tem jogo?" em diante responde que está abrindo
        let loads_from = |n: usize| -> PlayingFn {
            let asked = Arc::new(AtomicUsize::new(0));
            Box::new(move || {
                if asked.fetch_add(1, SeqCst) + 1 >= n {
                    Playing::Loading
                } else {
                    Playing::Idle
                }
            })
        };
        let engine = |n: usize| {
            Engine::new(
                r.sync.clone(),
                r.db.clone(),
                dirs_in(&r.root),
                loads_from(n),
                Box::new(|_| {}),
                counting_reload(&r.reloads),
            )
        };
        let ticks = ticks(&["core-options", "audio"]);

        // ao abrir a transação: nem o backup é gravado
        let e = engine(2);
        assert_eq!(
            e.settings_copy(&r.alva, OTHER, &ticks, false).await,
            Err(SyncError::GameOpen)
        );
        assert!(r.backups().is_empty());
        // logo antes de confirmar: o backup já foi, os grupos não ficam
        let e = engine(3);
        assert_eq!(
            e.settings_copy(&r.alva, OTHER, &ticks, false).await,
            Err(SyncError::GameOpen)
        );
        assert_eq!(settings_rows(&r.db).await, before);
        assert_eq!(copy_marker(&r).await, None);
        assert_eq!(r.reload_count(), 0);

        // o "Desfazer" confere de novo antes de confirmar
        r.copy(&["core-options", "audio"], false).await.unwrap();
        let copied = settings_rows(&r.db).await;
        let e = engine(2);
        assert_eq!(e.settings_undo().await, Err(SyncError::GameOpen));
        assert_eq!(settings_rows(&r.db).await, copied);
        assert!(copy_marker(&r).await.is_some());
        assert_eq!(r.reload_count(), 1);
    }

    /// R39: um documento da nuvem com a marca de backup (forjado: um backup
    /// nunca sobe) é recusado na conferência. Na prévia só o grupo dele fica
    /// marcado; a cópia não grava nem o backup.
    #[tokio::test]
    async fn a_remote_document_marked_as_backup_is_refused_at_the_check() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-forged-backup").await;
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device(OTHER, "Notebook")],
        )
        .await;
        opt(&r.db, "snes9x", None, "snes9x_region", "pal").await;
        let there = There::new(&r).await;
        opt(&there.db, "snes9x", None, "snes9x_region", "ntsc").await;
        let mut forged = there.doc(Group::CoreOptions).await;
        forged["backup"] = json!(true);
        let mut rows = there.rows(&[Group::Audio]).await;
        rows.push(config_row(
            "c-forged",
            "core-options",
            &forged,
            Some(OTHER),
            "2026-11-13T10:00:00Z",
        ));
        mount_configs(&r.server, rows).await;
        let before = settings_rows(&r.db).await;

        let p = r
            .engine
            .settings_preview(&r.alva, OTHER, &HardwarePayload::default())
            .await
            .unwrap();
        let errors: Vec<(&str, Option<&str>)> = p
            .groups
            .iter()
            .filter(|g| g.present)
            .map(|g| (g.group.as_str(), g.error.as_deref()))
            .collect();
        assert_eq!(
            errors,
            [("core-options", Some("settings_invalid")), ("audio", None)]
        );
        assert_eq!(
            r.copy(&["core-options", "audio"], false).await,
            Err(SyncError::SettingsInvalid)
        );
        assert_eq!(settings_rows(&r.db).await, before);
        assert!(r.backups().is_empty());
        assert_eq!(copy_marker(&r).await, None);
    }

    /// Um documento que chega diferente do hash da nuvem marca só o grupo dele
    /// na prévia (`failed`: vale tentar de novo); a cópia dele não acontece.
    #[tokio::test]
    async fn a_document_that_does_not_match_its_hash_marks_only_its_group() {
        let _serial = SERIAL.lock().await;
        let r = rig("settings-hash").await;
        mount_devices(
            &r.server,
            vec![device("dev1", "Sala"), device(OTHER, "Notebook")],
        )
        .await;
        let there = There::new(&r).await;
        let mut rows = there.rows(&[Group::CoreOptions, Group::Audio]).await;
        rows[0].0["current"]["content_hash"] = json!(sha(b"outros bytes"));
        mount_configs(&r.server, rows).await;
        let before = settings_rows(&r.db).await;

        let p = r
            .engine
            .settings_preview(&r.alva, OTHER, &HardwarePayload::default())
            .await
            .unwrap();
        let errors: Vec<(&str, Option<&str>)> = p
            .groups
            .iter()
            .filter(|g| g.present)
            .map(|g| (g.group.as_str(), g.error.as_deref()))
            .collect();
        assert_eq!(errors, [("core-options", Some("failed")), ("audio", None)]);
        assert!(matches!(
            r.copy(&["core-options", "audio"], false).await,
            Err(SyncError::Failed(_))
        ));
        assert_eq!(settings_rows(&r.db).await, before);
        assert!(r.backups().is_empty());
    }

    // --- os cinco grupos no motor (Review Focus 4) ---------------------------

    /// GUID de um controle desta máquina (formato SDL).
    const GUID: &str = "030000005e0400008e02000000007200";

    /// Um preset atribuído num lugar, com os parâmetros, como a tela de
    /// shaders grava: o id é o `source_path`.
    async fn shader_on(
        db: &db::Db,
        scope: domain::shader_chain::AssignmentScope,
        target: Option<&str>,
        source: &str,
        params: &[(&str, &str)],
    ) {
        use domain::shader_chain::{AssignmentScope, ShaderChainStore, ShaderFormat, ShaderPreset};
        let repo = db::ShaderChainRepo::new(db.clone());
        let name = Path::new(source)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(source);
        repo.upsert_preset(&ShaderPreset {
            id: source.into(),
            name: name.into(),
            source_path: source.into(),
            format: ShaderFormat::Slang,
            is_builtin: !source.ends_with(".slangp"),
            includes_bezel: false,
        })
        .await
        .unwrap();
        let (sys, rom) = match scope {
            AssignmentScope::Default => (None, None),
            AssignmentScope::System => (target, None),
            AssignmentScope::Rom => (None, target),
        };
        repo.set_assignment(scope, sys, rom, source).await.unwrap();
        for (k, v) in params {
            repo.set_parameter_override(scope, sys, rom, k, v)
                .await
                .unwrap();
        }
    }

    /// Uma moldura do The Bezel Project, no pacote que `bezel_pack` importa
    /// (o mesmo em toda máquina), com o `id` que o `DecorationRepo` grava.
    async fn bezel_on(db: &db::Db, catalog: &Catalog, scope: &str, target: &str, rel: &str) {
        let base = catalog.bezels.display().to_string();
        let pack = format!("pack:{base}");
        sqlx::query(
            "INSERT OR IGNORE INTO decoration_packs (id, name, source, base_path) \
             VALUES (?1, 'bezelproject', 'user_imported', ?2)",
        )
        .bind(&pack)
        .bind(&base)
        .execute(db)
        .await
        .unwrap();
        let mut asset = catalog.bezels.clone();
        asset.extend(rel.split('/'));
        let (sys, rom) = match scope {
            "system" => (target, ""),
            _ => ("", target),
        };
        sqlx::query(
            "INSERT INTO decoration_assignments (id, scope, system_id, rom_id, pack_id, asset_path) \
             VALUES (?1, ?2, NULLIF(?3, ''), NULLIF(?4, ''), ?5, ?6)",
        )
        .bind(format!("{pack}:{scope}:{target}"))
        .bind(scope)
        .bind(sys)
        .bind(rom)
        .bind(&pack)
        .bind(asset.to_string_lossy().into_owned())
        .execute(db)
        .await
        .unwrap();
    }

    async fn hotkey_on(
        db: &db::Db,
        action: domain::hotkeys::SystemAction,
        trigger: domain::input::RawInputEvent,
    ) {
        use domain::hotkeys::{HotkeyBinding, SystemHotkeyRepository};
        db::SystemHotkeysRepo::new(db.clone())
            .set(&HotkeyBinding {
                action,
                trigger: vec![trigger],
                device_guid: None,
            })
            .await
            .unwrap();
    }

    fn key(code: &str) -> domain::input::RawInputEvent {
        domain::input::RawInputEvent::Keyboard {
            scancode: input_desktop::keymap::key_scancode(code),
        }
    }

    async fn core_pref(db: &db::Db, system: &str, core: &str) {
        sqlx::query("INSERT INTO system_core_prefs (system_id, core_id) VALUES (?1, ?2)")
            .bind(system)
            .bind(core)
            .execute(db)
            .await
            .unwrap();
    }

    /// Review Focus 4 no motor: a outra máquina tem outra GPU, e a pessoa marca
    /// à mão os cinco grupos do banco, com um preset do pacote `slang-shaders`
    /// e uma moldura do The Bezel Project baixados aqui, atalhos e teclado. A
    /// hotkey de controle e o MAC do PSP desta máquina não mudam. Copiar e
    /// desfazer devolve as tabelas byte a byte; fica só o catálogo
    /// (`shader_presets` e o pacote do The Bezel Project que a cópia criou),
    /// que nenhuma escolha usa depois do "Desfazer".
    #[tokio::test]
    async fn copy_then_undo_of_every_group_restores_byte_for_byte() {
        use domain::hotkeys::SystemAction;
        use domain::shader_chain::AssignmentScope as At;
        let _serial = SERIAL.lock().await;
        let r = rig("settings-all-groups").await;
        mount_devices(
            &r.server,
            vec![
                machine_row("dev1", "Sala", "RTX 3070", "Vulkan"),
                machine_row(OTHER, "Notebook", "Intel Iris Xe", "OpenGL"),
            ],
        )
        .await;
        // os catálogos baixados aqui e o core que a outra prefere
        let here = Catalog::new(&r.path("shaders"), &r.path("decorations"));
        let slang = |c: &Catalog, rel: &str| {
            let mut p = c.shader_pack.clone();
            p.extend(rel.split('/'));
            p
        };
        for rel in ["crt/crt-royale.slangp", "handheld/lcd-grid-v2.slangp"] {
            let file = slang(&here, rel);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, b"x").unwrap();
        }
        let mut snes_png = here.bezels.clone();
        snes_png.extend(["snes", "snes.png"]);
        std::fs::create_dir_all(snes_png.parent().unwrap()).unwrap();
        std::fs::write(&snes_png, b"x").unwrap();
        r.write(
            &format!("cores/mgba_libretro{}", std::env::consts::DLL_SUFFIX),
            b"x",
        );

        // esta máquina
        rom_crc(&r.db, "h-1", "AAAA0001").await;
        opt(&r.db, "snes9x", None, "snes9x_region", "pal").await;
        opt(&r.db, "snes9x", Some("h-1"), "snes9x_overclock", "off").await;
        opt(&r.db, "ppsspp_libretro", None, MAC, "12").await;
        core_pref(&r.db, "snes", "snes9x_libretro").await;
        let royale = slang(&here, "crt/crt-royale.slangp");
        let royale = royale.to_string_lossy();
        shader_on(&r.db, At::System, Some("snes"), &royale, &[("MASK", "2")]).await;
        shader_on(&r.db, At::Default, None, "crt", &[("GAMMA", "2.2")]).await;
        hotkey_on(&r.db, SystemAction::QuickSave, key("F5")).await;
        let pad = domain::input::RawInputEvent::GamepadButton {
            device_guid: GUID.into(),
            index: 2,
        };
        hotkey_on(&r.db, SystemAction::QuickLoad, pad).await;
        db::KeyboardBindingsRepo::new(r.db.clone())
            .set("R2", "KeyR")
            .await
            .unwrap();
        audio(&r.db, false, "fone-daqui").await;
        video(&r.db, false).await;
        aspect(&r.db, "default", "", "4:3").await;
        aspect(&r.db, "rom", "h-1", "fill").await;

        // a outra
        let there = There::new(&r).await;
        rom_crc(&there.db, "t-1", "AAAA0001").await;
        opt(&there.db, "snes9x", None, "snes9x_region", "ntsc").await;
        opt(&there.db, "snes9x", Some("t-1"), "snes9x_overclock", "max").await;
        core_pref(&there.db, "snes", "mgba_libretro").await;
        shader_on(&there.db, At::System, Some("snes"), "lcd", &[]).await;
        let grid = slang(&there.catalog, "handheld/lcd-grid-v2.slangp");
        let grid = grid.to_string_lossy();
        shader_on(&there.db, At::Rom, Some("t-1"), &grid, &[("GRID", "0.5")]).await;
        shader_on(&there.db, At::Default, None, "crt", &[("GAMMA", "2.4")]).await;
        bezel_on(&there.db, &there.catalog, "system", "snes", "snes/snes.png").await;
        hotkey_on(&there.db, SystemAction::QuickSave, key("F6")).await;
        hotkey_on(&there.db, SystemAction::QuickLoad, key("F9")).await;
        let keys = db::KeyboardBindingsRepo::new(there.db.clone());
        keys.set("R2", "KeyT").await.unwrap();
        keys.set("L2", "KeyY").await.unwrap();
        audio(&there.db, true, "caixa-de-la").await;
        video(&there.db, true).await;
        aspect(&there.db, "default", "", "16:9").await;
        aspect(&there.db, "rom", "t-1", "core").await;
        mount_configs(&r.server, there.rows(&Group::ALL).await).await;

        // a prévia desmarca o vídeo; a pessoa marca tudo à mão
        let preview = r
            .engine
            .settings_preview(&r.alva, OTHER, &HardwarePayload::default())
            .await
            .unwrap();
        assert!(preview.video_default_off);
        let before = settings_rows(&r.db).await;
        let report = r
            .copy(
                &[
                    "core-options",
                    "shaders-decorations",
                    "hotkeys-keyboard",
                    "audio",
                    "video",
                ],
                false,
            )
            .await
            .unwrap();
        assert_eq!(report.per_group.len(), 5);
        for g in &report.per_group {
            assert!(
                g.report.missing_files.is_empty(),
                "{}: {:?}",
                g.group,
                g.report
            );
            assert!(g.report.cores_not_installed.is_empty(), "{}", g.group);
        }
        let kept: u32 = report.per_group.iter().map(|g| g.report.kept_local).sum();
        assert_eq!(kept, 1, "a hotkey de controle daqui");

        // cada tabela que um grupo escreve mudou de fato
        let copied = settings_rows(&r.db).await;
        let table = |rows: &[String], t: &str| -> Vec<String> {
            let prefix = format!("{t}: ");
            rows.iter()
                .filter(|l| l.starts_with(&prefix))
                .cloned()
                .collect()
        };
        for t in [
            "core_options_values",
            "core_option_overrides",
            "system_core_prefs",
            "shader_chain_assignments",
            "shader_parameter_overrides",
            "decoration_assignments",
            "system_hotkeys",
            "keyboard_bindings",
            "audio_config",
            "video_config",
            "aspect_assignments",
        ] {
            assert_ne!(table(&copied, t), table(&before, t), "{t} não mudou");
        }
        let grid_here = slang(&here, "handheld/lcd-grid-v2.slangp");
        let grid_here = grid_here.to_string_lossy();
        assert!(
            table(&copied, "shader_chain_assignments")
                .iter()
                .any(|l| l.contains(grid_here.as_ref()) && l.contains("'h-1'")),
            "o preset do pacote aponta para o arquivo daqui"
        );
        let snes_png = snes_png.to_string_lossy();
        assert!(table(&copied, "decoration_assignments")
            .iter()
            .any(|l| l.contains(snes_png.as_ref())));
        // o que é só desta máquina ficou
        assert_eq!(option_value(&r.db, MAC).await.as_deref(), Some("12"));
        assert!(table(&copied, "system_hotkeys")
            .iter()
            .any(|l| l.contains(GUID)));
        let names: Vec<String> = r.backups().into_iter().map(|(n, _)| n).collect();
        assert_eq!(
            names,
            [
                "settings/audio.json",
                "settings/core-options.json",
                "settings/hotkeys-keyboard.json",
                "settings/shaders-decorations.json",
                "settings/video.json",
            ]
        );

        assert!(r.engine.settings_undo().await.unwrap());
        assert_eq!(settings_rows(&r.db).await, before);
        assert_eq!(r.reload_count(), 2);
        // o catálogo que fica: o pacote que a cópia criou, sem atribuição
        let packs: Vec<String> = sqlx::query_scalar("SELECT name FROM decoration_packs")
            .fetch_all(&r.db)
            .await
            .unwrap();
        assert_eq!(packs, ["bezelproject"]);
    }
}
