//! `input-desktop`: peças testáveis do input desktop.
//!
//! - `sdl_db` — parser do SDL_GameControllerDB (gamepad físico → RetroPad).
//! - `hotkeys` — `HotkeyResolver` com combinação (hold+press).
//! - `keymap` — teclado → RetroPad.
//!
//! - `pad_source` — de onde vêm os eventos de controle (SDL3 no Windows,
//!   `gilrs` nos outros); `gamepad` traduz pro RetroPad e pra navegação.

pub mod capture;
pub mod gamepad;
pub mod held;
pub mod hotkeys;
pub mod keymap;
pub mod mappings;
pub mod pad_source;
pub mod sdl_db;

pub use gamepad::{
    button_index, button_to_retropad, guid_hex, retropad_from_index, GamepadPoller, NavPulse,
    PollOutcome,
};
pub use hotkeys::ComboHotkeyResolver;
pub use sdl_db::{parse_db, parse_mapping, GamepadSource, ParsedMapping, SdlDbError};
