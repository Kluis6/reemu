//! O que a pessoa pede pela tela: resolver um conflito, voltar uma versão
//! pelo histórico, listar versões e conflitos e o estado do save de um jogo.
//! Saiu de `engine.rs` sem mudança de comportamento.

use super::*;

impl Engine {
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

    pub(super) async fn keep_remote(
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
    pub(super) async fn file_for(&self, key: &str) -> Result<SaveFile, SyncError> {
        let library = local::library(&self.roms, &self.dirs.saves, &self.dirs.system)
            .await
            .map_err(|e| failed(format!("saves locais: {e}")))?;
        library
            .slot_for_key(key)
            .ok_or_else(|| failed(format!("{}: arquivo fora da biblioteca", log_tag(key))))
    }

    /// Depois de resolver ou restaurar: a página relê (a contagem de
    /// conflitos vem do SQLite).
    pub(super) async fn after_change(&self) {
        self.emit(SyncEvent::Status);
    }
}
