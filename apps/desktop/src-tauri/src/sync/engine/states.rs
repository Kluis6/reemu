//! Os save states desta máquina na nuvem (D116, D122, D127). Só sobem: a
//! linha é desta máquina (o GameKave a guarda com ela de dona), só ela escreve
//! nela, e nada desce por cima de um state. Nenhum arquivo é escrito aqui.
//!
//! - A passada de states é a dela, com o trinco dela: um state grande subindo
//!   devagar nunca segura a checagem de antes de abrir um jogo, e a passada
//!   dos game saves guarda o relatório dela. Ela lê a linha desta máquina
//!   (D128) mas nunca varre temporários nem abre pasta de backup.
//! - O nome na nuvem é `<stem cru da ROM>__<quick|slotN>.state`, o stem do
//!   `.srm` dela, igual em toda máquina; o core e a versão vão em campos
//!   próprios. A miniatura sobe ao lado, com o mesmo nome mais `.png`, e só
//!   com o state já na nuvem.
//! - Um state sem a versão do core registrada fica só aqui (`unversioned`).
//! - A linha apagada no site ("apagar tudo") marca a base e deixa o arquivo:
//!   o state fica só aqui até mudar de bytes ou até a pessoa pedir
//!   (`upload_old_states`).
//! - Os bytes que sobem são os que foram lidos para o hash, numa leitura só,
//!   e só quando são os da linha que dá a versão: lidos sem mudar no meio,
//!   com a linha ainda lá antes e depois, e sem mtime mais novo que ela. Senão
//!   o save em andamento sobe na passada dele.

use super::*;
use crate::sync::local::{device_states, StateFile};
use domain::save_state::SaveStateRepository;
use std::io::Read;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

/// Por que um state fica só nesta máquina (a página conta).
pub(super) const UNVERSIONED: &str = "unversioned";
pub(super) const WIPED: &str = "wiped";
pub(super) const REJECTED: &str = "rejected";
/// Outra ROM tem um state mais novo com o mesmo nome na nuvem.
pub(super) const DUPLICATE: &str = "duplicate";

/// Quanto o mtime do arquivo pode passar do `created_at` da linha: `save`
/// grava os bytes e só depois a linha, e há disco que arredonda o mtime.
const ROW_SLACK_SECS: i64 = 2;

/// O que uma leitura de um state deixou: o hash daqueles bytes, com o
/// tamanho e o mtime de quando foram lidos. Vale enquanto os dois não mudam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Seen {
    size: u64,
    /// `None`: o arquivo mudou durante a leitura, e o hash não fica guardado.
    modified: Option<SystemTime>,
    hash: String,
    /// Segundos Unix, para o `client_mtime` e a conferência com a linha.
    mtime: i64,
}

impl Seen {
    fn local(&self) -> Local {
        Local {
            hash: self.hash.clone(),
            size: self.size,
            mtime: self.mtime,
        }
    }

    /// O que a base guardou do arquivo, quando ela tem tamanho e mtime.
    fn from_base(b: &BaseRow) -> Option<Self> {
        let size = u64::try_from(b.local_size?).ok()?;
        let ns = u64::try_from(b.local_mtime_ns?).ok()?;
        Some(Self {
            size,
            modified: Some(UNIX_EPOCH + Duration::from_nanos(ns)),
            hash: b.local_hash.clone(),
            mtime: i64::try_from(ns / 1_000_000_000).unwrap_or(0),
        })
    }

    /// O mtime em nanossegundos Unix, para a base.
    fn mtime_ns(&self) -> Option<i64> {
        self.modified
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| i64::try_from(d.as_nanos()).ok())
    }
}

/// Um state em disco agora: o que se sabe dele e, quando ele foi lido, os
/// bytes.
struct Look {
    seen: Option<Seen>,
    bytes: Option<Vec<u8>>,
    /// Lido sem mudar no meio (ou nem lido: o hash guardado valeu).
    steady: bool,
}

/// O trabalho de states de uma passada, tirado do SQLite (sem disco nem rede).
pub(super) struct StateWork {
    /// Os states do escopo, com versão ou não.
    files: Vec<StateFile>,
    /// Os do escopo que perderam o nome na nuvem para um de outra ROM.
    duplicates: Vec<StateFile>,
    /// As bases dos states do escopo e das miniaturas deles, pela chave.
    bases: HashMap<String, BaseRow>,
    /// Bases sem state nesta máquina (só na passada de todos): saem.
    orphans: Vec<String>,
    /// A plataforma da listagem quando o escopo é uma ROM.
    platform: Option<&'static str>,
    /// `None`: todos os states; senão os de uma ROM.
    rom: Option<String>,
}

impl StateWork {
    /// Há o que perguntar à nuvem: um state com versão, ou uma base a decidir.
    fn needs_network(&self) -> bool {
        self.files.iter().any(|f| f.version().is_some()) || !self.bases.is_empty()
    }
}

/// Como um state (ou a miniatura) terminou a passada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kept {
    /// Na nuvem com os bytes de agora.
    InCloud,
    /// Fica só nesta máquina, pelo motivo.
    LocalOnly(&'static str),
    /// Adiado, erro passageiro, ou o arquivo não existe mais.
    Later,
}

/// O que sobe: o state ou a miniatura dele.
struct Object<'a> {
    key: String,
    filename: String,
    path: &'a Path,
    file: &'a StateFile,
    thumb: bool,
}

impl Engine {
    /// A passada dos save states (de uma ROM, ou de todos): o trinco dela, a
    /// linha desta máquina (D128), a nuvem e cada state. Nada em disco.
    pub(super) async fn state_pass(
        &self,
        alva: &Alvanista,
        scope: &PassScope,
    ) -> Result<PassReport, SyncError> {
        let _one = self.states_lock.lock().await;
        let device = self.ready(alva).await?;
        if !self.states_on().await? {
            return Ok(PassReport::default());
        }
        let rom = match scope {
            PassScope::State(id) => Some(id.clone()),
            _ => None,
        };
        let Some(work) = self.state_work(rom).await? else {
            return Ok(PassReport::default());
        };
        let result = self.run_state_pass(alva, &device, work).await;
        self.finish_states(&result);
        result
    }

    /// Depois de fechar um jogo (D117): a passada dos game saves daquela ROM
    /// e, depois, a dos states dela (o que falhou no save de state, de novo).
    pub async fn after_close(
        &self,
        alva: &Alvanista,
        rom_id: &str,
    ) -> (Result<PassReport, SyncError>, Result<PassReport, SyncError>) {
        let saves = self.pass(alva, PassScope::Rom(rom_id.to_owned())).await;
        let states = self.pass(alva, PassScope::State(rom_id.to_owned())).await;
        (saves, states)
    }

    /// O trabalho de states do escopo. `None` quando não há state nem base de
    /// state: nada a fazer, nem a rede. A conta da página já fica certa aqui.
    async fn state_work(&self, rom: Option<String>) -> Result<Option<StateWork>, SyncError> {
        let found = device_states(&self.states, &self.roms)
            .await
            .map_err(|e| failed(format!("states locais: {e}")))?;
        let in_scope = |f: &StateFile| rom.as_deref().map_or(true, |id| f.rom_id == id);
        let files: Vec<StateFile> = found.files.into_iter().filter(in_scope).collect();
        let duplicates: Vec<StateFile> = found.duplicates.into_iter().filter(in_scope).collect();
        let keys: HashSet<String> = files
            .iter()
            .flat_map(|f| [f.key.clone(), f.thumb_key()])
            .collect();
        let mut bases = HashMap::new();
        let mut orphans = Vec::new();
        for b in self.repo.base_all("state").await.map_err(repo_err)? {
            if keys.contains(&b.key) {
                bases.insert(b.key.clone(), b);
            } else if rom.is_none() && !files.iter().any(|f| f.key == b.key) {
                orphans.push(b.key);
            }
        }
        if files.is_empty() && duplicates.is_empty() && orphans.is_empty() {
            // Nenhum state no escopo: nenhum fica só aqui (M1), e sem state
            // nenhum não há state recusado a dizer.
            self.record_local_only(rom.as_deref(), HashMap::new());
            if rom.is_none() {
                self.sync.runtime().state_error = None;
            }
            return Ok(None);
        }
        Ok(Some(StateWork {
            platform: rom
                .as_ref()
                .and(files.first().or(duplicates.first()).map(|f| f.platform)),
            files,
            duplicates,
            bases,
            orphans,
            rom,
        }))
    }

    /// A linha desta máquina (D128) só quando há o que perguntar à nuvem;
    /// depois cada state. Sem varrer temporários nem abrir pasta de backup.
    async fn run_state_pass(
        &self,
        alva: &Alvanista,
        device: &Device,
        work: StateWork,
    ) -> Result<PassReport, SyncError> {
        let mut report = PassReport::default();
        let mut cx = if work.needs_network() {
            let devices = self.sync.partner_devices(alva).await?;
            let receive = self.check_row(&devices, &device.id, true).await?;
            let token = self.token()?;
            Some(Ctx {
                alva,
                mode: Mode::Pass,
                token,
                devices,
                receive,
                uploads_stopped: false,
            })
        } else {
            None
        };
        self.run_states(cx.as_mut(), &device.id, work, &mut report)
            .await?;
        Ok(report)
    }

    /// Cada state do trabalho, com a miniatura; as bases órfãs saem; a conta
    /// dos que ficam só aqui é refeita para o escopo.
    async fn run_states(
        &self,
        mut cx: Option<&mut Ctx<'_>>,
        me: &str,
        work: StateWork,
        report: &mut PassReport,
    ) -> Result<(), SyncError> {
        let rows = match cx.as_deref() {
            Some(cx) if work.files.iter().any(|f| f.version().is_some()) => {
                self.list_states(cx, me, work.platform).await?
            }
            _ => Vec::new(),
        };
        let mut local_only: HashMap<(String, String), &'static str> = HashMap::new();
        for file in &work.duplicates {
            local_only.insert((file.rom_id.clone(), file.key.clone()), DUPLICATE);
        }
        for file in &work.files {
            let reason = match (file.version(), cx.as_deref_mut()) {
                // D122: sem a versão do core, outra máquina não saberia se ele
                // carrega.
                (None, _) => Some(UNVERSIONED),
                (Some(_), Some(cx)) => {
                    self.state_step(cx, me, file, &work.bases, &rows, report)
                        .await?
                }
                (Some(_), None) => None,
            };
            if let Some(reason) = reason {
                local_only.insert((file.rom_id.clone(), file.key.clone()), reason);
            }
        }
        for key in &work.orphans {
            // O state saiu desta máquina: a linha da nuvem fica, só a base sai.
            if let Err(e) = self.repo.base_delete(key).await {
                report.errors.push((key.clone(), repo_err(e)));
            }
        }
        self.record_local_only(work.rom.as_deref(), local_only);
        Ok(())
    }

    /// A conta da página: a passada de todos troca a lista inteira, a de uma
    /// ROM só as entradas dela.
    fn record_local_only(
        &self,
        rom: Option<&str>,
        entries: HashMap<(String, String), &'static str>,
    ) {
        let mut rt = self.sync.runtime();
        match rom {
            None => rt.states_local = entries,
            Some(id) => {
                rt.states_local.retain(|(r, _), _| r != id);
                rt.states_local.extend(entries);
            }
        }
    }

    /// O que a página lê depois de uma passada de states. Não mexe no que a
    /// passada dos game saves disse (`last_error`, `last_pass_at`, o aviso de
    /// enviados): só no aviso dos states, e só com o que a pessoa pode fazer
    /// algo (um state recusado, a cota, o plano). Uma falha de rede fica no
    /// log; a próxima passada tenta de novo.
    fn finish_states(&self, result: &Result<PassReport, SyncError>) {
        match result {
            Ok(report) => {
                let shown = report
                    .errors
                    .iter()
                    .filter(|(_, e)| {
                        matches!(
                            e,
                            SyncError::StateTooLarge
                                | SyncError::Rejected
                                | SyncError::Quota
                                | SyncError::NoTier
                        )
                    })
                    .max_by_key(|(_, e)| severity(e));
                self.sync.runtime().state_error = shown.map(|(key, e)| {
                    let file = key_file_name(key).filter(|_| is_permanent(e));
                    (e.clone(), file.map(str::to_owned))
                });
            }
            Err(e) => log::info!("sync: passada dos states terminou em {e}"),
        }
        self.emit(SyncEvent::Status);
    }

    /// As linhas de state desta máquina, as apagadas no site inclusive (sem
    /// elas um "apagar tudo" pareceria "não existe" e tudo subiria de novo).
    async fn list_states(
        &self,
        cx: &Ctx<'_>,
        me: &str,
        platform: Option<&str>,
    ) -> Result<Vec<RemoteFile>, SyncError> {
        let filter = FileFilter {
            kind: "state",
            platform,
            device_token_id: Some(me),
            include_deleted: true,
            ..FileFilter::default()
        };
        match self.sync.api.list_files(&cx.token, &filter).await {
            Ok(rows) => Ok(rows
                .into_iter()
                .filter(|r| r.kind == "state" && r.device_token_id.as_deref() == Some(me))
                .collect()),
            Err(err) => Err(self.ending(cx.alva, err).await),
        }
    }

    /// Um state com versão e a miniatura dele. Devolve o motivo quando o
    /// state fica só nesta máquina.
    async fn state_step(
        &self,
        cx: &mut Ctx<'_>,
        me: &str,
        file: &StateFile,
        bases: &HashMap<String, BaseRow>,
        rows: &[RemoteFile],
        report: &mut PassReport,
    ) -> Result<Option<&'static str>, SyncError> {
        let state = Object {
            key: file.key.clone(),
            filename: file.filename.clone(),
            path: &file.path,
            file,
            thumb: false,
        };
        match self
            .state_object(cx, me, &state, bases, rows, report)
            .await?
        {
            Kept::LocalOnly(WIPED) => {
                // A miniatura segue o state: fica marcada junto, e só sobe de
                // novo com ele.
                let thumb_key = file.thumb_key();
                if bases.get(&thumb_key).is_some_and(|b| b.wiped_at.is_none()) {
                    if let Err(e) = self
                        .base_only(&thumb_key, Action::MarkBaseWipedKeepLocal)
                        .await
                    {
                        report.errors.push((thumb_key, e));
                    }
                }
                return Ok(Some(WIPED));
            }
            Kept::LocalOnly(reason) => return Ok(Some(reason)),
            Kept::Later => return Ok(None),
            Kept::InCloud => {}
        }
        match &file.thumbnail {
            // A miniatura só acompanha um state que está na nuvem.
            Some(path) => {
                let thumb = Object {
                    key: file.thumb_key(),
                    filename: file.thumb_filename(),
                    path,
                    file,
                    thumb: true,
                };
                self.state_object(cx, me, &thumb, bases, rows, report)
                    .await?;
            }
            // Gravado de novo sem miniatura: a base da antiga sai.
            None if bases.contains_key(&file.thumb_key()) => {
                if let Err(e) = self.repo.base_delete(&file.thumb_key()).await {
                    report.errors.push((file.thumb_key(), repo_err(e)));
                }
            }
            None => {}
        }
        Ok(None)
    }

    /// Decide e age sobre um objeto: só sobe, adota, marca ou esquece a base.
    async fn state_object(
        &self,
        cx: &mut Ctx<'_>,
        me: &str,
        obj: &Object<'_>,
        bases: &HashMap<String, BaseRow>,
        rows: &[RemoteFile],
        report: &mut PassReport,
    ) -> Result<Kept, SyncError> {
        let base = bases.get(&obj.key);
        let version = obj.file.version().unwrap_or_default();
        let remote = state_row(rows, obj, version, base).and_then(remote_of);
        // A linha antes de ler o arquivo: com ela de novo depois da leitura, os
        // bytes são os dela (a vaga gravada de novo apaga a linha antes de
        // escrever os bytes).
        let before = match self.states.get_state(&obj.file.id).await {
            Ok(Some(row)) => row,
            Ok(None) => return Ok(Kept::Later),
            Err(e) => {
                report.errors.push((obj.key.clone(), repo_err(e)));
                return Ok(Kept::Later);
            }
        };
        let decide_on = |look: &Look| {
            let local = look.seen.as_ref().map(Seen::local);
            decide(Kind::State, local.as_ref(), base, remote.as_ref(), me)
        };
        let unreadable = |report: &mut PassReport, err: io::Error| {
            report.errors.push((
                obj.key.clone(),
                failed(format!("{}: {err}", log_tag(&obj.key))),
            ));
            Kept::Later
        };
        let look = match self.look_state(obj.path, base, false).await {
            Ok(look) => look,
            Err(err) => return Ok(unreadable(report, err)),
        };
        let action = decide_on(&look);
        let base_err = |report: &mut PassReport, e: SyncError| {
            report.errors.push((obj.key.clone(), e));
            Kept::Later
        };
        Ok(match action {
            Action::Nothing => match (&look.seen, base) {
                (None, _) => Kept::Later,
                (Some(_), Some(b)) if b.wiped_at.is_some() => Kept::LocalOnly(WIPED),
                (Some(_), _) => Kept::InCloud,
            },
            Action::Adopt => match (&look.seen, &remote) {
                (Some(seen), Some(r)) => {
                    match self
                        .put_state_base(obj, seen, &r.save_id, &r.version_id, &r.hash)
                        .await
                    {
                        Ok(()) => Kept::InCloud,
                        Err(e) => base_err(report, e),
                    }
                }
                _ => Kept::Later,
            },
            Action::Upload | Action::ReUpload => {
                self.upload_state(cx, obj, &before, look, &decide_on, report)
                    .await?
            }
            Action::MarkBaseWipedKeepLocal => match self.base_only(&obj.key, action).await {
                Ok(()) => Kept::LocalOnly(WIPED),
                Err(e) => base_err(report, e),
            },
            Action::DropBase => match self.base_only(&obj.key, action).await {
                Ok(()) => Kept::Later,
                Err(e) => base_err(report, e),
            },
            // `decide` nunca dá estas para um state; nada se faz.
            Action::Download | Action::Conflict | Action::LocalToBackupAndClearBase => {
                log::warn!(
                    "sync: {} decidiu {action:?} para um state; nada feito",
                    log_tag(&obj.key)
                );
                Kept::Later
            }
        })
    }

    /// Lê os bytes de agora (uma vez), confere que são os da linha que dá a
    /// versão, manda e, com o 201, grava a base com o hash e o stat deles.
    async fn upload_state(
        &self,
        cx: &mut Ctx<'_>,
        obj: &Object<'_>,
        before: &domain::save_state::SaveStateMetadata,
        look: Look,
        decide_on: &impl Fn(&Look) -> Action,
        report: &mut PassReport,
    ) -> Result<Kept, SyncError> {
        if cx.uploads_stopped {
            report.deferred += 1;
            return Ok(Kept::Later);
        }
        // Maior que o teto que um 413 já disse: nem é enviado.
        let ceiling = *lock(&self.state_ceiling);
        if let (Some(max), Some(seen)) = (ceiling, &look.seen) {
            if i64::try_from(seen.size).unwrap_or(i64::MAX) > max {
                report
                    .errors
                    .push((obj.key.clone(), SyncError::StateTooLarge));
                return Ok(Kept::LocalOnly(REJECTED));
            }
        }
        let deferred = |report: &mut PassReport, why: &str| {
            log::info!("sync: {} adiado: {why}", log_tag(&obj.key));
            report.deferred += 1;
            Kept::Later
        };
        let look = if look.bytes.is_some() {
            look
        } else {
            match self.look_state(obj.path, None, true).await {
                Ok(look) => look,
                Err(err) => {
                    report.errors.push((
                        obj.key.clone(),
                        failed(format!("{}: {err}", log_tag(&obj.key))),
                    ));
                    return Ok(Kept::Later);
                }
            }
        };
        let Ok(Some(after)) = self.states.get_state(&obj.file.id).await else {
            return Ok(deferred(report, "a vaga foi gravada de novo"));
        };
        let (Some(seen), Some(bytes)) = (&look.seen, &look.bytes) else {
            return Ok(Kept::Later);
        };
        if !look.steady || seen.mtime > after.created_at + ROW_SLACK_SECS || *before != after {
            // Um save está gravando estes bytes agora (ou já gravou depois da
            // linha): a passada dele os sobe com a linha certa.
            return Ok(deferred(report, "o arquivo mudou durante a leitura"));
        }
        // Com os bytes lidos a decisão pode mudar (eles podem ser os da base).
        match decide_on(&look) {
            Action::Upload | Action::ReUpload => {}
            Action::Adopt | Action::Nothing => return Ok(Kept::Later),
            other => {
                log::info!(
                    "sync: {} decidiu {other:?} depois de ler; fica para a próxima",
                    log_tag(&obj.key)
                );
                return Ok(Kept::Later);
            }
        }
        // A versão é a da linha lida junto com os bytes.
        let Some(version) = after
            .core_version
            .as_deref()
            .filter(|v| !v.trim().is_empty())
        else {
            return Ok(Kept::LocalOnly(UNVERSIONED));
        };
        if bytes.is_empty() {
            report.deferred += 1;
            return Ok(Kept::Later);
        }
        // Os mesmos bytes que a nuvem já recusou de vez não sobem de novo.
        if let Some(err) = self.still_rejected(&obj.key, &seen.hash) {
            report.errors.push((obj.key.clone(), err));
            return Ok(Kept::LocalOnly(REJECTED));
        }
        let meta = UploadMeta {
            platform: obj.file.platform,
            kind: "state",
            filename: &obj.filename,
            core: Some(&obj.file.core),
            core_version: Some(version),
            client_mtime: Some(rfc3339(seen.mtime)),
        };
        let seen = seen.clone();
        let up: Uploaded = match self
            .upload_bytes(
                cx.alva,
                &cx.token,
                &obj.key,
                &meta,
                bytes.clone(),
                &seen.hash,
            )
            .await
        {
            Ok(up) => up,
            Err(Fail::File(e)) => {
                let kept = if is_permanent(&e) {
                    Kept::LocalOnly(REJECTED)
                } else {
                    Kept::Later
                };
                report.errors.push((obj.key.clone(), e));
                return Ok(kept);
            }
            Err(Fail::StopUploads(e)) => {
                cx.uploads_stopped = true;
                if e == SyncError::Quota {
                    self.quota_event().await;
                }
                report.errors.push((obj.key.clone(), e));
                return Ok(Kept::Later);
            }
            Err(Fail::End(e)) => return Err(e),
        };
        if let Err(e) = self
            .put_state_base(obj, &seen, &up.save_id, &up.version_id, &seen.hash)
            .await
        {
            report.errors.push((obj.key.clone(), e));
            return Ok(Kept::Later);
        }
        if !obj.thumb {
            report.states_uploaded += 1;
        }
        Ok(Kept::InCloud)
    }

    /// A base de um objeto, com o tamanho e o mtime dos bytes: com os dois
    /// iguais no disco, a próxima passada (mesmo depois de abrir o app de
    /// novo) não relê o arquivo.
    async fn put_state_base(
        &self,
        obj: &Object<'_>,
        seen: &Seen,
        save_id: &str,
        version_id: &str,
        remote_hash: &str,
    ) -> Result<(), SyncError> {
        self.repo
            .base_put(&BaseRow {
                key: obj.key.clone(),
                kind: "state".into(),
                remote_save_id: save_id.into(),
                remote_version_id: version_id.into(),
                remote_hash: remote_hash.to_ascii_lowercase(),
                local_hash: seen.hash.clone(),
                local_path: obj.path.to_string_lossy().into_owned(),
                rom_crc32: None,
                synced_at: now_secs(),
                wiped_at: None,
                backup_path: None,
                local_size: i64::try_from(seen.size).ok(),
                local_mtime_ns: seen.mtime_ns(),
            })
            .await
            .map_err(repo_err)
    }

    /// O state em disco, fora das tarefas do Tokio. Com o tamanho e o mtime
    /// de uma leitura anterior (desta sessão, ou da base), o hash dela vale;
    /// `read`: os bytes são lidos assim mesmo (para subir).
    async fn look_state(
        &self,
        path: &Path,
        base: Option<&BaseRow>,
        read: bool,
    ) -> io::Result<Look> {
        let known = lock(&self.state_hashes)
            .get(path)
            .cloned()
            .or_else(|| base.and_then(Seen::from_base));
        let at = path.to_path_buf();
        let look = tokio::task::spawn_blocking(move || look_at(&at, known, read))
            .await
            .unwrap_or_else(|j| Err(io::Error::other(j.to_string())))?;
        let mut cache = lock(&self.state_hashes);
        match &look.seen {
            Some(seen) if seen.modified.is_some() => {
                cache.insert(path.to_path_buf(), seen.clone());
            }
            Some(_) => {}
            None => {
                cache.remove(path);
            }
        }
        Ok(look)
    }

    /// "Enviar os states antigos também" (D127): as bases que um "apagar
    /// tudo" no site marcou saem, e esses states sobem como novos. Com os
    /// states desligados nada muda.
    pub async fn upload_old_states(&self, alva: &Alvanista) -> Result<PassReport, SyncError> {
        {
            let _one = self.states_lock.lock().await;
            self.ready(alva).await?;
            if !self.states_on().await? {
                return Ok(PassReport::default());
            }
            for b in self.repo.base_all("state").await.map_err(repo_err)? {
                if b.wiped_at.is_some() {
                    self.repo.base_delete(&b.key).await.map_err(repo_err)?;
                }
            }
        }
        self.pass(alva, PassScope::States).await
    }

    /// A senha de uma gravação de state da ROM; a mais nova é a que vale.
    pub(super) fn state_ticket(&self, rom_id: &str) -> u64 {
        let ticket = self.state_tickets.fetch_add(1, Ordering::Relaxed) + 1;
        lock(&self.state_bursts).insert(rom_id.to_owned(), ticket);
        ticket
    }

    /// Espera a rajada aquietar. `true` só para a última gravação da ROM: as
    /// de antes desistem, e uma passada só sobe os bytes de agora.
    pub(super) async fn state_settled(&self, rom_id: &str, ticket: u64) -> bool {
        tokio::time::sleep(self.state_debounce).await;
        lock(&self.state_bursts).get(rom_id) == Some(&ticket)
    }
}

/// A linha da nuvem de um objeto: a que a base conhece, se ainda listada
/// (viva ou apagada no site); senão a viva com o mesmo core e a mesma versão,
/// que é a identidade dela no GameKave.
fn state_row<'a>(
    rows: &'a [RemoteFile],
    obj: &Object<'_>,
    version: &str,
    base: Option<&BaseRow>,
) -> Option<&'a RemoteFile> {
    let same = |r: &&'a RemoteFile| {
        r.platform == obj.file.platform
            && r.core.as_deref() == Some(obj.file.core.as_str())
            && r.filename == obj.filename
    };
    if let Some(b) = base {
        if let Some(r) = rows
            .iter()
            .filter(same)
            .find(|r| r.save_id == b.remote_save_id)
        {
            return Some(r);
        }
    }
    rows.iter()
        .filter(same)
        .filter(|r| r.deleted_at.is_none() && r.core_version.as_deref() == Some(version))
        .max_by(|a, b| rank(a).cmp(&rank(b)))
}

/// O state em disco. Sem `read` e com o tamanho e o mtime de antes, vale o
/// hash guardado; senão os bytes são lidos uma vez e o hash é deles.
fn look_at(path: &Path, known: Option<Seen>, read: bool) -> io::Result<Look> {
    let absent = Look {
        seen: None,
        bytes: None,
        steady: true,
    };
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(absent),
        Err(e) => return Err(e),
    };
    let modified = meta.modified().ok();
    if !read {
        if let Some(seen) =
            known.filter(|k| modified.is_some() && k.modified == modified && k.size == meta.len())
        {
            return Ok(Look {
                seen: Some(seen),
                bytes: None,
                steady: true,
            });
        }
    }
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(absent),
        Err(e) => return Err(e),
    };
    let before = file.metadata()?;
    let mut bytes = Vec::with_capacity(before.len() as usize);
    file.read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let modified = after.modified().ok();
    // Gravado durante a leitura: o hash vale para estes bytes, mas eles não
    // sobem nem ficam guardados para o arquivo.
    let steady = modified.is_some()
        && before.modified().ok() == modified
        && before.len() == after.len()
        && after.len() == bytes.len() as u64;
    let mtime = modified
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    Ok(Look {
        seen: Some(Seen {
            size: bytes.len() as u64,
            modified: modified.filter(|_| steady),
            hash: sha256_hex(&bytes),
            mtime,
        }),
        bytes: Some(bytes),
        steady,
    })
}
