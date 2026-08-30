//! Overrides de mapeamento de controle carregados do DB (`controller_mappings`),
//! global e único — o `GamepadPoller` (thread de `emu-session`, sem acesso ao
//! banco) lê daqui. O app popula com `set()` no startup e sempre que o usuário
//! edita um binding.
//!
//! Sem entrada pro `guid` = o poller usa o mapa fixo do `gilrs` (que já
//! normaliza pelo SDL_GameControllerDB embutido).

use domain::input::{ControllerLayoutEntry, ControllerMapping, RawInputEvent, RetroPadButton};
use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock};

fn store() -> &'static RwLock<Vec<ControllerMapping>> {
    static MAPPINGS: OnceLock<RwLock<Vec<ControllerMapping>>> = OnceLock::new();
    MAPPINGS.get_or_init(|| RwLock::new(Vec::new()))
}

fn port_store() -> &'static RwLock<HashMap<String, usize>> {
    static PORTS: OnceLock<RwLock<HashMap<String, usize>>> = OnceLock::new();
    PORTS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Substitui a lista inteira (o app passa o resultado de `repo.list()`).
pub fn set(list: Vec<ControllerMapping>) {
    *store().write().unwrap_or_else(|p| p.into_inner()) = list;
}

/// Publica as atribuições fixas `guid` → porta (tabela `device_port_assignment`).
pub fn set_ports(map: HashMap<String, usize>) {
    *port_store().write().unwrap_or_else(|p| p.into_inner()) = map;
}

/// Porta fixa do `guid`, se o usuário atribuiu uma.
pub fn port_for(guid: &str) -> Option<usize> {
    port_store()
        .read()
        .unwrap_or_else(|p| p.into_inner())
        .get(guid)
        .copied()
}

/// `true` se algum `guid` tem override salvo (o poller pula a recomposição
/// custosa quando não há nenhum).
pub fn is_empty() -> bool {
    store().read().unwrap_or_else(|p| p.into_inner()).is_empty()
}

/// Índice do botão físico dentro de um `RawInputEvent::GamepadButton`.
fn button_index(ev: &RawInputEvent) -> Option<u32> {
    match ev {
        RawInputEvent::GamepadButton { index, .. } => Some(*index),
        _ => None,
    }
}

fn entry_matches(entry: &ControllerLayoutEntry, held: &[u32]) -> bool {
    let mut any = false;
    for ev in &entry.trigger {
        match button_index(ev) {
            Some(i) if held.contains(&i) => any = true,
            Some(_) => return false, // parte da combinação não está segurada
            None => return false,    // trigger não-gamepad num mapa de controle
        }
    }
    any
}

/// Botões RetroPad que devem estar pressionados no `guid`, dado o conjunto de
/// índices físicos segurados agora. `None` = sem override pra esse `guid`
/// (o caller cai no mapa fixo do `gilrs`).
pub fn resolve(guid: &str, held: &[u32]) -> Option<HashSet<RetroPadButton>> {
    let guard = store().read().unwrap_or_else(|p| p.into_inner());
    let mapping = guard.iter().find(|m| m.guid == guid)?;
    Some(
        mapping
            .layout
            .iter()
            .filter(|e| entry_matches(e, held))
            .map(|e| e.button)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::input::MappingSource;

    fn gp(index: u32) -> RawInputEvent {
        RawInputEvent::GamepadButton {
            device_guid: "g".into(),
            index,
        }
    }

    #[test]
    fn override_resolves_single_and_combo() {
        set(vec![ControllerMapping {
            guid: "g".into(),
            display_name: "x".into(),
            source: MappingSource::UserOverride,
            layout: vec![
                ControllerLayoutEntry {
                    trigger: vec![gp(0)],
                    button: RetroPadButton::A,
                },
                ControllerLayoutEntry {
                    trigger: vec![gp(10), gp(11)],
                    button: RetroPadButton::Start,
                },
            ],
        }]);

        assert_eq!(resolve("desconhecido", &[0]), None);
        assert_eq!(resolve("g", &[0]), Some(HashSet::from([RetroPadButton::A])));
        assert_eq!(resolve("g", &[10]), Some(HashSet::new())); // só metade do combo
        assert_eq!(
            resolve("g", &[10, 11]),
            Some(HashSet::from([RetroPadButton::Start]))
        );

        set(Vec::new());
        assert!(is_empty());
    }
}
