//! `emu-session`: orquestra o core numa **thread dedicada** (libretro não é
//! thread-safe e `retro_run` tem que ficar sempre na mesma thread), com uma
//! API de comandos (`load`/`pause`/`save state`/...) e saída de frames+áudio
//! por buffers compartilhados. Mais a state machine de foco
//! (`domain::focus::FocusManager`) que pausa/resume o core na transição
//! `GameFocused <-> MenuFocused`.
//!
//! O shell Tauri (etapa 03) consome isto: a surface nativa lê
//! `take_latest_frame()`, o `AudioSink` (etapa 06) lê `drain_audio()`, e os
//! comandos vêm de `#[tauri::command]`.

mod focus;
mod session;

pub use focus::FocusController;
pub use session::{EmuSession, SessionConfig, SessionError, SessionState};

/// Estado do RetroPad — escreva aqui pra mandar input pro core.
pub use core_loader_desktop::{retropad, RetroPadState};
