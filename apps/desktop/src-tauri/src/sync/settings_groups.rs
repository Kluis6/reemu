//! Configurações copiadas entre máquinas (D114): cada grupo vira um documento
//! JSON portátil (`v: 1`) e volta ao banco numa transação.
//!
//! Um documento nunca leva o que é desta máquina: pastas da biblioteca,
//! mapeamento de controle e portas, GUID de controle (a hotkey presa a um
//! botão de controle fica aqui), o aparelho de áudio, os nits de HDR medidos
//! nesta tela, as opções que um core grava como identidade da máquina (o MAC
//! do PSP) e qualquer caminho absoluto. Um jogo vai pelo CRC32 da ROM. Um
//! preset vai pela identidade: embutido, `curated:<id>` ou
//! `slang-shaders:<caminho>` dentro do pacote que o ReEmu baixa; uma moldura
//! do The Bezel Project vai pelo caminho dentro da pasta dele (D126). Um preset
//! ou pacote importado do disco vai só pelo nome e pelo lugar, em `missing`.
//!
//! Dois modos:
//! - `Merge` ("Copiar") grava o que o documento traz e não apaga nada que ele
//!   não traz: o desta máquina que ele não menciona fica;
//! - `Restore` ("Desfazer") recebe o backup desta máquina e o grupo volta a ser
//!   o dele.
//!
//! Nos dois, o que é só desta máquina não é tocado: quando o documento quer o
//! lugar de uma atribuição a um preset importado, de uma hotkey de controle ou
//! de uma escolha de jogo fora da biblioteca, o desta máquina fica e conta em
//! `kept_local`. Por isso `Restore` com o backup de antes de uma cópia devolve
//! a máquina igual.
//!
//! Os grupos marcados são conferidos todos ([`check_all`]) antes de abrir a
//! transação e entram juntos nela ([`apply_in`], [`apply_all`]): ou todos, ou
//! nenhum (D114). Aplicar só grava o banco: quem chama recarrega o que o app
//! guarda em memória (hotkeys, teclado, áudio, vídeo).

use db::{
    AspectRow, AudioSettings, Db, OptionOverrideRow, Put, SettingsRepo, SettingsTx, VideoSettings,
};
use domain::decoration::{DecorationAssignment, DecorationPack, DecorationPackSource};
use domain::error::RepoError;
use domain::hotkeys::SystemAction;
use domain::input::RawInputEvent;
use domain::shader_chain::{AssignmentScope, ShaderFormat, ShaderPreset};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

/// A versão dos documentos que esta máquina escreve e aceita.
pub const DOC_VERSION: u64 = 1;
/// Erro: o documento é de uma versão que esta máquina não conhece.
pub const ERR_VERSION: &str = "settings_version";
/// Erro: o documento não tem o formato do grupo.
pub const ERR_INVALID: &str = "settings_invalid";

/// As proporções que esta versão conhece (as mesmas de `set_aspect`).
const ASPECTS: [&str; 4] = ["core", "4:3", "16:9", "fill"];

/// Um `.slangp` do pacote `slang-shaders` que o ReEmu baixa, pelo caminho
/// dentro dele.
const SLANG_PACK: &str = "slang-shaders:";

/// O catálogo de molduras do The Bezel Project (`bezel_pack`), pelo id que é o
/// mesmo em toda máquina.
const BEZEL_PROJECT: &str = "bezelproject";

/// Opções que um core grava sozinho como identidade desta máquina
/// (`persist_core_set_options`): o PPSSPP sorteia o MAC do PSP emulado uma vez
/// e o guarda em `ppsspp_change_mac_address01..12`, e saves amarrados ao
/// console só abrem com ele. Nunca saem num documento, e nenhum documento as
/// troca ou apaga. Prefixos de chave.
const LOCAL_ONLY_OPTIONS: &[&str] = &["ppsspp_change_mac_address"];

fn is_local_only(key: &str) -> bool {
    LOCAL_ONLY_OPTIONS.iter().any(|p| key.starts_with(p))
}

/// Um grupo de configurações: uma linha `reemu/<grupo>.json` na nuvem. A
/// aparência mora no frontend (`lib/appearanceGroup.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Group {
    CoreOptions,
    ShadersDecorations,
    HotkeysKeyboard,
    Audio,
    Video,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::CoreOptions,
        Group::ShadersDecorations,
        Group::HotkeysKeyboard,
        Group::Audio,
        Group::Video,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Group::CoreOptions => "core-options",
            Group::ShadersDecorations => "shaders-decorations",
            Group::HotkeysKeyboard => "hotkeys-keyboard",
            Group::Audio => "audio",
            Group::Video => "video",
        }
    }
}

/// Como um documento entra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyMode {
    /// "Copiar": grava o que o documento traz; o que ele não traz fica.
    Merge,
    /// "Desfazer": o documento é o backup desta máquina e o grupo volta a ser
    /// o dele.
    Restore,
}

impl ApplyMode {
    fn as_str(self) -> &'static str {
        match self {
            ApplyMode::Merge => "merge",
            ApplyMode::Restore => "restore",
        }
    }

    /// No `Restore` o grupo foi esvaziado antes e a mesma chave duas vezes é
    /// um backup que não é desta máquina: recusa. No `Merge` a chave que já
    /// existe aqui recebe o valor do documento.
    fn put(self) -> Put {
        match self {
            ApplyMode::Merge => Put::Upsert,
            ApplyMode::Restore => Put::Insert,
        }
    }
}

/// Onde ficam, nesta máquina, os catálogos que o ReEmu baixa (D126).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// O pacote `slang-shaders` (`shader_pack::install_dir`).
    pub shader_pack: PathBuf,
    /// As molduras do The Bezel Project (`bezel_pack::pack_root`).
    pub bezels: PathBuf,
}

impl Catalog {
    /// A partir de `AppState::shaders_dir` e `AppState::decorations_dir`.
    pub fn new(shaders_dir: &Path, decorations_dir: &Path) -> Self {
        Self {
            shader_pack: crate::shader_pack::install_dir(shaders_dir),
            bezels: crate::bezel_pack::pack_root(decorations_dir),
        }
    }
}

/// O que esta máquina tem e o banco não diz.
pub struct ThisMachine<'a> {
    /// `(rom_id, crc32)` da biblioteca.
    pub roms: &'a [(String, String)],
    /// Os cores instalados (`emu_session::installed_core_ids`).
    pub cores: &'a [String],
    pub catalog: &'a Catalog,
}

/// Por que um preset ou uma moldura do documento não veio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MissingKind {
    /// Um arquivo que a outra máquina importou do disco dela.
    ImportedElsewhere,
    /// Um item do catálogo que esta máquina não baixou (ou que esta versão do
    /// ReEmu não conhece).
    NotDownloadedHere,
}

impl MissingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MissingKind::ImportedElsewhere => "imported_elsewhere",
            MissingKind::NotDownloadedHere => "not_downloaded_here",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingItem {
    Shader,
    Decoration,
}

/// Um preset ou uma moldura que não veio. O lugar desta máquina que ele
/// ocuparia fica como estava.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MissingFile {
    pub kind: MissingKind,
    pub item: MissingItem,
    pub name: String,
    /// O lugar, como no documento: `default`, `system` (com `system_id`) ou
    /// `rom` (com `crc32`).
    pub scope: String,
    pub system_id: Option<String>,
    pub crc32: Option<String>,
}

/// O que uma aplicação fez.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Itens do documento gravados: um valor de opção, uma preferência de
    /// core, uma atribuição, uma hotkey, uma tecla, uma proporção; o áudio e a
    /// linha de vídeo contam um.
    pub applied: u32,
    /// Jogos (CRC32 distintos) do documento que esta máquina não tem.
    pub skipped_unknown_games: u32,
    /// Lugares em que a escolha só desta máquina ficou no lugar da do
    /// documento.
    pub kept_local: u32,
    /// Presets e molduras que não vieram, em ordem.
    pub missing_files: Vec<MissingFile>,
    /// Cores preferidos lá que esta máquina não tem: a preferência daqui fica.
    pub cores_not_installed: Vec<String>,
}

// --- documentos ------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct CoreOptionsDoc {
    v: u64,
    /// `{core_id: {chave: valor}}`, sem as opções de identidade.
    values: BTreeMap<String, BTreeMap<String, String>>,
    /// Opções por jogo, o jogo pelo CRC32.
    overrides: Vec<OverrideEntry>,
    /// `{system_id: core_id}`: o core preferido de cada plataforma.
    #[serde(default)]
    preferred_cores: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OverrideEntry {
    crc32: String,
    core_id: String,
    key: String,
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShadersDoc {
    v: u64,
    assignments: Vec<ShaderEntry>,
    decorations: Vec<DecorationEntry>,
    /// Presets e pacotes importados do disco nesta máquina, com o lugar.
    missing: Vec<MissingEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShaderEntry {
    /// `default`, `system` (com `system_id`) ou `rom` (com `crc32`).
    scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    system_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    crc32: Option<String>,
    /// Um nome embutido, `curated:<id>` ou `slang-shaders:<caminho com />`.
    preset: String,
    /// O nome que a outra máquina mostra: o que aparece aqui quando o preset
    /// não está.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    params: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DecorationEntry {
    scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    system_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    crc32: Option<String>,
    /// O catálogo: `bezelproject`.
    pack: String,
    /// A imagem dentro da pasta do catálogo, separada por `/`; o primeiro
    /// pedaço é o sistema.
    asset: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct MissingEntry {
    item: MissingItem,
    name: String,
    scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    system_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    crc32: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HotkeysDoc {
    v: u64,
    /// Só as de teclado, sem `device_guid`.
    hotkeys: Vec<HotkeyEntry>,
    keyboard: Vec<KeyEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HotkeyEntry {
    action: String,
    trigger: Vec<RawInputEvent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyEntry {
    target: String,
    code: String,
}

/// Sem o aparelho de saída (id e nome), que é desta máquina.
#[derive(Debug, Serialize, Deserialize)]
struct AudioDoc {
    v: u64,
    rate_control_enabled: bool,
    rate_control_delta: f64,
    sample_rate_preference: Option<u32>,
}

/// Sem os nits de HDR, medidos na tela desta máquina.
#[derive(Debug, Serialize, Deserialize)]
struct VideoDoc {
    v: u64,
    integer_scaling: bool,
    hdr: bool,
    hdr_highlights: bool,
    /// `target`: o CRC32 do jogo, o id da plataforma ou `""` no padrão.
    aspect: Vec<AspectEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AspectEntry {
    scope: String,
    target: String,
    aspect: String,
}

enum Doc {
    CoreOptions(CoreOptionsDoc),
    Shaders(ShadersDoc),
    Hotkeys(HotkeysDoc),
    Audio(AudioDoc),
    Video(VideoDoc),
}

impl Doc {
    fn parse(group: Group, doc: &Value) -> Result<Self, String> {
        fn de<T: DeserializeOwned>(doc: &Value) -> Result<T, String> {
            T::deserialize(doc).map_err(|_| invalid())
        }
        Ok(match group {
            Group::CoreOptions => Doc::CoreOptions(de(doc)?),
            Group::ShadersDecorations => Doc::Shaders(de(doc)?),
            Group::HotkeysKeyboard => Doc::Hotkeys(de(doc)?),
            Group::Audio => Doc::Audio(de(doc)?),
            Group::Video => Doc::Video(de(doc)?),
        })
    }

    /// O que não depende desta máquina: alvos, caminhos relativos, gatilhos.
    fn validate(&self) -> Result<(), String> {
        match self {
            Doc::CoreOptions(d) => {
                for e in &d.overrides {
                    game_crc(&e.crc32)?;
                }
                if d.preferred_cores
                    .iter()
                    .any(|(system, core)| system.is_empty() || core.is_empty())
                {
                    return Err(invalid());
                }
            }
            Doc::Shaders(d) => {
                for e in &d.assignments {
                    Target::parse(&e.scope, e.system_id.as_deref(), e.crc32.as_deref())?;
                    if e.preset.is_empty() {
                        return Err(invalid());
                    }
                    if let Some(rel) = e.preset.strip_prefix(SLANG_PACK) {
                        rel_parts(rel)?;
                    }
                }
                for e in &d.decorations {
                    Target::parse(&e.scope, e.system_id.as_deref(), e.crc32.as_deref())?;
                    if e.pack.is_empty() {
                        return Err(invalid());
                    }
                    rel_parts(&e.asset)?;
                }
                for m in &d.missing {
                    Target::parse(&m.scope, m.system_id.as_deref(), m.crc32.as_deref())?;
                }
            }
            Doc::Hotkeys(d) => {
                if d.hotkeys
                    .iter()
                    .any(|e| e.trigger.is_empty() || !e.trigger.iter().all(is_key))
                    || d.keyboard.iter().any(|k| k.target.is_empty())
                {
                    return Err(invalid());
                }
            }
            Doc::Audio(_) => {}
            Doc::Video(d) => {
                for e in &d.aspect {
                    aspect_target(e)?;
                }
            }
        }
        Ok(())
    }
}

/// Um documento conferido (versão, formato, alvos), pronto para
/// [`apply_in`].
pub struct Checked {
    group: Group,
    doc: Doc,
}

impl Checked {
    pub fn group(&self) -> Group {
        self.group
    }
}

// --- entrada ---------------------------------------------------------------

/// O documento do grupo, com o que esta máquina tem agora.
pub async fn export(group: Group, db: &Db, catalog: &Catalog) -> Result<Value, String> {
    let mut tx = SettingsRepo::new(db.clone()).read().await.map_err(repo)?;
    let doc = match group {
        Group::CoreOptions => serde_json::to_value(export_core_options(&mut tx).await?),
        Group::ShadersDecorations => serde_json::to_value(export_shaders(&mut tx, catalog).await?),
        Group::HotkeysKeyboard => serde_json::to_value(export_hotkeys(&mut tx).await?),
        Group::Audio => serde_json::to_value(export_audio(&mut tx).await?),
        Group::Video => serde_json::to_value(export_video(&mut tx).await?),
    }
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(repo)?;
    Ok(doc)
}

/// Confere um documento sem tocar no banco: [`ERR_VERSION`] para outra versão
/// (`v`), [`ERR_INVALID`] para outro formato.
pub fn check(group: Group, doc: &Value) -> Result<Checked, String> {
    if !doc.is_object() {
        return Err(invalid());
    }
    if doc.get("v").and_then(Value::as_u64) != Some(DOC_VERSION) {
        return Err(ERR_VERSION.to_string());
    }
    let doc = Doc::parse(group, doc)?;
    doc.validate()?;
    Ok(Checked { group, doc })
}

/// Confere todos os documentos de uma vez, antes da transação: um só que não
/// serve e nenhum entra. O mesmo grupo duas vezes não serve.
pub fn check_all(docs: &[(Group, &Value)]) -> Result<Vec<Checked>, String> {
    let mut seen = HashSet::new();
    docs.iter()
        .map(|(group, doc)| {
            if !seen.insert(*group) {
                return Err(invalid());
            }
            check(*group, doc)
        })
        .collect()
}

/// Aplica um documento conferido dentro de uma transação aberta por quem
/// chama (`SettingsRepo::write`). Sem `commit`, nada fica.
pub async fn apply_in(
    tx: &mut SettingsTx,
    checked: Checked,
    mode: ApplyMode,
    here: &ThisMachine<'_>,
) -> Result<ApplyReport, String> {
    let roms = LocalRoms::new(here.roms);
    let mut tally = Tally::default();
    match checked.doc {
        Doc::CoreOptions(d) => apply_core_options(tx, d, mode, here, &roms, &mut tally).await?,
        Doc::Shaders(d) => apply_shaders(tx, d, mode, here, &roms, &mut tally).await?,
        Doc::Hotkeys(d) => apply_hotkeys(tx, d, mode, &mut tally).await?,
        Doc::Audio(d) => apply_audio(tx, d, &mut tally).await?,
        Doc::Video(d) => apply_video(tx, d, mode, &roms, &mut tally).await?,
    }
    Ok(tally.into_report())
}

/// Confere todos os documentos e aplica todos numa transação só (D114): ou
/// entram todos, ou nenhum.
pub async fn apply_all(
    db: &Db,
    docs: &[(Group, &Value)],
    mode: ApplyMode,
    here: &ThisMachine<'_>,
) -> Result<Vec<(Group, ApplyReport)>, String> {
    let checked = check_all(docs)?;
    let mut tx = SettingsRepo::new(db.clone()).write().await.map_err(repo)?;
    let mut reports = Vec::with_capacity(checked.len());
    for c in checked {
        let group = c.group();
        reports.push((group, apply_in(&mut tx, c, mode, here).await?));
    }
    tx.commit().await.map_err(repo)?;
    for (group, r) in &reports {
        // só contagens: nome de jogo, de preset ou de pasta não vai para o log
        log::info!(
            "sync: configurações {} ({}): {} itens, {} jogos que esta máquina não tem, {} \
             lugares desta máquina mantidos, {} arquivos que não vieram, {} cores não \
             instalados",
            group.as_str(),
            mode.as_str(),
            r.applied,
            r.skipped_unknown_games,
            r.kept_local,
            r.missing_files.len(),
            r.cores_not_installed.len()
        );
    }
    Ok(reports)
}

/// Um grupo só, numa transação dele ([`apply_all`] com um documento).
pub async fn apply(
    group: Group,
    db: &Db,
    doc: &Value,
    mode: ApplyMode,
    here: &ThisMachine<'_>,
) -> Result<ApplyReport, String> {
    let mut reports = apply_all(db, &[(group, doc)], mode, here).await?;
    Ok(reports.remove(0).1)
}

// --- apoio -----------------------------------------------------------------

fn repo(e: RepoError) -> String {
    e.to_string()
}

fn invalid() -> String {
    ERR_INVALID.to_string()
}

fn normalize_crc(crc: &str) -> String {
    crc.trim().to_ascii_uppercase()
}

/// O CRC32 de um jogo do documento; vazio não é jogo.
fn game_crc(crc: &str) -> Result<String, String> {
    let crc = normalize_crc(crc);
    if crc.is_empty() {
        return Err(invalid());
    }
    Ok(crc)
}

/// O nome que vai num documento ou no relatório: nunca um caminho, só o
/// último pedaço.
fn bare_name(name: &str) -> String {
    name.rsplit(['/', '\\']).next().unwrap_or(name).to_string()
}

/// O caminho dentro de `root`, separado por `/`; `None` quando ele não está
/// dentro.
fn relative_to(root: &Path, path: &str) -> Option<String> {
    let rel = Path::new(path).strip_prefix(root).ok()?;
    let parts = rel
        .components()
        .map(|c| match c {
            Component::Normal(p) => p.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Os pedaços de um caminho relativo do documento. Recusa o que sairia da
/// pasta do catálogo ou seria um caminho absoluto.
fn rel_parts(rel: &str) -> Result<Vec<&str>, String> {
    let parts: Vec<&str> = rel.split('/').collect();
    let clean = parts
        .iter()
        .all(|p| !p.is_empty() && *p != "." && *p != ".." && !p.contains(['\\', ':']));
    if clean {
        Ok(parts)
    } else {
        Err(invalid())
    }
}

fn under(root: &Path, parts: &[&str]) -> PathBuf {
    let mut path = root.to_path_buf();
    path.extend(parts);
    path
}

/// `rom_id` → CRC32 das ROMs desta biblioteca (lido na transação).
async fn crc_by_rom(tx: &mut SettingsTx) -> Result<HashMap<String, String>, String> {
    Ok(tx
        .rom_crc32s()
        .await
        .map_err(repo)?
        .into_iter()
        .map(|(id, crc)| (id, normalize_crc(&crc)))
        .filter(|(_, crc)| !crc.is_empty())
        .collect())
}

/// As ROMs desta máquina, por CRC32 (o mesmo jogo pode estar em duas pastas)
/// e por id.
struct LocalRoms<'a> {
    by_crc: HashMap<String, Vec<&'a str>>,
    ids: HashSet<&'a str>,
}

impl<'a> LocalRoms<'a> {
    fn new(roms: &'a [(String, String)]) -> Self {
        let mut by_crc: HashMap<String, Vec<&'a str>> = HashMap::new();
        let mut ids = HashSet::new();
        for (id, crc) in roms {
            let crc = normalize_crc(crc);
            if crc.is_empty() {
                continue;
            }
            by_crc.entry(crc).or_default().push(id.as_str());
            ids.insert(id.as_str());
        }
        Self { by_crc, ids }
    }

    fn has(&self, rom_id: &str) -> bool {
        self.ids.contains(rom_id)
    }

    fn of(&self, crc: &str) -> &[&'a str] {
        self.by_crc.get(crc).map(Vec::as_slice).unwrap_or(&[])
    }

    /// A escolha é de todos, de uma plataforma ou de um jogo desta
    /// biblioteca (a de uma ROM que saiu dela não é de cópia nenhuma).
    fn in_library(&self, scope: AssignmentScope, rom_id: Option<&str>) -> bool {
        scope != AssignmentScope::Rom || rom_id.is_some_and(|id| self.has(id))
    }
}

#[derive(Default)]
struct Tally {
    applied: u32,
    kept_local: u32,
    unknown_games: BTreeSet<String>,
    missing: BTreeSet<MissingFile>,
    cores: BTreeSet<String>,
}

impl Tally {
    /// Os lugares do alvo nesta máquina; um jogo que ela não tem é contado.
    fn slots(&mut self, target: &Target, roms: &LocalRoms<'_>) -> Vec<Slot> {
        let slots = target.slots(roms);
        if let (true, Target::Game(crc)) = (slots.is_empty(), target) {
            self.unknown_games.insert(crc.clone());
        }
        slots
    }

    fn missing(&mut self, kind: MissingKind, item: MissingItem, name: String, target: &Target) {
        let (scope, system_id, crc32) = target.parts();
        self.missing.insert(MissingFile {
            kind,
            item,
            name,
            scope,
            system_id,
            crc32,
        });
    }

    fn into_report(self) -> ApplyReport {
        ApplyReport {
            applied: self.applied,
            skipped_unknown_games: u32::try_from(self.unknown_games.len()).unwrap_or(u32::MAX),
            kept_local: self.kept_local,
            missing_files: self.missing.into_iter().collect(),
            cores_not_installed: self.cores.into_iter().collect(),
        }
    }
}

/// Onde uma escolha vale, na forma do documento. A ordem do `Ord` é a ordem
/// do documento: todos, plataformas, jogos.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Default,
    System(String),
    Game(String),
}

/// O lugar de uma atribuição nesta máquina (escopo e alvo), o que os índices
/// únicos de `shader_chain_assignments` e `decoration_assignments` travam.
type SlotKey = (AssignmentScope, String);

fn slot_key(scope: AssignmentScope, system_id: Option<&str>, rom_id: Option<&str>) -> SlotKey {
    let target = match scope {
        AssignmentScope::Default => "",
        AssignmentScope::System => system_id.unwrap_or(""),
        AssignmentScope::Rom => rom_id.unwrap_or(""),
    };
    (scope, target.to_string())
}

struct Slot {
    scope: AssignmentScope,
    system_id: Option<String>,
    rom_id: Option<String>,
}

impl Slot {
    fn key(&self) -> SlotKey {
        slot_key(
            self.scope,
            self.system_id.as_deref(),
            self.rom_id.as_deref(),
        )
    }
}

impl Target {
    /// `(scope, system_id, crc32)` do documento.
    fn parts(&self) -> (String, Option<String>, Option<String>) {
        match self {
            Target::Default => ("default".into(), None, None),
            Target::System(s) => ("system".into(), Some(s.clone()), None),
            Target::Game(c) => ("rom".into(), None, Some(c.clone())),
        }
    }

    fn parse(scope: &str, system_id: Option<&str>, crc32: Option<&str>) -> Result<Self, String> {
        match (scope, system_id, crc32) {
            ("default", _, _) => Ok(Target::Default),
            ("system", Some(s), _) if !s.is_empty() => Ok(Target::System(s.to_string())),
            ("rom", _, Some(c)) => Ok(Target::Game(game_crc(c)?)),
            _ => Err(invalid()),
        }
    }

    /// A atribuição desta máquina na forma do documento; `None` para a de um
    /// jogo que não está na biblioteca.
    fn of_row(
        scope: AssignmentScope,
        system_id: Option<&str>,
        rom_id: Option<&str>,
        crc_of: &HashMap<String, String>,
    ) -> Option<Self> {
        match scope {
            AssignmentScope::Default => Some(Target::Default),
            AssignmentScope::System => system_id
                .filter(|s| !s.is_empty())
                .map(|s| Target::System(s.to_string())),
            AssignmentScope::Rom => rom_id
                .and_then(|id| crc_of.get(id))
                .map(|c| Target::Game(c.clone())),
        }
    }

    /// Os lugares desta máquina para o alvo: um jogo pode estar em mais de
    /// uma ROM, ou em nenhuma.
    fn slots(&self, roms: &LocalRoms<'_>) -> Vec<Slot> {
        match self {
            Target::Default => vec![Slot {
                scope: AssignmentScope::Default,
                system_id: None,
                rom_id: None,
            }],
            Target::System(s) => vec![Slot {
                scope: AssignmentScope::System,
                system_id: Some(s.clone()),
                rom_id: None,
            }],
            Target::Game(crc) => roms
                .of(crc)
                .iter()
                .map(|id| Slot {
                    scope: AssignmentScope::Rom,
                    system_id: None,
                    rom_id: Some(id.to_string()),
                })
                .collect(),
        }
    }
}

// --- opções de core ----------------------------------------------------------

async fn export_core_options(tx: &mut SettingsTx) -> Result<CoreOptionsDoc, String> {
    let crc_of = crc_by_rom(tx).await?;
    let mut values: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (core, key, value) in tx.option_values().await.map_err(repo)? {
        if !is_local_only(&key) {
            values.entry(core).or_default().insert(key, value);
        }
    }
    // o mesmo jogo em duas ROMs: vale a primeira, na ordem do id
    let mut overrides = BTreeMap::new();
    for o in tx.option_overrides().await.map_err(repo)? {
        if is_local_only(&o.key) {
            continue;
        }
        if let Some(crc) = crc_of.get(&o.rom_id) {
            overrides
                .entry((crc.clone(), o.core_id, o.key))
                .or_insert(o.value);
        }
    }
    let preferred_cores = tx.core_prefs().await.map_err(repo)?.into_iter().collect();
    Ok(CoreOptionsDoc {
        v: DOC_VERSION,
        values,
        overrides: overrides
            .into_iter()
            .map(|((crc32, core_id, key), value)| OverrideEntry {
                crc32,
                core_id,
                key,
                value,
            })
            .collect(),
        preferred_cores,
    })
}

async fn apply_core_options(
    tx: &mut SettingsTx,
    doc: CoreOptionsDoc,
    mode: ApplyMode,
    here: &ThisMachine<'_>,
    roms: &LocalRoms<'_>,
    tally: &mut Tally,
) -> Result<(), String> {
    let put = mode.put();
    if mode == ApplyMode::Restore {
        // o backup desta máquina volta inteiro; a identidade nunca sai
        for (core, key, _) in tx.option_values().await.map_err(repo)? {
            if !is_local_only(&key) {
                tx.delete_option_value(&core, &key).await.map_err(repo)?;
            }
        }
        for o in tx.option_overrides().await.map_err(repo)? {
            if roms.has(&o.rom_id) && !is_local_only(&o.key) {
                tx.delete_option_override(&o).await.map_err(repo)?;
            }
        }
        tx.clear_core_prefs().await.map_err(repo)?;
    }
    for (core, options) in &doc.values {
        for (key, value) in options {
            if is_local_only(key) {
                continue;
            }
            tx.put_option_value(put, core, key, value)
                .await
                .map_err(repo)?;
            tally.applied += 1;
        }
    }
    for e in &doc.overrides {
        if is_local_only(&e.key) {
            continue;
        }
        let crc = game_crc(&e.crc32)?;
        let ids = roms.of(&crc);
        if ids.is_empty() {
            tally.unknown_games.insert(crc);
            continue;
        }
        for rom_id in ids {
            let row = OptionOverrideRow {
                rom_id: rom_id.to_string(),
                core_id: e.core_id.clone(),
                key: e.key.clone(),
                value: e.value.clone(),
            };
            tx.put_option_override(put, &row).await.map_err(repo)?;
        }
        tally.applied += 1;
    }
    for (system, core) in &doc.preferred_cores {
        // um core que esta máquina não tem: a preferência daqui fica
        if mode == ApplyMode::Merge && !here.cores.iter().any(|c| c == core) {
            tally.cores.insert(core.clone());
            continue;
        }
        tx.put_core_pref(put, system, core).await.map_err(repo)?;
        tally.applied += 1;
    }
    Ok(())
}

// --- shaders e molduras ------------------------------------------------------

/// O preset na forma do documento, `(preset, nome)`; `None` para um importado
/// do disco.
fn portable_preset(
    p: &ShaderPreset,
    catalog: &Catalog,
    builtins: &[String],
) -> Option<(String, Option<String>)> {
    if builtins.contains(&p.source_path) {
        return Some((p.source_path.clone(), None));
    }
    if p.source_path.starts_with("curated:") {
        return Some((p.source_path.clone(), Some(p.name.clone())));
    }
    let rel = relative_to(&catalog.shader_pack, &p.source_path)?;
    Some((format!("{SLANG_PACK}{rel}"), Some(p.name.clone())))
}

/// A linha de `shader_presets` como a tela de shaders grava (`set_shader`).
fn preset_row(source: &str, name: &str, is_builtin: bool) -> ShaderPreset {
    ShaderPreset {
        id: source.to_string(),
        name: name.to_string(),
        source_path: source.to_string(),
        format: ShaderFormat::Slang,
        is_builtin,
        includes_bezel: source.to_lowercase().contains("bezel"),
    }
}

/// O preset do documento nesta máquina; `Err(nome)` quando ele não está aqui:
/// o pacote não foi baixado, o arquivo não existe nesta versão do pacote, ou
/// esta versão do ReEmu não conhece o preset.
fn local_preset(
    e: &ShaderEntry,
    catalog: &Catalog,
    builtins: &[String],
) -> Result<ShaderPreset, String> {
    let shown = |fallback: &str| e.name.clone().unwrap_or_else(|| bare_name(fallback));
    if builtins.contains(&e.preset) {
        return Ok(preset_row(&e.preset, &e.preset, true));
    }
    if e.preset.starts_with("curated:") {
        let Some(c) = crate::gpu::curated_by_wire(&e.preset) else {
            return Err(shown(&e.preset));
        };
        // o embutido vem com o app; o do pacote só vale com o pacote aqui
        if !c.bundled && !catalog.shader_pack.join(c.relpath).is_file() {
            return Err(c.label.to_string());
        }
        return Ok(preset_row(&e.preset, c.label, true));
    }
    if let Some(rel) = e.preset.strip_prefix(SLANG_PACK) {
        let Ok(parts) = rel_parts(rel) else {
            return Err(shown(rel));
        };
        let file = under(&catalog.shader_pack, &parts);
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("preset")
            .to_string();
        if !file.is_file() {
            return Err(e.name.clone().unwrap_or(stem));
        }
        return Ok(preset_row(&file.to_string_lossy(), &stem, false));
    }
    Err(shown(&e.preset))
}

/// A imagem dentro da pasta do The Bezel Project; `None` para a de um pacote
/// importado do disco.
fn bezel_asset(pack: &DecorationPack, asset_path: &str, catalog: &Catalog) -> Option<String> {
    if Path::new(&pack.base_path) != catalog.bezels.as_path() {
        return None;
    }
    relative_to(&catalog.bezels, asset_path)
}

/// O pacote do The Bezel Project como `bezel_pack` o importa
/// (`decoration::import_pack`).
fn bezel_pack_row(catalog: &Catalog) -> DecorationPack {
    DecorationPack {
        id: format!("pack:{}", catalog.bezels.display()),
        name: catalog
            .bezels
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(BEZEL_PROJECT)
            .to_string(),
        source: DecorationPackSource::UserImported,
        base_path: catalog.bezels.display().to_string(),
    }
}

async fn export_shaders(tx: &mut SettingsTx, catalog: &Catalog) -> Result<ShadersDoc, String> {
    let builtins = crate::gpu::builtin_preset_names();
    let crc_of = crc_by_rom(tx).await?;
    let presets: HashMap<String, ShaderPreset> = tx
        .shader_presets()
        .await
        .map_err(repo)?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();
    let mut missing = BTreeSet::new();
    let missing_entry = |item, name: &str, target: &Target| {
        let (scope, system_id, crc32) = target.parts();
        MissingEntry {
            item,
            name: bare_name(name),
            scope,
            system_id,
            crc32,
        }
    };

    let mut assignments = BTreeMap::new();
    for a in tx.shader_assignments().await.map_err(repo)? {
        let target = Target::of_row(
            a.scope,
            a.system_id.as_deref(),
            a.rom_id.as_deref(),
            &crc_of,
        );
        let (Some(target), Some(preset)) = (target, presets.get(&a.preset_id)) else {
            continue;
        };
        let Some((id, name)) = portable_preset(preset, catalog, &builtins) else {
            missing.insert(missing_entry(MissingItem::Shader, &preset.name, &target));
            continue;
        };
        let (scope, system_id, crc32) = target.parts();
        assignments.entry(target).or_insert(ShaderEntry {
            scope,
            system_id,
            crc32,
            preset: id,
            name,
            params: a.params.into_iter().collect(),
        });
    }

    let packs: HashMap<String, DecorationPack> = tx
        .decoration_packs()
        .await
        .map_err(repo)?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();
    let mut decorations = BTreeMap::new();
    for d in tx.decoration_assignments().await.map_err(repo)? {
        let target = Target::of_row(
            d.scope,
            d.system_id.as_deref(),
            d.rom_id.as_deref(),
            &crc_of,
        );
        let (Some(target), Some(pack)) = (target, packs.get(&d.pack_id)) else {
            continue;
        };
        let Some(asset) = bezel_asset(pack, &d.asset_path, catalog) else {
            missing.insert(missing_entry(MissingItem::Decoration, &pack.name, &target));
            continue;
        };
        let (scope, system_id, crc32) = target.parts();
        decorations.entry(target).or_insert(DecorationEntry {
            scope,
            system_id,
            crc32,
            pack: BEZEL_PROJECT.to_string(),
            asset,
        });
    }
    Ok(ShadersDoc {
        v: DOC_VERSION,
        assignments: assignments.into_values().collect(),
        decorations: decorations.into_values().collect(),
        missing: missing.into_iter().collect(),
    })
}

async fn apply_shaders(
    tx: &mut SettingsTx,
    doc: ShadersDoc,
    mode: ApplyMode,
    here: &ThisMachine<'_>,
    roms: &LocalRoms<'_>,
    tally: &mut Tally,
) -> Result<(), String> {
    let catalog = here.catalog;
    let builtins = crate::gpu::builtin_preset_names();

    // Onde a outra máquina tem um arquivo importado do disco: o lugar desta
    // fica como está (nem esvaziado no `Restore`). O backup desta máquina lista
    // os importados daqui, que estão aqui: só a cópia os relata.
    let mut missing_slots: HashSet<(MissingItem, SlotKey)> = HashSet::new();
    for m in &doc.missing {
        let target = Target::parse(&m.scope, m.system_id.as_deref(), m.crc32.as_deref())?;
        for slot in target.slots(roms) {
            missing_slots.insert((m.item, slot.key()));
        }
        if mode == ApplyMode::Merge {
            let name = bare_name(&m.name);
            tally.missing(MissingKind::ImportedElsewhere, m.item, name, &target);
        }
    }

    // Presets. `local`: o que ocupa cada lugar, e se é do catálogo (troca) ou
    // só desta máquina (fica).
    let presets: HashMap<String, ShaderPreset> = tx
        .shader_presets()
        .await
        .map_err(repo)?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();
    let mut local: HashMap<SlotKey, (String, bool)> = HashMap::new();
    for a in tx.shader_assignments().await.map_err(repo)? {
        let key = slot_key(a.scope, a.system_id.as_deref(), a.rom_id.as_deref());
        let portable = roms.in_library(a.scope, a.rom_id.as_deref())
            && presets
                .get(&a.preset_id)
                .is_some_and(|p| portable_preset(p, catalog, &builtins).is_some());
        let protected = missing_slots.contains(&(MissingItem::Shader, key.clone()));
        if mode == ApplyMode::Restore && portable && !protected {
            tx.delete_shader_assignment(&a.id).await.map_err(repo)?;
            continue;
        }
        local.insert(key, (a.id, portable));
    }
    for e in doc.assignments {
        let target = Target::parse(&e.scope, e.system_id.as_deref(), e.crc32.as_deref())?;
        let preset = match local_preset(&e, catalog, &builtins) {
            Ok(p) => p,
            Err(name) => {
                tally.missing(
                    MissingKind::NotDownloadedHere,
                    MissingItem::Shader,
                    name,
                    &target,
                );
                continue;
            }
        };
        let params: Vec<(String, String)> = e.params.into_iter().collect();
        let mut wrote = false;
        for slot in tally.slots(&target, roms) {
            let key = slot.key();
            match local.get(&key) {
                Some((_, false)) => {
                    tally.kept_local += 1;
                    continue;
                }
                Some((id, true)) => tx.delete_shader_assignment(id).await.map_err(repo)?,
                None => {}
            }
            tx.ensure_shader_preset(&preset).await.map_err(repo)?;
            let id = tx
                .insert_shader_assignment(
                    slot.scope,
                    slot.system_id.as_deref(),
                    slot.rom_id.as_deref(),
                    &preset.id,
                    &params,
                )
                .await
                .map_err(repo)?;
            local.insert(key, (id, true));
            wrote = true;
        }
        tally.applied += u32::from(wrote);
    }

    // Molduras: as do The Bezel Project trocam; as de um pacote importado do
    // disco são desta máquina e ficam.
    let packs = tx.decoration_packs().await.map_err(repo)?;
    let pack_by_id: HashMap<&str, &DecorationPack> =
        packs.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut local: HashMap<SlotKey, (String, bool)> = HashMap::new();
    for d in tx.decoration_assignments().await.map_err(repo)? {
        let key = slot_key(d.scope, d.system_id.as_deref(), d.rom_id.as_deref());
        let portable = roms.in_library(d.scope, d.rom_id.as_deref())
            && pack_by_id
                .get(d.pack_id.as_str())
                .is_some_and(|p| bezel_asset(p, &d.asset_path, catalog).is_some());
        let protected = missing_slots.contains(&(MissingItem::Decoration, key.clone()));
        if mode == ApplyMode::Restore && portable && !protected {
            tx.delete_decoration_assignment(&d.id).await.map_err(repo)?;
            continue;
        }
        local.insert(key, (d.id, portable));
    }
    // o pacote que `bezel_pack` importou aqui, ou o mesmo criado agora
    let bezel_pack = packs
        .iter()
        .find(|p| Path::new(&p.base_path) == catalog.bezels.as_path())
        .cloned()
        .unwrap_or_else(|| bezel_pack_row(catalog));
    let mut pack_ready = false;
    for e in doc.decorations {
        let target = Target::parse(&e.scope, e.system_id.as_deref(), e.crc32.as_deref())?;
        let parts = rel_parts(&e.asset)?;
        let file = under(&catalog.bezels, &parts);
        // o sistema não foi baixado aqui (ou é de um catálogo que esta versão
        // não conhece): a moldura não aponta para nada
        if e.pack != BEZEL_PROJECT || !file.is_file() {
            let name = if e.pack == BEZEL_PROJECT {
                parts[0].to_string()
            } else {
                bare_name(&e.pack)
            };
            tally.missing(
                MissingKind::NotDownloadedHere,
                MissingItem::Decoration,
                name,
                &target,
            );
            continue;
        }
        let asset_path = file.display().to_string();
        let mut wrote = false;
        for slot in tally.slots(&target, roms) {
            let key = slot.key();
            match local.get(&key) {
                Some((_, false)) => {
                    tally.kept_local += 1;
                    continue;
                }
                Some((id, true)) => tx.delete_decoration_assignment(id).await.map_err(repo)?,
                None => {}
            }
            if !pack_ready {
                tx.ensure_decoration_pack(&bezel_pack).await.map_err(repo)?;
                pack_ready = true;
            }
            let a = DecorationAssignment {
                scope: slot.scope,
                system_id: slot.system_id,
                rom_id: slot.rom_id,
                pack_id: bezel_pack.id.clone(),
                asset_path: asset_path.clone(),
            };
            let id = tx.insert_decoration_assignment(&a).await.map_err(repo)?;
            local.insert(key, (id, true));
            wrote = true;
        }
        tally.applied += u32::from(wrote);
    }
    Ok(())
}

// --- hotkeys e teclado -------------------------------------------------------

fn is_key(e: &RawInputEvent) -> bool {
    matches!(e, RawInputEvent::Keyboard { .. })
}

/// O gatilho de uma hotkey que pode ir para outra máquina: só teclas. Um
/// botão de controle leva o GUID do controle, e uma linha com `device_guid`
/// vale só para aquele aparelho: as duas ficam nesta máquina.
fn portable_trigger(h: &db::HotkeyRow) -> Option<Vec<RawInputEvent>> {
    if h.device_guid.is_some() {
        return None;
    }
    let trigger: Vec<RawInputEvent> = serde_json::from_str(&h.trigger_json).ok()?;
    (!trigger.is_empty() && trigger.iter().all(is_key)).then_some(trigger)
}

async fn export_hotkeys(tx: &mut SettingsTx) -> Result<HotkeysDoc, String> {
    let mut hotkeys = BTreeMap::new();
    for h in tx.hotkeys().await.map_err(repo)? {
        if let Some(trigger) = portable_trigger(&h) {
            hotkeys.entry(h.action).or_insert(trigger);
        }
    }
    let keyboard = tx
        .keyboard_bindings()
        .await
        .map_err(repo)?
        .into_iter()
        .map(|(target, code)| KeyEntry { target, code })
        .collect();
    Ok(HotkeysDoc {
        v: DOC_VERSION,
        hotkeys: hotkeys
            .into_iter()
            .map(|(action, trigger)| HotkeyEntry { action, trigger })
            .collect(),
        keyboard,
    })
}

async fn apply_hotkeys(
    tx: &mut SettingsTx,
    doc: HotkeysDoc,
    mode: ApplyMode,
    tally: &mut Tally,
) -> Result<(), String> {
    // Uma linha por ação. A de teclado troca; a de controle é desta máquina e
    // fica. Uma ação que o documento não traz (lá ela pode estar no controle)
    // fica como está na cópia.
    let mut local: HashMap<String, (String, bool)> = HashMap::new();
    for h in tx.hotkeys().await.map_err(repo)? {
        let portable = portable_trigger(&h).is_some();
        if mode == ApplyMode::Restore && portable {
            tx.delete_hotkey(&h.id).await.map_err(repo)?;
            continue;
        }
        local.insert(h.action, (h.id, portable));
    }
    for e in &doc.hotkeys {
        // ação de uma versão mais nova: esta não sabe o que fazer com ela
        if SystemAction::from_wire(&e.action).is_none() {
            continue;
        }
        match local.get(&e.action) {
            Some((_, false)) => {
                tally.kept_local += 1;
                continue;
            }
            Some((id, true)) => tx.delete_hotkey(id).await.map_err(repo)?,
            None => {}
        }
        let json = serde_json::to_string(&e.trigger).map_err(|_| invalid())?;
        tx.insert_hotkey(&e.action, &json).await.map_err(repo)?;
        local.insert(e.action.clone(), (e.action.clone(), true));
        tally.applied += 1;
    }

    if mode == ApplyMode::Restore {
        tx.clear_keyboard_bindings().await.map_err(repo)?;
    }
    for k in &doc.keyboard {
        // na cópia: um alvo de uma versão mais nova, ou o Esc que o menu
        // reserva (`set_keyboard_binding`), não entra
        let foreign =
            input_desktop::keymap::target_by_name(&k.target).is_none() || k.code == "Escape";
        if mode == ApplyMode::Merge && foreign {
            continue;
        }
        tx.put_keyboard_binding(mode.put(), &k.target, &k.code)
            .await
            .map_err(repo)?;
        tally.applied += 1;
    }
    Ok(())
}

// --- áudio e vídeo -----------------------------------------------------------

async fn export_audio(tx: &mut SettingsTx) -> Result<AudioDoc, String> {
    let a = tx.audio().await.map_err(repo)?;
    Ok(AudioDoc {
        v: DOC_VERSION,
        rate_control_enabled: a.rate_control_enabled,
        rate_control_delta: a.rate_control_delta,
        sample_rate_preference: a.sample_rate_preference.and_then(|v| u32::try_from(v).ok()),
    })
}

async fn apply_audio(tx: &mut SettingsTx, doc: AudioDoc, tally: &mut Tally) -> Result<(), String> {
    tx.set_audio(&AudioSettings {
        rate_control_enabled: doc.rate_control_enabled,
        rate_control_delta: doc.rate_control_delta,
        sample_rate_preference: doc.sample_rate_preference.map(i64::from),
    })
    .await
    .map_err(repo)?;
    tally.applied += 1;
    Ok(())
}

/// O alvo de uma proporção do documento.
fn aspect_target(e: &AspectEntry) -> Result<Target, String> {
    match e.scope.as_str() {
        "rom" => Target::parse("rom", None, Some(&e.target)),
        scope => Target::parse(scope, Some(&e.target), None),
    }
}

/// `(scope, target)` de `aspect_assignments` na forma do documento; `None`
/// para a de um jogo fora da biblioteca.
fn aspect_key(row: &AspectRow, crc_of: &HashMap<String, String>) -> Option<(String, String)> {
    match row.scope.as_str() {
        "default" => Some(("default".into(), String::new())),
        "system" if !row.target.is_empty() => Some(("system".into(), row.target.clone())),
        "rom" => crc_of
            .get(&row.target)
            .map(|crc| ("rom".into(), crc.clone())),
        _ => None,
    }
}

async fn export_video(tx: &mut SettingsTx) -> Result<VideoDoc, String> {
    let crc_of = crc_by_rom(tx).await?;
    let v = tx.video().await.map_err(repo)?;
    let mut aspect = BTreeMap::new();
    for row in tx.aspects().await.map_err(repo)? {
        if let Some(key) = aspect_key(&row, &crc_of) {
            aspect.entry(key).or_insert(row.aspect);
        }
    }
    Ok(VideoDoc {
        v: DOC_VERSION,
        integer_scaling: v.integer_scaling,
        hdr: v.hdr,
        hdr_highlights: v.hdr_highlights,
        aspect: aspect
            .into_iter()
            .map(|((scope, target), aspect)| AspectEntry {
                scope,
                target,
                aspect,
            })
            .collect(),
    })
}

async fn apply_video(
    tx: &mut SettingsTx,
    doc: VideoDoc,
    mode: ApplyMode,
    roms: &LocalRoms<'_>,
    tally: &mut Tally,
) -> Result<(), String> {
    tx.set_video(&VideoSettings {
        integer_scaling: doc.integer_scaling,
        hdr: doc.hdr,
        hdr_highlights: doc.hdr_highlights,
    })
    .await
    .map_err(repo)?;
    tally.applied += 1;

    if mode == ApplyMode::Restore {
        // a proporção de um jogo fora da biblioteca não é de cópia nenhuma e fica
        for row in tx.aspects().await.map_err(repo)? {
            let portable = match row.scope.as_str() {
                "default" | "system" => true,
                "rom" => roms.has(&row.target),
                _ => false,
            };
            if portable {
                tx.delete_aspect(&row.scope, &row.target)
                    .await
                    .map_err(repo)?;
            }
        }
    }
    for e in &doc.aspect {
        let target = aspect_target(e)?;
        // proporção de uma versão mais nova: esta não sabe desenhar
        if !ASPECTS.contains(&e.aspect.as_str()) {
            continue;
        }
        let mut wrote = false;
        for slot in tally.slots(&target, roms) {
            let (scope, target) = match slot.scope {
                AssignmentScope::Default => ("default", String::new()),
                AssignmentScope::System => ("system", slot.system_id.unwrap_or_default()),
                AssignmentScope::Rom => ("rom", slot.rom_id.unwrap_or_default()),
            };
            let row = AspectRow {
                scope: scope.to_string(),
                target,
                aspect: e.aspect.clone(),
            };
            tx.put_aspect(mode.put(), &row).await.map_err(repo)?;
            wrote = true;
        }
        tally.applied += u32::from(wrote);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::audio::{AudioConfig, AudioConfigRepository};
    use domain::core_options::CoreOptionsStore;
    use domain::hotkeys::{HotkeyBinding, SystemAction, SystemHotkeyRepository};
    use domain::input::{
        ControllerLayoutEntry, ControllerMapping, ControllerMappingRepository,
        DevicePortRepository, MappingSource, RawInputEvent, RetroPadButton,
    };
    use domain::shader_chain::{AssignmentScope, ShaderChainStore, ShaderFormat, ShaderPreset};
    use domain::video::{VideoConfig, VideoConfigRepository};
    use serde_json::json;
    use std::path::Path;
    use ApplyMode::{Merge, Restore};

    /// GUID de um controle (formato SDL), o desta máquina e o de outra.
    const GUID: &str = "030000005e0400008e02000000007200";
    const OTHER_GUID: &str = "03000000de280000ff11000001000000";
    const MAC: &str = "ppsspp_change_mac_address";

    const CO: Group = Group::CoreOptions;
    const SD: Group = Group::ShadersDecorations;
    const HK: Group = Group::HotkeysKeyboard;

    /// Uma máquina: o banco, as pastas dos catálogos (no disco, numa pasta
    /// temporária só dela) e os cores instalados.
    struct Machine {
        db: Db,
        catalog: Catalog,
        cores: Vec<String>,
        root: std::path::PathBuf,
    }

    impl Drop for Machine {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    impl Machine {
        async fn new(tag: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("reemu-settings-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            Machine {
                db: db::connect_in_memory().await.unwrap(),
                catalog: Catalog::new(&root.join("shaders"), &root.join("decorations")),
                cores: vec!["snes9x_libretro".into(), "mgba_libretro".into()],
                root,
            }
        }

        async fn roms(&self) -> Vec<(String, String)> {
            sqlx::query_as("SELECT id, crc32 FROM roms ORDER BY id")
                .fetch_all(&self.db)
                .await
                .unwrap()
        }

        async fn export(&self, group: Group) -> Value {
            export(group, &self.db, &self.catalog).await.unwrap()
        }

        async fn apply_all(
            &self,
            docs: &[(Group, &Value)],
            mode: ApplyMode,
        ) -> Result<Vec<(Group, ApplyReport)>, String> {
            let roms = self.roms().await;
            let here = ThisMachine {
                roms: &roms,
                cores: &self.cores,
                catalog: &self.catalog,
            };
            apply_all(&self.db, docs, mode, &here).await
        }

        async fn apply(
            &self,
            group: Group,
            doc: &Value,
            mode: ApplyMode,
        ) -> Result<ApplyReport, String> {
            let roms = self.roms().await;
            let here = ThisMachine {
                roms: &roms,
                cores: &self.cores,
                catalog: &self.catalog,
            };
            apply(group, &self.db, doc, mode, &here).await
        }

        /// Um `.slangp` do pacote baixado nesta máquina.
        fn slang(&self, rel: &str) -> String {
            join(&self.catalog.shader_pack, rel)
        }

        /// Uma imagem do The Bezel Project baixada nesta máquina.
        fn bezel(&self, rel: &str) -> String {
            join(&self.catalog.bezels, rel)
        }

        /// O pacote do The Bezel Project como `bezel_pack` o importa.
        async fn bezel_pack(&self) -> String {
            let id = format!("pack:{}", self.catalog.bezels.display());
            let base = self.catalog.bezels.display().to_string();
            pack(&self.db, &id, "bezelproject", false, &base).await;
            id
        }
    }

    fn join(root: &Path, rel: &str) -> String {
        let mut p = root.to_path_buf();
        p.extend(rel.split('/'));
        p.to_string_lossy().into_owned()
    }

    /// O arquivo existe nesta máquina.
    fn touch(path: &str) {
        let p = Path::new(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    async fn run(db: &Db, sql: &str, args: &[&str]) {
        let mut q = sqlx::query(sql);
        for a in args {
            q = q.bind(*a);
        }
        q.execute(db).await.unwrap();
    }

    /// Um caminho absoluto de verdade nesta máquina, fora dos catálogos.
    fn abs(parts: &[&str]) -> String {
        let mut p = std::env::temp_dir();
        for part in parts {
            p.push(part);
        }
        p.to_string_lossy().into_owned()
    }

    async fn rom(db: &Db, id: &str, crc32: &str) {
        let path = abs(&["Jogos", &format!("{id}.sfc")]);
        run(
            db,
            "INSERT INTO roms (id, file_path, crc32, md5, system_id, added_at) \
             VALUES (?1, ?2, ?3, 'md5', 'snes', 0)",
            &[id, &path, crc32],
        )
        .await;
    }

    /// Cada linha da tabela, como texto, em ordem.
    async fn dump(db: &Db, table: &str) -> Vec<String> {
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
        sqlx::query_scalar(&format!("SELECT {expr} FROM {table} ORDER BY 1"))
            .fetch_all(db)
            .await
            .unwrap()
    }

    /// As tabelas que os grupos escrevem e as que nunca viajam. Sem
    /// `shader_presets`: é o catálogo de presets usados (a tela também nunca
    /// apaga uma linha dele), não uma escolha (R36).
    const TABLES: &[&str] = &[
        "core_options_values",
        "core_option_overrides",
        "system_core_prefs",
        "shader_chain_assignments",
        "shader_parameter_overrides",
        "decoration_packs",
        "decoration_assignments",
        "system_hotkeys",
        "keyboard_bindings",
        "audio_config",
        "video_config",
        "aspect_assignments",
        "library_folders",
        "controller_mappings",
        "device_port_assignment",
    ];

    async fn snapshot(db: &Db) -> Vec<String> {
        let mut out = Vec::new();
        for t in TABLES {
            for row in dump(db, t).await {
                out.push(format!("{t}: {row}"));
            }
        }
        out
    }

    async fn option(db: &Db, core: &str, rom: Option<&str>, key: &str, value: &str) {
        db::CoreOptionsRepo::new(db.clone())
            .set_scoped_value(core, rom, key, Some(value))
            .await
            .unwrap();
    }

    async fn core_pref(db: &Db, system: &str, core: &str) {
        run(
            db,
            "INSERT INTO system_core_prefs (system_id, core_id) VALUES (?1, ?2)",
            &[system, core],
        )
        .await;
    }

    async fn preset(db: &Db, source: &str, name: &str) {
        db::ShaderChainRepo::new(db.clone())
            .upsert_preset(&ShaderPreset {
                id: source.into(),
                name: name.into(),
                source_path: source.into(),
                format: ShaderFormat::Slang,
                is_builtin: !source.ends_with(".slangp"),
                includes_bezel: false,
            })
            .await
            .unwrap();
    }

    async fn shader(
        db: &Db,
        scope: AssignmentScope,
        target: Option<&str>,
        source: &str,
        params: &[(&str, &str)],
    ) {
        let repo = db::ShaderChainRepo::new(db.clone());
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

    async fn pack(db: &Db, id: &str, name: &str, bundled: bool, base: &str) {
        let source = if bundled { "bundled" } else { "user_imported" };
        run(
            db,
            "INSERT INTO decoration_packs (id, name, source, base_path) VALUES (?1, ?2, ?3, ?4)",
            &[id, name, source, base],
        )
        .await;
    }

    /// Atribuição de moldura com o `id` que o `DecorationRepo` grava.
    async fn deco(db: &Db, scope: &str, target: &str, pack_id: &str, asset: &str) {
        let id = match scope {
            "default" => format!("{pack_id}:default"),
            _ => format!("{pack_id}:{scope}:{target}"),
        };
        let (sys, rom) = match scope {
            "system" => (target, ""),
            "rom" => ("", target),
            _ => ("", ""),
        };
        run(
            db,
            "INSERT INTO decoration_assignments (id, scope, system_id, rom_id, pack_id, asset_path) \
             VALUES (?1, ?2, NULLIF(?3, ''), NULLIF(?4, ''), ?5, ?6)",
            &[&id, scope, sys, rom, pack_id, asset],
        )
        .await;
    }

    fn key(code: &str) -> RawInputEvent {
        RawInputEvent::Keyboard {
            scancode: input_desktop::keymap::key_scancode(code),
        }
    }

    fn pad(guid: &str, index: u32) -> RawInputEvent {
        RawInputEvent::GamepadButton {
            device_guid: guid.into(),
            index,
        }
    }

    async fn hotkey(db: &Db, action: SystemAction, trigger: Vec<RawInputEvent>) {
        db::SystemHotkeysRepo::new(db.clone())
            .set(&HotkeyBinding {
                action,
                trigger,
                device_guid: None,
            })
            .await
            .unwrap();
    }

    async fn key_binding(db: &Db, target: &str, code: &str) {
        db::KeyboardBindingsRepo::new(db.clone())
            .set(target, code)
            .await
            .unwrap();
    }

    async fn set_audio(db: &Db, c: AudioConfig) {
        db::AudioConfigRepo::new(db.clone())
            .update(&c)
            .await
            .unwrap();
    }

    async fn set_video(db: &Db, c: VideoConfig) {
        db::VideoConfigRepo::new(db.clone())
            .update(&c)
            .await
            .unwrap();
    }

    async fn aspect(db: &Db, scope: &str, target: &str, value: &str) {
        db::AspectRepo::new(db.clone())
            .set(scope, target, Some(value))
            .await
            .unwrap();
    }

    async fn rows<T>(db: &Db, sql: &str) -> Vec<T>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        sqlx::query_as(sql).fetch_all(db).await.unwrap()
    }

    fn s3(a: &str, b: &str, c: &str) -> (String, String, String) {
        (a.into(), b.into(), c.into())
    }

    fn s4(a: &str, b: &str, c: &str, d: &str) -> (String, String, String, String) {
        (a.into(), b.into(), c.into(), d.into())
    }

    /// Uma máquina com alguma coisa em cada grupo; `n` muda os valores.
    async fn machine(tag: &str, n: &str) -> Machine {
        let m = Machine::new(tag).await;
        let db = &m.db;
        let a = n == "a";
        rom(db, &format!("{n}-1"), "AAAA0001").await;
        rom(db, &format!("{n}-2"), "AAAA0002").await;
        option(db, "snes9x", None, "snes9x_region", n).await;
        option(db, "mgba", None, "mgba_solar", n).await;
        option(db, "snes9x", Some(&format!("{n}-1")), "snes9x_overclock", n).await;
        core_pref(
            db,
            "snes",
            if a {
                "snes9x_libretro"
            } else {
                "mgba_libretro"
            },
        )
        .await;
        preset(db, "crt", "crt").await;
        preset(db, "lcd", "lcd").await;
        shader(db, AssignmentScope::Default, None, "crt", &[("GAMMA", n)]).await;
        let rom2 = format!("{n}-2");
        let game_shader = if a { "lcd" } else { "crt" };
        shader(db, AssignmentScope::Rom, Some(&rom2), game_shader, &[]).await;
        let save = if a { "F5" } else { "F6" };
        hotkey(db, SystemAction::QuickSave, vec![key(save)]).await;
        key_binding(db, "R2", if a { "KeyR" } else { "KeyT" }).await;
        set_audio(
            db,
            AudioConfig {
                output_device_id: Some(format!("aparelho-{n}")),
                output_device_name: Some(format!("Saída {n}")),
                rate_control_enabled: a,
                rate_control_delta: if a { 0.5 } else { 0.25 },
                sample_rate_preference: a.then_some(48000),
            },
        )
        .await;
        set_video(
            db,
            VideoConfig {
                integer_scaling: a,
                hdr: a,
                hdr_paper_white_nits: 100,
                hdr_highlights: !a,
                hdr_peak_nits: 400,
            },
        )
        .await;
        aspect(db, "default", "", if a { "4:3" } else { "16:9" }).await;
        aspect(db, "system", "snes", if a { "fill" } else { "core" }).await;
        m
    }

    // --- opções de core ----------------------------------------------------

    #[tokio::test]
    async fn export_then_apply_round_trips_core_options() {
        let src = Machine::new("co-src").await;
        rom(&src.db, "src-1", "AAAA0001").await;
        rom(&src.db, "src-2", "AAAA0002").await;
        option(&src.db, "snes9x", None, "snes9x_region", "pal").await;
        option(&src.db, "snes9x", None, "snes9x_overclock", "on").await;
        option(&src.db, "mgba", None, "mgba_solar", "5").await;
        option(&src.db, "snes9x", Some("src-1"), "snes9x_region", "ntsc").await;
        option(&src.db, "mgba", Some("src-2"), "mgba_solar", "10").await;
        core_pref(&src.db, "snes", "snes9x_libretro").await;
        core_pref(&src.db, "psx", "swanstation_libretro").await;

        // os mesmos jogos com outros ids, e escolhas que o backup troca
        let dst = Machine::new("co-dst").await;
        rom(&dst.db, "dst-1", "AAAA0001").await;
        rom(&dst.db, "dst-2", "AAAA0002").await;
        option(&dst.db, "snes9x", None, "snes9x_region", "ntsc").await;
        option(&dst.db, "genesis_plus_gx", None, "gpgx_region", "auto").await;
        option(&dst.db, "snes9x", Some("dst-1"), "snes9x_overclock", "off").await;
        core_pref(&dst.db, "snes", "mgba_libretro").await;

        let doc = src.export(CO).await;
        assert_eq!(
            doc,
            json!({
                "v": 1,
                "values": {
                    "mgba": {"mgba_solar": "5"},
                    "snes9x": {"snes9x_overclock": "on", "snes9x_region": "pal"},
                },
                "overrides": [
                    {"crc32": "AAAA0001", "core_id": "snes9x", "key": "snes9x_region", "value": "ntsc"},
                    {"crc32": "AAAA0002", "core_id": "mgba", "key": "mgba_solar", "value": "10"},
                ],
                "preferred_cores": {"psx": "swanstation_libretro", "snes": "snes9x_libretro"},
            })
        );
        let report = dst.apply(CO, &doc, Restore).await.unwrap();
        assert_eq!(
            report,
            ApplyReport {
                applied: 7,
                ..ApplyReport::default()
            }
        );
        assert_eq!(dst.export(CO).await, doc);
    }

    #[tokio::test]
    async fn merge_keeps_options_of_cores_and_games_the_document_lacks() {
        let src = Machine::new("merge-co-src").await;
        rom(&src.db, "s1", "AAAA0001").await;
        option(&src.db, "snes9x", None, "snes9x_region", "pal").await;
        option(&src.db, "snes9x", Some("s1"), "snes9x_overclock", "on").await;
        let doc = src.export(CO).await;

        let here = Machine::new("merge-co-here").await;
        rom(&here.db, "h1", "AAAA0001").await;
        rom(&here.db, "h2", "AAAA0002").await;
        option(&here.db, "snes9x", None, "snes9x_region", "ntsc").await;
        // o que o documento não traz: outra opção do mesmo core, outro core,
        // outra opção do mesmo jogo e um jogo que a outra máquina não tem
        option(&here.db, "snes9x", None, "snes9x_frameskip", "2").await;
        option(&here.db, "genesis_plus_gx", None, "gpgx_region", "auto").await;
        option(&here.db, "snes9x", Some("h1"), "snes9x_region", "eu").await;
        option(&here.db, "snes9x", Some("h2"), "snes9x_region", "jp").await;

        let report = here.apply(CO, &doc, Merge).await.unwrap();
        assert_eq!(
            report,
            ApplyReport {
                applied: 2,
                ..ApplyReport::default()
            }
        );
        let values: Vec<(String, String, String)> = rows(
            &here.db,
            "SELECT core_id, option_key, value FROM core_options_values ORDER BY core_id, option_key",
        )
        .await;
        assert_eq!(
            values,
            vec![
                s3("genesis_plus_gx", "gpgx_region", "auto"),
                s3("snes9x", "snes9x_frameskip", "2"),
                s3("snes9x", "snes9x_region", "pal"),
            ]
        );
        let overrides: Vec<(String, String, String, String)> = rows(
            &here.db,
            "SELECT rom_id, core_id, option_key, value FROM core_option_overrides \
             ORDER BY rom_id, option_key",
        )
        .await;
        assert_eq!(
            overrides,
            vec![
                s4("h1", "snes9x", "snes9x_overclock", "on"),
                s4("h1", "snes9x", "snes9x_region", "eu"),
                s4("h2", "snes9x", "snes9x_region", "jp"),
            ]
        );
    }

    #[tokio::test]
    async fn apply_core_options_skips_games_this_machine_lacks() {
        let src = Machine::new("skip-src").await;
        rom(&src.db, "a", "0000000A").await;
        rom(&src.db, "b", "0000000B").await;
        rom(&src.db, "c", "0000000C").await;
        option(&src.db, "snes9x", None, "snes9x_region", "pal").await;
        option(&src.db, "snes9x", Some("a"), "k", "1").await;
        option(&src.db, "snes9x", Some("b"), "k", "2").await;
        option(&src.db, "snes9x", Some("b"), "j", "3").await;
        option(&src.db, "mgba", Some("c"), "k", "4").await;
        let doc = src.export(CO).await;

        // o mesmo jogo duas vezes na biblioteca (duas pastas): os dois recebem
        let dst = Machine::new("skip-dst").await;
        rom(&dst.db, "x1", "0000000A").await;
        rom(&dst.db, "x2", "0000000A").await;
        // opção de um jogo que já saiu da biblioteca: não é da cópia, fica
        option(&dst.db, "snes9x", Some("sumiu"), "k", "9").await;

        for mode in [Merge, Restore] {
            let report = dst.apply(CO, &doc, mode).await.unwrap();
            assert_eq!(
                report.skipped_unknown_games, 2,
                "{mode:?}: B e C, não três opções"
            );
            assert_eq!(
                report.applied, 2,
                "{mode:?}: o valor do core e a opção do jogo A"
            );
            let overrides: Vec<(String, String, String, String)> = rows(
                &dst.db,
                "SELECT rom_id, core_id, option_key, value FROM core_option_overrides \
                 ORDER BY rom_id",
            )
            .await;
            assert_eq!(
                overrides,
                vec![
                    s4("sumiu", "snes9x", "k", "9"),
                    s4("x1", "snes9x", "k", "1"),
                    s4("x2", "snes9x", "k", "1"),
                ]
            );
        }
    }

    #[tokio::test]
    async fn local_only_options_never_travel_and_never_change() {
        let src = Machine::new("mac-src").await;
        let core = "ppsspp_libretro";
        option(&src.db, core, None, &format!("{MAC}01"), "5").await;
        option(&src.db, core, None, &format!("{MAC}02"), "0").await;
        option(&src.db, core, None, "ppsspp_internal_resolution", "960x544").await;
        let doc = src.export(CO).await;
        assert_eq!(
            doc["values"],
            json!({core: {"ppsspp_internal_resolution": "960x544"}})
        );

        // o MAC que o PPSSPP sorteou nesta máquina e os saves dela esperam
        let here = Machine::new("mac-here").await;
        option(&here.db, core, None, &format!("{MAC}01"), "12").await;
        option(&here.db, core, None, &format!("{MAC}02"), "3").await;
        let mac_sql = "SELECT core_id, option_key, value FROM core_options_values \
                       WHERE option_key LIKE 'ppsspp_change_mac_address%' ORDER BY option_key";
        let before: Vec<(String, String, String)> = rows(&here.db, mac_sql).await;
        assert_eq!(before.len(), 2);

        // um documento que traz o MAC (de outra versão, ou mexido à mão)
        let mut forged = doc.clone();
        forged["values"][core][format!("{MAC}01")] = json!("7");
        forged["values"]["outro_core"] = json!({format!("{MAC}03"): "9"});
        for d in [&doc, &forged] {
            for mode in [Merge, Restore] {
                here.apply(CO, d, mode).await.unwrap();
                let now: Vec<(String, String, String)> = rows(&here.db, mac_sql).await;
                assert_eq!(now, before, "{mode:?}");
            }
        }
        let res: Vec<(String,)> = rows(
            &here.db,
            "SELECT value FROM core_options_values WHERE option_key = 'ppsspp_internal_resolution'",
        )
        .await;
        assert_eq!(res, vec![("960x544".to_string(),)]);
    }

    #[tokio::test]
    async fn preferred_cores_travel_only_for_cores_installed_here() {
        let src = Machine::new("pref-src").await;
        core_pref(&src.db, "snes", "snes9x_libretro").await;
        core_pref(&src.db, "psx", "swanstation_libretro").await;
        let doc = src.export(CO).await;

        let here = Machine::new("pref-here").await;
        core_pref(&here.db, "psx", "pcsx_rearmed_libretro").await;
        core_pref(&here.db, "gba", "mgba_libretro").await;
        let report = here.apply(CO, &doc, Merge).await.unwrap();
        assert_eq!(report.applied, 1);
        assert_eq!(report.cores_not_installed, vec!["swanstation_libretro"]);
        let prefs: Vec<(String, String)> = rows(
            &here.db,
            "SELECT system_id, core_id FROM system_core_prefs ORDER BY system_id",
        )
        .await;
        let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(
            prefs,
            vec![
                pair("gba", "mgba_libretro"),
                pair("psx", "pcsx_rearmed_libretro"),
                pair("snes", "snes9x_libretro"),
            ]
        );
    }

    // --- shaders e molduras ------------------------------------------------

    #[tokio::test]
    async fn export_then_apply_round_trips_shaders_decorations() {
        use AssignmentScope::*;
        let src = Machine::new("sd-src").await;
        rom(&src.db, "src-1", "AAAA0001").await;
        preset(&src.db, "crt", "crt").await;
        preset(&src.db, "curated:xbr", "Suavizar pixel art (xBR)").await;
        let royale = src.slang("crt/crt-royale.slangp");
        preset(&src.db, &royale, "crt-royale").await;
        shader(
            &src.db,
            Default,
            None,
            "curated:xbr",
            &[("XBR_EDGE_STR", "0.6")],
        )
        .await;
        shader(&src.db, System, Some("snes"), "crt", &[]).await;
        shader(&src.db, System, Some("psx"), &royale, &[]).await;
        let gamma = [("CRT_GAMMA", "2.2"), ("MASK", "1")];
        shader(&src.db, Rom, Some("src-1"), "crt", &gamma).await;
        let pid = src.bezel_pack().await;
        deco(&src.db, "system", "snes", &pid, &src.bezel("snes/snes.png")).await;
        let game = src.bezel("snes/games/AAAA0001.png");
        deco(&src.db, "rom", "src-1", &pid, &game).await;

        // os dois pacotes baixados aqui também, em outra pasta, e um shader que
        // o backup troca
        let dst = Machine::new("sd-dst").await;
        rom(&dst.db, "dst-1", "AAAA0001").await;
        preset(&dst.db, "plain", "plain").await;
        shader(&dst.db, Default, None, "plain", &[]).await;
        touch(&dst.slang("crt/crt-royale.slangp"));
        touch(&dst.slang("edge-smoothing/xbr/other presets/xbr-lv2-standalone.slangp"));
        touch(&dst.bezel("snes/snes.png"));
        touch(&dst.bezel("snes/games/AAAA0001.png"));

        let doc = src.export(SD).await;
        assert_eq!(
            doc,
            json!({
                "v": 1,
                "assignments": [
                    {"scope": "default", "preset": "curated:xbr",
                     "name": "Suavizar pixel art (xBR)", "params": {"XBR_EDGE_STR": "0.6"}},
                    {"scope": "system", "system_id": "psx",
                     "preset": "slang-shaders:crt/crt-royale.slangp", "name": "crt-royale"},
                    {"scope": "system", "system_id": "snes", "preset": "crt"},
                    {"scope": "rom", "crc32": "AAAA0001", "preset": "crt",
                     "params": {"CRT_GAMMA": "2.2", "MASK": "1"}},
                ],
                "decorations": [
                    {"scope": "system", "system_id": "snes", "pack": "bezelproject",
                     "asset": "snes/snes.png"},
                    {"scope": "rom", "crc32": "AAAA0001", "pack": "bezelproject",
                     "asset": "snes/games/AAAA0001.png"},
                ],
                "missing": [],
            })
        );
        let report = dst.apply(SD, &doc, Restore).await.unwrap();
        assert_eq!(
            report,
            ApplyReport {
                applied: 6,
                ..ApplyReport::default()
            }
        );
        assert_eq!(dst.export(SD).await, doc);
        // o preset e a moldura apontam para os arquivos DESTA máquina
        let psx: String = sqlx::query_scalar(
            "SELECT preset_id FROM shader_chain_assignments WHERE system_id = 'psx'",
        )
        .fetch_one(&dst.db)
        .await
        .unwrap();
        assert_eq!(psx, dst.slang("crt/crt-royale.slangp"));
        let asset: String = sqlx::query_scalar(
            "SELECT asset_path FROM decoration_assignments WHERE scope = 'system'",
        )
        .fetch_one(&dst.db)
        .await
        .unwrap();
        assert_eq!(asset, dst.bezel("snes/snes.png"));
    }

    #[tokio::test]
    async fn downloaded_shader_presets_travel_when_the_pack_is_here() {
        use AssignmentScope::*;
        let src = Machine::new("slang-src").await;
        let royale = src.slang("crt/crt-royale.slangp");
        preset(&src.db, &royale, "crt-royale").await;
        shader(&src.db, System, Some("snes"), &royale, &[("MASK", "2")]).await;
        preset(&src.db, "curated:xbr", "Suavizar pixel art (xBR)").await;
        shader(&src.db, System, Some("gba"), "curated:xbr", &[]).await;
        let doc = src.export(SD).await;
        assert_eq!(
            doc["assignments"],
            json!([
                {"scope": "system", "system_id": "gba", "preset": "curated:xbr",
                 "name": "Suavizar pixel art (xBR)"},
                {"scope": "system", "system_id": "snes",
                 "preset": "slang-shaders:crt/crt-royale.slangp", "name": "crt-royale",
                 "params": {"MASK": "2"}},
            ])
        );

        // o pacote baixado aqui: os dois apontam para os arquivos desta máquina
        let here = Machine::new("slang-here").await;
        touch(&here.slang("crt/crt-royale.slangp"));
        touch(&here.slang("edge-smoothing/xbr/other presets/xbr-lv2-standalone.slangp"));
        let report = here.apply(SD, &doc, Merge).await.unwrap();
        assert_eq!(
            report,
            ApplyReport {
                applied: 2,
                ..ApplyReport::default()
            }
        );
        assert_eq!(here.export(SD).await["assignments"], doc["assignments"]);
        let snes: String = sqlx::query_scalar(
            "SELECT preset_id FROM shader_chain_assignments WHERE system_id = 'snes'",
        )
        .fetch_one(&here.db)
        .await
        .unwrap();
        assert_eq!(snes, here.slang("crt/crt-royale.slangp"));

        // sem o pacote: os dois ficam de fora, pelo nome, e o SNES daqui continua
        let bare = Machine::new("slang-bare").await;
        preset(&bare.db, "lcd", "lcd").await;
        shader(&bare.db, System, Some("snes"), "lcd", &[]).await;
        let before = snapshot(&bare.db).await;
        let report = bare.apply(SD, &doc, Merge).await.unwrap();
        let missing = |name: &str, system: &str| MissingFile {
            kind: MissingKind::NotDownloadedHere,
            item: MissingItem::Shader,
            name: name.into(),
            scope: "system".into(),
            system_id: Some(system.into()),
            crc32: None,
        };
        assert_eq!(report.applied, 0);
        assert_eq!(
            report.missing_files,
            vec![
                missing("Suavizar pixel art (xBR)", "gba"),
                missing("crt-royale", "snes"),
            ]
        );
        assert_eq!(snapshot(&bare.db).await, before);

        // um curado que esta versão não conhece aparece pelo nome que veio
        let newer = json!({"v": 1, "decorations": [], "missing": [], "assignments": [
            {"scope": "default", "preset": "curated:novo", "name": "Shader Novo"},
        ]});
        let report = bare.apply(SD, &newer, Merge).await.unwrap();
        assert_eq!(report.missing_files[0].name, "Shader Novo");
        assert_eq!(report.missing_files[0].kind, MissingKind::NotDownloadedHere);
    }

    #[tokio::test]
    async fn bezel_project_decorations_travel_when_downloaded_here() {
        let src = Machine::new("bezel-src").await;
        rom(&src.db, "src-1", "AAAA0001").await;
        let pid = src.bezel_pack().await;
        deco(&src.db, "system", "snes", &pid, &src.bezel("snes/snes.png")).await;
        let chrono = src.bezel("snes/games/Chrono Trigger.png");
        deco(&src.db, "rom", "src-1", &pid, &chrono).await;
        let doc = src.export(SD).await;
        assert_eq!(
            doc["decorations"],
            json!([
                {"scope": "system", "system_id": "snes", "pack": "bezelproject",
                 "asset": "snes/snes.png"},
                {"scope": "rom", "crc32": "AAAA0001", "pack": "bezelproject",
                 "asset": "snes/games/Chrono Trigger.png"},
            ])
        );
        assert_eq!(doc["missing"], json!([]));

        // baixadas aqui também: apontam para a pasta desta máquina, no pacote
        // igual ao que `bezel_pack` importa
        let here = Machine::new("bezel-here").await;
        rom(&here.db, "here-1", "AAAA0001").await;
        touch(&here.bezel("snes/snes.png"));
        touch(&here.bezel("snes/games/Chrono Trigger.png"));
        let report = here.apply(SD, &doc, Merge).await.unwrap();
        assert_eq!(report.applied, 2);
        assert!(report.missing_files.is_empty());
        assert_eq!(here.export(SD).await["decorations"], doc["decorations"]);
        let game: String = sqlx::query_scalar(
            "SELECT asset_path FROM decoration_assignments WHERE rom_id = 'here-1'",
        )
        .fetch_one(&here.db)
        .await
        .unwrap();
        assert_eq!(game, here.bezel("snes/games/Chrono Trigger.png"));
        let packs: Vec<(String, String)> =
            rows(&here.db, "SELECT name, source FROM decoration_packs").await;
        assert_eq!(packs, vec![("bezelproject".into(), "user_imported".into())]);

        // não baixadas aqui: ficam de fora, pelo sistema, e nada muda
        let bare = Machine::new("bezel-bare").await;
        rom(&bare.db, "bare-1", "AAAA0001").await;
        let before = snapshot(&bare.db).await;
        let report = bare.apply(SD, &doc, Merge).await.unwrap();
        let missing = |scope: &str, system_id: Option<&str>, crc32: Option<&str>| MissingFile {
            kind: MissingKind::NotDownloadedHere,
            item: MissingItem::Decoration,
            name: "snes".into(),
            scope: scope.into(),
            system_id: system_id.map(Into::into),
            crc32: crc32.map(Into::into),
        };
        assert_eq!(report.applied, 0);
        assert_eq!(
            report.missing_files,
            vec![
                missing("rom", None, Some("AAAA0001")),
                missing("system", Some("snes"), None),
            ]
        );
        assert_eq!(snapshot(&bare.db).await, before);
    }

    #[tokio::test]
    async fn shaders_skip_imported_files_and_report_them() {
        use AssignmentScope::*;
        let src = Machine::new("imp-src").await;
        let mine = abs(&["Meus Shaders", "crt-do-ed.slangp"]);
        preset(&src.db, &mine, "crt-do-ed").await;
        shader(&src.db, System, Some("gba"), &mine, &[("GAMMA", "2.4")]).await;
        preset(&src.db, "crt", "crt").await;
        shader(&src.db, Default, None, "crt", &[]).await;
        let user_base = abs(&["Molduras do Ed"]);
        let user_pack = format!("pack:{user_base}");
        pack(&src.db, &user_pack, "Molduras do Ed", false, &user_base).await;
        let user_png = abs(&["Molduras do Ed", "padrao.png"]);
        deco(&src.db, "default", "", &user_pack, &user_png).await;

        let doc = src.export(SD).await;
        assert_eq!(
            doc["missing"],
            json!([
                {"item": "shader", "name": "crt-do-ed", "scope": "system", "system_id": "gba"},
                {"item": "decoration", "name": "Molduras do Ed", "scope": "default"},
            ])
        );
        assert_eq!(
            doc["assignments"],
            json!([{"scope": "default", "preset": "crt"}])
        );
        assert_eq!(doc["decorations"], json!([]));

        // aqui: lcd no GBA (do catálogo) e um preset importado DESTA máquina no
        // lugar de todos os jogos
        let here = Machine::new("imp-here").await;
        preset(&here.db, "lcd", "lcd").await;
        shader(&here.db, System, Some("gba"), "lcd", &[]).await;
        let own = abs(&["Shaders Daqui", "meu.slangp"]);
        preset(&here.db, &own, "meu").await;
        shader(&here.db, Default, None, &own, &[]).await;

        let report = here.apply(SD, &doc, Merge).await.unwrap();
        let missing = |item, name: &str, scope: &str, system_id: Option<&str>| MissingFile {
            kind: MissingKind::ImportedElsewhere,
            item,
            name: name.into(),
            scope: scope.into(),
            system_id: system_id.map(Into::into),
            crc32: None,
        };
        assert_eq!(
            report,
            ApplyReport {
                applied: 0,
                skipped_unknown_games: 0,
                kept_local: 1,
                missing_files: vec![
                    missing(MissingItem::Shader, "crt-do-ed", "system", Some("gba")),
                    missing(MissingItem::Decoration, "Molduras do Ed", "default", None),
                ],
                cores_not_installed: vec![],
            }
        );
        // o GBA daqui fica com o lcd, o padrão com o arquivo daqui, e nada
        // aponta para um arquivo que esta máquina não tem
        let shaders: Vec<(String, Option<String>, String)> = rows(
            &here.db,
            "SELECT scope, system_id, preset_id FROM shader_chain_assignments ORDER BY scope",
        )
        .await;
        assert_eq!(
            shaders,
            vec![
                ("default".into(), None, own.clone()),
                ("system".into(), Some("gba".into()), "lcd".into()),
            ]
        );
        assert!(dump(&here.db, "decoration_assignments").await.is_empty());
    }

    #[tokio::test]
    async fn merge_keeps_per_game_rows_of_games_the_document_lacks() {
        use AssignmentScope::*;
        let src = Machine::new("game-src").await;
        rom(&src.db, "s1", "AAAA0001").await;
        preset(&src.db, "crt", "crt").await;
        shader(&src.db, Rom, Some("s1"), "crt", &[]).await;
        let pid = src.bezel_pack().await;
        deco(&src.db, "rom", "s1", &pid, &src.bezel("snes/games/A.png")).await;
        aspect(&src.db, "rom", "s1", "4:3").await;
        let shaders_doc = src.export(SD).await;
        let video_doc = src.export(Group::Video).await;

        // o mesmo jogo, e um jogo que só esta máquina tem, com escolhas dele;
        // e uma plataforma que o documento não menciona
        let here = Machine::new("game-here").await;
        rom(&here.db, "h1", "AAAA0001").await;
        rom(&here.db, "h2", "AAAA0002").await;
        preset(&here.db, "lcd", "lcd").await;
        shader(&here.db, Rom, Some("h2"), "lcd", &[]).await;
        shader(&here.db, System, Some("gba"), "lcd", &[]).await;
        touch(&here.bezel("snes/games/A.png"));
        touch(&here.bezel("snes/games/B.png"));
        let here_pid = here.bezel_pack().await;
        deco(
            &here.db,
            "rom",
            "h2",
            &here_pid,
            &here.bezel("snes/games/B.png"),
        )
        .await;
        aspect(&here.db, "rom", "h2", "16:9").await;
        aspect(&here.db, "system", "gba", "fill").await;

        here.apply(SD, &shaders_doc, Merge).await.unwrap();
        here.apply(Group::Video, &video_doc, Merge).await.unwrap();

        let shaders: Vec<(String, String)> = rows(
            &here.db,
            "SELECT id, preset_id FROM shader_chain_assignments ORDER BY id",
        )
        .await;
        let pair = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(
            shaders,
            vec![
                pair("rom:h1", "crt"),
                pair("rom:h2", "lcd"),
                pair("system:gba", "lcd")
            ]
        );
        let decos: Vec<(String, String)> = rows(
            &here.db,
            "SELECT rom_id, asset_path FROM decoration_assignments ORDER BY rom_id",
        )
        .await;
        assert_eq!(
            decos,
            vec![
                pair("h1", &here.bezel("snes/games/A.png")),
                pair("h2", &here.bezel("snes/games/B.png")),
            ]
        );
        let aspects: Vec<(String, String, String)> = rows(
            &here.db,
            "SELECT scope, target, aspect FROM aspect_assignments ORDER BY scope, target",
        )
        .await;
        assert_eq!(
            aspects,
            vec![
                s3("rom", "h1", "4:3"),
                s3("rom", "h2", "16:9"),
                s3("system", "gba", "fill"),
            ]
        );
    }

    // --- hotkeys e teclado -------------------------------------------------

    #[tokio::test]
    async fn export_then_apply_round_trips_hotkeys_keyboard() {
        let src = Machine::new("hk-src").await;
        hotkey(&src.db, SystemAction::ToggleMenuOverlay, vec![key("F1")]).await;
        hotkey(
            &src.db,
            SystemAction::QuickSave,
            vec![key("ShiftLeft"), key("F5")],
        )
        .await;
        // botão de controle: o GUID é deste controle, a hotkey fica nesta máquina
        hotkey(&src.db, SystemAction::QuickLoad, vec![pad(GUID, 4)]).await;
        key_binding(&src.db, "R2", "KeyR").await;
        key_binding(&src.db, "LStickUp", "").await;

        let dst = Machine::new("hk-dst").await;
        hotkey(&dst.db, SystemAction::QuickSave, vec![key("F2")]).await;
        hotkey(&dst.db, SystemAction::QuickLoad, vec![pad(OTHER_GUID, 7)]).await;
        key_binding(&dst.db, "A", "KeyZ").await;

        let doc = src.export(HK).await;
        let sc = input_desktop::keymap::key_scancode;
        assert_eq!(
            doc,
            json!({
                "v": 1,
                "hotkeys": [
                    {"action": "quick_save", "trigger": [
                        {"Keyboard": {"scancode": sc("ShiftLeft")}},
                        {"Keyboard": {"scancode": sc("F5")}},
                    ]},
                    {"action": "toggle_menu_overlay", "trigger": [
                        {"Keyboard": {"scancode": sc("F1")}},
                    ]},
                ],
                "keyboard": [
                    {"target": "LStickUp", "code": ""},
                    {"target": "R2", "code": "KeyR"},
                ],
            })
        );
        let report = dst.apply(HK, &doc, Restore).await.unwrap();
        assert_eq!(report.applied, 4);
        assert_eq!(dst.export(HK).await, doc);
        // o botão do controle desta máquina continua onde estava
        let load: String = sqlx::query_scalar(
            "SELECT trigger_json FROM system_hotkeys WHERE action = 'quick_load'",
        )
        .fetch_one(&dst.db)
        .await
        .unwrap();
        assert!(load.contains(OTHER_GUID));
    }

    #[tokio::test]
    async fn merge_keeps_the_keyboard_hotkey_when_the_other_machine_uses_a_controller() {
        let src = Machine::new("hkm-src").await;
        // lá o menu fica no controle: o documento não fala dele
        let menu_pad = vec![pad(GUID, 6), pad(GUID, 7)];
        hotkey(&src.db, SystemAction::ToggleMenuOverlay, menu_pad).await;
        hotkey(&src.db, SystemAction::QuickSave, vec![key("F5")]).await;
        hotkey(&src.db, SystemAction::QuickLoad, vec![key("F9")]).await;
        key_binding(&src.db, "R2", "KeyR").await;
        let doc = src.export(HK).await;

        let here = Machine::new("hkm-here").await;
        hotkey(&here.db, SystemAction::ToggleMenuOverlay, vec![key("F1")]).await;
        hotkey(&here.db, SystemAction::QuickSave, vec![key("F2")]).await;
        // aqui o load fica no controle desta máquina
        hotkey(&here.db, SystemAction::QuickLoad, vec![pad(OTHER_GUID, 3)]).await;
        key_binding(&here.db, "A", "KeyZ").await;

        let report = here.apply(HK, &doc, Merge).await.unwrap();
        assert_eq!(report.applied, 2, "quick_save e R2");
        assert_eq!(report.kept_local, 1, "o quick_load do controle daqui");
        let trigger = |t: Vec<RawInputEvent>| serde_json::to_string(&t).unwrap();
        let hotkeys: Vec<(String, String)> = rows(
            &here.db,
            "SELECT action, trigger_json FROM system_hotkeys ORDER BY action",
        )
        .await;
        assert_eq!(
            hotkeys,
            vec![
                ("quick_load".into(), trigger(vec![pad(OTHER_GUID, 3)])),
                ("quick_save".into(), trigger(vec![key("F5")])),
                ("toggle_menu_overlay".into(), trigger(vec![key("F1")])),
            ]
        );
        let keys: Vec<(String, String)> = rows(
            &here.db,
            "SELECT target, code FROM keyboard_bindings ORDER BY target",
        )
        .await;
        assert_eq!(
            keys,
            vec![
                ("A".to_string(), "KeyZ".to_string()),
                ("R2".to_string(), "KeyR".to_string())
            ]
        );
    }

    // --- áudio e vídeo -----------------------------------------------------

    #[tokio::test]
    async fn export_then_apply_round_trips_audio() {
        let src = Machine::new("audio-src").await;
        set_audio(
            &src.db,
            AudioConfig {
                output_device_id: Some("{0.0.0.00000000}.{8f1c}".into()),
                output_device_name: Some("Alto-falantes (Realtek)".into()),
                rate_control_enabled: false,
                rate_control_delta: 0.0078125,
                sample_rate_preference: Some(48000),
            },
        )
        .await;
        let dst = Machine::new("audio-dst").await;
        let dst_audio = AudioConfig {
            output_device_id: Some("fones-usb".into()),
            output_device_name: Some("Fones USB".into()),
            rate_control_enabled: true,
            rate_control_delta: 0.005,
            sample_rate_preference: None,
        };
        set_audio(&dst.db, dst_audio.clone()).await;

        let doc = src.export(Group::Audio).await;
        assert_eq!(
            doc,
            json!({
                "v": 1,
                "rate_control_enabled": false,
                "rate_control_delta": 0.0078125,
                "sample_rate_preference": 48000,
            })
        );
        let report = dst.apply(Group::Audio, &doc, Restore).await.unwrap();
        assert_eq!(report.applied, 1);
        assert_eq!(dst.export(Group::Audio).await, doc);
        // o aparelho de saída é desta máquina
        let now = db::AudioConfigRepo::new(dst.db.clone())
            .get()
            .await
            .unwrap();
        assert_eq!(now.output_device_id, dst_audio.output_device_id);
        assert_eq!(now.output_device_name, dst_audio.output_device_name);

        // O documento chega como texto (nuvem, backup do "Desfazer") e o
        // delta que a tela grava vem de um f32: o REAL volta com os mesmos bits.
        set_audio(
            &src.db,
            AudioConfig {
                rate_control_delta: 0.0037,
                ..dst_audio
            },
        )
        .await;
        let text = src.export(Group::Audio).await.to_string();
        let doc: Value = serde_json::from_str(&text).unwrap();
        dst.apply(Group::Audio, &doc, Merge).await.unwrap();
        let delta = "SELECT rate_control_delta FROM audio_config";
        let here: f64 = sqlx::query_scalar(delta).fetch_one(&src.db).await.unwrap();
        let there: f64 = sqlx::query_scalar(delta).fetch_one(&dst.db).await.unwrap();
        assert_eq!(there.to_bits(), here.to_bits(), "{here} virou {there}");
    }

    #[tokio::test]
    async fn export_then_apply_round_trips_video() {
        let src = Machine::new("video-src").await;
        rom(&src.db, "src-1", "AAAA0001").await;
        set_video(
            &src.db,
            VideoConfig {
                integer_scaling: true,
                hdr: true,
                hdr_paper_white_nits: 203,
                hdr_highlights: false,
                hdr_peak_nits: 1000,
            },
        )
        .await;
        aspect(&src.db, "default", "", "4:3").await;
        aspect(&src.db, "system", "snes", "16:9").await;
        aspect(&src.db, "rom", "src-1", "fill").await;

        let dst = Machine::new("video-dst").await;
        rom(&dst.db, "dst-1", "AAAA0001").await;
        set_video(
            &dst.db,
            VideoConfig {
                integer_scaling: false,
                hdr: false,
                hdr_paper_white_nits: 80,
                hdr_highlights: true,
                hdr_peak_nits: 600,
            },
        )
        .await;
        aspect(&dst.db, "default", "", "core").await;
        aspect(&dst.db, "system", "gba", "16:9").await;
        // proporção de um jogo que já saiu da biblioteca: não é da cópia, fica
        aspect(&dst.db, "rom", "sumiu", "fill").await;

        let doc = src.export(Group::Video).await;
        assert_eq!(
            doc,
            json!({
                "v": 1,
                "integer_scaling": true,
                "hdr": true,
                "hdr_highlights": false,
                "aspect": [
                    {"scope": "default", "target": "", "aspect": "4:3"},
                    {"scope": "rom", "target": "AAAA0001", "aspect": "fill"},
                    {"scope": "system", "target": "snes", "aspect": "16:9"},
                ],
            })
        );
        let report = dst.apply(Group::Video, &doc, Restore).await.unwrap();
        assert_eq!(report.applied, 4);
        assert_eq!(dst.export(Group::Video).await, doc);
        // os nits foram medidos na tela desta máquina
        let now = db::VideoConfigRepo::new(dst.db.clone())
            .get()
            .await
            .unwrap();
        assert_eq!((now.hdr_paper_white_nits, now.hdr_peak_nits), (80, 600));
        assert!(dump(&dst.db, "aspect_assignments")
            .await
            .contains(&"'rom'|'sumiu'|'fill'".to_string()));
    }

    // --- o que nunca viaja -------------------------------------------------

    /// Toda chave e todo texto do documento; os números à parte.
    fn walk(v: &Value, strings: &mut Vec<String>, numbers: &mut Vec<f64>) {
        match v {
            Value::String(s) => strings.push(s.clone()),
            Value::Number(n) => numbers.extend(n.as_f64()),
            Value::Array(a) => a.iter().for_each(|x| walk(x, strings, numbers)),
            Value::Object(o) => {
                for (k, x) in o {
                    strings.push(k.clone());
                    walk(x, strings, numbers);
                }
            }
            Value::Bool(_) | Value::Null => {}
        }
    }

    fn looks_absolute(s: &str) -> bool {
        let b = s.as_bytes();
        Path::new(s).is_absolute()
            || s.starts_with('/')
            || s.starts_with('\\')
            || (b.len() >= 3 && b[1] == b':' && matches!(b[2], b'\\' | b'/'))
            || s.contains(":\\")
            || s.contains(":/")
            || s.contains(&*std::env::temp_dir().to_string_lossy())
    }

    #[tokio::test]
    async fn library_folders_and_controllers_are_never_exported() {
        use AssignmentScope::*;
        let m = Machine::new("never").await;
        let db = &m.db;
        let folders = db::LibraryFoldersRepo::new(db.clone());
        let library = abs(&["Biblioteca", "SNES"]);
        for f in [
            library.as_str(),
            "D:\\Biblioteca\\PS2",
            "/home/kluis/Biblioteca",
        ] {
            folders.add(f, 0).await.unwrap();
        }
        db::ControllerMappingsRepo::new(db.clone())
            .upsert(&ControllerMapping {
                guid: GUID.into(),
                display_name: "Controle do Ed".into(),
                layout: vec![ControllerLayoutEntry {
                    trigger: vec![pad(GUID, 0)],
                    button: RetroPadButton::A,
                }],
                source: MappingSource::UserOverride,
            })
            .await
            .unwrap();
        db::DevicePortsRepo::new(db.clone())
            .set(GUID, 1)
            .await
            .unwrap();
        rom(db, "r1", "AAAA0001").await;
        option(db, "snes9x", Some("r1"), "snes9x_region", "pal").await;
        option(db, "ppsspp_libretro", None, &format!("{MAC}01"), "12").await;
        set_audio(
            db,
            AudioConfig {
                output_device_id: Some("{0.0.0.00000000}.{8f1c-ed}".into()),
                output_device_name: Some("Alto-falantes do Ed".into()),
                rate_control_enabled: true,
                rate_control_delta: 0.005,
                sample_rate_preference: Some(44100),
            },
        )
        .await;
        set_video(
            db,
            VideoConfig {
                integer_scaling: false,
                hdr: true,
                hdr_paper_white_nits: 437,
                hdr_highlights: true,
                hdr_peak_nits: 1234,
            },
        )
        .await;
        aspect(db, "rom", "r1", "4:3").await;
        hotkey(db, SystemAction::QuickSave, vec![key("F5")]).await;
        hotkey(db, SystemAction::QuickLoad, vec![pad(GUID, 3), key("F9")]).await;
        // uma hotkey presa a um aparelho pela coluna, mesmo sendo de teclado
        run(
            db,
            "INSERT INTO system_hotkeys (id, action, trigger_json, device_guid) \
             VALUES ('toggle_menu_overlay', 'toggle_menu_overlay', ?1, ?2)",
            &[&serde_json::to_string(&[key("F1")]).unwrap(), GUID],
        )
        .await;
        let mine = abs(&["Meus Shaders", "crt-do-ed.slangp"]);
        preset(db, &mine, "crt-do-ed").await;
        shader(db, System, Some("gba"), &mine, &[]).await;
        let royale = m.slang("crt/crt-royale.slangp");
        preset(db, &royale, "crt-royale").await;
        shader(db, System, Some("psx"), &royale, &[]).await;
        preset(db, "crt", "crt").await;
        shader(db, Rom, Some("r1"), "crt", &[]).await;
        let user_base = abs(&["Molduras do Ed"]);
        let user_pack = format!("pack:{user_base}");
        pack(db, &user_pack, "Molduras do Ed", false, &user_base).await;
        deco(
            db,
            "default",
            "",
            &user_pack,
            &abs(&["Molduras do Ed", "a.png"]),
        )
        .await;
        let pid = m.bezel_pack().await;
        deco(db, "system", "snes", &pid, &m.bezel("snes/moldura.png")).await;

        let secrets = [
            GUID,
            "Controle do Ed",
            "Biblioteca",
            "{0.0.0.00000000}.{8f1c-ed}",
            "Alto-falantes do Ed",
            "Jogos",
        ];
        let forbidden_keys = [
            "library_folders",
            "controller_mappings",
            "device_port_assignment",
            "port_index",
            "guid",
            "output_device",
            "hdr_paper_white_nits",
            "hdr_peak_nits",
            "base_path",
            "file_path",
            MAC,
        ];
        for g in Group::ALL {
            let doc = m.export(g).await;
            let (mut strings, mut numbers) = (Vec::new(), Vec::new());
            walk(&doc, &mut strings, &mut numbers);
            for s in &strings {
                assert!(!looks_absolute(s), "{g:?}: caminho absoluto: {s}");
                for secret in secrets {
                    assert!(!s.contains(secret), "{g:?}: {secret} em {s}");
                }
                for k in forbidden_keys {
                    assert!(!s.contains(k), "{g:?}: {k} em {s}");
                }
            }
            for n in [437.0, 1234.0] {
                assert!(!numbers.contains(&n), "{g:?}: nits {n} desta tela");
            }
        }

        // e o que é portátil foi junto: o teste não passa por documento vazio
        let shaders = m.export(SD).await;
        assert_eq!(shaders["decorations"][0]["asset"], "snes/moldura.png");
        assert_eq!(
            shaders["assignments"][0]["preset"],
            "slang-shaders:crt/crt-royale.slangp"
        );
        assert_eq!(shaders["assignments"][1]["crc32"], "AAAA0001");
        let hotkeys = m.export(HK).await;
        assert_eq!(hotkeys["hotkeys"].as_array().unwrap().len(), 1);
        assert_eq!(hotkeys["hotkeys"][0]["action"], "quick_save");
        let video = m.export(Group::Video).await;
        assert_eq!(video["hdr"], true);
        assert_eq!(video["aspect"][0]["target"], "AAAA0001");
        let audio = m.export(Group::Audio).await;
        assert_eq!(audio["sample_rate_preference"], 44100);
        let options = m.export(CO).await;
        assert_eq!(options["overrides"][0]["crc32"], "AAAA0001");
    }

    // --- versão e transação ------------------------------------------------

    #[tokio::test]
    async fn apply_refuses_an_unknown_version() {
        let src = machine("ver-src", "a").await;
        let dst = machine("ver-dst", "b").await;
        let before = snapshot(&dst.db).await;
        for g in Group::ALL {
            let mut doc = src.export(g).await;
            doc["v"] = json!(2);
            for mode in [Merge, Restore] {
                let refused = dst.apply(g, &doc, mode).await;
                assert_eq!(refused, Err(ERR_VERSION.to_string()), "{g:?} {mode:?}");
            }
            doc.as_object_mut().unwrap().remove("v");
            let refused = dst.apply(g, &doc, Merge).await;
            assert_eq!(refused, Err(ERR_VERSION.to_string()), "{g:?} sem v");
        }
        assert_eq!(snapshot(&dst.db).await, before);
    }

    /// Um backup com a mesma chave duas vezes não é desta máquina: o banco
    /// recusa no meio do grupo, depois de outras escritas, e nada fica.
    #[tokio::test]
    async fn apply_is_all_or_nothing_per_group() {
        let src = machine("aon-src", "a").await;
        let dst = machine("aon-dst", "b").await;
        let before = snapshot(&dst.db).await;

        // opções: os valores já foram trocados quando a opção do jogo repete
        let mut doc = src.export(CO).await;
        let again = doc["overrides"][0].clone();
        doc["overrides"].as_array_mut().unwrap().push(again);
        assert!(dst.apply(CO, &doc, Restore).await.is_err());

        // teclado: as hotkeys já foram trocadas quando o mesmo alvo repete
        let mut doc = src.export(HK).await;
        let again = json!({"target": "R2", "code": "KeyQ"});
        doc["keyboard"].as_array_mut().unwrap().push(again);
        assert!(dst.apply(HK, &doc, Restore).await.is_err());

        // vídeo: a linha de vídeo já foi gravada quando a proporção repete
        let mut doc = src.export(Group::Video).await;
        let again = doc["aspect"][0].clone();
        doc["aspect"].as_array_mut().unwrap().push(again);
        assert!(dst.apply(Group::Video, &doc, Restore).await.is_err());

        assert_eq!(snapshot(&dst.db).await, before);
    }

    #[tokio::test]
    async fn apply_all_checks_every_document_before_writing() {
        let src = machine("check-src", "a").await;
        let dst = machine("check-dst", "b").await;
        let before = snapshot(&dst.db).await;
        let options = src.export(CO).await;
        let video = src.export(Group::Video).await;

        // a versão do último não serve: nem os dois primeiros entram
        let mut newer = src.export(HK).await;
        newer["v"] = json!(2);
        let docs = [(CO, &options), (Group::Video, &video), (HK, &newer)];
        for mode in [Merge, Restore] {
            let refused = dst.apply_all(&docs, mode).await;
            assert_eq!(refused.unwrap_err(), ERR_VERSION, "{mode:?}");
        }

        // o formato do último não serve (moldura fora da pasta do catálogo)
        let mut shaders = src.export(SD).await;
        shaders["decorations"] =
            json!([{"scope": "default", "pack": "bezelproject", "asset": "../fora.png"}]);
        let refused = dst
            .apply_all(&[(CO, &options), (SD, &shaders)], Merge)
            .await;
        assert_eq!(refused.unwrap_err(), ERR_INVALID);

        // o mesmo grupo duas vezes
        let refused = dst
            .apply_all(&[(CO, &options), (CO, &options)], Merge)
            .await;
        assert_eq!(refused.unwrap_err(), ERR_INVALID);

        assert_eq!(snapshot(&dst.db).await, before);
    }

    /// D114: os grupos marcados entram numa transação só. O terceiro falha no
    /// banco, depois de os dois primeiros terem sido gravados: nada fica.
    #[tokio::test]
    async fn apply_all_is_one_transaction() {
        let src = machine("one-src", "a").await;
        let dst = machine("one-dst", "b").await;
        let before = snapshot(&dst.db).await;
        let options = src.export(CO).await;
        let video = src.export(Group::Video).await;
        let mut keys = src.export(HK).await;
        let again = keys["keyboard"][0].clone();
        keys["keyboard"].as_array_mut().unwrap().push(again);

        let docs = [(CO, &options), (Group::Video, &video), (HK, &keys)];
        assert!(dst.apply_all(&docs, Restore).await.is_err());
        assert_eq!(snapshot(&dst.db).await, before);

        // sem o defeito, os três entram juntos
        let keys = src.export(HK).await;
        let docs = [(CO, &options), (Group::Video, &video), (HK, &keys)];
        let reports = dst.apply_all(&docs, Merge).await.unwrap();
        let groups: Vec<Group> = reports.iter().map(|(g, _)| *g).collect();
        assert_eq!(groups, vec![CO, Group::Video, HK]);
        assert_eq!(dst.export(Group::Video).await, video);
    }

    /// O "Desfazer" aplica em `Restore` o documento desta máquina guardado
    /// antes da cópia (`Merge`): como nada só desta máquina é tocado, ela volta
    /// igual.
    #[tokio::test]
    async fn applying_the_own_export_after_a_copy_restores_every_group() {
        use AssignmentScope::*;
        let here = machine("undo-here", "b").await;
        // só desta máquina: preset importado no GBA, molduras importadas,
        // hotkey de controle, escolhas de um jogo fora da biblioteca e o MAC
        let mine = abs(&["Meus Shaders", "crt-do-ed.slangp"]);
        preset(&here.db, &mine, "crt-do-ed").await;
        shader(&here.db, System, Some("gba"), &mine, &[("GAMMA", "2.4")]).await;
        let user_base = abs(&["Molduras do Ed"]);
        let user_pack = format!("pack:{user_base}");
        pack(&here.db, &user_pack, "Molduras do Ed", false, &user_base).await;
        let png = abs(&["Molduras do Ed", "padrao.png"]);
        deco(&here.db, "default", "", &user_pack, &png).await;
        hotkey(&here.db, SystemAction::QuickLoad, vec![pad(GUID, 2)]).await;
        option(&here.db, "snes9x", Some("sumiu"), "snes9x_region", "pal").await;
        aspect(&here.db, "rom", "sumiu", "fill").await;
        option(&here.db, "ppsspp_libretro", None, &format!("{MAC}01"), "12").await;
        // os catálogos baixados aqui também
        here.bezel_pack().await;
        touch(&here.slang("crt/crt-royale.slangp"));
        touch(&here.bezel("nes/nes.png"));

        // a outra máquina: shader do catálogo onde esta tem o importado, um
        // preset e uma moldura dos pacotes baixados, e o load no teclado
        let there = machine("undo-there", "a").await;
        shader(&there.db, System, Some("gba"), "lcd", &[]).await;
        let royale = there.slang("crt/crt-royale.slangp");
        preset(&there.db, &royale, "crt-royale").await;
        shader(&there.db, System, Some("nes"), &royale, &[]).await;
        let pid = there.bezel_pack().await;
        deco(
            &there.db,
            "system",
            "nes",
            &pid,
            &there.bezel("nes/nes.png"),
        )
        .await;
        hotkey(&there.db, SystemAction::QuickLoad, vec![key("F9")]).await;
        option(&there.db, "genesis_plus_gx", None, "gpgx_region", "auto").await;

        let before = snapshot(&here.db).await;
        let mut backup = Vec::new();
        for g in Group::ALL {
            backup.push((g, here.export(g).await));
        }
        let mut copy = Vec::new();
        for g in Group::ALL {
            copy.push((g, there.export(g).await));
        }
        let docs: Vec<(Group, &Value)> = copy.iter().map(|(g, d)| (*g, d)).collect();
        let reports = here.apply_all(&docs, Merge).await.unwrap();
        let kept: u32 = reports.iter().map(|(_, r)| r.kept_local).sum();
        assert_eq!(kept, 2, "o importado no GBA e o load do controle");
        let copied = snapshot(&here.db).await;
        assert_ne!(copied, before);
        // o que é só desta máquina sobreviveu à cópia
        for kept in [mine.as_str(), GUID, "sumiu", MAC] {
            assert!(
                copied.iter().any(|r| r.contains(kept)),
                "{kept} sumiu na cópia"
            );
        }

        let docs: Vec<(Group, &Value)> = backup.iter().map(|(g, d)| (*g, d)).collect();
        here.apply_all(&docs, Restore).await.unwrap();
        assert_eq!(snapshot(&here.db).await, before);
    }
}
