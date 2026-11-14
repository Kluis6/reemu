//! Um arquivo por vez dentro da passada: subir, descer, aceitar o que a
//! nuvem apagou, guardar o conflito e andar a base. Saiu de `engine.rs` sem
//! mudança de comportamento.

use super::*;

impl Engine {
    pub(super) async fn step(
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
    pub(super) async fn quota_event(&self) {
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
    pub(super) async fn adopt(
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

    pub(super) async fn upload_step(
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
    pub(super) fn still_rejected(&self, key: &str, hash: &str) -> Option<SyncError> {
        lock(&self.rejected)
            .get(key)
            .filter(|(seen, _)| seen == hash)
            .map(|(_, err)| err.clone())
    }

    /// Lê o arquivo, manda, e só com o 201 grava a base.
    pub(super) async fn send(
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
        let up = self
            .upload_bytes(alva, token, &file.key, &meta, bytes, &hash)
            .await?;
        self.put_base(file, &hash, &up.save_id, &up.version_id, &hash)
            .await
            .map_err(Fail::File)?;
        Ok(Done::Uploaded)
    }

    /// Manda os bytes de uma chave. A recusa de vez (413, 400, 422) fica
    /// lembrada para estes bytes (`hash`); um envio que passa a esquece. O 413
    /// de um state é o teto de objeto da nuvem, não o da plataforma.
    pub(super) async fn upload_bytes(
        &self,
        alva: &Alvanista,
        token: &str,
        key: &str,
        meta: &UploadMeta<'_>,
        bytes: Vec<u8>,
        hash: &str,
    ) -> Result<Uploaded, Fail> {
        match self.sync.api.upload(token, meta, bytes).await {
            Ok(up) => {
                lock(&self.rejected).remove(key);
                if up.deduplicated {
                    log::debug!("sync: {} já estava na nuvem", log_tag(key));
                }
                Ok(up)
            }
            Err(err) => {
                if let (
                    ApiError::TooLarge {
                        max_bytes: Some(max),
                    },
                    "state",
                ) = (&err, meta.kind)
                {
                    *lock(&self.state_ceiling) = Some(*max);
                }
                let fail = match err {
                    // Um state que falha na rede fica para a próxima, e a
                    // passada segue com os outros (M1); quem conta as falhas
                    // seguidas é a passada de states.
                    ApiError::Transient(e) if meta.kind == "state" => {
                        log::info!("sync: {} não subiu agora: {e}", log_tag(key));
                        Fail::File(SyncError::Offline)
                    }
                    err => match self.file_error(alva, err, true).await {
                        Fail::File(SyncError::TooLarge) if meta.kind == "state" => {
                            Fail::File(SyncError::StateTooLarge)
                        }
                        other => other,
                    },
                };
                if let Fail::File(e) = &fail {
                    if is_permanent(e) {
                        lock(&self.rejected).insert(key.to_owned(), (hash.to_owned(), e.clone()));
                    }
                }
                Err(fail)
            }
        }
    }

    pub(super) async fn download_step(
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
    pub(super) fn place(
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
    pub(super) async fn accept_deletion(&self, cx: &Ctx<'_>, e: &Entry) -> Result<Done, Fail> {
        if !receives(cx, e.file.platform) || !self.may_write(cx.mode) {
            return Ok(Done::Deferred);
        }
        self.removed_on_site(&e.file).await.map_err(Fail::File)?;
        Ok(Done::BackedUp)
    }

    /// Muda o arquivo daqui para o backup e deixa a base como marca de
    /// "apagado no site", com o lugar para onde ele foi.
    pub(super) async fn removed_on_site(&self, file: &SaveFile) -> Result<(), SyncError> {
        let moved = self.clear_away(file)?;
        let moved = moved.map(|p| p.to_string_lossy().into_owned());
        self.repo
            .base_mark_wiped(&file.key, now_secs(), moved.as_deref())
            .await
            .map_err(repo_err)
    }

    /// Muda o arquivo (se existir) para a pasta de backup; devolve para onde.
    pub(super) fn clear_away(&self, file: &SaveFile) -> Result<Option<PathBuf>, SyncError> {
        let fail = |err: io::Error| failed(format!("backup de {}: {err}", log_tag(&file.key)));
        if present(&file.path).map_err(fail)? {
            return self.move_to_backup(&file.path).map(Some).map_err(fail);
        }
        Ok(None)
    }

    pub(super) async fn conflict_step(
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
    pub(super) async fn base_only(&self, key: &str, action: Action) -> Result<(), SyncError> {
        let done = match action {
            Action::MarkBaseWipedKeepLocal => {
                self.repo.base_mark_wiped(key, now_secs(), None).await
            }
            Action::DropBase => self.repo.base_delete(key).await,
            _ => Ok(()),
        };
        done.map_err(repo_err)
    }

    pub(super) async fn put_base(
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
                local_size: None,
                local_mtime_ns: None,
                local_row: None,
            })
            .await
            .map_err(repo_err)
    }

    /// Escrever save só sem jogo aberto. Antes de abrir, quem espera é o
    /// próprio jogo que está abrindo: aí vale.
    pub(super) fn may_write(&self, mode: Mode) -> bool {
        match self.playing() {
            Playing::Idle => true,
            Playing::Loading => mode == Mode::BeforeLaunch,
            Playing::Running(_) => false,
        }
    }

    /// Ler para subir vale com jogo aberto, menos os arquivos dele e os
    /// cartões e VMUs (que o core pode estar gravando).
    pub(super) fn may_upload(&self, file: &SaveFile) -> bool {
        match self.playing() {
            Playing::Idle => true,
            Playing::Loading | Playing::Running(None) => false,
            Playing::Running(Some(rom)) => file.rom_id.as_deref().is_some_and(|r| r != rom),
        }
    }
}
