//! Gamepad físico via `gilrs` (que já normaliza os botões usando o
//! SDL_GameControllerDB embutido). `GamepadPoller` é confinado a uma thread
//! — `gilrs::Gilrs` não é `Sync`.
//!
//! Escopo: botões digitais + d-pad → `RetroPadState`, primeira controle
//! conectada = porta 0, a próxima = porta 1, etc. Eixos analógicos e a UI de
//! binding vêm depois.

use crate::{capture, held, mappings};
use core_loader_desktop::{AnalogState, RetroPadState};
use domain::input::{RawInputEvent, RetroPadButton};
use gilrs::{Axis, Button, Event, EventType, GamepadId, Gilrs, GilrsBuilder};
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

/// `f32` normalizado do `gilrs` (`-1.0..=1.0`) → eixo libretro
/// (`-0x8000..=0x7fff`).
/// Zona morta RADIAL dos analógicos. O mapeamento SDL do `gilrs` não traz
/// zona morta pra todo controle (DualSense: `deadzone = 0.0`), e um stick
/// parado mandava (6%, 10%) pro jogo. Abaixo de `STICK_DEADZONE` do raio vira
/// zero; acima, reescala pra começar do 0 sem salto (sem perder a direção).
const STICK_DEADZONE: f32 = 0.15;

/// Gatilho analógico (L2/R2) com o "zero" calibrado. O DualSense medido no
/// app (hid-playstation, `ABS_Z`/`ABS_RZ` 0–255) ficava em 168/173 SOLTO —
/// ~66% do curso: com o limiar de 25% os dois gatilhos contavam como
/// apertados o tempo todo (no MSR: freio + acelerador juntos, o carro não
/// saía). O zero é o menor valor já visto; o aperto é medido a partir dele e
/// reescalado pra 0..1. Controle com zero em 0 (o normal) não muda nada.
#[derive(Debug, Clone, Copy)]
struct TriggerCal {
    rest: f32,
    pressed: bool,
    /// Pressão atual já calibrada, `0..=1`.
    level: f32,
}

/// Limiares de aperto do gatilho (sobre o valor já calibrado), com histerese.
const TRIGGER_PRESS: f32 = 0.25;
const TRIGGER_RELEASE: f32 = 0.15;
/// Abaixo disto a pressão analógica mandada ao core é 0.
const TRIGGER_DEADZONE: f32 = 0.04;

impl TriggerCal {
    /// 1º valor visto vira o zero — exceto 1.0 (gatilho digital apertado, ou
    /// o analógico já no fundo), que conta como zero em 0.
    fn new(first: f32) -> Self {
        Self {
            rest: if first >= 0.99 { 0.0 } else { first },
            pressed: false,
            level: 0.0,
        }
    }

    /// Atualiza com um valor novo; devolve `Some(apertado)` se o estado mudou.
    fn update(&mut self, value: f32) -> Option<bool> {
        self.rest = self.rest.min(value);
        let span = 1.0 - self.rest;
        let eff = if span < 0.05 {
            0.0
        } else {
            ((value - self.rest) / span).clamp(0.0, 1.0)
        };
        // zona morta curta: ruído perto do zero não vira pressão
        self.level = if eff < TRIGGER_DEADZONE { 0.0 } else { eff };
        let now = if self.pressed {
            eff > TRIGGER_RELEASE
        } else {
            eff >= TRIGGER_PRESS
        };
        (now != self.pressed).then(|| {
            self.pressed = now;
            now
        })
    }
}

fn is_analog_trigger(b: Button) -> bool {
    matches!(b, Button::LeftTrigger2 | Button::RightTrigger2)
}

fn radial_deadzone((x, y): (f32, f32)) -> (f32, f32) {
    let r = (x * x + y * y).sqrt();
    if r < STICK_DEADZONE {
        return (0.0, 0.0);
    }
    let scaled = ((r - STICK_DEADZONE) / (1.0 - STICK_DEADZONE)).min(1.0);
    (x / r * scaled, y / r * scaled)
}

fn to_axis(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
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
    /// `GamepadId` (a CONEXÃO física — não confundir com o GUID do
    /// SDL_GameControllerDB, que é por MODELO) → porta RetroPad (0..3).
    /// Atribuída na 1ª conexão. Duas unidades idênticas (mesmo GUID) têm
    /// `GamepadId`s distintos, então cada uma ganha a sua própria porta —
    /// indexar por GUID aqui fazia as duas colidirem na mesma porta e
    /// misturar os botões de uma no estado da outra.
    ports: HashMap<GamepadId, usize>,
    next_port: usize,
    /// Botões físicos (numeração de [`gilrs_button_index`]) segurados agora,
    /// por conexão física — base pra recompor o RetroPad a cada evento.
    down: HashMap<GamepadId, Vec<u32>>,
    /// Posição atual `(x, y)` do stick esquerdo, por conexão física — vira
    /// direção de d-pad em [`Self::held_indices`] (só quando o core não lê
    /// analógico) e vai sempre pro `RETRO_DEVICE_ANALOG` esquerdo.
    stick: HashMap<GamepadId, (f32, f32)>,
    /// Idem para o stick direito → `RETRO_DEVICE_ANALOG` direito.
    rstick: HashMap<GamepadId, (f32, f32)>,
    /// Posição atual `(x, y)` do d-pad quando ele chega como eixo/hat (ex:
    /// DualSense) em vez de botões. Mesma convenção do stick (Y+ = cima).
    hat: HashMap<GamepadId, (f32, f32)>,
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
    /// Zero calibrado de L2/R2 por conexão física (ver [`TriggerCal`]).
    triggers: HashMap<(GamepadId, Button), TriggerCal>,
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
    /// Gamepads que conectaram NESTA rodada: `(guid_hex, nome)`. O shell
    /// emite cada um pro frontend (toast + ícone na topbar).
    pub connected: Vec<(String, String)>,
    /// Guids que desconectaram NESTA rodada.
    pub disconnected: Vec<String>,
}

impl GamepadPoller {
    pub fn new() -> Result<Self, Box<gilrs::Error>> {
        Ok(Self {
            // Gatilho analógico vira botão a partir de 25% do curso (solta
            // abaixo de 15%). O padrão do gilrs é 75%/65% — no DualSense, de
            // curso longo, meia pressão não contava e o jogo não acelerava.
            gilrs: GilrsBuilder::new()
                .set_axis_to_btn(0.25, 0.15)
                .build()
                .map_err(Box::new)?,
            ports: HashMap::new(),
            next_port: 0,
            down: HashMap::new(),
            stick: HashMap::new(),
            rstick: HashMap::new(),
            hat: HashMap::new(),
            applied: HashMap::new(),
            nav_repeat: HashMap::new(),
            nav_confirm_down: false,
            nav_search_down: false,
            nav_context_down: false,
            nav_back_down: false,
            triggers: HashMap::new(),
        })
    }

    /// Deriva os pulsos de navegação de menu do estado segurado (união de
    /// todos os gamepads): d-pad/stick → setas (com auto-repeat), A/B →
    /// confirm/back (borda de subida). Chamado uma vez por `poll`.
    fn nav_pulses(&mut self) -> Vec<NavPulse> {
        let ids: Vec<GamepadId> = self.gilrs.gamepads().map(|(id, _)| id).collect();
        let mut held: HashSet<u32> = HashSet::new();
        for id in ids {
            // Navegação de menu: o stick esquerdo sempre conta como direção.
            held.extend(self.held_indices(id, true));
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

    /// Botões físicos segurados + d-pad-como-eixo (hat), para esta conexão
    /// (`GamepadId`). O stick esquerdo só entra como d-pad quando
    /// `stick_as_dpad` (menu, ou core sem analógico) — no jogo com analógico
    /// ele vai só pro `RETRO_DEVICE_ANALOG`.
    fn held_indices(&self, id: GamepadId, stick_as_dpad: bool) -> Vec<u32> {
        let mut v = self.down.get(&id).cloned().unwrap_or_default();
        let mut sticks = vec![self.hat.get(&id)];
        if stick_as_dpad {
            sticks.push(self.stick.get(&id));
        }
        for pos in sticks.into_iter().flatten() {
            for i in stick_dpad(pos.0, pos.1) {
                if !v.contains(&i) {
                    v.push(i);
                }
            }
        }
        v
    }

    /// Manda a posição atual dos dois sticks pro `RETRO_DEVICE_ANALOG` da porta.
    fn push_analog(&mut self, id: GamepadId, uuid: [u8; 16], analog: &AnalogState) {
        let port = self.port_for(id, uuid);
        let (lx, ly) = radial_deadzone(self.stick.get(&id).copied().unwrap_or((0.0, 0.0)));
        let (rx, ry) = radial_deadzone(self.rstick.get(&id).copied().unwrap_or((0.0, 0.0)));
        // `gilrs`: Y+ = cima; libretro: Y+ = baixo → inverte Y.
        analog.set_stick(port, 0, to_axis(lx), to_axis(-ly));
        analog.set_stick(port, 1, to_axis(rx), to_axis(-ry));
    }

    /// `id` identifica a conexão física (uma porta por `GamepadId`, mesmo se
    /// duas unidades tiverem o mesmo `uuid`/GUID); `uuid` só entra pra achar
    /// a atribuição FIXA salva pelo usuário (`device_port_assignment`), que é
    /// por modelo — duas unidades idênticas com override salvo ainda
    /// competem pela mesma porta preferida, mas isso é inerente ao GUID do
    /// SDL não ter número de série (mesma limitação do RetroArch).
    fn port_for(&mut self, id: GamepadId, uuid: [u8; 16]) -> usize {
        // Atribuição fixa do usuário vence a ordem de conexão.
        if let Some(p) = mappings::port_for(&guid_hex(uuid)) {
            self.ports.insert(id, p);
            return p;
        }
        let next = &mut self.next_port;
        *self.ports.entry(id).or_insert_with(|| {
            let p = (*next).min(3);
            *next += 1;
            p
        })
    }

    /// Recompõe o RetroPad da `port` a partir dos índices segurados: usa o
    /// override do `guid` (`mappings`) se houver, senão o mapa fixo do `gilrs`.
    /// Faz o diff contra o último estado aplicado (trata combinação e release).
    fn recompute(
        &mut self,
        id: GamepadId,
        uuid: [u8; 16],
        pad: &RetroPadState,
        analog: &AnalogState,
    ) {
        let port = self.port_for(id, uuid);
        // Core lendo analógico (N64…): o stick esquerdo não dobra como d-pad.
        let stick_as_dpad = !analog.is_used();
        let down = self.held_indices(id, stick_as_dpad);
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

    /// Drena os eventos pendentes e reflete em `pad`/`analog`. Chame ~120Hz.
    pub fn poll(&mut self, pad: &RetroPadState, analog: &AnalogState) -> PollOutcome {
        let mut out = PollOutcome::default();
        let capturing = capture::is_capturing();
        static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let debug = *DEBUG.get_or_init(|| std::env::var_os("REEMU_INPUT_DEBUG").is_some());
        while let Some(Event { id, event, .. }) = self.gilrs.next_event() {
            let uuid = self.gilrs.gamepad(id).uuid();
            // Diagnóstico: evento cru do gilrs (botão/eixo + valor + código
            // evdev) — pra ver o que um controle manda de verdade.
            if debug && !matches!(event, EventType::AxisChanged(_, v, _) if v.abs() < 0.05) {
                log::info!("controle {}: {event:?}", self.gilrs.gamepad(id).name());
            }
            match event {
                EventType::Connected => {
                    let name = self.gilrs.gamepad(id).name().to_string();
                    out.connected.push((guid_hex(uuid), name));
                }
                EventType::Disconnected => {
                    out.disconnected.push(guid_hex(uuid));
                    self.down.remove(&id);
                    self.stick.remove(&id);
                    self.rstick.remove(&id);
                    self.hat.remove(&id);
                    self.triggers.retain(|(g, _), _| *g != id);
                    if let Some(&port) = self.ports.get(&id) {
                        analog.set_stick(port, 0, 0, 0);
                        analog.set_stick(port, 1, 0, 0);
                        analog.set_triggers(port, 0, 0);
                    }
                    held::clear();
                    if let Some(port) = self.ports.get(&id).copied() {
                        for b in self.applied.remove(&port).unwrap_or_default() {
                            pad.set(port, b, false);
                        }
                    }
                }
                // Stick esquerdo e d-pad-como-eixo → d-pad (o `gilrs` já
                // aplicou deadzone). Só fora do modo de captura (só botão).
                EventType::AxisChanged(axis, value, _) if !capturing => {
                    match axis {
                        Axis::LeftStickX => self.stick.entry(id).or_insert((0.0, 0.0)).0 = value,
                        Axis::LeftStickY => self.stick.entry(id).or_insert((0.0, 0.0)).1 = value,
                        Axis::RightStickX => self.rstick.entry(id).or_insert((0.0, 0.0)).0 = value,
                        Axis::RightStickY => self.rstick.entry(id).or_insert((0.0, 0.0)).1 = value,
                        Axis::DPadX => self.hat.entry(id).or_insert((0.0, 0.0)).0 = value,
                        Axis::DPadY => self.hat.entry(id).or_insert((0.0, 0.0)).1 = value,
                        _ => continue,
                    }
                    self.push_analog(id, uuid, analog);
                    self.recompute(id, uuid, pad, analog);
                }
                // L2/R2: aperto decidido aqui, pelo valor com o zero calibrado
                // — o press/release do gilrs usa o valor cru.
                EventType::ButtonChanged(btn, value, _) if is_analog_trigger(btn) => {
                    let changed = match self.triggers.entry((id, btn)) {
                        std::collections::hash_map::Entry::Vacant(e) => {
                            e.insert(TriggerCal::new(value)).update(value)
                        }
                        std::collections::hash_map::Entry::Occupied(mut e) => {
                            e.get_mut().update(value)
                        }
                    };
                    if !capturing {
                        self.push_triggers(id, uuid, analog);
                    }
                    if let Some(pressed) = changed {
                        self.button_edge(id, uuid, btn, pressed, capturing, &mut out, pad, analog);
                    }
                }
                EventType::ButtonPressed(btn, _) | EventType::ButtonReleased(btn, _)
                    if is_analog_trigger(btn) => {}
                EventType::ButtonPressed(btn, _) | EventType::ButtonReleased(btn, _) => {
                    let pressed = matches!(event, EventType::ButtonPressed(..));
                    self.button_edge(id, uuid, btn, pressed, capturing, &mut out, pad, analog);
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

    /// Pressão calibrada de L2/R2 desta conexão → `analog` da porta dela.
    fn push_triggers(&mut self, id: GamepadId, uuid: [u8; 16], analog: &AnalogState) {
        let port = self.port_for(id, uuid);
        let level = |b| {
            self.triggers
                .get(&(id, b))
                .map_or(0, |t: &TriggerCal| (t.level * 32767.0).round() as u16)
        };
        analog.set_triggers(
            port,
            level(Button::LeftTrigger2),
            level(Button::RightTrigger2),
        );
    }

    /// Botão físico apertado/solto: captura de binding, conjunto segurado e
    /// recomposição do RetroPad.
    #[allow(clippy::too_many_arguments)]
    fn button_edge(
        &mut self,
        id: GamepadId,
        uuid: [u8; 16],
        btn: Button,
        pressed: bool,
        capturing: bool,
        out: &mut PollOutcome,
        pad: &RetroPadState,
        analog: &AnalogState,
    ) {
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
            return;
        }
        // Conjunto segurado — físico (recompor RetroPad) + global
        // (`held`, pra resolução de hotkey de combinação).
        let slot = self.down.entry(id).or_default();
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
        self.recompute(id, uuid, pad, analog);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stick_at_rest_is_zero_and_full_tilt_stays_full() {
        // o repouso do DualSense medido no app: (2048, -3328) / 32767
        assert_eq!(radial_deadzone((0.0625, -0.1016)), (0.0, 0.0));
        let (x, y) = radial_deadzone((1.0, 0.0));
        assert!((x - 1.0).abs() < 1e-6 && y == 0.0);
        // logo depois da borda da zona morta: pequeno, sem salto
        let (x, _) = radial_deadzone((0.16, 0.0));
        assert!(x > 0.0 && x < 0.02, "{x}");
    }

    #[test]
    fn trigger_with_offset_rest_is_not_pressed() {
        // DualSense medido: L2 solto em 168/255 com jitter de ±1
        let mut t = TriggerCal::new(168.0 / 255.0);
        assert_eq!(t.update(169.0 / 255.0), None);
        assert_eq!(t.update(167.0 / 255.0), None);
        // apertado até o fundo → conta; solto de volta → solta
        assert_eq!(t.update(1.0), Some(true));
        assert_eq!(t.update(168.0 / 255.0), Some(false));
    }

    #[test]
    fn trigger_level_is_calibrated_pressure() {
        let mut t = TriggerCal::new(0.0);
        t.update(0.02);
        assert_eq!(t.level, 0.0); // zona morta
        t.update(0.5);
        assert!((t.level - 0.5).abs() < 1e-6);
        let mut off = TriggerCal::new(0.6); // zero deslocado
        off.update(0.8);
        assert!((off.level - 0.5).abs() < 1e-3, "{}", off.level);
    }

    #[test]
    fn trigger_with_zero_rest_keeps_the_usual_threshold() {
        let mut t = TriggerCal::new(0.0);
        assert_eq!(t.update(0.2), None);
        assert_eq!(t.update(0.3), Some(true));
        assert_eq!(t.update(0.2), None); // histerese: só solta abaixo de 15%
        assert_eq!(t.update(0.1), Some(false));
        // gatilho digital: 1º evento já é o fundo → zero em 0
        let mut d = TriggerCal::new(1.0);
        assert_eq!(d.update(1.0), Some(true));
        assert_eq!(d.update(0.0), Some(false));
    }

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
