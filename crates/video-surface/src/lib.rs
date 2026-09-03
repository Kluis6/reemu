//! `video-surface`: renderiza os frames do core (via `domain::frame_source`)
//! numa surface wgpu, fora da WebView.
//!
//! Escopo: caminho **software** (`FrameOrigin::SoftwareRawBuffer`) — sobe o
//! buffer cru numa textura e desenha um quad com letterbox. O caminho
//! hardware (textura GL/Vulkan vinda do core) é o passo 4 da etapa 02.
//!
//! Ver `examples/play.rs` pra um player standalone (winit + `emu-session`).

mod renderer;
mod window_target;

pub use domain::frame_source::to_rgba8;
pub use renderer::{create_device, create_device_with, Renderer};
pub use window_target::WindowTarget;

pub use raw_window_handle;
