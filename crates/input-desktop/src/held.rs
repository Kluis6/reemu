//! Conjunto de inputs segurados **agora** (teclado + gamepad juntos), global
//! e único. O resolvedor de hotkeys (`ComboHotkeyResolver::resolve`) consulta
//! este snapshot: a "janela hold+press" da resolução é simplesmente "ambos
//! os eventos estão segurados neste instante".
//!
//! Escrito pelo comando `input_key` (teclado) e pelo `GamepadPoller` (gamepad,
//! só fora do modo de captura). Ninguém além disso deve mexer aqui.

use domain::input::RawInputEvent;
use std::sync::{Mutex, OnceLock};

fn store() -> &'static Mutex<Vec<RawInputEvent>> {
    static HELD: OnceLock<Mutex<Vec<RawInputEvent>>> = OnceLock::new();
    HELD.get_or_init(|| Mutex::new(Vec::new()))
}

/// Marca `ev` como segurado (idempotente).
pub fn press(ev: RawInputEvent) {
    let mut h = store().lock().unwrap_or_else(|p| p.into_inner());
    if !h.contains(&ev) {
        h.push(ev);
    }
}

/// Solta `ev` (no-op se não estava segurado).
pub fn release(ev: &RawInputEvent) {
    store()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|e| e != ev);
}

pub fn snapshot() -> Vec<RawInputEvent> {
    store().lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Solta tudo — chamado nas transições de foco pra não deixar tecla "grudada".
pub fn clear() {
    store().lock().unwrap_or_else(|p| p.into_inner()).clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkeys::ComboHotkeyResolver;
    use domain::hotkeys::{HotkeyBinding, HotkeyResolver, SystemAction};

    fn key(sc: u32) -> RawInputEvent {
        RawInputEvent::Keyboard { scancode: sc }
    }

    #[test]
    fn press_release_snapshot_and_combo_resolution() {
        // Isolado: este teste é o único a mexer no global (roda serial no crate).
        clear();
        let resolver = ComboHotkeyResolver::new(vec![HotkeyBinding {
            action: SystemAction::QuickSave,
            trigger: vec![key(1), key(2)],
            device_guid: None,
        }]);

        press(key(1));
        assert_eq!(resolver.resolve(&snapshot()), None); // só metade da combinação

        press(key(2));
        press(key(1)); // idempotente
        assert_eq!(snapshot().len(), 2);
        assert_eq!(resolver.resolve(&snapshot()), Some(SystemAction::QuickSave));

        release(&key(2));
        assert_eq!(resolver.resolve(&snapshot()), None);

        clear();
        assert!(snapshot().is_empty());
    }
}
