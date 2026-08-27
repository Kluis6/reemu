//! `FrameSource`: abstração que unifica a saída de vídeo de cores
//! software-only (buffer de pixels crus) e cores hardware-accelerated
//! (FBO/VkImage já renderizado). O pós-processamento (`shader_chain`,
//! `decoration`) consome só essa abstração e nunca precisa saber a origem.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FrameMetadata {
    pub native_width: u32,
    pub native_height: u32,
    pub aspect_ratio: f32,
    pub rotation_degrees: u16,
}

/// Handle opaco pra uma textura GPU já pronta pro pipeline de pós-processamento
/// consumir. A implementação real (wgpu::Texture, GLuint, VkImage...) vive
/// inteiramente no adapter — o domínio só carrega o handle adiante.
pub trait GpuTextureHandle: Send {}

pub enum FrameOrigin {
    /// Core software-only: buffer de pixels crus que precisa ser subido
    /// pra uma textura pela camada global antes de entrar no pipeline.
    SoftwareRawBuffer { data: Vec<u8>, pitch: u32 },
    /// Core hardware-accelerated: já entregou uma textura pronta via
    /// negociação de HW render (GL ou Vulkan).
    HardwareTexture(Box<dyn GpuTextureHandle>),
}

pub struct Frame {
    pub origin: FrameOrigin,
    pub metadata: FrameMetadata,
}

pub trait FrameSource: Send {
    fn next_frame(&mut self) -> Option<Frame>;
}
