//! `emu-session`: orquestra o core num **processo filho descartável**
//! (`reemu-core-host`, um novo a cada `load` — ver `session.rs`), com uma
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

pub use core_loader_desktop::{AnalogState, RetroPadState};
/// Estado do RetroPad/analógico **deste processo** (o pai) — a thread de
/// gamepad e o teclado do shell escrevem aqui; a `EmuSession` manda o
/// snapshot pro processo filho por IPC. Não confundir com
/// `core_loader_desktop::retropad()`/`analog()`, que são os globais do
/// FILHO (lidos pelo `input_state_cb` do core).
pub use session::{analog, retropad};

/// Descoberta de cores instalados (`<dados>/cores/*_libretro.<suf>`) — probe
/// leve (dlopen sem `retro_init`), roda neste processo sem conflitar com o
/// core carregado no filho.
pub use core_loader_desktop::{discover_cores, DiscoveredCore};
