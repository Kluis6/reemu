//! Os save states das outras máquinas (D116, D122). A página do jogo e o menu
//! de pausa listam o que as outras máquinas da conta subiram para o jogo; o
//! que bate com o core daqui, na versão exata, desce para uma vaga local.
//!
//! - Uma listagem só, `kind=state&platform=<plataforma da ROM>`, de todas as
//!   máquinas: o GameKave compara o `filename` por igualdade, então não há
//!   como pedir "todas as vagas de um nome". O filtro por nome é daqui.
//! - O nome na nuvem é `<stem cru da ROM>__<quick|slotN>.state`; a miniatura
//!   é o mesmo nome mais `.png`. Os rows desta máquina e de outros jogos
//!   ficam de fora.
//! - Só a linha mais nova de cada (máquina, nome, core): atualizar o core
//!   deixa a linha da versão velha viva na nuvem, e a página nunca oferece
//!   uma cópia mais velha da vaga.
//! - Ler nunca muda nada: um erro de listagem dá lista vazia e `offline`,
//!   sem tocar no aparelho, na base ou no disco.
//! - Descer: o state de agora da vaga (se há) vai para
//!   `<dados>/sync-backups/<carimbo>/states/` ANTES de ser trocado, e a troca
//!   passa por `save_state::save` com a versão do core instalada daqui: o
//!   state vira desta máquina e sobe de novo como dela (D116).

use super::*;
use crate::sync::api::{FileFilter, RemoteFile};
use crate::sync::local::rom_stem;
use domain::save_state::{SaveStateMetadata, SaveStateRepository};

/// Um state de outra máquina, como a tela o recebe.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct RemoteState {
    /// O `save_id` da linha na nuvem.
    pub id: String,
    /// O nome da máquina que o gravou; vazio quando a nuvem não o diz.
    pub machine: String,
    /// A vaga local que ele ocupa ao descer: `slotN` é `Some(N)` (`slot0` é o
    /// QuickSave do ReEmu) e `quick` é `None`.
    pub slot: Option<u32>,
    /// Segundos Unix, da versão atual na nuvem.
    pub created_at: i64,
    pub core: String,
    /// `None`: foi gravado sem a versão do core registrada.
    pub core_version: Option<String>,
    /// O core é o escolhido aqui e a versão é igual, letra por letra (D122).
    pub available: bool,
    /// `other_core`, `other_version` ou `unversioned`; `None` quando disponível.
    pub reason: Option<String>,
    pub has_thumbnail: bool,
    pub size: i64,
}

/// A resposta de `sync_states_for`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct RemoteStates {
    /// Os mais novos primeiro. Vazia quando nada foi lido.
    pub states: Vec<RemoteState>,
    /// Nada foi lido: sync desligado, states desligados, conta desconhecida
    /// ou a listagem falhou. A página não mostra nada nem alarma.
    pub offline: bool,
    /// A versão do core escolhido que está instalada aqui, para a linha
    /// "{{versão}} aqui".
    pub installed_version: Option<String>,
}

/// Um state achado, com o que o download e a miniatura precisam.
#[derive(Debug, Clone)]
pub(in crate::sync) struct Found {
    state: RemoteState,
    version_id: String,
    hash: String,
    thumb: Option<Thumb>,
}

#[derive(Debug, Clone)]
struct Thumb {
    save_id: String,
    version_id: String,
    size: i64,
}

/// `Some(vaga)` quando `filename` é `<stem>__<quick|slotN>.state`.
fn slot_of(filename: &str, stem: &str) -> Option<Option<u32>> {
    let rest = filename.strip_prefix(stem)?.strip_prefix("__")?;
    let slot = rest.strip_suffix(".state")?;
    if slot == "quick" {
        return Some(None);
    }
    let n = slot.strip_prefix("slot")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok().map(Some)
}

/// Quando a linha foi escrita, para escolher a mais nova.
fn written(f: &RemoteFile) -> (i64, u32) {
    f.current
        .as_ref()
        .and_then(|v| parse_time(&v.server_created_at))
        .unwrap_or((0, 0))
}

fn newest<'a>(
    rows: impl Iterator<Item = (&'a RemoteFile, String, String, String)>,
) -> HashMap<(String, String, String), &'a RemoteFile> {
    let mut best: HashMap<(String, String, String), &RemoteFile> = HashMap::new();
    for (row, device, filename, core) in rows {
        let key = (device, filename, core);
        match best.get(&key) {
            Some(old) if written(old) >= written(row) => {}
            _ => {
                best.insert(key, row);
            }
        }
    }
    best
}

fn known(version: Option<&str>) -> Option<&str> {
    version.map(str::trim).filter(|v| !v.is_empty())
}

/// D122: o core é o escolhido e a versão é a mesma, as duas conhecidas.
fn verdict(
    row_core: &str,
    row_version: Option<&str>,
    core: Option<&str>,
    installed: Option<&str>,
) -> Option<&'static str> {
    if core != Some(row_core) {
        return Some("other_core");
    }
    match (known(row_version), known(installed)) {
        (Some(remote), Some(here)) => {
            crate::save_state::version_changed(Some(remote), Some(here)).then_some("other_version")
        }
        _ => Some("unversioned"),
    }
}

impl Engine {
    /// Os states das outras máquinas para uma ROM. `core`: o core escolhido
    /// aqui (o que vai abrir o jogo, ou o que está aberto); `installed`: a
    /// versão dele instalada. Só lê.
    pub async fn states_for(
        &self,
        alva: &Alvanista,
        rom_id: &str,
        core: Option<&str>,
        installed: Option<&str>,
    ) -> RemoteStates {
        let installed_version = known(installed).map(str::to_owned);
        match self.list_remote(alva, rom_id, core, installed).await {
            Ok(found) => RemoteStates {
                states: found.into_iter().map(|f| f.state).collect(),
                offline: false,
                installed_version,
            },
            Err(e) => {
                log::info!("sync: states de outras máquinas não lidos: {e}");
                RemoteStates {
                    states: Vec::new(),
                    offline: true,
                    installed_version,
                }
            }
        }
    }

    /// A listagem, sem nenhum efeito: nem 401 vira revogação aqui (quem trata
    /// é a próxima passada).
    async fn list_remote(
        &self,
        alva: &Alvanista,
        rom_id: &str,
        core: Option<&str>,
        installed: Option<&str>,
    ) -> Result<Vec<Found>, SyncError> {
        let device = self.ready_device(alva).await?;
        if !self.states_on().await? {
            return Err(SyncError::NotEnabled);
        }
        if alva.account_id().await.is_none() {
            return Err(SyncError::Offline);
        }
        let Some(rom) = self
            .roms
            .get(rom_id)
            .await
            .map_err(|e| failed(e.to_string()))?
        else {
            return Ok(Vec::new());
        };
        let Some(stem) = rom_stem(&rom.file_path) else {
            return Ok(Vec::new());
        };
        let token = self.token()?;
        let platform = platform_for_system(&rom.system_id);
        let filter = FileFilter {
            kind: "state",
            platform: Some(platform),
            ..FileFilter::default()
        };
        let rows = self
            .sync
            .api
            .list_files(&token, &filter)
            .await
            .map_err(SyncError::from)?;
        let live: Vec<&RemoteFile> = rows
            .iter()
            .filter(|r| {
                r.kind == "state"
                    && r.deleted_at.is_none()
                    && r.platform == platform
                    && r.core.is_some()
                    && r.current.is_some()
                    && r.device_token_id.as_deref().is_some_and(|d| d != device.id)
            })
            .collect();
        let key = |r: &RemoteFile| {
            (
                r.device_token_id.clone().unwrap_or_default(),
                r.filename.clone(),
                r.core.clone().unwrap_or_default(),
            )
        };
        let states = newest(live.iter().filter_map(|r| {
            slot_of(&r.filename, &stem)?;
            let (d, f, c) = key(r);
            Some((*r, d, f, c))
        }));
        let thumbs = newest(live.iter().filter_map(|r| {
            let state_name = r.filename.strip_suffix(".png")?;
            slot_of(state_name, &stem)?;
            let (d, f, c) = key(r);
            Some((*r, d, f, c))
        }));
        // O nome da máquina: o que a listagem traz; senão a lista de
        // aparelhos da conta (lida só quando alguma linha ficou sem nome).
        let devices = if states.values().any(|r| r.device_label.is_none()) {
            self.sync.partner_devices(alva).await.unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut found: Vec<Found> = states
            .values()
            .filter_map(|r| {
                let slot = slot_of(&r.filename, &stem)?;
                let version = r.current.as_ref()?;
                let owner = r.device_token_id.as_deref().unwrap_or_default();
                let core_id = r.core.clone().unwrap_or_default();
                let reason = verdict(&core_id, r.core_version.as_deref(), core, installed);
                let machine = devices
                    .iter()
                    .find(|d| d.id == owner)
                    .map(|d| d.label.clone())
                    .or_else(|| r.device_label.clone())
                    .or_else(|| version.device_label.clone())
                    .filter(|m| !m.is_empty())
                    .unwrap_or_default();
                let thumb = thumbs
                    .get(&(
                        owner.to_owned(),
                        format!("{}.png", r.filename),
                        core_id.clone(),
                    ))
                    .filter(|t| t.core_version == r.core_version)
                    .and_then(|t| {
                        let v = t.current.as_ref()?;
                        Some(Thumb {
                            save_id: t.save_id.clone(),
                            version_id: v.version_id.clone(),
                            size: v.size_bytes,
                        })
                    });
                Some(Found {
                    state: RemoteState {
                        id: r.save_id.clone(),
                        machine,
                        slot,
                        created_at: parse_time(&version.server_created_at).map_or(0, |t| t.0),
                        core: core_id,
                        core_version: known(r.core_version.as_deref()).map(str::to_owned),
                        available: reason.is_none(),
                        reason: reason.map(str::to_owned),
                        has_thumbnail: thumb.is_some(),
                        size: version.size_bytes,
                    },
                    version_id: version.version_id.clone(),
                    hash: version.content_hash.clone(),
                    thumb,
                })
            })
            .collect();
        found.sort_by(|a, b| {
            b.state
                .created_at
                .cmp(&a.state.created_at)
                .then_with(|| a.state.id.cmp(&b.state.id))
        });
        {
            let mut index = lock(&self.remote_found);
            if index.len() > 500 {
                index.clear();
            }
            for f in &found {
                index.insert(f.state.id.clone(), f.clone());
            }
        }
        Ok(found)
    }

    /// Baixa o state `save_id` de outra máquina para a vaga dele nesta.
    /// `Err` é um código: `other_core`, `other_version`, `unversioned`,
    /// `slot_occupied`, `not_found`, `loading`, `backup_failed`,
    /// `hash_mismatch`, `failed` ou o de um `SyncError` (`offline`...).
    pub async fn download_state(
        &self,
        alva: &Alvanista,
        rom_id: &str,
        core: Option<&str>,
        installed: Option<&str>,
        save_id: &str,
        overwrite: bool,
    ) -> Result<SaveStateMetadata, String> {
        if self.playing() == Playing::Loading {
            return Err("loading".into());
        }
        let found = self
            .list_remote(alva, rom_id, core, installed)
            .await
            .map_err(|e| e.to_string())?;
        let item = found
            .into_iter()
            .find(|f| f.state.id == save_id)
            .ok_or("not_found")?;
        if let Some(reason) = &item.state.reason {
            return Err(reason.clone());
        }
        // A vaga é do core de agora, que é o do state (D122).
        let core_id = item.state.core.clone();
        let slot = item.state.slot;
        let old = self
            .local_slot(rom_id, &core_id, slot)
            .await
            .map_err(|e| e.to_string())?;
        if old.is_some() && !overwrite {
            return Err("slot_occupied".into());
        }
        let token = self.token().map_err(|e| e.to_string())?;
        let bytes = self
            .sync
            .api
            .download(&token, save_id, &item.version_id, item.state.size)
            .await
            .map_err(|e| SyncError::from(e).to_string())?;
        if sha256_hex(&bytes) != item.hash {
            log::warn!("sync: o state baixado não confere com o hash da nuvem");
            return Err("hash_mismatch".into());
        }
        let thumb = match &item.thumb {
            Some(t) => self.thumbnail_bytes(&token, t).await,
            None => None,
        };
        // O de agora vai para a pasta de resguardo antes de qualquer troca:
        // `save_state::save` apaga o arquivo, a miniatura e a linha dele.
        if let Some(old) = &old {
            if let Err(e) = self.backup_state(old) {
                log::warn!("sync: o state de agora não foi para o resguardo: {e}");
                return Err("backup_failed".into());
            }
        }
        let meta = crate::save_state::save(
            &self.states,
            &self.dirs.saves,
            rom_id,
            &core_id,
            slot,
            &bytes,
            thumb.as_deref(),
            None,
            installed.map(str::to_owned),
        )
        .await
        .map_err(|e| {
            log::warn!("sync: não gravou o state baixado: {e}");
            "failed".to_owned()
        })?;
        // Sem vaga numérica `save` não troca a linha de antes: sai a velha.
        if let Some(old) = old.filter(|o| o.slot.is_none() && o.id != meta.id) {
            if let Err(e) = self.states.delete_state(&old.id).await {
                log::warn!("sync: a linha velha do state ficou: {e}");
            }
        }
        Ok(meta)
    }

    /// O state local que ocupa a vaga (mesma ROM, mesmo core).
    async fn local_slot(
        &self,
        rom_id: &str,
        core_id: &str,
        slot: Option<u32>,
    ) -> Result<Option<SaveStateMetadata>, domain::error::RepoError> {
        match slot {
            Some(n) => self.states.find_state_in_slot(rom_id, core_id, n).await,
            None => Ok(self
                .states
                .list_states_for_rom(rom_id)
                .await?
                .into_iter()
                .find(|s| s.slot.is_none() && s.core_id == core_id)),
        }
    }

    /// Copia o state e a miniatura para `<dados>/sync-backups/<carimbo>/states/`
    /// (o carimbo é o de agora, no formato da pasta dos saves). A cópia vai ao
    /// disco antes de a troca começar; um arquivo que já não existe não é erro.
    fn backup_state(&self, old: &SaveStateMetadata) -> io::Result<()> {
        self.new_stamp();
        let mut files = vec![PathBuf::from(&old.file_path)];
        if let Some(t) = &old.thumbnail_path {
            files.push(PathBuf::from(t));
        }
        for from in files {
            if !present(&from)? {
                continue;
            }
            let name = from.file_name().unwrap_or_default();
            let target = self.backup_target_at(PathBuf::from("states").join(name))?;
            copy_synced(&from, &target)?;
        }
        Ok(())
    }

    async fn thumbnail_bytes(&self, token: &str, t: &Thumb) -> Option<Vec<u8>> {
        let key = (t.save_id.clone(), t.version_id.clone());
        if let Some(bytes) = lock(&self.remote_thumbs).get(&key) {
            return Some(bytes.clone());
        }
        let bytes = self
            .sync
            .api
            .download(token, &t.save_id, &t.version_id, t.size)
            .await
            .ok()?;
        let mut cache = lock(&self.remote_thumbs);
        if cache.len() > 128 {
            cache.clear();
        }
        cache.insert(key, bytes.clone());
        Some(bytes)
    }

    /// A miniatura do state `save_id` da última listagem, baixada uma vez e
    /// guardada na memória. Vazia quando não há (ou a nuvem não respondeu).
    pub async fn state_thumbnail(&self, alva: &Alvanista, save_id: &str) -> Vec<u8> {
        let thumb = lock(&self.remote_found)
            .get(save_id)
            .and_then(|f| f.thumb.clone());
        let Some(thumb) = thumb else {
            return Vec::new();
        };
        let key = (thumb.save_id.clone(), thumb.version_id.clone());
        if let Some(bytes) = lock(&self.remote_thumbs).get(&key) {
            return bytes.clone();
        }
        if self.ready_device(alva).await.is_err() || !self.states_on().await.unwrap_or(false) {
            return Vec::new();
        }
        let Ok(token) = self.token() else {
            return Vec::new();
        };
        self.thumbnail_bytes(&token, &thumb)
            .await
            .unwrap_or_default()
    }
}
