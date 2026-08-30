//! Gamepad físico via `gilrs` (que já normaliza os botões usando o
//! SDL_GameControllerDB embutido). `GamepadPoller` é confinado a uma thread
//! — `gilrs::Gilrs` não é `Sync`.
//!
//! Escopo: botões digitais + d-pad → `RetroPadState`, primeira controle
//! conectada = porta 0, a próxima = porta 1, etc. Eixos analógicos e a UI de
//! binding vêm depois.

use crate::{capture, held, mappings};
use core_loader_desktop::RetroPadState;
use domain::input::{RawInputEvent, RetroPadButton};
use gilrs::{Axis, Button, Event, EventType, Gilrs};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Fora deste módulo do centro, o stick esquerdo conta como direção do d-pad.
const STICK_THRESHOLD: f32 = 0.5;

/// Navegação de menu: atraso antes de a direção segurada começar a repetir, e
/// intervalo entre repetições.
const NAV_REPEAT_DELAY: Duration = Duration::from_millis(380);
const NAV_REPEAT_EVERY: Duration = Duration::from_millis(150);

/// Pulso de navegação de menu derivado do gamepad (d-pad / stick esquerdo / A /
/// B), já com edge-detection e auto-repeat. O shell emite pro frontend como
/// evento `menu-nav` — a Gamepad API do WebKitGTK não enxerga o controle nesse
/// setup, então a navegação da UI passa por aqui.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavPulse {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    /* Y (North) = busca · Start (☰) = menu de contexto — como no dashboard Xbox */
    Search,
    Context,
}

/// Índices de d-pad (numeração de [`gilrs_button_index`]) que o stick esquerdo
/// implica agora, dado `(x, y)` já com deadzone aplicada pelo `gilrs`.
/// Convenção `gilrs`: Y positivo = cima.
fn stick_dpad(x: f32, y: f32) -> Vec<u32> {
    let mut v = Vec::new();
    if y >= STICK_THRESHOLD {
        v.push(15); // Up
    } else if y <= -STICK_THRESHOLD {
        v.push(16); // Down
    }
    if x <= -STICK_THRESHOLD {
        v.push(17); // Left
    } else if x >= STICK_THRESHOLD {
        v.push(18); // Right
    }
    v
}

/// `[u8;16]` (uuid do gamepad, via `gilrs`) → string hex minúscula, o
/// `device_guid` que vai em `RawInputEvent` e na tabela `controller_mappings`.
pub fn guid_hex(uuid: [u8; 16]) -> String {
    use std::fmt::Write;
    uuid.iter().fold(String::with_capacity(32), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Índice estável do botão `gilrs` (já normalizado pelo SDL_GameControllerDB)
/// para uso em `RawInputEvent::GamepadButton`. É a nossa própria numeração —
/// só precisa ser consistente entre captura e resolução.
pub fn gilrs_button_index(b: Button) -> u32 {
    match b {
        Button::South => 0,
        Button::East => 1,
        Button::North => 2,
        Button::West => 3,
        Button::C => 4,
        Button::Z => 5,
        Button::LeftTrigger => 6,
        Button::LeftTrigger2 => 7,
        Button::RightTrigger => 8,
        Button::RightTrigger2 => 9,
        Button::Select => 10,
        Button::Start => 11,
        Button::Mode => 12,
        Button::LeftThumb => 13,
        Button::RightThumb => 14,
        Button::DPadUp => 15,
        Button::DPadDown => 16,
        Button::DPadLeft => 17,
        Button::DPadRight => 18,
        _ => u32::MAX,
    }
}

/// Inverso de [`gilrs_button_index`] + [`gilrs_button_to_retropad`]: índice
/// físico → RetroPad no mapa fixo. `None` p/ `C`/`Z`/`Mode` (4/5/12).
pub fn retropad_from_index(i: u32) -> Option<RetroPadButton> {
    use RetroPadButton::*;
    Some(match i {
        0 => B,
        1 => A,
        2 => X,
        3 => Y,
        6 => L1,
        7 => L2,
        8 => R1,
        9 => R2,
        10 => Select,
        11 => Start,
        13 => L3,
        14 => R3,
        15 => Up,
        16 => Down,
        17 => Left,
        18 => Right,
        _ => return None,
    })
}

/// `gilrs::Button` (já normalizado) → RetroPad. `None` = não mapeia
/// (ex: `Mode`/`C`/`Z`). Convenção libretro: South→B, East→A, West→Y, North→X.
pub fn gilrs_button_to_retropad(b: Button) -> Option<RetroPadButton> {
    use RetroPadButton::*;
    Some(match b {
        Button::South => B,
        Button::East => A,
        Button::West => Y,
        Button::North => X,
        Button::LeftTrigger => L1,
        Button::RightTrigger => R1,
        Button::LeftTrigger2 => L2,
        Button::RightTrigger2 => R2,
        Button::LeftThumb => L3,
        Button::RightThumb => R3,
        Button::Select => Select,
        Button::Start => Start,
        Button::DPadUp => Up,
        Button::DPadDown => Down,
        Button::DPadLeft => Left,
        Button::DPadRight => Right,
        _ => return None,
    })
}

pub struct GamepadPoller {
    gilrs: Gilrs,
    /// uuid do gamepad → porta RetroPad (0..3). Atribuída na 1ª conexão.
    ports: HashMap<[u8; 16], usize>,
    next_port: usize,
    /// Botões físicos (numeração de [`gilrs_button_index`]) segurados agora,
    /// por gamepad — base pra recompor o RetroPad a cada evento.
    down: HashMap<[u8; 16], Vec<u32>>,
    /// Posição atual `(x, y)` do stick esquerdo, por gamepad — vira direção
    /// de d-pad em [`Self::held_indices`].
    stick: HashMap<[u8; 16], (f32, f32)>,
    /// Posição atual `(x, y)` do d-pad quando ele chega como eixo/hat (ex:
    /// DualSense) em vez de botões. Mesma convenção do stick (Y+ = cima).
    hat: HashMap<[u8; 16], (f32, f32)>,
    /// Último conjunto RetroPad aplicado por porta — pro diff de press/release.
    applied: HashMap<usize, HashSet<RetroPadButton>>,
    /// Auto-repeat da navegação de menu: direção segurada → instante do
    /// próximo pulso.
    nav_repeat: HashMap<NavPulse, Instant>,
    /// Estado anterior de A/B (confirm/back) — pulso só na borda de subida.
    nav_confirm_down: bool,
    nav_search_down: bool,
    nav_context_down: bool,
    nav_back_down: bool,
}

/// O que o poll observou de interessante além do RetroPad (pro caller
/// tratar hotkey de menu no gamepad, por ex.).
#[derive(Debug, Default, PartialEq)]
pub struct PollOutcome {
    /// Botão `Mode`/guide pressionado nesta rodada (candidato a "abrir menu").
    pub menu_pressed: bool,
    /// Eventos brutos capturados nesta rodada (só quando `capture::is_capturing()`);
    /// vão pro frontend em vez de irem pro RetroPad.
    pub captured: Vec<RawInputEvent>,
    /// Gamepads conectados agora: `(guid_hex, nome)`. Sempre preenchido.
    pub gamepads: Vec<(String, String)>,
    /// Pulsos de navegação de menu (d-pad/stick/A/B) desta rodada. O shell só
    /// age neles quando a UI está em menu (fora do jogo ou pausado).
    pub nav: Vec<NavPulse>,
}

impl GamepadPoller {
    pub fn new() -> Result<Self, Box<gilrs::Error>> {
        Ok(Self {
            gilrs: Gilrs::new().map_err(Box::new)?,
            ports: HashMap::new(),
            next_port: 0,
            down: HashMap::new(),
            stick: HashMap::new(),
            hat: HashMap::new(),
            applied: HashMap::new(),
            nav_repeat: HashMap::new(),
            nav_confirm_down: false,
            nav_search_down: false,
            nav_context_down: false,
            nav_back_down: false,
        })
    }

    /// Deriva os pulsos de navegação de menu do estado segurado (união de
    /// todos os gamepads): d-pad/stick → setas (com auto-repeat), A/B →
    /// confirm/back (borda de subida). Chamado uma vez por `poll`.
    fn nav_pulses(&mut self) -> Vec<NavPulse> {
        let uuids: Vec<[u8; 16]> = self.gilrs.gamepads().map(|(_, g)| g.uuid()).collect();
        let mut held: HashSet<u32> = HashSet::new();
        for uuid in uuids {
            held.extend(self.held_indices(uuid));
        }
        let mut out = Vec::new();
        let now = Instant::now();
        for (idx, dir) in [
            (15u32, NavPulse::Up),
            (16, NavPulse::Down),
            (17, NavPulse::Left),
            (18, NavPulse::Right),
        ] {
            if held.contains(&idx) {
                match self.nav_repeat.get(&dir) {
                    None => {
                        out.push(dir);
                        self.nav_repeat.insert(dir, now + NAV_REPEAT_DELAY);
                    }
                    Some(&next) if now >= next => {
                        out.push(dir);
                        self.nav_repeat.insert(dir, now + NAV_REPEAT_EVERY);
                    }
                    _ => {}
                }
            } else {
                self.nav_repeat.remove(&dir);
            }
        }
        let confirm = held.contains(&0);
        if confirm && !self.nav_confirm_down {
            out.push(NavPulse::Confirm);
        }
        self.nav_confirm_down = confirm;
        let back = held.contains(&1);
        if back && !self.nav_back_down {
            out.push(NavPulse::Back);
        }
        self.nav_back_down = back;

        let search = held.contains(&2); // North / Y
        if search && !self.nav_search_down {
            out.push(NavPulse::Search);
        }
        self.nav_search_down = search;
        let context = held.contains(&11); // Start / ☰
        if context && !self.nav_context_down {
            out.push(NavPulse::Context);
        }
        self.nav_context_down = context;
        out
    }

    /// Botões físicos segurados + direção do stick esquerdo + d-pad-como-eixo,
    /// para o `uuid`.
    fn held_indices(&self, uuid: [u8; 16]) -> Vec<u32> {
        let mut v = self.down.get(&uuid).cloned().unwrap_or_default();
        for pos in [self.stick.get(&uuid), self.hat.get(&uuid)]
            .into_iter()
            .flatten()
        {
            for i in stick_dpad(pos.0, pos.1) {
                if !v.contains(&i) {
                    v.push(i);
                }
            }
        }
        v
    }

    fn port_for(&mut self, uuid: [u8; 16]) -> usize {
        // Atribuição fixa do usuário (`device_port_assignment`) vence a ordem
        // de conexão.
        if let Some(p) = mappings::port_for(&guid_hex(uuid)) {
            self.ports.insert(uuid, p);
            return p;
        }
        let next = &mut self.next_port;
        *self.ports.entry(uuid).or_insert_with(|| {
            let p = (*next).min(3);
            *next += 1;
            p
        })
    }

    /// Recompõe o RetroPad da `port` a partir dos índices segurados: usa o
    /// override do `guid` (`mappings`) se houver, senão o mapa fixo do `gilrs`.
    /// Faz o diff contra o último estado aplicado (trata combinação e release).
    fn recompute(&mut self, uuid: [u8; 16], pad: &RetroPadState) {
        let port = self.port_for(uuid);
        let down = self.held_indices(uuid);
        let desired: HashSet<RetroPadButton> = mappings::resolve(&guid_hex(uuid), &down)
            .unwrap_or_else(|| {
                down.iter()
                    .filter_map(|i| retropad_from_index(*i))
                    .collect()
            });
        let prev = self.applied.entry(port).or_default();
        for b in prev.difference(&desired) {
            pad.set(port, *b, false);
        }
        for b in desired.difference(prev) {
            pad.set(port, *b, true);
        }
        *prev = desired;
    }

    /// Drena os eventos pendentes e reflete em `pad`. Chame ~120Hz.
    pub fn poll(&mut self, pad: &RetroPadState) -> PollOutcome {
        let mut out = PollOutcome::default();
        let capturing = capture::is_capturing();
        while let Some(Event { id, event, .. }) = self.gilrs.next_event() {
            let uuid = self.gilrs.gamepad(id).uuid();
            match event {
                EventType::Connected => {}
                EventType::Disconnected => {
                    self.down.remove(&uuid);
                    self.stick.remove(&uuid);
                    self.hat.remove(&uuid);
                    held::clear();
                    if let Some(port) = self.ports.get(&uuid).copied() {
                        for b in self.applied.remove(&port).unwrap_or_default() {
                            pad.set(port, b, false);
                        }
                    }
                }
                // Stick esquerdo e d-pad-como-eixo → d-pad (o `gilrs` já
                // aplicou deadzone). Só fora do modo de captura (só botão).
                EventType::AxisChanged(axis, value, _) if !capturing => {
                    match axis {
                        Axis::LeftStickX => self.stick.entry(uuid).or_insert((0.0, 0.0)).0 = value,
                        Axis::LeftStickY => self.stick.entry(uuid).or_insert((0.0, 0.0)).1 = value,
                        Axis::DPadX => self.hat.entry(uuid).or_insert((0.0, 0.0)).0 = value,
                        Axis::DPadY => self.hat.entry(uuid).or_insert((0.0, 0.0)).1 = value,
                        _ => continue,
                    }
                    self.recompute(uuid, pad);
                }
                EventType::ButtonPressed(btn, _) | EventType::ButtonReleased(btn, _) => {
                    let pressed = matches!(event, EventType::ButtonPressed(..));
                    let index = gilrs_button_index(btn);
                    if capturing {
                        // Em captura, só o press interessa (o frontend agrupa a
                        // combinação); nada vai pro RetroPad.
                        if pressed {
                            out.captured.push(RawInputEvent::GamepadButton {
                                device_guid: guid_hex(uuid),
                                index,
                            });
                        }
                        continue;
                    }
                    // Conjunto segurado — físico (recompor RetroPad) + global
                    // (`held`, pra resolução de hotkey de combinação).
                    let slot = self.down.entry(uuid).or_default();
                    slot.retain(|i| *i != index);
                    let ev = RawInputEvent::GamepadButton {
                        device_guid: guid_hex(uuid),
                        index,
                    };
                    if pressed {
                        slot.push(index);
                        held::press(ev);
                    } else {
                        held::release(&ev);
                    }
                    if btn == Button::Mode && pressed {
                        out.menu_pressed = true;
                    }
                    self.recompute(uuid, pad);
                }
                _ => {}
            }
        }
        out.nav = self.nav_pulses();
        out.gamepads = self
            .gilrs
            .gamepads()
            .map(|(_, g)| (guid_hex(g.uuid()), g.name().to_string()))
            .collect();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_mapping_libretro_convention() {
        assert_eq!(
            gilrs_button_to_retropad(Button::South),
            Some(RetroPadButton::B)
        );
        assert_eq!(
            gilrs_button_to_retropad(Button::East),
            Some(RetroPadButton::A)
        );
        assert_eq!(
            gilrs_button_to_retropad(Button::West),
            Some(RetroPadButton::Y)
        );
        assert_eq!(
            gilrs_button_to_retropad(Button::North),
            Some(RetroPadButton::X)
        );
        assert_eq!(
            gilrs_button_to_retropad(Button::DPadLeft),
            Some(RetroPadButton::Left)
        );
        assert_eq!(gilrs_button_to_retropad(Button::Mode), None);
        assert_eq!(gilrs_button_to_retropad(Button::C), None);
    }

    #[test]
    fn guid_hex_is_lowercase_32_chars() {
        let mut uuid = [0u8; 16];
        uuid[0] = 0x03;
        uuid[15] = 0xab;
        let s = guid_hex(uuid);
        assert_eq!(s.len(), 32);
        assert!(s.starts_with("03"));
        assert!(s.ends_with("ab"));
        assert_eq!(s, s.to_lowercase());
    }

    #[test]
    fn stick_maps_to_dpad_past_threshold() {
        assert_eq!(stick_dpad(0.0, 0.0), Vec::<u32>::new());
        assert_eq!(stick_dpad(0.2, -0.2), Vec::<u32>::new()); // dentro do limiar
        assert_eq!(stick_dpad(0.0, 1.0), vec![15]); // cima
        assert_eq!(stick_dpad(0.0, -1.0), vec![16]); // baixo
        assert_eq!(stick_dpad(-1.0, 0.0), vec![17]); // esquerda
        assert_eq!(stick_dpad(0.9, 0.9), vec![15, 18]); // diagonal cima-direita
                                                        // e o índice bate com o mapa fixo → RetroPad
        assert_eq!(retropad_from_index(15), Some(RetroPadButton::Up));
        assert_eq!(retropad_from_index(18), Some(RetroPadButton::Right));
    }

    #[test]
    fn button_index_is_distinct_for_known_buttons() {
        let known = [
            Button::South,
            Button::East,
            Button::North,
            Button::West,
            Button::Select,
            Button::Start,
            Button::DPadUp,
        ];
        let mut seen: Vec<u32> = known.iter().map(|b| gilrs_button_index(*b)).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), known.len());
    }
}
