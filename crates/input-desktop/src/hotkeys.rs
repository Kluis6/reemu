//! `HotkeyResolver` com suporte a combinação (hold+press).
//!
//! O resolver é puro: recebe o **conjunto de eventos segurados agora** e
//! diz se alguma hotkey casa. A janela de tempo do hold+press é
//! responsabilidade de quem rastreia o estado ao longo do tempo (o
//! `InputManager`). Combinação vence tecla única (trigger mais longo primeiro).

use domain::hotkeys::{HotkeyBinding, HotkeyResolver, SystemAction};
use domain::input::RawInputEvent;

#[derive(Default)]
pub struct ComboHotkeyResolver {
    bindings: Vec<HotkeyBinding>,
}

impl ComboHotkeyResolver {
    pub fn new(bindings: Vec<HotkeyBinding>) -> Self {
        Self { bindings }
    }

    pub fn bindings(&self) -> &[HotkeyBinding] {
        &self.bindings
    }
}

impl HotkeyResolver for ComboHotkeyResolver {
    fn resolve(&self, held: &[RawInputEvent]) -> Option<SystemAction> {
        self.bindings
            .iter()
            .filter(|b| !b.trigger.is_empty())
            .filter(|b| b.trigger.iter().all(|t| held.contains(t)))
            .max_by_key(|b| b.trigger.len())
            .map(|b| b.action)
    }

    fn set_binding(&mut self, binding: HotkeyBinding) -> Result<(), String> {
        if binding.trigger.is_empty() {
            return Err("trigger vazio".into());
        }
        self.bindings.retain(|b| b.action != binding.action);
        self.bindings.push(binding);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(sc: u32) -> RawInputEvent {
        RawInputEvent::Keyboard { scancode: sc }
    }

    fn binding(action: SystemAction, trigger: Vec<RawInputEvent>) -> HotkeyBinding {
        HotkeyBinding {
            action,
            trigger,
            device_guid: None,
        }
    }

    #[test]
    fn single_key_binding() {
        let r =
            ComboHotkeyResolver::new(vec![binding(SystemAction::ToggleMenuOverlay, vec![key(1)])]);
        assert_eq!(r.resolve(&[key(1)]), Some(SystemAction::ToggleMenuOverlay));
        assert_eq!(r.resolve(&[key(2)]), None);
        assert_eq!(r.resolve(&[]), None);
    }

    #[test]
    fn combo_needs_all_events_held() {
        let r = ComboHotkeyResolver::new(vec![binding(
            SystemAction::QuickSave,
            vec![key(10), key(20)],
        )]);
        assert_eq!(r.resolve(&[key(10)]), None);
        assert_eq!(
            r.resolve(&[key(10), key(20)]),
            Some(SystemAction::QuickSave)
        );
        assert_eq!(
            r.resolve(&[key(20), key(10), key(99)]),
            Some(SystemAction::QuickSave)
        );
    }

    #[test]
    fn combo_beats_single_when_both_match() {
        let r = ComboHotkeyResolver::new(vec![
            binding(SystemAction::ToggleMenuOverlay, vec![key(10)]),
            binding(SystemAction::QuickSave, vec![key(10), key(20)]),
        ]);
        // só a tecla única
        assert_eq!(r.resolve(&[key(10)]), Some(SystemAction::ToggleMenuOverlay));
        // combo segurado -> o combo ganha
        assert_eq!(
            r.resolve(&[key(10), key(20)]),
            Some(SystemAction::QuickSave)
        );
    }

    #[test]
    fn set_binding_replaces_same_action() {
        let mut r = ComboHotkeyResolver::default();
        r.set_binding(binding(SystemAction::QuickLoad, vec![key(1)]))
            .unwrap();
        r.set_binding(binding(SystemAction::QuickLoad, vec![key(2)]))
            .unwrap();
        assert_eq!(r.bindings().len(), 1);
        assert_eq!(r.resolve(&[key(1)]), None);
        assert_eq!(r.resolve(&[key(2)]), Some(SystemAction::QuickLoad));
        assert!(r
            .set_binding(binding(SystemAction::QuickLoad, vec![]))
            .is_err());
    }
}
