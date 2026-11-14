//! As configurações desta máquina na nuvem e as de outra aqui (D114). Nada
//! disso roda sozinho, é a pessoa quem pede, e por isso não há interruptor
//! como o dos saves e o dos states:
//!
//! - "Enviar as desta máquina" sobe um documento por grupo como linha `config`
//!   deste aparelho: `platform=other`, sem `core` (o GameKave guarda um `core`
//!   vazio como nenhum, e a linha é a mesma), `filename=reemu/<grupo>.json`. A
//!   aparência mora na tela e chega pronta. O hash de cada documento enviado
//!   fica guardado: é o que diz se um grupo mudou desde o envio;
//! - a prévia lê as linhas de outra máquina, aplica os documentos numa
//!   transação que não fica (o relatório é o que a cópia diria) e compara a
//!   GPU e o backend das duas, para o vídeo começar desmarcado quando diferem;
//! - copiar baixa os grupos marcados e confere todos; na transação de escrita
//!   grava antes, no disco, o backup do que a cópia troca, aplica os grupos e
//!   o marcador do "Desfazer", e só então confirma. Um documento ruim ou um
//!   backup que não grava, e nada muda;
//! - o backup é desta máquina e nunca sai dela: os jogos vão pela ROM (R38),
//!   em `<dados>/sync-backups/<carimbo>/settings/<grupo>.json`. O "Desfazer"
//!   volta o backup e apaga o marcador na mesma transação: vale uma vez por
//!   cópia, e a cópia seguinte troca o marcador;
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
use crate::sync::device::{HardwarePayload, LAST_COPY_BACKUP_KEY};
use crate::sync::settings_groups::{
    apply_in, check, check_all, dry_run, export, export_backup_in, log_applied, ApplyMode,
    ApplyReport, Catalog, Checked, Group, MissingFile, ThisMachine, DOC_VERSION, ERR_INVALID,
    ERR_VERSION,
};
use crate::sync::SyncMachine;
use db::SettingsRepo;
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
    /// `settings_version` (veio de um ReEmu mais novo) ou `settings_invalid`.
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
    pub changed: Vec<String>,
}

/// O marcador do "Desfazer": o arquivo do backup de cada grupo copiado.
#[derive(Debug, Default, Serialize, Deserialize)]
struct CopyBackup {
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
    /// da tela. Um grupo que a nuvem recusa de vez fica de fora e os outros
    /// sobem; a cota ou o plano param o resto; o aparelho removido, pausado
    /// ou a rede caída acabam o envio. O que subiu fica guardado com o hash
    /// (o "mudou desde o envio"), e o erro mais grave volta à tela.
    pub async fn settings_upload(
        &self,
        alva: &Alvanista,
        appearance: &Value,
    ) -> Result<i64, SyncError> {
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
        let (_, mut sent) = self.sync.devices.settings_sent().await.map_err(failed)?;
        let now = now_secs();
        let mut worst: Option<SyncError> = None;
        let mut keep = |e: SyncError| {
            if worst.as_ref().map_or(true, |w| severity(&e) > severity(w)) {
                worst = Some(e);
            }
        };
        let mut uploaded = 0;
        for (group, doc) in &docs {
            let bytes = canonical_bytes(doc);
            let hash = sha256_hex(&bytes);
            let filename = filename_of(group);
            let key = format!("config:{filename}");
            if let Some(err) = self.still_rejected(&key, &hash) {
                keep(err);
                continue;
            }
            let meta = UploadMeta {
                platform: "other",
                kind: "config",
                filename: &filename,
                core: None,
                core_version: None,
                client_mtime: Some(rfc3339(now)),
            };
            match self
                .upload_bytes(alva, &token, &key, &meta, bytes, &hash)
                .await
            {
                Ok(_) => {
                    sent.insert((*group).to_owned(), hash);
                    uploaded += 1;
                }
                Err(Fail::File(e)) => keep(e),
                Err(Fail::StopUploads(e)) => {
                    if e == SyncError::Quota {
                        self.quota_event().await;
                    }
                    keep(e);
                    break;
                }
                Err(Fail::End(e)) => {
                    keep(e);
                    break;
                }
            }
        }
        if uploaded > 0 {
            self.sync
                .devices
                .set_settings_sent(now, &sent)
                .await
                .map_err(failed)?;
        }
        log::info!(
            "sync: configurações enviadas: {uploaded} de {} grupos",
            docs.len()
        );
        match worst {
            Some(e) => Err(e),
            None => Ok(now),
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

    /// O documento da versão atual de uma linha, conferido pelo hash.
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
    /// documentos entram numa transação desfeita no fim. `here_hw`: o
    /// hardware desta máquina agora (sem GPU conhecida, vale o que a lista do
    /// site tem dela).
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
                Err(e @ (SyncError::SettingsInvalid | SyncError::SettingsMissing)) => {
                    errors.insert(group, e.to_string());
                    continue;
                }
                Err(e) => return Err(e),
            };
            let verdict = match group_by_name(group) {
                Some(g) => check(g, &doc).map(|c| checked.push(c)),
                None => appearance_doc(&doc).map(|_| ()).map_err(|e| e.to_string()),
            };
            if let Err(e) = verdict {
                errors.insert(group, settings_err(e).to_string());
            }
        }
        let here = self.here().await?;
        let reports = dry_run(&self.db, checked, ApplyMode::Merge, &here.view())
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
        let (_, _, token) = self.settings_online(alva).await?;
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
        let checked = check_all(&refs).map_err(settings_err)?;
        let (per_group, backup_dir) = self.copy_here(checked).await?;
        (self.reload)().await;
        Ok(CopyReport {
            per_group,
            backup_dir,
            appearance_doc,
        })
    }

    /// A metade local da cópia, numa transação de escrita: o backup do que
    /// muda vai ao disco primeiro, os grupos entram, o marcador do "Desfazer"
    /// entra junto, e só então a transação fica. Um jogo que começou a abrir
    /// no meio, um backup que não grava ou um grupo que falha, e nada fica.
    async fn copy_here(
        &self,
        checked: Vec<Checked>,
    ) -> Result<(Vec<SettingsGroupReport>, Option<String>), SyncError> {
        self.settings_idle()?;
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
        // o backup é o que esta transação vai trocar
        let mut backups = Vec::with_capacity(checked.len());
        for c in &checked {
            let doc = export_backup_in(&mut tx, c.group(), &here.catalog)
                .await
                .map_err(settings_err)?;
            backups.push((c.group(), doc));
        }
        let stamp = local_stamp();
        let files = self.write_settings_backup(&stamp, backups).await?;
        let mut reports = Vec::with_capacity(checked.len());
        for c in checked {
            let group = c.group();
            let report = apply_in(&mut tx, c, ApplyMode::Merge, &here.view())
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
        let record = serde_json::to_string(&CopyBackup { files })
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
        Ok((per_group, backup_dir))
    }

    /// Grava o backup de cada grupo em
    /// `<dados>/sync-backups/<carimbo>/settings/<grupo>.json`, no disco
    /// (`fsync`), e devolve onde ficou cada um.
    async fn write_settings_backup(
        &self,
        stamp: &str,
        docs: Vec<(Group, Value)>,
    ) -> Result<BTreeMap<String, String>, SyncError> {
        let mut jobs = Vec::with_capacity(docs.len());
        for (group, doc) in docs {
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
            jobs.push((group.as_str().to_owned(), target, bytes));
        }
        let written = tokio::task::spawn_blocking(move || {
            let mut files = BTreeMap::new();
            for (group, target, bytes) in jobs {
                write_atomic(&target, &bytes)?;
                files.insert(group, target.to_string_lossy().into_owned());
            }
            Ok::<_, io::Error>(files)
        })
        .await
        .unwrap_or_else(|j| Err(io::Error::other(j.to_string())));
        written.map_err(backup_err)
    }

    /// "Desfazer": volta o backup da última cópia (`Restore`, os jogos pela
    /// ROM) e apaga o marcador na mesma transação. `false` quando não há
    /// cópia a desfazer. Recusa com um jogo aberto ou abrindo. Sem rede: o
    /// backup é desta máquina e vale com o sync desligado.
    pub async fn settings_undo(&self) -> Result<bool, SyncError> {
        let _one = self.settings_lock.lock().await;
        self.settings_idle()?;
        let Some(raw) = self.sync.devices.last_copy_backup().await.map_err(failed)? else {
            return Ok(false);
        };
        let record: CopyBackup = serde_json::from_str(&raw).map_err(|_| {
            log::warn!("sync: o marcador do backup das configurações está ilegível");
            SyncError::BackupFailed
        })?;
        let mut paths = Vec::with_capacity(record.files.len());
        for (name, path) in record.files {
            let group = group_by_name(&name).ok_or(SyncError::BackupFailed)?;
            paths.push((group, PathBuf::from(path)));
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
        let checked = check_all(&refs).map_err(|e| {
            log::warn!("sync: o backup das configurações não confere ({e})");
            SyncError::BackupFailed
        })?;
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
        let mut reports = Vec::with_capacity(checked.len());
        for c in checked {
            let group = c.group();
            let report = apply_in(&mut tx, c, ApplyMode::Restore, &here.view())
                .await
                .map_err(settings_err)?;
            reports.push((group, report));
        }
        tx.delete_state(LAST_COPY_BACKUP_KEY)
            .await
            .map_err(db_err)?;
        self.settings_idle()?;
        tx.commit().await.map_err(db_err)?;
        log_applied(&reports, ApplyMode::Restore);
        (self.reload)().await;
        Ok(true)
    }
}
