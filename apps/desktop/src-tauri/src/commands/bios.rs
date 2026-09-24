//! BIOS (arquivos de sistema).

use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiosStatusDto {
    pub system_id: String,
    pub filename: String,
    pub required: bool,
    pub note: String,
    pub present: bool,
    pub hash_ok: Option<bool>,
}

/// Confere `<dados>/system` contra a tabela de BIOS conhecida
/// (`domain::bios`) — presença + MD5 quando documentado. Nunca baixa nada.
#[tauri::command]
pub fn list_bios_status(state: State<'_, AppState>) -> Vec<BiosStatusDto> {
    crate::bios::check_all(&state.system_dir)
        .into_iter()
        .map(|s| BiosStatusDto {
            system_id: s.system_id,
            filename: s.filename,
            required: s.required,
            note: s.note,
            present: s.present,
            hash_ok: s.hash_ok,
        })
        .collect()
}

/// Copia `path` (escolhido no file picker do frontend) pro lugar/nome
/// esperado dentro de `<dados>/system`.
#[tauri::command]
pub fn import_bios_file(
    state: State<'_, AppState>,
    system_id: String,
    filename: String,
    path: String,
) -> Result<(), String> {
    crate::bios::import_bios_file(
        &state.system_dir,
        &system_id,
        &filename,
        std::path::Path::new(&path),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_bios_file(
    state: State<'_, AppState>,
    system_id: String,
    filename: String,
) -> Result<(), String> {
    crate::bios::remove_bios_file(&state.system_dir, &system_id, &filename)
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreOptionDto {
    pub key: String,
    pub display_name: String,
    /// Escolhas possíveis (as opções libretro são sempre enumeradas).
    pub choices: Vec<String>,
    pub default_value: String,
    /// Valor efetivo (rom → core → default).
    pub value: String,
    /// Valor salvo por core (`null` = usa o default do schema).
    pub core_value: Option<String>,
    /// Override deste jogo (`null` = herda o valor por core). Só quando
    /// `rom_id` foi passado.
    pub rom_value: Option<String>,
}

/// Schema + valores das core options em cascata. `rom_id` (opcional) traz
/// também o override do jogo. Schema vem do core carregado se for esse
/// `core_id` (mais atual); os valores vêm sempre do DB.
#[tauri::command]
pub async fn get_core_options(
    state: State<'_, AppState>,
    core_id: String,
    rom_id: Option<String>,
) -> Result<Vec<CoreOptionDto>, String> {
    use domain::core_options::{CoreOptionType, CoreOptionsStore};

    let live_matches = state.session.loaded_core().as_deref() == Some(core_id.as_str());
    let (defs, core_values, rom_values) = if let Some(pool) = state.db.clone() {
        let repo = db::CoreOptionsRepo::new(pool);
        let defs = if live_matches {
            state.session.core_options().0
        } else {
            repo.schema_for(&core_id).await.map_err(|e| e.to_string())?
        };
        let core_values = repo.values_for(&core_id).await.map_err(|e| e.to_string())?;
        let rom_values = match &rom_id {
            Some(rid) => repo
                .overrides_for_rom(rid, &core_id)
                .await
                .map_err(|e| e.to_string())?,
            None => Default::default(),
        };
        (defs, core_values, rom_values)
    } else if live_matches {
        (
            state.session.core_options().0,
            Default::default(),
            Default::default(),
        )
    } else {
        (Vec::new(), Default::default(), Default::default())
    };

    Ok(defs
        .into_iter()
        .map(|d| {
            let choices = match d.option_type {
                CoreOptionType::Combo { choices } => choices,
                CoreOptionType::Bool => vec!["disabled".into(), "enabled".into()],
                CoreOptionType::Range { .. } => Vec::new(),
            };
            let core_value = core_values.get(&d.option_key).cloned();
            let rom_value = rom_values.get(&d.option_key).cloned();
            let value = rom_value
                .clone()
                .or_else(|| core_value.clone())
                .unwrap_or_else(|| d.default_value.clone());
            CoreOptionDto {
                key: d.option_key,
                display_name: d.display_name,
                choices,
                default_value: d.default_value,
                value,
                core_value,
                rom_value,
            }
        })
        .collect())
}

/// Troca (ou limpa, com `value` vazio) uma core option num escopo:
/// `rom_id` ausente → valor por core; presente → override do jogo. Aplica no
/// core carregado se for esse core E (sem rom_id, ou a rom carregada bate).
#[tauri::command]
pub async fn set_core_option(
    state: State<'_, AppState>,
    core_id: String,
    key: String,
    value: String,
    rom_id: Option<String>,
) -> Result<(), String> {
    use domain::core_options::CoreOptionsStore;

    let clear = value.is_empty();
    let live_rom = state
        .current_rom
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    let live = state.session.loaded_core().as_deref() == Some(core_id.as_str())
        && match (&rom_id, &live_rom) {
            (None, _) => true,
            (Some(rid), Some(lr)) => rid == lr,
            _ => false,
        };
    if live && !clear && !state.session.set_core_option(&key, &value) {
        return Err("opção ou valor inválido pro core carregado".into());
    }
    if let Some(pool) = state.db.clone() {
        db::CoreOptionsRepo::new(pool)
            .set_scoped_value(
                &core_id,
                rom_id.as_deref(),
                &key,
                (!clear).then_some(value.as_str()),
            )
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Limpa TODOS os valores de um escopo (volta pro default do schema, ou pro
/// valor por core no caso de escopo de jogo). Não recarrega o core — vale no
/// próximo load.
#[tauri::command]
pub async fn reset_core_options(
    state: State<'_, AppState>,
    core_id: String,
    rom_id: Option<String>,
) -> Result<(), String> {
    use domain::core_options::CoreOptionsStore;
    if let Some(pool) = state.db.clone() {
        db::CoreOptionsRepo::new(pool)
            .reset_scope(&core_id, rom_id.as_deref())
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Pasta `PPSSPP/` (assets do core de PSP) instalada em `<system_dir>/`?
#[tauri::command]
pub fn ppsspp_assets_installed(state: State<'_, AppState>) -> bool {
    crate::system_files::ppsspp_installed(&state.system_dir)
}

/// Baixa os assets do PPSSPP do buildbot da libretro (GPL — ao contrário de
/// BIOS, pode ser baixado). Devolve quantos arquivos foram instalados.
#[tauri::command]
pub async fn download_ppsspp_assets(state: State<'_, AppState>) -> Result<usize, String> {
    crate::system_files::download_ppsspp(&state.system_dir).await
}
