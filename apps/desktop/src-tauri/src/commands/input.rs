//! Input e captura de binding (etapa 05).

use super::*;

/// Recebe `KeyboardEvent.code` da webview (keydown/keyup).
///
/// - Em modo de captura de binding: o evento vai pro frontend, não pro jogo.
/// - `Escape` sempre alterna o menu (rede de segurança independente do DB).
/// - Qualquer tecla entra/sai do conjunto segurado (`input_desktop::held`) —
///   o loop de eventos resolve as hotkeys configuradas a partir dele, com
///   prioridade sobre o input de jogo.
/// - O mapa teclado→RetroPad só vale em `GameFocused`.
#[tauri::command]
pub fn input_key(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
    pressed: bool,
) -> Result<(), String> {
    use domain::focus::{FocusManager, InputFocus};

    // Modo de captura de binding: o evento vai pro frontend, não pro jogo.
    if input_desktop::capture::is_capturing() {
        if pressed && !code.is_empty() {
            let ev = domain::input::RawInputEvent::Keyboard {
                scancode: input_desktop::keymap::key_scancode(&code),
            };
            let _ = app.emit("raw-input-captured", &ev);
        }
        return Ok(());
    }

    if pressed && code == "Escape" {
        toggle_and_emit(&app);
        return Ok(());
    }

    // Conjunto segurado (resolução de hotkey de combinação no loop de eventos).
    if !code.is_empty() {
        let kev = domain::input::RawInputEvent::Keyboard {
            scancode: input_desktop::keymap::key_scancode(&code),
        };
        if pressed {
            input_desktop::held::press(kev);
        } else {
            input_desktop::held::release(&kev);
        }
    }

    let game_focused = state
        .focus
        .lock()
        .map(|f| f.current())
        .unwrap_or(InputFocus::GameFocused)
        == InputFocus::GameFocused;

    if let Some((port, button)) = input_desktop::keymap::web_code_to_retropad(&code) {
        emu_session::retropad().set(port as usize, button, pressed && game_focused);
    }
    Ok(())
}

// --- captura de binding (etapa 05) -----------------------------------

fn retropad_from_str(s: &str) -> Option<domain::input::RetroPadButton> {
    use domain::input::RetroPadButton::*;
    Some(match s {
        "A" => A,
        "B" => B,
        "X" => X,
        "Y" => Y,
        "L1" => L1,
        "L2" => L2,
        "L3" => L3,
        "R1" => R1,
        "R2" => R2,
        "R3" => R3,
        "Up" => Up,
        "Down" => Down,
        "Left" => Left,
        "Right" => Right,
        "Start" => Start,
        "Select" => Select,
        _ => return None,
    })
}

/// Entra em modo de captura: os próximos eventos brutos (teclado via
/// `input_key`, gamepad via a thread de `emu-session`) vão pro frontend por
/// `raw-input-captured` em vez de irem pro jogo.
#[tauri::command]
pub fn start_binding_capture() -> Result<(), String> {
    emu_session::retropad().clear();
    input_desktop::capture::begin();
    Ok(())
}

#[tauri::command]
pub fn cancel_binding_capture() -> Result<(), String> {
    input_desktop::capture::end();
    Ok(())
}

/// Grava a combinação capturada. `target`:
/// - `"system_hotkey"` → `target_key` = `SystemAction::as_wire()`
/// - `"controller_mapping"` → `target_key` = `"<guid>::<display_name>::<Botão>"`
#[tauri::command]
pub async fn save_binding(
    state: State<'_, AppState>,
    target: String,
    target_key: String,
    trigger: Vec<domain::input::RawInputEvent>,
) -> Result<(), String> {
    input_desktop::capture::end();
    if trigger.is_empty() {
        return Err("nenhum input capturado".into());
    }
    match target.as_str() {
        "system_hotkey" => {
            use domain::hotkeys::SystemHotkeyRepository;
            let action = SystemAction::from_wire(&target_key)
                .ok_or_else(|| format!("ação desconhecida: {target_key}"))?;
            let pool = pool(&state)?;
            db::SystemHotkeysRepo::new(pool.clone())
                .set(&HotkeyBinding {
                    action,
                    trigger,
                    device_guid: None,
                })
                .await
                .map_err(|e| e.to_string())?;
            refresh_hotkey_resolver(&state, &pool).await
        }
        "controller_mapping" => {
            use domain::input::{
                ControllerLayoutEntry, ControllerMapping, ControllerMappingRepository,
                MappingSource,
            };
            let mut parts = target_key.splitn(3, "::");
            let guid = parts.next().unwrap_or_default().to_string();
            let display_name = parts.next().unwrap_or("Controle").to_string();
            let button = parts
                .next()
                .and_then(retropad_from_str)
                .ok_or_else(|| format!("target_key inválido: {target_key}"))?;
            if guid.is_empty() {
                return Err("guid vazio".into());
            }
            let repo = db::ControllerMappingsRepo::new(pool(&state)?);
            let mut mapping =
                repo.get(&guid)
                    .await
                    .map_err(|e| e.to_string())?
                    .unwrap_or(ControllerMapping {
                        guid: guid.clone(),
                        display_name: display_name.clone(),
                        layout: Vec::new(),
                        source: MappingSource::UserOverride,
                    });
            mapping.display_name = display_name;
            mapping.source = MappingSource::UserOverride;
            mapping.layout.retain(|e| e.button != button);
            mapping
                .layout
                .push(ControllerLayoutEntry { trigger, button });
            repo.upsert(&mapping).await.map_err(|e| e.to_string())?;
            refresh_controller_mappings(&state).await
        }
        other => Err(format!("target desconhecido: {other}")),
    }
}

/// Relê `controller_mappings` do DB e publica no override global lido pela
/// thread de gamepad (`input_desktop::mappings`).
async fn refresh_controller_mappings(state: &AppState) -> Result<(), String> {
    use domain::input::ControllerMappingRepository;
    let list = db::ControllerMappingsRepo::new(pool(state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?;
    input_desktop::mappings::set(list);
    Ok(())
}

/// Carrega `controller_mappings` + `device_port_assignment` nos overrides
/// globais. Best-effort no startup.
pub async fn load_controller_mappings(pool: &db::Db) {
    use domain::input::{ControllerMappingRepository, DevicePortRepository};
    if let Ok(list) = db::ControllerMappingsRepo::new(pool.clone()).list().await {
        input_desktop::mappings::set(list);
    }
    if let Ok(ports) = db::DevicePortsRepo::new(pool.clone()).list().await {
        input_desktop::mappings::set_ports(
            ports.into_iter().map(|(g, p)| (g, p as usize)).collect(),
        );
    }
}

#[tauri::command]
pub async fn clear_controller_mapping(
    state: State<'_, AppState>,
    guid: String,
) -> Result<(), String> {
    use domain::input::ControllerMappingRepository;
    db::ControllerMappingsRepo::new(pool(&state)?)
        .delete(&guid)
        .await
        .map_err(|e| e.to_string())?;
    refresh_controller_mappings(&state).await
}

async fn refresh_device_ports(state: &AppState) -> Result<(), String> {
    use domain::input::DevicePortRepository;
    let ports = db::DevicePortsRepo::new(pool(state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?;
    input_desktop::mappings::set_ports(ports.into_iter().map(|(g, p)| (g, p as usize)).collect());
    Ok(())
}

#[tauri::command]
pub async fn set_device_port(
    state: State<'_, AppState>,
    guid: String,
    port: u8,
) -> Result<(), String> {
    use domain::input::DevicePortRepository;
    db::DevicePortsRepo::new(pool(&state)?)
        .set(&guid, port)
        .await
        .map_err(|e| e.to_string())?;
    refresh_device_ports(&state).await
}

#[tauri::command]
pub async fn clear_device_port(state: State<'_, AppState>, guid: String) -> Result<(), String> {
    use domain::input::DevicePortRepository;
    db::DevicePortsRepo::new(pool(&state)?)
        .clear(&guid)
        .await
        .map_err(|e| e.to_string())?;
    refresh_device_ports(&state).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePortDto {
    pub guid: String,
    pub port: u8,
}

#[tauri::command]
pub async fn list_device_ports(state: State<'_, AppState>) -> Result<Vec<DevicePortDto>, String> {
    use domain::input::DevicePortRepository;
    Ok(db::DevicePortsRepo::new(pool(&state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(guid, port)| DevicePortDto { guid, port })
        .collect())
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GamepadDto {
    pub guid: String,
    pub name: String,
}

#[tauri::command]
pub fn list_gamepads(state: State<'_, AppState>) -> Vec<GamepadDto> {
    state
        .session
        .connected_gamepads()
        .into_iter()
        .map(|(guid, name)| GamepadDto { guid, name })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyBindingDto {
    pub action: String,
    pub trigger: Vec<domain::input::RawInputEvent>,
}

#[tauri::command]
pub async fn list_system_hotkeys(
    state: State<'_, AppState>,
) -> Result<Vec<HotkeyBindingDto>, String> {
    use domain::hotkeys::SystemHotkeyRepository;
    let bindings = db::SystemHotkeysRepo::new(pool(&state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?;
    Ok(bindings
        .into_iter()
        .map(|b| HotkeyBindingDto {
            action: b.action.as_wire().to_string(),
            trigger: b.trigger,
        })
        .collect())
}

#[tauri::command]
pub async fn clear_system_hotkey(state: State<'_, AppState>, action: String) -> Result<(), String> {
    use domain::hotkeys::SystemHotkeyRepository;
    let action =
        SystemAction::from_wire(&action).ok_or_else(|| format!("ação desconhecida: {action}"))?;
    let pool = pool(&state)?;
    db::SystemHotkeysRepo::new(pool.clone())
        .delete(action)
        .await
        .map_err(|e| e.to_string())?;
    refresh_hotkey_resolver(&state, &pool).await
}

/// Relê `system_hotkeys` do DB e recompõe o `ComboHotkeyResolver` do `AppState`.
async fn refresh_hotkey_resolver(state: &AppState, pool: &db::Db) -> Result<(), String> {
    let bindings = load_system_hotkeys(pool).await.map_err(|e| e.to_string())?;
    *state.hotkeys.lock().unwrap_or_else(|p| p.into_inner()) = ComboHotkeyResolver::new(bindings);
    Ok(())
}

/// Carrega as hotkeys do DB, semeando o default (`ToggleMenuOverlay` = `F1`)
/// se a tabela estiver vazia. Usado no startup e por `refresh_hotkey_resolver`.
pub async fn load_system_hotkeys(pool: &db::Db) -> Result<Vec<HotkeyBinding>, db::DbError> {
    use domain::hotkeys::SystemHotkeyRepository;
    let repo = db::SystemHotkeysRepo::new(pool.clone());
    let existing = repo
        .list()
        .await
        .map_err(|e| db::DbError::Sqlite(e.to_string()))?;
    if existing.is_empty() {
        let default = HotkeyBinding {
            action: SystemAction::ToggleMenuOverlay,
            trigger: vec![domain::input::RawInputEvent::Keyboard {
                scancode: input_desktop::keymap::key_scancode("F1"),
            }],
            device_guid: None,
        };
        if repo.set(&default).await.is_ok() {
            return Ok(vec![default]);
        }
    }
    Ok(existing)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerEntryDto {
    pub button: String,
    pub trigger: Vec<domain::input::RawInputEvent>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerMappingDto {
    pub guid: String,
    pub display_name: String,
    pub source: String,
    pub entries: Vec<ControllerEntryDto>,
}

#[tauri::command]
pub async fn list_controller_mappings(
    state: State<'_, AppState>,
) -> Result<Vec<ControllerMappingDto>, String> {
    use domain::input::ControllerMappingRepository;
    let mappings = db::ControllerMappingsRepo::new(pool(&state)?)
        .list()
        .await
        .map_err(|e| e.to_string())?;
    Ok(mappings
        .into_iter()
        .map(|m| ControllerMappingDto {
            guid: m.guid,
            display_name: m.display_name,
            source: m.source.as_wire().to_string(),
            entries: m
                .layout
                .into_iter()
                .map(|e| ControllerEntryDto {
                    button: format!("{:?}", e.button),
                    trigger: e.trigger,
                })
                .collect(),
        })
        .collect())
}
