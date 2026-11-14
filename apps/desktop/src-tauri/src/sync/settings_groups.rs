//! Configurações copiadas entre máquinas (D114): cada grupo vira um documento
//! JSON portátil (`v: 1`) e volta ao banco numa transação só.
//!
//! Um documento nunca leva o que é desta máquina: pastas da biblioteca,
//! mapeamento de controle e portas, GUID de controle (a hotkey presa a um
//! botão de controle fica aqui), o aparelho de áudio, os nits de HDR medidos
//! nesta tela e qualquer caminho absoluto. Um jogo vai pelo CRC32 da ROM; um
//! preset do ReEmu (embutido ou `curated:<id>`) pelo `source_path`; um pacote
//! de molduras do catálogo pelo nome, com a imagem relativa à pasta dele. Um
//! preset ou pacote importado do disco vai só pelo nome, em `missing`, e a
//! atribuição dele fica (D126).
//!
//! Aplicar troca o que o grupo tem de portátil pelo que o documento traz e
//! não toca no que é só desta máquina (atribuição a um preset importado,
//! hotkey de botão de controle, escolha de um jogo fora da biblioteca):
//! quando o documento quer o mesmo lugar, o desta máquina fica. Por isso
//! aplicar o documento desta máquina, guardado antes de uma cópia, a devolve
//! igual. Aplicar só grava o banco: quem chama recarrega o que o app guarda em
//! memória (hotkeys, teclado, áudio, vídeo, shader do jogo aberto).

use db::{
    AspectRow, AudioSettings, Db, OptionOverrideRow, SettingsRepo, SettingsTx, VideoSettings,
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

/// O que uma aplicação fez.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Itens do documento gravados: um valor de opção, uma atribuição, uma
    /// hotkey, uma tecla, uma proporção; o áudio e a linha de vídeo contam um.
    pub applied: u32,
    /// Jogos (CRC32 distintos) do documento que esta máquina não tem.
    pub skipped_unknown_games: u32,
    /// Presets e pacotes de moldura que não vieram, por nome, em ordem: os
    /// importados do disco na outra máquina (D126) e os do catálogo que esta
    /// não tem.
    pub missing_files: Vec<String>,
}

// --- documentos ------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct CoreOptionsDoc {
    v: u64,
    /// `{core_id: {chave: valor}}`.
    values: BTreeMap<String, BTreeMap<String, String>>,
    /// Opções por jogo, o jogo pelo CRC32.
    overrides: Vec<OverrideEntry>,
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
    /// Presets e pacotes importados do disco nesta máquina, por nome.
    missing: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShaderEntry {
    /// `default`, `system` (com `system_id`) ou `rom` (com `crc32`).
    scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    system_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    crc32: Option<String>,
    /// `source_path` do preset: um nome embutido ou `curated:<id>`.
    preset: String,
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
    /// Nome do pacote do catálogo.
    pack: String,
    /// A imagem relativa à pasta do pacote, separada por `/`.
    asset: String,
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
}

// --- entrada ---------------------------------------------------------------

/// O documento do grupo, com o que esta máquina tem agora.
pub async fn export(group: Group, db: &Db) -> Result<Value, String> {
    let mut tx = SettingsRepo::new(db.clone()).read().await.map_err(repo)?;
    let doc = match group {
        Group::CoreOptions => serde_json::to_value(export_core_options(&mut tx).await?),
        Group::ShadersDecorations => serde_json::to_value(export_shaders(&mut tx).await?),
        Group::HotkeysKeyboard => serde_json::to_value(export_hotkeys(&mut tx).await?),
        Group::Audio => serde_json::to_value(export_audio(&mut tx).await?),
        Group::Video => serde_json::to_value(export_video(&mut tx).await?),
    }
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(repo)?;
    Ok(doc)
}

/// Aplica o documento do grupo numa transação: dá certo inteiro ou não muda
/// nada. `local_roms` são os `(rom_id, crc32)` desta biblioteca. Um documento
/// de outra versão (`v`) é recusado com [`ERR_VERSION`] sem tocar no banco.
pub async fn apply(
    group: Group,
    db: &Db,
    doc: &Value,
    local_roms: &[(String, String)],
) -> Result<ApplyReport, String> {
    if !doc.is_object() {
        return Err(invalid());
    }
    if doc.get("v").and_then(Value::as_u64) != Some(DOC_VERSION) {
        return Err(ERR_VERSION.to_string());
    }
    let parsed = Doc::parse(group, doc)?;
    let roms = LocalRoms::new(local_roms);
    let mut tally = Tally::default();
    let mut tx = SettingsRepo::new(db.clone()).write().await.map_err(repo)?;
    match parsed {
        Doc::CoreOptions(d) => apply_core_options(&mut tx, d, &roms, &mut tally).await?,
        Doc::Shaders(d) => apply_shaders(&mut tx, d, &roms, &mut tally).await?,
        Doc::Hotkeys(d) => apply_hotkeys(&mut tx, d, &mut tally).await?,
        Doc::Audio(d) => apply_audio(&mut tx, d, &mut tally).await?,
        Doc::Video(d) => apply_video(&mut tx, d, &roms, &mut tally).await?,
    }
    tx.commit().await.map_err(repo)?;
    let report = tally.into_report();
    // só contagens: nome de jogo, de preset ou de pasta não vai para o log
    log::info!(
        "sync: configurações {} aplicadas: {} itens, {} jogos que esta máquina não tem, {} \
         arquivos que não vieram",
        group.as_str(),
        report.applied,
        report.skipped_unknown_games,
        report.missing_files.len()
    );
    Ok(report)
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

    /// Uma atribuição desta máquina que um documento substitui: a de todos,
    /// a de uma plataforma ou a de um jogo desta biblioteca.
    fn in_library(&self, scope: AssignmentScope, rom_id: Option<&str>) -> bool {
        scope != AssignmentScope::Rom || rom_id.is_some_and(|id| self.has(id))
    }
}

#[derive(Default)]
struct Tally {
    applied: u32,
    unknown_games: BTreeSet<String>,
    missing: BTreeSet<String>,
}

impl Tally {
    fn into_report(self) -> ApplyReport {
        ApplyReport {
            applied: self.applied,
            skipped_unknown_games: u32::try_from(self.unknown_games.len()).unwrap_or(u32::MAX),
            missing_files: self.missing.into_iter().collect(),
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
    /// uma ROM, ou em nenhuma (contado como jogo que esta máquina não tem).
    fn slots(&self, roms: &LocalRoms<'_>, tally: &mut Tally) -> Vec<Slot> {
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
            Target::Game(crc) => {
                let ids = roms.of(crc);
                if ids.is_empty() {
                    tally.unknown_games.insert(crc.clone());
                }
                ids.iter()
                    .map(|id| Slot {
                        scope: AssignmentScope::Rom,
                        system_id: None,
                        rom_id: Some(id.to_string()),
                    })
                    .collect()
            }
        }
    }
}

// --- opções de core ----------------------------------------------------------

async fn export_core_options(tx: &mut SettingsTx) -> Result<CoreOptionsDoc, String> {
    let crc_of = crc_by_rom(tx).await?;
    let mut values: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (core, key, value) in tx.option_values().await.map_err(repo)? {
        values.entry(core).or_default().insert(key, value);
    }
    // o mesmo jogo em duas ROMs: vale a primeira, na ordem do id
    let mut overrides = BTreeMap::new();
    for o in tx.option_overrides().await.map_err(repo)? {
        if let Some(crc) = crc_of.get(&o.rom_id) {
            overrides
                .entry((crc.clone(), o.core_id, o.key))
                .or_insert(o.value);
        }
    }
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
    })
}

async fn apply_core_options(
    tx: &mut SettingsTx,
    doc: CoreOptionsDoc,
    roms: &LocalRoms<'_>,
    tally: &mut Tally,
) -> Result<(), String> {
    tx.clear_option_values().await.map_err(repo)?;
    for (core, options) in &doc.values {
        for (key, value) in options {
            tx.insert_option_value(core, key, value)
                .await
                .map_err(repo)?;
            tally.applied += 1;
        }
    }
    // as opções dos jogos desta biblioteca saem; as de uma ROM que já saiu
    // dela não são de cópia nenhuma e ficam
    let mut cleared = HashSet::new();
    for o in tx.option_overrides().await.map_err(repo)? {
        if roms.has(&o.rom_id) && cleared.insert(o.rom_id.clone()) {
            tx.delete_option_overrides(&o.rom_id).await.map_err(repo)?;
        }
    }
    for e in doc.overrides {
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
            tx.insert_option_override(&row).await.map_err(repo)?;
        }
        tally.applied += 1;
    }
    Ok(())
}

// --- shaders e molduras ------------------------------------------------------

/// Preset que o ReEmu conhece pelo nome: embutido ou `curated:<id>`. Um
/// `source_path` que é caminho de `.slangp` foi importado do disco.
fn is_catalog_preset(source_path: &str, builtins: &[String]) -> bool {
    builtins.iter().any(|b| b == source_path) || source_path.starts_with("curated:")
}

/// A linha de `shader_presets` de um preset do catálogo que ESTA versão
/// conhece, como a tela de shaders gravaria; `None` para um que ela não tem.
fn catalog_preset(source: &str, builtins: &[String]) -> Option<ShaderPreset> {
    let name = if builtins.iter().any(|b| b == source) {
        source.to_string()
    } else {
        crate::gpu::curated_by_wire(source)?.label.to_string()
    };
    Some(ShaderPreset {
        id: source.to_string(),
        name,
        source_path: source.to_string(),
        format: ShaderFormat::Slang,
        is_builtin: true,
        // a mesma regra de `set_shader`
        includes_bezel: source.to_lowercase().contains("bezel"),
    })
}

/// A imagem relativa à pasta do pacote, separada por `/`; `None` quando ela
/// não está dentro da pasta.
fn relative_asset(base: &str, asset: &str) -> Option<String> {
    let rel = Path::new(asset).strip_prefix(base).ok()?;
    let parts = rel
        .components()
        .map(|c| match c {
            Component::Normal(p) => p.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Os pedaços de uma imagem relativa do documento. Recusa o que sairia da
/// pasta do pacote ou seria um caminho absoluto.
fn asset_parts(rel: &str) -> Result<Vec<&str>, String> {
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

async fn export_shaders(tx: &mut SettingsTx) -> Result<ShadersDoc, String> {
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
        if !is_catalog_preset(&preset.source_path, &builtins) {
            missing.insert(bare_name(&preset.name));
            continue;
        }
        let (scope, system_id, crc32) = target.parts();
        assignments.entry(target).or_insert(ShaderEntry {
            scope,
            system_id,
            crc32,
            preset: preset.source_path.clone(),
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
        let asset = match pack.source {
            DecorationPackSource::Bundled => relative_asset(&pack.base_path, &d.asset_path),
            DecorationPackSource::UserImported => None,
        };
        let Some(asset) = asset else {
            missing.insert(bare_name(&pack.name));
            continue;
        };
        let (scope, system_id, crc32) = target.parts();
        decorations.entry(target).or_insert(DecorationEntry {
            scope,
            system_id,
            crc32,
            pack: pack.name.clone(),
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
    roms: &LocalRoms<'_>,
    tally: &mut Tally,
) -> Result<(), String> {
    let builtins = crate::gpu::builtin_preset_names();
    for name in &doc.missing {
        tally.missing.insert(bare_name(name));
    }

    // As atribuições do catálogo saem. A de um preset importado do disco ou
    // de um jogo fora da biblioteca é desta máquina: fica, e o lugar dela não
    // recebe a do documento.
    let presets: HashMap<String, ShaderPreset> = tx
        .shader_presets()
        .await
        .map_err(repo)?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();
    let mut kept = HashSet::new();
    for a in tx.shader_assignments().await.map_err(repo)? {
        let catalog = presets
            .get(&a.preset_id)
            .is_some_and(|p| is_catalog_preset(&p.source_path, &builtins));
        if catalog && roms.in_library(a.scope, a.rom_id.as_deref()) {
            tx.delete_shader_assignment(&a.id).await.map_err(repo)?;
        } else {
            kept.insert(slot_key(
                a.scope,
                a.system_id.as_deref(),
                a.rom_id.as_deref(),
            ));
        }
    }
    for e in doc.assignments {
        let target = Target::parse(&e.scope, e.system_id.as_deref(), e.crc32.as_deref())?;
        let Some(preset) = catalog_preset(&e.preset, &builtins) else {
            tally.missing.insert(bare_name(&e.preset));
            continue;
        };
        let params: Vec<(String, String)> = e.params.into_iter().collect();
        let mut wrote = false;
        for slot in target.slots(roms, tally) {
            if kept.contains(&slot.key()) {
                continue;
            }
            tx.ensure_shader_preset(&preset).await.map_err(repo)?;
            tx.insert_shader_assignment(
                slot.scope,
                slot.system_id.as_deref(),
                slot.rom_id.as_deref(),
                &preset.id,
                &params,
            )
            .await
            .map_err(repo)?;
            wrote = true;
        }
        tally.applied += u32::from(wrote);
    }

    // Molduras: as de um pacote do catálogo saem; as de um pacote importado
    // ficam, como os presets importados.
    let packs = tx.decoration_packs().await.map_err(repo)?;
    let pack_by_id: HashMap<&str, &DecorationPack> =
        packs.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut bundled_by_name: HashMap<&str, &DecorationPack> = HashMap::new();
    for p in packs
        .iter()
        .filter(|p| p.source == DecorationPackSource::Bundled)
    {
        bundled_by_name.entry(p.name.as_str()).or_insert(p);
    }
    let mut kept = HashSet::new();
    for d in tx.decoration_assignments().await.map_err(repo)? {
        let catalog = pack_by_id.get(d.pack_id.as_str()).is_some_and(|p| {
            p.source == DecorationPackSource::Bundled
                && relative_asset(&p.base_path, &d.asset_path).is_some()
        });
        if catalog && roms.in_library(d.scope, d.rom_id.as_deref()) {
            tx.delete_decoration_assignment(&d.id).await.map_err(repo)?;
        } else {
            kept.insert(slot_key(
                d.scope,
                d.system_id.as_deref(),
                d.rom_id.as_deref(),
            ));
        }
    }
    for e in doc.decorations {
        let target = Target::parse(&e.scope, e.system_id.as_deref(), e.crc32.as_deref())?;
        let parts = asset_parts(&e.asset)?;
        // pacote que esta máquina não baixou: a moldura não aponta para nada
        let Some(pack) = bundled_by_name.get(e.pack.as_str()) else {
            tally.missing.insert(bare_name(&e.pack));
            continue;
        };
        let mut path = PathBuf::from(&pack.base_path);
        path.extend(parts);
        let asset_path = path.to_string_lossy().into_owned();
        let mut wrote = false;
        for slot in target.slots(roms, tally) {
            if kept.contains(&slot.key()) {
                continue;
            }
            let a = DecorationAssignment {
                scope: slot.scope,
                system_id: slot.system_id,
                rom_id: slot.rom_id,
                pack_id: pack.id.clone(),
                asset_path: asset_path.clone(),
            };
            tx.insert_decoration_assignment(&a).await.map_err(repo)?;
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
    tally: &mut Tally,
) -> Result<(), String> {
    // as de teclado saem; a de controle é desta máquina e fica com a ação dela
    let mut kept = HashSet::new();
    for h in tx.hotkeys().await.map_err(repo)? {
        if portable_trigger(&h).is_some() {
            tx.delete_hotkey(&h.id).await.map_err(repo)?;
        } else {
            kept.insert(h.action);
        }
    }
    for e in doc.hotkeys {
        if e.trigger.is_empty() || !e.trigger.iter().all(is_key) {
            return Err(invalid());
        }
        // ação de uma versão mais nova: esta não sabe o que fazer com ela
        if SystemAction::from_wire(&e.action).is_none() || kept.contains(&e.action) {
            continue;
        }
        let json = serde_json::to_string(&e.trigger).map_err(|_| invalid())?;
        tx.insert_hotkey(&e.action, &json).await.map_err(repo)?;
        tally.applied += 1;
    }
    tx.clear_keyboard_bindings().await.map_err(repo)?;
    for k in doc.keyboard {
        if k.target.is_empty() {
            return Err(invalid());
        }
        tx.insert_keyboard_binding(&k.target, &k.code)
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
    for e in doc.aspect {
        let target = match e.scope.as_str() {
            "rom" => Target::parse("rom", None, Some(&e.target))?,
            scope => Target::parse(scope, Some(&e.target), None)?,
        };
        // proporção de uma versão mais nova: esta não sabe desenhar
        if !ASPECTS.contains(&e.aspect.as_str()) {
            continue;
        }
        let mut wrote = false;
        for slot in target.slots(roms, tally) {
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
            tx.insert_aspect(&row).await.map_err(repo)?;
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

    /// GUID de um controle (formato SDL), o desta máquina e o de outra.
    const GUID: &str = "030000005e0400008e02000000007200";
    const OTHER_GUID: &str = "03000000de280000ff11000001000000";

    async fn mem_db() -> Db {
        db::connect_in_memory().await.unwrap()
    }

    async fn run(db: &Db, sql: &str, args: &[&str]) {
        let mut q = sqlx::query(sql);
        for a in args {
            q = q.bind(*a);
        }
        q.execute(db).await.unwrap();
    }

    /// Um caminho absoluto de verdade nesta máquina (Windows ou Linux).
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

    async fn local_roms(db: &Db) -> Vec<(String, String)> {
        sqlx::query_as("SELECT id, crc32 FROM roms ORDER BY id")
            .fetch_all(db)
            .await
            .unwrap()
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
    /// apaga uma linha dele), não uma escolha.
    const TABLES: &[&str] = &[
        "core_options_values",
        "core_option_overrides",
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

    /// Uma máquina com alguma coisa em cada grupo; `n` muda os valores.
    async fn machine(n: &str) -> Db {
        let db = mem_db().await;
        let a = n == "a";
        rom(&db, &format!("{n}-1"), "AAAA0001").await;
        rom(&db, &format!("{n}-2"), "AAAA0002").await;
        option(&db, "snes9x", None, "snes9x_region", n).await;
        option(&db, "mgba", None, "mgba_solar", n).await;
        option(
            &db,
            "snes9x",
            Some(&format!("{n}-1")),
            "snes9x_overclock",
            n,
        )
        .await;
        preset(&db, "crt", "crt").await;
        preset(&db, "lcd", "lcd").await;
        shader(&db, AssignmentScope::Default, None, "crt", &[("GAMMA", n)]).await;
        let rom2 = format!("{n}-2");
        let game_shader = if a { "lcd" } else { "crt" };
        shader(&db, AssignmentScope::Rom, Some(&rom2), game_shader, &[]).await;
        let save = if a { "F5" } else { "F6" };
        hotkey(&db, SystemAction::QuickSave, vec![key(save)]).await;
        key_binding(&db, "R2", if a { "KeyR" } else { "KeyT" }).await;
        set_audio(
            &db,
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
            &db,
            VideoConfig {
                integer_scaling: a,
                hdr: a,
                hdr_paper_white_nits: 100,
                hdr_highlights: !a,
                hdr_peak_nits: 400,
            },
        )
        .await;
        aspect(&db, "default", "", if a { "4:3" } else { "16:9" }).await;
        aspect(&db, "system", "snes", if a { "fill" } else { "core" }).await;
        db
    }

    fn s4(a: &str, b: &str, c: &str, d: &str) -> (String, String, String, String) {
        (a.into(), b.into(), c.into(), d.into())
    }

    #[tokio::test]
    async fn export_then_apply_round_trips_core_options() {
        let src = mem_db().await;
        rom(&src, "src-1", "AAAA0001").await;
        rom(&src, "src-2", "AAAA0002").await;
        option(&src, "snes9x", None, "snes9x_region", "pal").await;
        option(&src, "snes9x", None, "snes9x_overclock", "on").await;
        option(&src, "mgba", None, "mgba_solar", "5").await;
        option(&src, "snes9x", Some("src-1"), "snes9x_region", "ntsc").await;
        option(&src, "mgba", Some("src-2"), "mgba_solar", "10").await;

        // os mesmos jogos com outros ids, e escolhas que a cópia troca
        let dst = mem_db().await;
        rom(&dst, "dst-1", "AAAA0001").await;
        rom(&dst, "dst-2", "AAAA0002").await;
        option(&dst, "snes9x", None, "snes9x_region", "ntsc").await;
        option(&dst, "genesis_plus_gx", None, "gpgx_region", "auto").await;
        option(&dst, "snes9x", Some("dst-1"), "snes9x_overclock", "off").await;

        let doc = export(Group::CoreOptions, &src).await.unwrap();
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
            })
        );
        let report = apply(Group::CoreOptions, &dst, &doc, &local_roms(&dst).await)
            .await
            .unwrap();
        assert_eq!(
            report,
            ApplyReport {
                applied: 5,
                ..ApplyReport::default()
            }
        );
        assert_eq!(export(Group::CoreOptions, &dst).await.unwrap(), doc);
    }

    #[tokio::test]
    async fn apply_core_options_skips_games_this_machine_lacks() {
        let src = mem_db().await;
        rom(&src, "a", "0000000A").await;
        rom(&src, "b", "0000000B").await;
        rom(&src, "c", "0000000C").await;
        option(&src, "snes9x", None, "snes9x_region", "pal").await;
        option(&src, "snes9x", Some("a"), "k", "1").await;
        option(&src, "snes9x", Some("b"), "k", "2").await;
        option(&src, "snes9x", Some("b"), "j", "3").await;
        option(&src, "mgba", Some("c"), "k", "4").await;
        let doc = export(Group::CoreOptions, &src).await.unwrap();

        // o mesmo jogo duas vezes na biblioteca (duas pastas): os dois recebem
        let dst = mem_db().await;
        rom(&dst, "x1", "0000000A").await;
        rom(&dst, "x2", "0000000A").await;
        // opção de um jogo que já saiu da biblioteca: não é da cópia, fica
        option(&dst, "snes9x", Some("sumiu"), "k", "9").await;

        let report = apply(Group::CoreOptions, &dst, &doc, &local_roms(&dst).await)
            .await
            .unwrap();
        assert_eq!(report.skipped_unknown_games, 2, "B e C, não três opções");
        assert_eq!(report.applied, 2, "o valor do core e a opção do jogo A");
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT rom_id, core_id, option_key, value FROM core_option_overrides ORDER BY rom_id",
        )
        .fetch_all(&dst)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![
                s4("sumiu", "snes9x", "k", "9"),
                s4("x1", "snes9x", "k", "1"),
                s4("x2", "snes9x", "k", "1"),
            ]
        );
    }

    #[tokio::test]
    async fn export_then_apply_round_trips_shaders_decorations() {
        use AssignmentScope::*;
        let src = mem_db().await;
        rom(&src, "src-1", "AAAA0001").await;
        preset(&src, "crt", "crt").await;
        preset(&src, "curated:xbr", "Suavizar pixel art (xBR)").await;
        shader(
            &src,
            Default,
            None,
            "curated:xbr",
            &[("XBR_EDGE_STR", "0.6")],
        )
        .await;
        shader(&src, System, Some("snes"), "crt", &[]).await;
        let gamma = [("CRT_GAMMA", "2.2"), ("MASK", "1")];
        shader(&src, Rom, Some("src-1"), "crt", &gamma).await;
        let src_base = abs(&["origem", "Molduras ReEmu"]);
        pack(&src, "bundled:reemu", "Molduras ReEmu", true, &src_base).await;
        let snes = abs(&["origem", "Molduras ReEmu", "snes", "moldura.png"]);
        deco(&src, "system", "snes", "bundled:reemu", &snes).await;
        let game = abs(&["origem", "Molduras ReEmu", "jogos", "AAAA0001.png"]);
        deco(&src, "rom", "src-1", "bundled:reemu", &game).await;

        // o mesmo pacote do catálogo em outra pasta, e um shader que a cópia troca
        let dst = mem_db().await;
        rom(&dst, "dst-1", "AAAA0001").await;
        preset(&dst, "plain", "plain").await;
        shader(&dst, Default, None, "plain", &[]).await;
        let dst_base = abs(&["destino", "Molduras"]);
        pack(&dst, "bundled:local", "Molduras ReEmu", true, &dst_base).await;

        let doc = export(Group::ShadersDecorations, &src).await.unwrap();
        assert_eq!(
            doc,
            json!({
                "v": 1,
                "assignments": [
                    {"scope": "default", "preset": "curated:xbr", "params": {"XBR_EDGE_STR": "0.6"}},
                    {"scope": "system", "system_id": "snes", "preset": "crt"},
                    {"scope": "rom", "crc32": "AAAA0001", "preset": "crt",
                     "params": {"CRT_GAMMA": "2.2", "MASK": "1"}},
                ],
                "decorations": [
                    {"scope": "system", "system_id": "snes", "pack": "Molduras ReEmu",
                     "asset": "snes/moldura.png"},
                    {"scope": "rom", "crc32": "AAAA0001", "pack": "Molduras ReEmu",
                     "asset": "jogos/AAAA0001.png"},
                ],
                "missing": [],
            })
        );
        let report = apply(
            Group::ShadersDecorations,
            &dst,
            &doc,
            &local_roms(&dst).await,
        )
        .await
        .unwrap();
        assert_eq!(
            report,
            ApplyReport {
                applied: 5,
                ..ApplyReport::default()
            }
        );
        assert_eq!(export(Group::ShadersDecorations, &dst).await.unwrap(), doc);
        // a moldura aponta para o pacote DESTA máquina
        let asset: String = sqlx::query_scalar(
            "SELECT asset_path FROM decoration_assignments WHERE scope = 'system'",
        )
        .fetch_one(&dst)
        .await
        .unwrap();
        let expected = Path::new(&dst_base).join("snes").join("moldura.png");
        assert_eq!(asset, expected.to_string_lossy());
    }

    #[tokio::test]
    async fn shaders_skip_imported_files_and_report_them() {
        use AssignmentScope::*;
        let src = mem_db().await;
        let mine = abs(&["Meus Shaders", "crt-do-ed.slangp"]);
        preset(&src, &mine, "crt-do-ed").await;
        shader(&src, System, Some("snes"), &mine, &[("GAMMA", "2.4")]).await;
        preset(&src, "curated:xbr", "xBR").await;
        shader(&src, Default, None, "curated:xbr", &[]).await;
        let user_base = abs(&["Molduras do Ed"]);
        let user_pack = format!("pack:{user_base}");
        pack(&src, &user_pack, "Molduras do Ed", false, &user_base).await;
        let user_png = abs(&["Molduras do Ed", "padrao.png"]);
        deco(&src, "default", "", &user_pack, &user_png).await;
        // um pacote do catálogo que a outra máquina não baixou
        let cat_base = abs(&["origem", "Molduras ReEmu"]);
        pack(&src, "bundled:reemu", "Molduras ReEmu", true, &cat_base).await;
        let gba = abs(&["origem", "Molduras ReEmu", "gba.png"]);
        deco(&src, "system", "gba", "bundled:reemu", &gba).await;

        let doc = export(Group::ShadersDecorations, &src).await.unwrap();
        assert_eq!(doc["missing"], json!(["Molduras do Ed", "crt-do-ed"]));
        assert_eq!(
            doc["assignments"],
            json!([{"scope": "default", "preset": "curated:xbr"}])
        );
        assert_eq!(
            doc["decorations"],
            json!([{"scope": "system", "system_id": "gba", "pack": "Molduras ReEmu",
                    "asset": "gba.png"}])
        );

        let dst = mem_db().await;
        let report = apply(
            Group::ShadersDecorations,
            &dst,
            &doc,
            &local_roms(&dst).await,
        )
        .await
        .unwrap();
        assert_eq!(
            report.missing_files,
            vec!["Molduras ReEmu", "Molduras do Ed", "crt-do-ed"]
        );
        assert_eq!(report.applied, 1);
        // nada aponta para um arquivo que esta máquina não tem
        assert!(dump(&dst, "decoration_assignments").await.is_empty());
        let shaders: Vec<(String, Option<String>, String)> =
            sqlx::query_as("SELECT scope, system_id, preset_id FROM shader_chain_assignments")
                .fetch_all(&dst)
                .await
                .unwrap();
        assert_eq!(
            shaders,
            vec![("default".into(), None, "curated:xbr".into())]
        );
    }

    #[tokio::test]
    async fn export_then_apply_round_trips_hotkeys_keyboard() {
        let src = mem_db().await;
        hotkey(&src, SystemAction::ToggleMenuOverlay, vec![key("F1")]).await;
        hotkey(
            &src,
            SystemAction::QuickSave,
            vec![key("ShiftLeft"), key("F5")],
        )
        .await;
        // botão de controle: o GUID é deste controle, a hotkey fica nesta máquina
        hotkey(&src, SystemAction::QuickLoad, vec![pad(GUID, 4)]).await;
        key_binding(&src, "R2", "KeyR").await;
        key_binding(&src, "LStickUp", "").await;

        let dst = mem_db().await;
        hotkey(&dst, SystemAction::QuickSave, vec![key("F2")]).await;
        hotkey(&dst, SystemAction::QuickLoad, vec![pad(OTHER_GUID, 7)]).await;
        key_binding(&dst, "A", "KeyZ").await;

        let doc = export(Group::HotkeysKeyboard, &src).await.unwrap();
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
        let report = apply(Group::HotkeysKeyboard, &dst, &doc, &[])
            .await
            .unwrap();
        assert_eq!(report.applied, 4);
        assert_eq!(export(Group::HotkeysKeyboard, &dst).await.unwrap(), doc);
        // o botão do controle desta máquina continua onde estava
        let load: String = sqlx::query_scalar(
            "SELECT trigger_json FROM system_hotkeys WHERE action = 'quick_load'",
        )
        .fetch_one(&dst)
        .await
        .unwrap();
        assert!(load.contains(OTHER_GUID));
    }

    #[tokio::test]
    async fn export_then_apply_round_trips_audio() {
        let src = mem_db().await;
        set_audio(
            &src,
            AudioConfig {
                output_device_id: Some("{0.0.0.00000000}.{8f1c}".into()),
                output_device_name: Some("Alto-falantes (Realtek)".into()),
                rate_control_enabled: false,
                rate_control_delta: 0.0078125,
                sample_rate_preference: Some(48000),
            },
        )
        .await;
        let dst = mem_db().await;
        let dst_audio = AudioConfig {
            output_device_id: Some("fones-usb".into()),
            output_device_name: Some("Fones USB".into()),
            rate_control_enabled: true,
            rate_control_delta: 0.005,
            sample_rate_preference: None,
        };
        set_audio(&dst, dst_audio.clone()).await;

        let doc = export(Group::Audio, &src).await.unwrap();
        assert_eq!(
            doc,
            json!({
                "v": 1,
                "rate_control_enabled": false,
                "rate_control_delta": 0.0078125,
                "sample_rate_preference": 48000,
            })
        );
        let report = apply(Group::Audio, &dst, &doc, &[]).await.unwrap();
        assert_eq!(report.applied, 1);
        assert_eq!(export(Group::Audio, &dst).await.unwrap(), doc);
        // o aparelho de saída é desta máquina
        let now = db::AudioConfigRepo::new(dst.clone()).get().await.unwrap();
        assert_eq!(now.output_device_id, dst_audio.output_device_id);
        assert_eq!(now.output_device_name, dst_audio.output_device_name);

        // O documento chega como texto (nuvem, backup do "Desfazer") e o
        // delta que a tela grava vem de um f32: o REAL volta com os mesmos bits.
        set_audio(
            &src,
            AudioConfig {
                rate_control_delta: 0.0037,
                ..dst_audio
            },
        )
        .await;
        let text = export(Group::Audio, &src).await.unwrap().to_string();
        let doc: Value = serde_json::from_str(&text).unwrap();
        apply(Group::Audio, &dst, &doc, &[]).await.unwrap();
        let delta = "SELECT rate_control_delta FROM audio_config";
        let bits = |v: f64| v.to_bits();
        let here: f64 = sqlx::query_scalar(delta).fetch_one(&src).await.unwrap();
        let there: f64 = sqlx::query_scalar(delta).fetch_one(&dst).await.unwrap();
        assert_eq!(bits(there), bits(here), "{here} virou {there}");
    }

    #[tokio::test]
    async fn export_then_apply_round_trips_video() {
        let src = mem_db().await;
        rom(&src, "src-1", "AAAA0001").await;
        set_video(
            &src,
            VideoConfig {
                integer_scaling: true,
                hdr: true,
                hdr_paper_white_nits: 203,
                hdr_highlights: false,
                hdr_peak_nits: 1000,
            },
        )
        .await;
        aspect(&src, "default", "", "4:3").await;
        aspect(&src, "system", "snes", "16:9").await;
        aspect(&src, "rom", "src-1", "fill").await;

        let dst = mem_db().await;
        rom(&dst, "dst-1", "AAAA0001").await;
        set_video(
            &dst,
            VideoConfig {
                integer_scaling: false,
                hdr: false,
                hdr_paper_white_nits: 80,
                hdr_highlights: true,
                hdr_peak_nits: 600,
            },
        )
        .await;
        aspect(&dst, "default", "", "core").await;
        aspect(&dst, "system", "gba", "16:9").await;
        // proporção de um jogo que já saiu da biblioteca: não é da cópia, fica
        aspect(&dst, "rom", "sumiu", "fill").await;

        let doc = export(Group::Video, &src).await.unwrap();
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
        let report = apply(Group::Video, &dst, &doc, &local_roms(&dst).await)
            .await
            .unwrap();
        assert_eq!(report.applied, 4);
        assert_eq!(export(Group::Video, &dst).await.unwrap(), doc);
        // os nits foram medidos na tela desta máquina
        let now = db::VideoConfigRepo::new(dst.clone()).get().await.unwrap();
        assert_eq!((now.hdr_paper_white_nits, now.hdr_peak_nits), (80, 600));
        assert!(dump(&dst, "aspect_assignments")
            .await
            .contains(&"'rom'|'sumiu'|'fill'".to_string()));
    }

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
        let db = mem_db().await;
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
        rom(&db, "r1", "AAAA0001").await;
        option(&db, "snes9x", Some("r1"), "snes9x_region", "pal").await;
        set_audio(
            &db,
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
            &db,
            VideoConfig {
                integer_scaling: false,
                hdr: true,
                hdr_paper_white_nits: 437,
                hdr_highlights: true,
                hdr_peak_nits: 1234,
            },
        )
        .await;
        aspect(&db, "rom", "r1", "4:3").await;
        hotkey(&db, SystemAction::QuickSave, vec![key("F5")]).await;
        hotkey(&db, SystemAction::QuickLoad, vec![pad(GUID, 3), key("F9")]).await;
        // uma hotkey presa a um aparelho pela coluna, mesmo sendo de teclado
        run(
            &db,
            "INSERT INTO system_hotkeys (id, action, trigger_json, device_guid) \
             VALUES ('toggle_menu_overlay', 'toggle_menu_overlay', ?1, ?2)",
            &[&serde_json::to_string(&[key("F1")]).unwrap(), GUID],
        )
        .await;
        let mine = abs(&["Meus Shaders", "crt-do-ed.slangp"]);
        preset(&db, &mine, "crt-do-ed").await;
        shader(&db, System, Some("gba"), &mine, &[]).await;
        preset(&db, "crt", "crt").await;
        shader(&db, Rom, Some("r1"), "crt", &[]).await;
        let user_base = abs(&["Molduras do Ed"]);
        let user_pack = format!("pack:{user_base}");
        pack(&db, &user_pack, "Molduras do Ed", false, &user_base).await;
        deco(
            &db,
            "default",
            "",
            &user_pack,
            &abs(&["Molduras do Ed", "a.png"]),
        )
        .await;
        let cat_base = abs(&["catalogo", "Molduras ReEmu"]);
        pack(&db, "bundled:reemu", "Molduras ReEmu", true, &cat_base).await;
        let snes = abs(&["catalogo", "Molduras ReEmu", "snes", "moldura.png"]);
        deco(&db, "system", "snes", "bundled:reemu", &snes).await;

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
        ];
        for g in Group::ALL {
            let doc = export(g, &db).await.unwrap();
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
        let shaders = export(Group::ShadersDecorations, &db).await.unwrap();
        assert_eq!(shaders["decorations"][0]["asset"], "snes/moldura.png");
        assert_eq!(shaders["assignments"][0]["crc32"], "AAAA0001");
        let hotkeys = export(Group::HotkeysKeyboard, &db).await.unwrap();
        assert_eq!(hotkeys["hotkeys"].as_array().unwrap().len(), 1);
        assert_eq!(hotkeys["hotkeys"][0]["action"], "quick_save");
        let video = export(Group::Video, &db).await.unwrap();
        assert_eq!(video["hdr"], true);
        assert_eq!(video["aspect"][0]["target"], "AAAA0001");
        let audio = export(Group::Audio, &db).await.unwrap();
        assert_eq!(audio["sample_rate_preference"], 44100);
    }

    #[tokio::test]
    async fn apply_refuses_an_unknown_version() {
        let src = machine("a").await;
        let dst = machine("b").await;
        let roms = local_roms(&dst).await;
        let before = snapshot(&dst).await;
        for g in Group::ALL {
            let mut doc = export(g, &src).await.unwrap();
            doc["v"] = json!(2);
            let refused = apply(g, &dst, &doc, &roms).await;
            assert_eq!(refused, Err(ERR_VERSION.to_string()), "{g:?}");
            doc.as_object_mut().unwrap().remove("v");
            let refused = apply(g, &dst, &doc, &roms).await;
            assert_eq!(refused, Err(ERR_VERSION.to_string()), "{g:?} sem v");
        }
        assert_eq!(snapshot(&dst).await, before);
    }

    #[tokio::test]
    async fn apply_is_all_or_nothing_per_group() {
        let src = machine("a").await;
        let dst = machine("b").await;
        let roms = local_roms(&dst).await;
        let before = snapshot(&dst).await;

        // opções: os valores por core já foram trocados quando a mesma opção
        // do mesmo jogo aparece de novo
        let mut doc = export(Group::CoreOptions, &src).await.unwrap();
        let again = doc["overrides"][0].clone();
        doc["overrides"].as_array_mut().unwrap().push(again);
        assert!(apply(Group::CoreOptions, &dst, &doc, &roms).await.is_err());

        // shaders: as atribuições já foram trocadas quando a moldura aponta
        // para fora do pacote
        let mut doc = export(Group::ShadersDecorations, &src).await.unwrap();
        doc["decorations"] = json!([{"scope": "default", "pack": "x", "asset": "../fora.png"}]);
        let refused = apply(Group::ShadersDecorations, &dst, &doc, &roms).await;
        assert_eq!(refused, Err(ERR_INVALID.to_string()));

        // teclado: as hotkeys já foram trocadas quando o mesmo alvo aparece de novo
        let mut doc = export(Group::HotkeysKeyboard, &src).await.unwrap();
        let again = json!({"target": "R2", "code": "KeyQ"});
        doc["keyboard"].as_array_mut().unwrap().push(again);
        assert!(apply(Group::HotkeysKeyboard, &dst, &doc, &roms)
            .await
            .is_err());

        // vídeo: a linha de vídeo já foi gravada quando aparece uma proporção
        // sem escopo válido
        let mut doc = export(Group::Video, &src).await.unwrap();
        let bad = json!({"scope": "tela", "target": "", "aspect": "4:3"});
        doc["aspect"].as_array_mut().unwrap().push(bad);
        let refused = apply(Group::Video, &dst, &doc, &roms).await;
        assert_eq!(refused, Err(ERR_INVALID.to_string()));

        assert_eq!(snapshot(&dst).await, before);
    }

    /// O "Desfazer" aplica o documento desta máquina guardado antes da cópia:
    /// como aplicar nunca toca no que é só desta máquina, ela volta igual.
    #[tokio::test]
    async fn applying_the_own_export_after_a_copy_restores_every_group() {
        use AssignmentScope::*;
        let here = machine("b").await;
        // só desta máquina: preset importado do disco no GBA, molduras
        // importadas, hotkey de controle e escolhas de um jogo fora da biblioteca
        let mine = abs(&["Meus Shaders", "crt-do-ed.slangp"]);
        preset(&here, &mine, "crt-do-ed").await;
        shader(&here, System, Some("gba"), &mine, &[("GAMMA", "2.4")]).await;
        let user_base = abs(&["Molduras do Ed"]);
        let user_pack = format!("pack:{user_base}");
        pack(&here, &user_pack, "Molduras do Ed", false, &user_base).await;
        let png = abs(&["Molduras do Ed", "padrao.png"]);
        deco(&here, "default", "", &user_pack, &png).await;
        hotkey(&here, SystemAction::QuickLoad, vec![pad(GUID, 2)]).await;
        option(&here, "snes9x", Some("sumiu"), "snes9x_region", "pal").await;
        aspect(&here, "rom", "sumiu", "fill").await;

        // a outra máquina tem um shader do catálogo onde esta tem o importado
        let there = machine("a").await;
        shader(&there, System, Some("gba"), "lcd", &[]).await;
        let curated = "curated:xbr";
        preset(&there, curated, "xBR").await;
        shader(&there, System, Some("snes"), curated, &[]).await;
        hotkey(&there, SystemAction::QuickLoad, vec![key("F9")]).await;

        let roms = local_roms(&here).await;
        let before = snapshot(&here).await;
        let mut backup = Vec::new();
        for g in Group::ALL {
            backup.push((g, export(g, &here).await.unwrap()));
        }
        for g in Group::ALL {
            let doc = export(g, &there).await.unwrap();
            apply(g, &here, &doc, &roms).await.unwrap();
        }
        let copied = snapshot(&here).await;
        assert_ne!(copied, before);
        // o que é só desta máquina sobreviveu à cópia
        for kept in [&mine, GUID, "sumiu"] {
            assert!(
                copied.iter().any(|r| r.contains(kept)),
                "{kept} sumiu na cópia"
            );
        }

        for (g, doc) in &backup {
            apply(*g, &here, doc, &roms).await.unwrap();
        }
        assert_eq!(snapshot(&here).await, before);
    }
}
