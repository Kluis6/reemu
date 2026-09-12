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
mod vk_context;
mod vk_frame;
mod vk_sys;

pub use crate::core::DesktopCore;
pub use crate::coreopts::{
    core_option_values, core_options, set_core_option, set_pending_core_option_values,
};
pub use crate::discover::{discover_cores, DiscoveredCore};
pub use crate::input::{analog, libretro_joypad_id, retropad, AnalogState, RetroPadState};
pub use crate::loader::DesktopCoreLoader;

/// Redireciona o **stdout** do processo pra `/dev/null`, PRA SEMPRE — vários
/// cores (Beetle PSX HW: `[hdcache]`, `Creating shader module`…) spammam via
/// `printf` cru, fora do log callback do libretro, e não dá pra filtrar isso
/// do lado Rust. Nossos logs vão pro **stderr** (`env_logger`), que fica
/// intacto. `REEMU_CORE_STDOUT=1` desliga a supressão. Idempotente.
///
/// Só pro **processo filho** (`reemu-core-host`) — a única coisa que roda
/// nesse processo é o core, então silenciar pra sempre não custa nada. NO
/// PROCESSO PRINCIPAL (caminho Vulkan in-process, `emu-session::local_core`)
/// isso mataria o stdout do Tauri/webview/wgpu também — usar
/// `with_core_stdout_silenced` lá (mute só durante o load, restaura depois).
pub fn silence_core_stdout() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if std::env::var_os("REEMU_CORE_STDOUT").is_some() {
            return;
        }
        match rustix::fs::open(
            "/dev/null",
            rustix::fs::OFlags::WRONLY,
            rustix::fs::Mode::empty(),
        ) {
            Ok(devnull) => {
                if let Err(e) = rustix::stdio::dup2_stdout(&devnull) {
                    log::warn!("silence_core_stdout: dup2 falhou: {e}");
                }
            }
            Err(e) => log::warn!("silence_core_stdout: abrir /dev/null: {e}"),
        }
    });
}

/// Versão do `silence_core_stdout` pro caminho **in-process** (processo
/// principal): redireciona stdout pra `/dev/null` só durante `f()` e
/// restaura o fd original depois — não pode ser permanente aqui porque o
/// próprio Tauri/webview/wgpu também loga no stdout deste processo (matar
/// pra sempre calaria o app inteiro, não só o core). Chamar em volta do
/// `open_core`/`retro_load_game` (é aí que o Beetle spamma `[hdcache]`/
/// `Creating shader module`), não em volta do loop de frames.
///
/// `REEMU_CORE_STDOUT` (qualquer valor) desliga, igual `silence_core_stdout`.
/// Falha em qualquer etapa (dup/open/dup2) só loga um aviso e segue sem
/// silenciar — nunca quebra o load do core por causa disso.
pub fn with_core_stdout_silenced<T>(f: impl FnOnce() -> T) -> T {
    use std::os::fd::{AsFd, BorrowedFd};

    if std::env::var_os("REEMU_CORE_STDOUT").is_some() {
        return f();
    }
    // SAFETY: só empresta o fd 1 atual pra duplicar (`dup`) — não toma posse
    // dele, então não fecha nada aqui.
    let current_stdout = unsafe { BorrowedFd::borrow_raw(rustix::stdio::raw_stdout()) };
    let saved = match rustix::io::dup(current_stdout) {
        Ok(fd) => fd,
        Err(e) => {
            log::warn!("with_core_stdout_silenced: dup do stdout falhou: {e}");
            return f();
        }
    };
    let devnull = match rustix::fs::open(
        "/dev/null",
        rustix::fs::OFlags::WRONLY,
        rustix::fs::Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(e) => {
            log::warn!("with_core_stdout_silenced: abrir /dev/null: {e}");
            return f();
        }
    };
    if let Err(e) = rustix::stdio::dup2_stdout(&devnull) {
        log::warn!("with_core_stdout_silenced: dup2 pro /dev/null falhou: {e}");
        return f();
    }
    let result = f();
    if let Err(e) = rustix::stdio::dup2_stdout(saved.as_fd()) {
        log::warn!("with_core_stdout_silenced: restaurar stdout falhou: {e}");
    }
    result
}

/// Caminho do core-fake em C (`fixtures/testcore.c`), compilado pelo build.rs.
/// Só pra testes (deste crate e do `emu-session`).
#[cfg(feature = "test-fixtures")]
pub fn testcore_path() -> &'static str {
    env!("REEMU_TESTCORE")
}
