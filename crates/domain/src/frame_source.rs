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
    /// RGBA8 já na ordem final de canal (R,G,B,A em memória). Não vem do
    /// libretro — é o que o readback de um FBO GL (`glReadPixels`) produz.
    Rgba8888,
}

impl SoftwarePixelFormat {
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            SoftwarePixelFormat::Rgb1555 | SoftwarePixelFormat::Rgb565 => 2,
            SoftwarePixelFormat::Xrgb8888 | SoftwarePixelFormat::Rgba8888 => 4,
        }
    }
}

/// Layout de um plano `dma_buf` RGBA8 pra importar como textura. Todos os
/// campos são inteiros crus — sem tipo de I/O de plataforma no domínio. O `fd`
/// tem **posse transferida** pra quem chama `take_plane` (fecha ao dropar).
#[derive(Debug)]
pub struct DmabufPlaneInfo {
    /// `RawFd` cru (Unix). Posse de quem recebe.
    pub fd: i32,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub offset: u32,
    /// `DRM_FORMAT_MOD_*` escolhido na alocação (0 = linear).
    pub modifier: u64,
    /// FourCC DRM (ex: `DRM_FORMAT_ABGR8888`).
    pub fourcc: u32,
}

/// Handle pra um frame de HW render já na GPU. A textura vive num ring de
/// slots do adapter; o pós-processamento importa o `dma_buf` uma vez por slot
/// e depois só referencia por índice. Impl real no adapter.
pub trait GpuTextureHandle: Send {
    /// Índice do slot no ring — o importador cacheia a textura wgpu por slot.
    fn slot(&self) -> u32;
    /// Plano `dma_buf` pra importar. `Some` só na 1ª vez que o slot aparece;
    /// depois `None` (a textura já está cacheada).
    fn take_plane(&self) -> Option<DmabufPlaneInfo>;
}

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
            SoftwarePixelFormat::Rgba8888 => {
                // já na ordem final — cópia direta da linha.
                dst[..w * 4].copy_from_slice(&row[..w * 4]);
            }
        }
    }
    out
}

/// Gira um buffer RGBA8 `w*h` por `degrees` (0/90/180/270, anti-horário — a
/// convenção do `SET_ROTATION` do libretro). Devolve `(rgba, w, h)` já com as
/// dimensões trocadas pra 90/270. `0` (ou valor inesperado) devolve o buffer
/// **sem copiar** (move) — é o caminho de 99% dos frames.
pub fn rotate_rgba(src: Vec<u8>, w: u32, h: u32, degrees: u16) -> (Vec<u8>, u32, u32) {
    let (wu, hu) = (w as usize, h as usize);
    if src.len() != wu * hu * 4 || !matches!(degrees, 90 | 180 | 270) {
        return (src, w, h);
    }
    let px = |x: usize, y: usize| {
        let i = (y * wu + x) * 4;
        [src[i], src[i + 1], src[i + 2], src[i + 3]]
    };
    let mut out = vec![0u8; src.len()];
    let (ow, oh) = match degrees {
        90 | 270 => (hu, wu),
        _ => (wu, hu),
    };
    for oy in 0..oh {
        for ox in 0..ow {
            let (sx, sy) = match degrees {
                // anti-horário: coluna de baixo → linha de cima
                90 => (wu - 1 - oy, ox),
                180 => (wu - 1 - ox, hu - 1 - oy),
                270 => (oy, hu - 1 - ox),
                _ => unreachable!(),
            };
            let o = (oy * ow + ox) * 4;
            out[o..o + 4].copy_from_slice(&px(sx, sy));
        }
    }
    (out, ow as u32, oh as u32)
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

    /// Rótula os 4 pixels de uma imagem 2×1 (A B, um por linha vira coluna).
    #[test]
    fn rotate_rgba_quarter_turns() {
        // 3 wide × 2 tall, valor de cada pixel = índice (0..5) no canal R.
        let mut src = vec![0u8; 3 * 2 * 4];
        for i in 0..6 {
            src[i * 4] = i as u8;
            src[i * 4 + 3] = 255;
        }
        // layout:  0 1 2
        //          3 4 5
        let r = |deg| rotate_rgba(src.clone(), 3, 2, deg);

        let (b0, w0, h0) = r(0); // sem rotação — devolve o buffer intacto
        assert_eq!((w0, h0), (3, 2));
        assert_eq!(b0, src);

        // 90° anti-horário → 2 wide × 3 tall:  2 5 / 1 4 / 0 3
        let (b90, w90, h90) = r(90);
        assert_eq!((w90, h90), (2, 3));
        assert_eq!([b90[0], b90[4], b90[8], b90[12]], [2, 5, 1, 4]);

        // 180° → 3×2:  5 4 3 / 2 1 0
        let (b180, w180, h180) = r(180);
        assert_eq!((w180, h180), (3, 2));
        assert_eq!([b180[0], b180[4], b180[8]], [5, 4, 3]);

        // 270° anti-horário → 2×3:  3 0 / 4 1 / 5 2
        let (b270, w270, h270) = r(270);
        assert_eq!((w270, h270), (2, 3));
        assert_eq!([b270[0], b270[4], b270[8], b270[12]], [3, 0, 4, 1]);
    }
}
