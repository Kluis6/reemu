//! Cena 2D do preview de shader: fase de plataforma em pixel art ao pôr do
//! sol (imagem enviada pelo dono do projeto), em 320×240. Tem o que mais
//! mostra a diferença entre shaders: degradê com dithering, bordas duras,
//! contornos finos, texto pequeno e cores chapadas lado a lado.
//!
//! A original é 1536×1024 (3:2). Pra caber no 4:3 da cena sem cortar o HUD
//! nem achatar, a faixa de terra e água de baixo foi repetida até
//! 1536×1152, e a imagem reduzida 4,8× (filtro box) — a grade da arte é
//! ~320 px de largura, então a redução quase coincide com os pixels dela.

use super::shader_preview::{PREVIEW_H, PREVIEW_W};

/// A cena em PNG RGB 8 bits, 320×240, embutida no binário.
const SCENE_PNG: &[u8] = include_bytes!("../../assets/shader-preview-2d.png");

/// Cena 2D em XRGB8888 (B, G, R, X), 320×240.
pub fn sample_scene() -> Vec<u8> {
    let mut reader = png::Decoder::new(SCENE_PNG)
        .read_info()
        .expect("cena 2D: PNG embutido inválido");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .expect("cena 2D: PNG embutido inválido");
    assert_eq!(
        (info.width, info.height, info.bit_depth),
        (PREVIEW_W, PREVIEW_H, png::BitDepth::Eight),
        "cena 2D: o PNG precisa ser 320×240, 8 bits"
    );
    let channels = info.color_type.samples();
    let mut out = Vec::with_capacity((PREVIEW_W * PREVIEW_H * 4) as usize);
    for p in buf[..info.buffer_size()].chunks_exact(channels) {
        out.extend_from_slice(&[p[2], p[1], p[0], 0xFF]);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn sample_scene_has_the_expected_size() {
        let px = super::sample_scene();
        assert_eq!(px.len(), (super::PREVIEW_W * super::PREVIEW_H * 4) as usize);
    }
}
