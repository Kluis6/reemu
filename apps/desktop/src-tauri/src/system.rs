//! Opções de sistema (Configurações › Sistema): iniciar junto com o sistema,
//! abrir em tela cheia e minimizar para a bandeja.
//!
//! - Iniciar com o sistema: plugin oficial `tauri-plugin-autostart`
//!   (v2.tauri.app/plugin/autostart). O estado mora no próprio sistema
//!   (registro no Windows), então não é guardado aqui: lê com
//!   `is_enabled()`, muda com `enable()`/`disable()`.
//! - Tela cheia ao abrir e minimizar para a bandeja: `app-settings.json` na
//!   pasta de dados. O Rust precisa delas antes da interface carregar (a
//!   janela já abre em tela cheia; o minimizar é tratado no evento da
//!   janela), por isso não ficam no `localStorage`.
//! - Bandeja: `tauri::tray` com o recurso `tray-icon`
//!   (v2.tauri.app/learn/system-tray). O ícone só existe enquanto a janela
//!   está escondida nela; restaurar remove o ícone.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_autostart::ManagerExt;

const FILE: &str = "app-settings.json";
const TRAY_ID: &str = "reemu-tray";

/// O que fica no `app-settings.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SystemPrefs {
    pub start_fullscreen: bool,
    pub minimize_to_tray: bool,
}

/// Rótulos do menu da bandeja, no idioma da interface (o frontend manda via
/// `set_tray_labels` quando o idioma muda). Padrão em pt-BR, a língua de
/// origem do app.
#[derive(Debug, Clone)]
struct TrayLabels {
    open: String,
    quit: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            open: "Abrir o ReEmu".into(),
            quit: "Sair".into(),
        }
    }
}

pub struct SystemState {
    prefs: Mutex<SystemPrefs>,
    labels: Mutex<TrayLabels>,
    path: PathBuf,
}

impl SystemState {
    fn prefs(&self) -> SystemPrefs {
        self.prefs.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn update(&self, f: impl FnOnce(&mut SystemPrefs)) -> Result<(), String> {
        let mut prefs = self.prefs.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut prefs);
        let json = serde_json::to_string_pretty(&*prefs).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| format!("{}: {e}", self.path.display()))
    }
}

/// Lê as preferências (arquivo ausente ou inválido = padrões) e registra o
/// estado. Chamar no `setup`.
pub fn init<R: Runtime>(app: &AppHandle<R>, data_dir: &Path) -> SystemPrefs {
    let path = data_dir.join(FILE);
    let prefs = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<SystemPrefs>(&s).ok())
        .unwrap_or_default();
    app.manage(SystemState {
        prefs: Mutex::new(prefs.clone()),
        labels: Mutex::new(TrayLabels::default()),
        path,
    });
    prefs
}

/// Estado completo pra tela de Configurações.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSettings {
    autostart: bool,
    start_fullscreen: bool,
    minimize_to_tray: bool,
}

#[tauri::command]
pub fn get_system_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SystemState>,
) -> SystemSettings {
    let prefs = state.prefs();
    SystemSettings {
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        start_fullscreen: prefs.start_fullscreen,
        minimize_to_tray: prefs.minimize_to_tray,
    }
}

#[tauri::command]
pub fn set_autostart<R: Runtime>(app: AppHandle<R>, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_start_fullscreen(state: State<'_, SystemState>, enabled: bool) -> Result<(), String> {
    state.update(|p| p.start_fullscreen = enabled)
}

#[tauri::command]
pub fn set_minimize_to_tray(state: State<'_, SystemState>, enabled: bool) -> Result<(), String> {
    state.update(|p| p.minimize_to_tray = enabled)
}

#[tauri::command]
pub fn set_tray_labels(state: State<'_, SystemState>, open: String, quit: String) {
    *state.labels.lock().unwrap_or_else(|p| p.into_inner()) = TrayLabels { open, quit };
}

/// Evento `Resized` da janela principal: com "minimizar para a bandeja"
/// ligado, uma janela minimizada vai pra bandeja (some da barra de tarefas).
pub fn on_main_resized<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<SystemState>() else {
        return;
    };
    if !state.prefs().minimize_to_tray {
        return;
    }
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if !win.is_minimized().unwrap_or(false) {
        return;
    }
    // Sem ícone na bandeja, esconder a janela a deixaria inalcançável.
    if let Err(e) = show_tray(app, &state) {
        log::warn!("bandeja: não deu pra criar o ícone ({e}) — janela fica minimizada");
        return;
    }
    let _ = win.hide();
}

fn show_tray<R: Runtime>(app: &AppHandle<R>, state: &SystemState) -> tauri::Result<()> {
    if app.tray_by_id(TRAY_ID).is_some() {
        return Ok(());
    }
    let labels = state.labels.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let open = MenuItem::with_id(app, "tray-open", &labels.open, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray-quit", &labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("ReEmu")
        .menu(&menu)
        // Clique esquerdo restaura; o menu fica no direito.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-open" => restore(app),
            // Mesmo caminho do X da janela: o `ExitRequested` descarrega o
            // jogo e grava a save RAM antes de sair.
            "tray-quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                restore(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Traz a janela de volta da bandeja e remove o ícone.
fn restore<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
    let _ = app.remove_tray_by_id(TRAY_ID);
}
