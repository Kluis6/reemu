//! Fonte `gilrs` (Linux/macOS): repassa os eventos dele no vocabulário
//! comum. `gilrs::Gilrs` não é `Sync` — a fonte fica na thread do poller.

use super::{Axis, Button, PadEvent, PadId};
use gilrs::{EventType, GamepadId, Gilrs, GilrsBuilder};
use std::collections::HashMap;

/// Nada a preparar na thread principal.
pub fn init_on_main_thread() {}

pub struct Source {
    gilrs: Gilrs,
    /// `PadId` ↔ `GamepadId` (o `GamepadId` não expõe o número).
    ids: HashMap<PadId, GamepadId>,
}

impl Source {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            // Gatilho analógico vira botão a partir de 25% do curso (solta
            // abaixo de 15%). O padrão do gilrs é 75%/65% — no DualSense, de
            // curso longo, meia pressão não contava e o jogo não acelerava.
            gilrs: GilrsBuilder::new()
                .set_axis_to_btn(0.25, 0.15)
                .build()
                .map_err(|e| e.to_string())?,
            ids: HashMap::new(),
        })
    }

    /// O `gilrs` entrega os eventos pela fila dele: nada a ler antes.
    pub fn refresh(&mut self) {}

    pub fn next_event(&mut self) -> Option<(PadId, PadEvent)> {
        loop {
            let ev = self.gilrs.next_event()?;
            let id = PadId(usize::from(ev.id) as u64);
            self.ids.insert(id, ev.id);
            let event = match ev.event {
                EventType::Connected => PadEvent::Connected,
                EventType::Disconnected => PadEvent::Disconnected,
                EventType::AxisChanged(axis, v, _) => match map_axis(axis) {
                    Some(a) => PadEvent::AxisChanged(a, v),
                    None => continue,
                },
                EventType::ButtonChanged(btn, v, _) => match map_button(btn) {
                    Some(b) => PadEvent::ButtonChanged(b, v),
                    None => continue,
                },
                EventType::ButtonPressed(btn, _) => match map_button(btn) {
                    Some(b) => PadEvent::ButtonPressed(b),
                    None => continue,
                },
                EventType::ButtonReleased(btn, _) => match map_button(btn) {
                    Some(b) => PadEvent::ButtonReleased(b),
                    None => continue,
                },
                _ => continue,
            };
            return Some((id, event));
        }
    }

    pub fn uuid(&self, id: PadId) -> [u8; 16] {
        self.ids
            .get(&id)
            .map(|g| self.gilrs.gamepad(*g).uuid())
            .unwrap_or_default()
    }

    pub fn name(&self, id: PadId) -> String {
        self.ids
            .get(&id)
            .map(|g| self.gilrs.gamepad(*g).name().to_string())
            .unwrap_or_default()
    }

    /// Conexões ativas agora.
    pub fn pads(&self) -> Vec<PadId> {
        self.gilrs
            .gamepads()
            .map(|(g, _)| PadId(usize::from(g) as u64))
            .collect()
    }
}

fn map_axis(a: gilrs::Axis) -> Option<Axis> {
    Some(match a {
        gilrs::Axis::LeftStickX => Axis::LeftStickX,
        gilrs::Axis::LeftStickY => Axis::LeftStickY,
        gilrs::Axis::RightStickX => Axis::RightStickX,
        gilrs::Axis::RightStickY => Axis::RightStickY,
        gilrs::Axis::DPadX => Axis::DPadX,
        gilrs::Axis::DPadY => Axis::DPadY,
        _ => return None,
    })
}

fn map_button(b: gilrs::Button) -> Option<Button> {
    Some(match b {
        gilrs::Button::South => Button::South,
        gilrs::Button::East => Button::East,
        gilrs::Button::North => Button::North,
        gilrs::Button::West => Button::West,
        gilrs::Button::C => Button::C,
        gilrs::Button::Z => Button::Z,
        gilrs::Button::LeftTrigger => Button::LeftTrigger,
        gilrs::Button::LeftTrigger2 => Button::LeftTrigger2,
        gilrs::Button::RightTrigger => Button::RightTrigger,
        gilrs::Button::RightTrigger2 => Button::RightTrigger2,
        gilrs::Button::Select => Button::Select,
        gilrs::Button::Start => Button::Start,
        gilrs::Button::Mode => Button::Mode,
        gilrs::Button::LeftThumb => Button::LeftThumb,
        gilrs::Button::RightThumb => Button::RightThumb,
        gilrs::Button::DPadUp => Button::DPadUp,
        gilrs::Button::DPadDown => Button::DPadDown,
        gilrs::Button::DPadLeft => Button::DPadLeft,
        gilrs::Button::DPadRight => Button::DPadRight,
        _ => return None,
    })
}
