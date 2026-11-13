//! Os save states desta máquina na nuvem (D116, D122, D127). Só sobem: a
//! linha é desta máquina (o GameKave a guarda com ela de dona), só ela escreve
//! nela, e nada desce por cima de um state. Nenhum arquivo é escrito aqui, por
//! isso subir com um jogo aberto vale.
//!
//! - O nome na nuvem é `<stem da ROM>__<quick|slotN>.state`, igual em toda
//!   máquina; o core e a versão dele vão em campos próprios. A miniatura sobe
//!   ao lado, com o mesmo nome mais `.png`, e só com o state já na nuvem.
//! - Um state sem a versão do core registrada fica só aqui (`unversioned`):
//!   outra máquina não saberia se ele carrega.
//! - A linha apagada no site ("apagar tudo") marca a base e deixa o arquivo:
//!   o state fica só aqui até mudar de bytes ou até a pessoa pedir
//!   (`upload_old_states`).
//! - Os bytes que sobem são os que foram lidos para o hash, numa leitura só.

use super::*;
use crate::sync::local::{device_states, StateFile};
use std::io::Read;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

/// Por que um state fica só nesta máquina (a página conta).
pub(super) const UNVERSIONED: &str = "unversioned";
pub(super) const WIPED: &str = "wiped";
pub(super) const REJECTED: &str = "rejected";

/// O que a última leitura de um state deixou: o hash daqueles bytes, com o
/// tamanho e o mtime de quando foram lidos. Vale enquanto os dois não mudam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Seen {
    size: u64,
    /// `None`: o arquivo mudou durante a leitura, e o hash não fica guardado.
    modified: Option<SystemTime>,
    hash: String,
    /// Segundos Unix, para o `client_mtime`.
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
}

/// Um state em disco agora: o que se sabe dele e, quando ele foi lido, os
/// bytes.
struct Look {
    seen: Option<Seen>,
    bytes: Option<Vec<u8>>,
}

/// O trabalho de states de uma passada, tirado do SQLite (sem disco nem rede).
pub(super) struct StateWork {
    /// Os states do escopo, com versão ou não.
    files: Vec<StateFile>,
    /// As bases dos states do escopo e das miniaturas deles, pela chave.
    bases: HashMap<String, BaseRow>,
    /// Bases sem state nesta máquina (só na passada de todos): saem.
    orphans: Vec<String>,
    /// A plataforma da listagem quando o escopo é uma ROM.
    platform: Option<&'static str>,
    /// A passada é de todos os states: a contagem da página é refeita.
    whole: bool,
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
    version: &'a str,
    thumb: bool,
}

impl Engine {
    /// O trabalho de states do escopo. `None` quando não há state nem base de
    /// state: nada a fazer, nem a rede.
    pub(super) async fn state_work(
        &self,
        scope: &PassScope,
    ) -> Result<Option<StateWork>, SyncError> {
        let rom = match scope {
            PassScope::State(id) => Some(id.as_str()),
            _ => None,
        };
        let files = device_states(&self.states, &self.roms, rom)
            .await
            .map_err(|e| failed(format!("states locais: {e}")))?;
        let keys: HashSet<String> = files
            .iter()
            .flat_map(|f| [f.key.clone(), f.thumb_key()])
            .collect();
        let mut bases = HashMap::new();
        let mut orphans = Vec::new();
        for b in self.repo.base_all("state").await.map_err(repo_err)? {
            if keys.contains(&b.key) {
                bases.insert(b.key.clone(), b);
            } else if rom.is_none() {
                orphans.push(b.key);
            }
        }
        if files.is_empty() && orphans.is_empty() {
            return Ok(None);
        }
        Ok(Some(StateWork {
            platform: rom.and(files.first().map(|f| f.platform)),
            files,
            bases,
            orphans,
            whole: rom.is_none(),
        }))
    }

    /// Os states da passada: as linhas desta máquina na nuvem e cada state,
    /// com a miniatura.
    pub(super) async fn run_states(
        &self,
        cx: &mut Ctx<'_>,
        me: &str,
        work: StateWork,
        report: &mut PassReport,
    ) -> Result<(), SyncError> {
        // Sem state com versão nada sobe: a nuvem nem é lida.
        let rows = if work.files.iter().any(|f| f.version().is_some()) {
            self.list_states(cx, me, work.platform).await?
        } else {
            Vec::new()
        };
        let mut local_only: HashMap<String, &'static str> = HashMap::new();
        for file in &work.files {
            if let Some(reason) = self
                .state_step(cx, me, file, &work.bases, &rows, report)
                .await?
            {
                local_only.insert(file.key.clone(), reason);
            }
        }
        for key in &work.orphans {
            // O state saiu desta máquina: a linha da nuvem fica, só a base sai.
            if let Err(e) = self.repo.base_delete(key).await {
                report.errors.push((key.clone(), repo_err(e)));
            }
        }
        let mut rt = self.sync.runtime();
        if work.whole {
            rt.states_local = local_only;
        } else {
            for f in &work.files {
                rt.states_local.remove(&f.key);
            }
            rt.states_local.extend(local_only);
        }
        Ok(())
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

    /// Um state e a miniatura dele. Devolve o motivo quando o state fica só
    /// nesta máquina.
    async fn state_step(
        &self,
        cx: &mut Ctx<'_>,
        me: &str,
        file: &StateFile,
        bases: &HashMap<String, BaseRow>,
        rows: &[RemoteFile],
        report: &mut PassReport,
    ) -> Result<Option<&'static str>, SyncError> {
        // D122: sem a versão do core, outra máquina não saberia se ele carrega.
        let Some(version) = file.version() else {
            return Ok(Some(UNVERSIONED));
        };
        let state = Object {
            key: file.key.clone(),
            filename: file.filename.clone(),
            path: &file.path,
            file,
            version,
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
                    version,
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
        let remote = state_row(rows, obj, base).and_then(remote_of);
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
        let mut look = match self.look_state(obj.path, false).await {
            Ok(look) => look,
            Err(err) => return Ok(unreadable(report, err)),
        };
        let mut action = decide_on(&look);
        if matches!(action, Action::Upload | Action::ReUpload) && look.bytes.is_none() {
            // Para subir, os bytes de agora: o hash é deles, e são eles que vão.
            look = match self.look_state(obj.path, true).await {
                Ok(look) => look,
                Err(err) => return Ok(unreadable(report, err)),
            };
            action = decide_on(&look);
        }
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
                        .put_state_base(obj, &seen.hash, &r.save_id, &r.version_id, &r.hash)
                        .await
                    {
                        Ok(()) => Kept::InCloud,
                        Err(e) => base_err(report, e),
                    }
                }
                _ => Kept::Later,
            },
            Action::Upload | Action::ReUpload => self.upload_state(cx, obj, look, report).await?,
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

    /// Manda os bytes lidos e, com o 201, grava a base com o hash deles.
    async fn upload_state(
        &self,
        cx: &mut Ctx<'_>,
        obj: &Object<'_>,
        look: Look,
        report: &mut PassReport,
    ) -> Result<Kept, SyncError> {
        let (Some(seen), Some(bytes)) = (look.seen, look.bytes) else {
            return Ok(Kept::Later);
        };
        // Arquivo vazio não é um state; a cota ou o plano pararam os envios.
        if bytes.is_empty() || cx.uploads_stopped {
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
            core_version: Some(obj.version),
            client_mtime: Some(rfc3339(seen.mtime)),
        };
        let up: Uploaded = match self
            .upload_bytes(cx.alva, &cx.token, &obj.key, &meta, bytes, &seen.hash)
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
            .put_state_base(obj, &seen.hash, &up.save_id, &up.version_id, &seen.hash)
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

    async fn put_state_base(
        &self,
        obj: &Object<'_>,
        local_hash: &str,
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
                local_hash: local_hash.into(),
                local_path: obj.path.to_string_lossy().into_owned(),
                rom_crc32: None,
                synced_at: now_secs(),
                wiped_at: None,
                backup_path: None,
            })
            .await
            .map_err(repo_err)
    }

    /// O state em disco, fora das tarefas do Tokio. `read`: os bytes são
    /// lidos mesmo com o hash guardado (para subir).
    async fn look_state(&self, path: &Path, read: bool) -> io::Result<Look> {
        let cached = lock(&self.state_hashes).get(path).cloned();
        let at = path.to_path_buf();
        let look = tokio::task::spawn_blocking(move || look_at(&at, cached, read))
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
            let _one = self.one_at_a_time.lock().await;
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
        .filter(|r| r.deleted_at.is_none() && r.core_version.as_deref() == Some(obj.version))
        .max_by(|a, b| rank(a).cmp(&rank(b)))
}

/// O state em disco. Sem `read` e com o tamanho e o mtime de antes, vale o
/// hash guardado; senão os bytes são lidos uma vez e o hash é deles.
fn look_at(path: &Path, cached: Option<Seen>, read: bool) -> io::Result<Look> {
    let absent = Look {
        seen: None,
        bytes: None,
    };
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(absent),
        Err(e) => return Err(e),
    };
    let modified = meta.modified().ok();
    if !read {
        if let Some(seen) =
            cached.filter(|c| modified.is_some() && c.modified == modified && c.size == meta.len())
        {
            return Ok(Look {
                seen: Some(seen),
                bytes: None,
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
    // Gravado durante a leitura: o hash vale para estes bytes, mas não fica
    // guardado para o arquivo.
    let steady = before.modified().ok() == modified && after.len() == bytes.len() as u64;
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
    })
}
