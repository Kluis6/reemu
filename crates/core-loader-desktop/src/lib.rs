//! Crate `core-loader-desktop`: adapter que implementa
//! `domain::core_loader::CoreLoader` (e produz `LoadedCore`/`FrameSource`)
//! carregando cores libretro via `libloading`.
//!
//! Escopo desta etapa (02): caminho **software-only** ponta a ponta —
//! `dlopen` → resolver `retro_*` → `retro_run` → `retro_video_refresh`
//! (buffer cru) → `FrameSource`. Detecção de HW render (`SET_HW_RENDER`) é
//! feita e persistida, mas a criação de contexto GL real (passo 4) e o
//! Vulkan por-core (etapa 12) ainda não existem — cores assim são
//! recusados com `CoreLoadError::HwRenderUnsupported`.
//!
//! Limitação conhecida: a API libretro é **um core por processo** (os
//! callbacks C não têm ponteiro de contexto → estado global). O
//! `DesktopCoreLoader` impõe isso.

mod core;
mod ffi_state;
mod loader;
mod raw;
mod sys;

pub use crate::core::DesktopCore;
pub use crate::loader::DesktopCoreLoader;

/// Caminho do core-fake em C (`fixtures/testcore.c`), compilado pelo build.rs.
/// Só pra testes (deste crate e do `emu-session`).
#[cfg(feature = "test-fixtures")]
pub fn testcore_path() -> &'static str {
    env!("REEMU_TESTCORE")
}
