//! Fonte SDL3 (Windows). Sem a fila de eventos do SDL: o `SDL_PollEvent`
//! "should only be called on the main thread" (wiki SDL3), e o poller roda
//! numa thread própria. Em vez disso, a cada rodada: `SDL_UpdateGamepads`
//! ("Manually pump gamepad updates if not using the loop"), lista de
//! controles, estado de cada botão/eixo — e os eventos saem da diferença com
//! a rodada anterior. Todas essas chamadas são "safe to call from any
//! thread" (`\threadsafety` em SDL_gamepad.h, SDL 3.4.18); só o `SDL_Init`
//! fica na thread principal ([`init_on_main_thread`]).

use super::{Axis, Button, PadEvent, PadId};
use sdl3_sys::error::SDL_GetError;
use sdl3_sys::gamepad::*;
use sdl3_sys::hints::{SDL_SetHint, SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS};
use sdl3_sys::init::{SDL_Init, SDL_INIT_GAMEPAD};
use sdl3_sys::joystick::{SDL_JoystickID, SDL_SetJoystickEventsEnabled};
use sdl3_sys::stdinc::SDL_free;
use std::collections::{HashMap, VecDeque};
use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};

static READY: AtomicBool = AtomicBool::new(false);

/// Botões lidos a cada rodada → vocabulário comum. Gatilhos (L2/R2) não
/// estão aqui: no SDL são eixos (ver [`TRIGGERS`]).
const BUTTONS: [(SDL_GamepadButton, Button); 15] = [
    (SDL_GAMEPAD_BUTTON_SOUTH, Button::South),
    (SDL_GAMEPAD_BUTTON_EAST, Button::East),
    (SDL_GAMEPAD_BUTTON_WEST, Button::West),
    (SDL_GAMEPAD_BUTTON_NORTH, Button::North),
    (SDL_GAMEPAD_BUTTON_BACK, Button::Select),
    (SDL_GAMEPAD_BUTTON_GUIDE, Button::Mode),
    (SDL_GAMEPAD_BUTTON_START, Button::Start),
    (SDL_GAMEPAD_BUTTON_LEFT_STICK, Button::LeftThumb),
    (SDL_GAMEPAD_BUTTON_RIGHT_STICK, Button::RightThumb),
    (SDL_GAMEPAD_BUTTON_LEFT_SHOULDER, Button::LeftTrigger),
    (SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER, Button::RightTrigger),
    (SDL_GAMEPAD_BUTTON_DPAD_UP, Button::DPadUp),
    (SDL_GAMEPAD_BUTTON_DPAD_DOWN, Button::DPadDown),
    (SDL_GAMEPAD_BUTTON_DPAD_LEFT, Button::DPadLeft),
    (SDL_GAMEPAD_BUTTON_DPAD_RIGHT, Button::DPadRight),
];

/// Sticks: "-32768 (up/left) to 32767 (down/right)" (`SDL_GetGamepadAxis`).
/// O `bool` inverte o sinal pra convenção comum (Y+ = cima).
const STICKS: [(SDL_GamepadAxis, Axis, bool); 4] = [
    (SDL_GAMEPAD_AXIS_LEFTX, Axis::LeftStickX, false),
    (SDL_GAMEPAD_AXIS_LEFTY, Axis::LeftStickY, true),
    (SDL_GAMEPAD_AXIS_RIGHTX, Axis::RightStickX, false),
    (SDL_GAMEPAD_AXIS_RIGHTY, Axis::RightStickY, true),
];

/// Gatilhos: "range from 0 when released to 32767 when fully pressed".
const TRIGGERS: [(SDL_GamepadAxis, Button); 2] = [
    (SDL_GAMEPAD_AXIS_LEFT_TRIGGER, Button::LeftTrigger2),
    (SDL_GAMEPAD_AXIS_RIGHT_TRIGGER, Button::RightTrigger2),
];

fn sdl_error() -> String {
    // SAFETY: o SDL devolve uma string C válida (vazia se não há erro).
    unsafe { CStr::from_ptr(SDL_GetError()) }
        .to_string_lossy()
        .into_owned()
}

/// Inicia o subsistema de gamepad. Chamar UMA vez, na thread principal,
/// antes de criar o [`Source`]: "This function should only be called on the
/// main thread" (`SDL_Init`, SDL_init.h).
pub fn init_on_main_thread() {
    if READY.load(Ordering::Relaxed) {
        return;
    }
    // SAFETY: strings C estáticas; chamadas de inicialização do SDL na thread
    // principal, como a documentação pede.
    unsafe {
        // Sem janela do SDL ele se considera em segundo plano e, por padrão,
        // descarta o input: "0: Disable joystick & gamecontroller input events
        // when the application is in the background. (default)".
        SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr());
        if !SDL_Init(SDL_INIT_GAMEPAD) {
            log::warn!("SDL: SDL_Init(GAMEPAD) falhou: {} — sem controle", sdl_error());
            return;
        }
        // Ninguém drena a fila de eventos (ver o cabeçalho do módulo): o
        // estado é lido direto, com `SDL_UpdateGamepads` por rodada.
        SDL_SetGamepadEventsEnabled(false);
        SDL_SetJoystickEventsEnabled(false);
    }
    log::info!("SDL: controles via SDL3 (HIDAPI/XInput/RawInput)");
    READY.store(true, Ordering::Relaxed);
}

struct Pad {
    handle: *mut SDL_Gamepad,
    uuid: [u8; 16],
    name: String,
    buttons: [bool; BUTTONS.len()],
    sticks: [i16; STICKS.len()],
    triggers: [i16; TRIGGERS.len()],
}

pub struct Source {
    pads: HashMap<SDL_JoystickID, Pad>,
    /// Desconectados na última rodada — o poller ainda pede o uuid deles ao
    /// tratar o `Disconnected`.
    gone: HashMap<SDL_JoystickID, ([u8; 16], String)>,
    queue: VecDeque<(PadId, PadEvent)>,
}

// Os `*mut SDL_Gamepad` só são usados pela thread dona do `Source`, e as
// funções do SDL que os recebem são "safe to call from any thread".
unsafe impl Send for Source {}

impl Source {
    pub fn new() -> Result<Self, String> {
        if !READY.load(Ordering::Relaxed) {
            return Err("SDL não iniciado (init_on_main_thread)".into());
        }
        Ok(Self {
            pads: HashMap::new(),
            gone: HashMap::new(),
            queue: VecDeque::new(),
        })
    }

    /// Lê o estado atual e enfileira o que mudou desde a rodada anterior.
    pub fn refresh(&mut self) {
        self.gone.clear();
        // SAFETY: SDL iniciado (`new`); o array do `SDL_GetGamepads` é nosso
        // e vai pro `SDL_free` ("This should be freed with SDL_free()").
        let ids: Vec<SDL_JoystickID> = unsafe {
            SDL_UpdateGamepads();
            let mut n = 0;
            let p = SDL_GetGamepads(&mut n);
            if p.is_null() {
                Vec::new()
            } else {
                let v = std::slice::from_raw_parts(p, n.max(0) as usize).to_vec();
                SDL_free(p.cast());
                v
            }
        };

        // Saíram
        let left: Vec<SDL_JoystickID> =
            self.pads.keys().filter(|k| !ids.contains(k)).copied().collect();
        for jid in left {
            if let Some(pad) = self.pads.remove(&jid) {
                // SAFETY: handle aberto por nós, fechado uma vez só.
                unsafe { SDL_CloseGamepad(pad.handle) };
                self.gone.insert(jid, (pad.uuid, pad.name));
                self.queue.push_back((pid(jid), PadEvent::Disconnected));
            }
        }
        // Entraram
        for &jid in &ids {
            if self.pads.contains_key(&jid) {
                continue;
            }
            // SAFETY: `jid` veio do `SDL_GetGamepads` desta rodada.
            let handle = unsafe { SDL_OpenGamepad(jid) };
            if handle.is_null() {
                log::warn!("SDL: não abriu o controle {}: {}", jid.0, sdl_error());
                continue;
            }
            // SAFETY: idem; o nome é do SDL (copiado aqui), pode ser nulo.
            let (uuid, name) = unsafe {
                let name = SDL_GetGamepadNameForID(jid);
                (
                    SDL_GetGamepadGUIDForID(jid).data,
                    if name.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(name).to_string_lossy().into_owned()
                    },
                )
            };
            log::info!("SDL: controle conectado: {name}");
            self.pads.insert(
                jid,
                Pad {
                    handle,
                    uuid,
                    name,
                    buttons: [false; BUTTONS.len()],
                    sticks: [0; STICKS.len()],
                    triggers: [0; TRIGGERS.len()],
                },
            );
            self.queue.push_back((pid(jid), PadEvent::Connected));
        }
        // Estado
        for (&jid, pad) in self.pads.iter_mut() {
            let id = pid(jid);
            for (i, (sb, b)) in BUTTONS.iter().enumerate() {
                // SAFETY: handle aberto e vivo.
                let down = unsafe { SDL_GetGamepadButton(pad.handle, *sb) };
                if down != pad.buttons[i] {
                    pad.buttons[i] = down;
                    let ev = if down {
                        PadEvent::ButtonPressed(*b)
                    } else {
                        PadEvent::ButtonReleased(*b)
                    };
                    self.queue.push_back((id, ev));
                }
            }
            for (i, (sa, a, flip)) in STICKS.iter().enumerate() {
                // SAFETY: idem.
                let v = unsafe { SDL_GetGamepadAxis(pad.handle, *sa) };
                if v != pad.sticks[i] {
                    pad.sticks[i] = v;
                    let f = (v as f32 / 32767.0).clamp(-1.0, 1.0);
                    self.queue
                        .push_back((id, PadEvent::AxisChanged(*a, if *flip { -f } else { f })));
                }
            }
            for (i, (sa, b)) in TRIGGERS.iter().enumerate() {
                // SAFETY: idem.
                let v = unsafe { SDL_GetGamepadAxis(pad.handle, *sa) };
                if v != pad.triggers[i] {
                    pad.triggers[i] = v;
                    let f = (v as f32 / 32767.0).clamp(0.0, 1.0);
                    self.queue.push_back((id, PadEvent::ButtonChanged(*b, f)));
                }
            }
        }
    }

    pub fn next_event(&mut self) -> Option<(PadId, PadEvent)> {
        self.queue.pop_front()
    }

    pub fn uuid(&self, id: PadId) -> [u8; 16] {
        let jid = SDL_JoystickID(id.0 as u32);
        self.pads
            .get(&jid)
            .map(|p| p.uuid)
            .or_else(|| self.gone.get(&jid).map(|g| g.0))
            .unwrap_or_default()
    }

    pub fn name(&self, id: PadId) -> String {
        let jid = SDL_JoystickID(id.0 as u32);
        self.pads
            .get(&jid)
            .map(|p| p.name.clone())
            .or_else(|| self.gone.get(&jid).map(|g| g.1.clone()))
            .unwrap_or_default()
    }

    /// Conexões ativas agora.
    pub fn pads(&self) -> Vec<PadId> {
        self.pads.keys().map(|&j| pid(j)).collect()
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        for pad in self.pads.values() {
            // SAFETY: handles abertos por nós.
            unsafe { SDL_CloseGamepad(pad.handle) };
        }
    }
}

fn pid(j: SDL_JoystickID) -> PadId {
    PadId(j.0 as u64)
}
