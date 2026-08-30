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

/// Reempacota `src` (com `pitch` bytes por linha, formato `fmt`) num buffer
/// RGBA8 apertado `width*height*4`. CPU-side — os framebuffers de core são
/// pequenos (ex: 256x240). Consumido pela textura wgpu e pelo `<canvas>` do
/// shell.
pub fn to_rgba8(
    src: &[u8],
    width: u32,
    height: u32,
    pitch: u32,
    fmt: SoftwarePixelFormat,
) -> Vec<u8> {
    let (w, h, pitch) = (width as usize, height as usize, pitch as usize);
    let mut out = vec![0u8; w * h * 4];

    for y in 0..h {
        let Some(row) = src.get(y * pitch..) else {
            break;
        };
        let dst = &mut out[y * w * 4..];
        match fmt {
            SoftwarePixelFormat::Rgb565 => {
                for x in 0..w {
                    let p = u16::from_le_bytes([row[x * 2], row[x * 2 + 1]]);
                    let r = ((p >> 11) & 0x1f) as u32;
                    let g = ((p >> 5) & 0x3f) as u32;
                    let b = (p & 0x1f) as u32;
                    dst[x * 4] = ((r * 255 + 15) / 31) as u8;
                    dst[x * 4 + 1] = ((g * 255 + 31) / 63) as u8;
                    dst[x * 4 + 2] = ((b * 255 + 15) / 31) as u8;
                    dst[x * 4 + 3] = 255;
                }
            }
            SoftwarePixelFormat::Rgb1555 => {
                for x in 0..w {
                    let p = u16::from_le_bytes([row[x * 2], row[x * 2 + 1]]);
                    let r = ((p >> 10) & 0x1f) as u32;
                    let g = ((p >> 5) & 0x1f) as u32;
                    let b = (p & 0x1f) as u32;
                    dst[x * 4] = ((r * 255 + 15) / 31) as u8;
                    dst[x * 4 + 1] = ((g * 255 + 15) / 31) as u8;
                    dst[x * 4 + 2] = ((b * 255 + 15) / 31) as u8;
                    dst[x * 4 + 3] = 255;
                }
            }
            SoftwarePixelFormat::Xrgb8888 => {
                // u32 LE 0x00RRGGBB -> em memória: B, G, R, X
                for x in 0..w {
                    dst[x * 4] = row[x * 4 + 2];
                    dst[x * 4 + 1] = row[x * 4 + 1];
                    dst[x * 4 + 2] = row[x * 4];
                    dst[x * 4 + 3] = 255;
                }
            }
        }
    }
    out
}

pub trait FrameSource: Send {
    fn next_frame(&mut self) -> Option<Frame>;
}

#[cfg(test)]
mod to_rgba8_tests {
    use super::*;

    #[test]
    fn rgb565_pure_colors() {
        assert_eq!(
            to_rgba8(
                &0xF800u16.to_le_bytes(),
                1,
                1,
                2,
                SoftwarePixelFormat::Rgb565
            ),
            [255, 0, 0, 255]
        );
        assert_eq!(
            to_rgba8(
                &0x07E0u16.to_le_bytes(),
                1,
                1,
                2,
                SoftwarePixelFormat::Rgb565
            ),
            [0, 255, 0, 255]
        );
    }

    #[test]
    fn xrgb8888_channel_order() {
        let px = [0x33, 0x22, 0x11, 0x00];
        assert_eq!(
            to_rgba8(&px, 1, 1, 4, SoftwarePixelFormat::Xrgb8888),
            [0x11, 0x22, 0x33, 255]
        );
    }

    #[test]
    fn respects_pitch_padding() {
        let mut src = vec![0u8; 12];
        src[0..2].copy_from_slice(&0xF800u16.to_le_bytes());
        src[2..4].copy_from_slice(&0x001Fu16.to_le_bytes());
        src[6..8].copy_from_slice(&0x07E0u16.to_le_bytes());
        let out = to_rgba8(&src, 2, 2, 6, SoftwarePixelFormat::Rgb565);
        assert_eq!(&out[0..4], [255, 0, 0, 255]);
        assert_eq!(&out[4..8], [0, 0, 255, 255]);
        assert_eq!(&out[8..12], [0, 255, 0, 255]);
    }
}
