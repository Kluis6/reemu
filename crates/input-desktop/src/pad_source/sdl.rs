//! Fonte SDL3 (Windows). Sem a fila de eventos do SDL: o `SDL_PollEvent`
//! "should only be called on the main thread" (wiki SDL3), e o poller roda
//! numa thread própria. Em vez disso, a cada rodada: `SDL_UpdateGamepads`
//! ("Manually pump gamepad updates if not using the loop"), lista de
//! controles, estado de cada botão/eixo — e os eventos saem da diferença com
//! a rodada anterior. Todas essas chamadas são "safe to call from any
//! thread" (`\threadsafety` em SDL_gamepad.h, SDL 3.4.18); só o `SDL_Init`
//! (e o `SDL_Quit` da troca de driver) fica na thread principal
//! ([`init_on_main_thread`], [`set_driver_on_main_thread`]).
//!
//! Troca de driver: os hints de XInput/DirectInput/WGI "should be set before
//! SDL is initialized" (SDL_hints.h), então a troca é `SDL_Quit` + hints +
//! `SDL_Init`. Que dá pra iniciar de novo depois do `SDL_Quit` a wiki não
//! diz; o fonte diz (SDL.c 3.4.18: o `SDL_Quit` zera a contagem dos
//! subsistemas — "we reset the subsystem refcount" — e chama
//! `SDL_QuitHints()`, por isso os hints são postos de novo a cada início).
//! Antes do `SDL_Quit`, o poller precisa ter fechado os controles dele: a
//! thread principal desliga [`READY`] e espera o poller anunciar em
//! [`PARKED`] que soltou tudo.

use super::{Axis, Button, Driver, PadEvent, PadId};
use sdl3_sys::error::SDL_GetError;
use sdl3_sys::gamepad::*;
use sdl3_sys::hints::{
    SDL_SetHint, SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, SDL_HINT_JOYSTICK_DIRECTINPUT,
    SDL_HINT_JOYSTICK_HIDAPI, SDL_HINT_JOYSTICK_RAWINPUT, SDL_HINT_JOYSTICK_WGI,
    SDL_HINT_XINPUT_ENABLED,
};
use sdl3_sys::init::{SDL_Init, SDL_Quit, SDL_INIT_GAMEPAD};
use sdl3_sys::joystick::{SDL_JoystickID, SDL_SetJoystickEventsEnabled};
use sdl3_sys::stdinc::SDL_free;
use std::collections::{HashMap, VecDeque};
use std::ffi::{c_char, CStr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// SDL iniciado e os controles podem ser usados.
static READY: AtomicBool = AtomicBool::new(false);
/// Sobe a cada início do SDL — um `Source` de uma geração antiga não usa mais
/// os handles que tinha.
static GEN: AtomicU64 = AtomicU64::new(0);
/// Geração em que o poller viu `READY` desligado e já fechou os controles.
static PARKED: AtomicU64 = AtomicU64::new(u64::MAX);
/// `Source`s vivos — sem nenhum, a troca não espera ninguém.
static SOURCES: AtomicUsize = AtomicUsize::new(0);
/// Driver do último início que deu certo — a troca volta pra ele se o novo
/// não iniciar.
static CURRENT: Mutex<Option<Driver>> = Mutex::new(None);

/// Quanto a troca de driver espera o poller soltar os controles (ele roda a
/// cada poucos ms; passar disso = algo travado, e a troca desiste).
const PARK_TIMEOUT: Duration = Duration::from_secs(1);

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

/// Hints de cada driver: (HIDAPI, XInput, DirectInput, WGI). RawInput fica
/// sempre no padrão "0". Padrões do SDL: HIDAPI "1", XInput "1",
/// DirectInput "1", WGI "0" (SDL_hints.h 3.4.18).
fn driver_hints(d: Driver) -> [(*const c_char, &'static CStr); 5] {
    let on = |b: bool| if b { c"1" } else { c"0" };
    let (hidapi, xinput, dinput, wgi) = match d {
        Driver::Auto => (true, true, true, false),
        Driver::XInput => (false, true, false, false),
        Driver::DirectInput => (false, false, true, false),
        Driver::Wgi => (false, false, false, true),
    };
    [
        (SDL_HINT_JOYSTICK_HIDAPI, on(hidapi)),
        (SDL_HINT_XINPUT_ENABLED, on(xinput)),
        (SDL_HINT_JOYSTICK_DIRECTINPUT, on(dinput)),
        (SDL_HINT_JOYSTICK_WGI, on(wgi)),
        (SDL_HINT_JOYSTICK_RAWINPUT, c"0"),
    ]
}

/// Hints + `SDL_Init(GAMEPAD)`. Só na thread principal.
fn start(driver: Driver) -> Result<(), String> {
    // SAFETY: strings C estáticas; chamadas de inicialização do SDL na thread
    // principal, como a documentação pede ("This function should only be
    // called on the main thread", `SDL_Init`).
    unsafe {
        // Sem janela do SDL ele se considera em segundo plano e, por padrão,
        // descarta o input: "0: Disable joystick & gamecontroller input events
        // when the application is in the background. (default)".
        SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, c"1".as_ptr());
        for (name, value) in driver_hints(driver) {
            SDL_SetHint(name, value.as_ptr());
        }
        if !SDL_Init(SDL_INIT_GAMEPAD) {
            return Err(sdl_error());
        }
        // Ninguém drena a fila de eventos (ver o cabeçalho do módulo): o
        // estado é lido direto, com `SDL_UpdateGamepads` por rodada.
        SDL_SetGamepadEventsEnabled(false);
        SDL_SetJoystickEventsEnabled(false);
    }
    *CURRENT.lock().unwrap_or_else(|p| p.into_inner()) = Some(driver);
    GEN.fetch_add(1, Ordering::SeqCst);
    READY.store(true, Ordering::SeqCst);
    log::info!("SDL: controles via SDL3 (driver {})", driver.key());
    Ok(())
}

/// Inicia o SDL com o driver salvo. Chamar uma vez, na thread principal,
/// antes de criar o [`Source`].
pub fn init_on_main_thread(driver: Driver) {
    if READY.load(Ordering::SeqCst) {
        return;
    }
    if let Err(e) = start(driver) {
        log::warn!("SDL: SDL_Init(GAMEPAD) falhou: {e} — sem controle");
    }
}

/// Troca o driver com o app aberto: o poller fecha os controles, o SDL
/// reinicia com os hints novos e o poller reabre o que achar. Na thread
/// principal (`SDL_Quit`/`SDL_Init`).
pub fn set_driver_on_main_thread(driver: Driver) -> Result<(), String> {
    let was_ready = READY.swap(false, Ordering::SeqCst);
    let gen = GEN.load(Ordering::SeqCst);
    if was_ready && SOURCES.load(Ordering::SeqCst) > 0 {
        let t = Instant::now();
        while PARKED.load(Ordering::SeqCst) != gen {
            if t.elapsed() > PARK_TIMEOUT {
                // Sem garantia de que os handles foram fechados: não derruba
                // o SDL embaixo deles.
                READY.store(true, Ordering::SeqCst);
                return Err("a leitura do controle não parou a tempo".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let previous = *CURRENT.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: thread principal; o poller já fechou todos os handles (esperado
    // acima) e não abre outros enquanto `READY` estiver desligado.
    unsafe { SDL_Quit() };
    start(driver).map_err(|e| {
        // Não deixa o app sem controle: volta pro driver que funcionava.
        if let Some(prev) = previous.filter(|p| *p != driver) {
            // SAFETY: idem — nada aberto, ainda na thread principal.
            unsafe { SDL_Quit() };
            if let Err(e2) = start(prev) {
                log::warn!("SDL: nem o driver anterior ({}) iniciou: {e2}", prev.key());
            }
        }
        format!("o SDL não iniciou com o driver {}: {e}", driver.key())
    })
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
    /// Geração do SDL dos handles em `pads` (ver [`GEN`]).
    gen: u64,
}

// Os `*mut SDL_Gamepad` só são usados pela thread dona do `Source`, e as
// funções do SDL que os recebem são "safe to call from any thread".
unsafe impl Send for Source {}

impl Source {
    /// Não falha: se o SDL não iniciou, a fonte fica sem controles até uma
    /// troca de driver dar certo.
    pub fn new() -> Result<Self, String> {
        SOURCES.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            pads: HashMap::new(),
            gone: HashMap::new(),
            queue: VecDeque::new(),
            gen: 0,
        })
    }

    /// Fecha todos os controles abertos (anunciando a saída de cada um).
    fn close_all(&mut self) {
        for (jid, pad) in self.pads.drain() {
            // SAFETY: handle aberto por nós, fechado uma vez só.
            unsafe { SDL_CloseGamepad(pad.handle) };
            self.gone.insert(jid, (pad.uuid, pad.name));
            self.queue.push_back((pid(jid), PadEvent::Disconnected));
        }
    }

    /// Lê o estado atual e enfileira o que mudou desde a rodada anterior.
    pub fn refresh(&mut self) {
        self.gone.clear();
        // Troca de driver em andamento: solta tudo e avisa a thread principal.
        if !READY.load(Ordering::SeqCst) {
            self.close_all();
            PARKED.store(GEN.load(Ordering::SeqCst), Ordering::SeqCst);
            return;
        }
        let gen = GEN.load(Ordering::SeqCst);
        if gen != self.gen {
            // SDL reiniciado: os handles antigos não valem mais (já fechados
            // na espera acima, a não ser na primeira rodada, sem nenhum).
            self.close_all();
            self.gen = gen;
        }
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
        let left: Vec<SDL_JoystickID> = self
            .pads
            .keys()
            .filter(|k| !ids.contains(k))
            .copied()
            .collect();
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
        // Só fecha se o SDL ainda é o mesmo dos handles.
        if READY.load(Ordering::SeqCst) && GEN.load(Ordering::SeqCst) == self.gen {
            for pad in self.pads.values() {
                // SAFETY: handles abertos por nós, nesta geração do SDL.
                unsafe { SDL_CloseGamepad(pad.handle) };
            }
        }
        SOURCES.fetch_sub(1, Ordering::SeqCst);
    }
}

fn pid(j: SDL_JoystickID) -> PadId {
    PadId(j.0 as u64)
}
