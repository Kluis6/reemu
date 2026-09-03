//! Crate `core-loader-desktop`: adapter que implementa
//! `domain::core_loader::CoreLoader` (e produz `LoadedCore`/`FrameSource`)
//! carregando cores libretro via `libloading`.
//!
//! Escopo: caminho **software-only** (`dlopen` → `retro_*` → `retro_run` →
//! `retro_video_refresh` buffer cru → `FrameSource`) **e HW render GL**
//! (`SET_HW_RENDER` context type OpenGL/GLES): contexto EGL offscreen em
//! `gl_context`, o core renderiza num FBO nosso e o frame sai por readback
//! (`glReadPixels`) — o interop zero-cópia Vulkan↔GL é o passo seguinte.
//! Vulkan por-core (etapa 12) segue recusado com `HwRenderUnsupported`.
//!
//! Limitação conhecida: a API libretro é **um core por processo** (os
//! callbacks C não têm ponteiro de contexto → estado global). O
//! `DesktopCoreLoader` impõe isso.

mod archive;
mod core;
mod coreopts;
mod discover;
mod dmabuf;
mod ffi_state;
mod gl_context;
mod input;
mod loader;
mod raw;
mod sys;

pub use crate::core::DesktopCore;
pub use crate::coreopts::{
    core_option_values, core_options, set_core_option, set_pending_core_option_values,
};
pub use crate::discover::{discover_cores, DiscoveredCore};
pub use crate::input::{libretro_joypad_id, retropad, RetroPadState};
pub use crate::loader::DesktopCoreLoader;

/// Caminho do core-fake em C (`fixtures/testcore.c`), compilado pelo build.rs.
/// Só pra testes (deste crate e do `emu-session`).
#[cfg(feature = "test-fixtures")]
pub fn testcore_path() -> &'static str {
    env!("REEMU_TESTCORE")
}
