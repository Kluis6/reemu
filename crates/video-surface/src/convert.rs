//! Conversão dos formatos de pixel do libretro para RGBA8 (o que a textura
//! wgpu recebe). CPU-side por simplicidade — os framebuffers de core são
//! pequenos (ex: 256x240). Um caminho shader-side entra se virar gargalo.

use domain::frame_source::SoftwarePixelFormat;

/// Reempacota `src` (com `pitch` bytes por linha, formato `fmt`) em um buffer
/// RGBA8 apertado `width*height*4`.
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
        let row = &src[y * pitch..];
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
                // u32 little-endian 0x00RRGGBB -> em memória: B, G, R, X
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_pure_colors() {
        // 1 pixel vermelho puro (0xF800), pitch = 2
        let red = to_rgba8(
            &0xF800u16.to_le_bytes(),
            1,
            1,
            2,
            SoftwarePixelFormat::Rgb565,
        );
        assert_eq!(red, [255, 0, 0, 255]);

        let green = to_rgba8(
            &0x07E0u16.to_le_bytes(),
            1,
            1,
            2,
            SoftwarePixelFormat::Rgb565,
        );
        assert_eq!(green, [0, 255, 0, 255]);

        let blue = to_rgba8(
            &0x001Fu16.to_le_bytes(),
            1,
            1,
            2,
            SoftwarePixelFormat::Rgb565,
        );
        assert_eq!(blue, [0, 0, 255, 255]);
    }

    #[test]
    fn xrgb8888_channel_order() {
        // 0x00_11_22_33 (RR=11 GG=22 BB=33) -> memória 33 22 11 00
        let px = [0x33, 0x22, 0x11, 0x00];
        let out = to_rgba8(&px, 1, 1, 4, SoftwarePixelFormat::Xrgb8888);
        assert_eq!(out, [0x11, 0x22, 0x33, 255]);
    }

    #[test]
    fn respects_pitch_padding() {
        // 2x2 RGB565, pitch 6 (2 bytes de padding por linha)
        let mut src = vec![0u8; 12];
        src[0..2].copy_from_slice(&0xF800u16.to_le_bytes()); // (0,0) vermelho
        src[2..4].copy_from_slice(&0x001Fu16.to_le_bytes()); // (1,0) azul
        src[6..8].copy_from_slice(&0x07E0u16.to_le_bytes()); // (0,1) verde
        let out = to_rgba8(&src, 2, 2, 6, SoftwarePixelFormat::Rgb565);
        assert_eq!(&out[0..4], [255, 0, 0, 255]);
        assert_eq!(&out[4..8], [0, 0, 255, 255]);
        assert_eq!(&out[8..12], [0, 255, 0, 255]);
    }
}
