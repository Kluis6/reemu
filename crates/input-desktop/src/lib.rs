//! `input-desktop`: peças testáveis do input desktop.
//!
//! - `sdl_db` — parser do SDL_GameControllerDB (gamepad físico → RetroPad).
//! - `hotkeys` — `HotkeyResolver` com combinação (hold+press).
//! - `keymap` — teclado → RetroPad.
//!
//! A enumeração/poll de gamepad via `gilrs` e a ponte com o event loop do
//! shell entram depois (precisam de hardware pra validar de verdade).

pub mod hotkeys;
pub mod keymap;
pub mod sdl_db;

pub use hotkeys::ComboHotkeyResolver;
pub use keymap::KeyboardMap;
pub use sdl_db::{parse_db, parse_mapping, GamepadSource, ParsedMapping, SdlDbError};
