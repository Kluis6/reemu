//! Cena 3D do preview de shader: uma fase de aventura 3D no jeito dos jogos
//! de PS1/N64 (imagem enviada pelo dono do projeto), em 320×240 — texturas
//! de baixa resolução, polígonos facetados, neblina e o céu do pôr do sol,
//! o tipo de imagem que mais muda com shaders de CRT, suavização e
//! dithering.
//!
//! A original é 1448×1086, já em 4:3: só foi reduzida pra 320×240 (filtro
//! box, ~4,5×). Substitui o rasterizador em software que desenhava a cena
//! antes (no histórico do git, até 2026-10-20).

use super::shader_preview::{PREVIEW_H, PREVIEW_W};

/// A cena em PNG RGB 8 bits, 320×240, embutida no binário.
const SCENE_PNG: &[u8] = include_bytes!("../../assets/shader-preview-3d.png");

/// Cena 3D em XRGB8888 (B, G, R, X), 320×240.
pub fn sample_scene_3d() -> Vec<u8> {
    let mut reader = png::Decoder::new(SCENE_PNG)
        .read_info()
        .expect("cena 3D: PNG embutido inválido");
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .expect("cena 3D: PNG embutido inválido");
    assert_eq!(
        (info.width, info.height, info.bit_depth),
        (PREVIEW_W, PREVIEW_H, png::BitDepth::Eight),
        "cena 3D: o PNG precisa ser 320×240, 8 bits"
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
    fn sample_scene_3d_has_the_expected_size() {
        let px = super::sample_scene_3d();
        assert_eq!(px.len(), (super::PREVIEW_W * super::PREVIEW_H * 4) as usize);
    }
}
