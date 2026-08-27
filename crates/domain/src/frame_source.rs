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

/// Formato dos pixels crus de um core software-only. Espelha
/// `retro_pixel_format` do libretro — a camada global converte pra textura.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoftwarePixelFormat {
    /// `RETRO_PIXEL_FORMAT_0RGB1555`, 16 bits, native endian (default libretro).
    Rgb1555,
    /// `RETRO_PIXEL_FORMAT_XRGB8888`, 32 bits.
    Xrgb8888,
    /// `RETRO_PIXEL_FORMAT_RGB565`, 16 bits.
    Rgb565,
}

impl SoftwarePixelFormat {
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            SoftwarePixelFormat::Rgb1555 | SoftwarePixelFormat::Rgb565 => 2,
            SoftwarePixelFormat::Xrgb8888 => 4,
        }
    }
}

/// Handle opaco pra uma textura GPU já pronta pro pipeline de pós-processamento
/// consumir. A implementação real (wgpu::Texture, GLuint, VkImage...) vive
/// inteiramente no adapter — o domínio só carrega o handle adiante.
pub trait GpuTextureHandle: Send {}

pub enum FrameOrigin {
    /// Core software-only: buffer de pixels crus que precisa ser subido
    /// pra uma textura pela camada global antes de entrar no pipeline.
    /// `pitch` é em bytes por linha (pode ser > width * bpp por padding).
    SoftwareRawBuffer {
        data: Vec<u8>,
        pitch: u32,
        format: SoftwarePixelFormat,
    },
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
