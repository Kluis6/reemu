//! Helpers de textura, tamanho de passe e mips.

use super::*;

pub(super) fn axis_size(scale: Scale, cur: u32, native: u32, viewport: u32) -> u32 {
    match scale {
        Scale::Source(m) => (cur as f32 * m).round().max(1.0) as u32,
        Scale::Absolute(px) => px.max(1),
        Scale::Viewport(m) => {
            let base = if viewport > 0 { viewport } else { native * 3 };
            (base as f32 * m).round().max(1.0) as u32
        }
    }
}

/// std140: `w, h, 1/w, 1/h`.
pub(super) fn size_vec(w: u32, h: u32) -> [f32; 4] {
    [
        w as f32,
        h as f32,
        1.0 / w.max(1) as f32,
        1.0 / h.max(1) as f32,
    ]
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_slang(
    layout: &UniformLayout,
    params: &HashMap<String, f32>,
    src: (u32, u32),
    out: (u32, u32),
    orig: (u32, u32),
    final_vp: (u32, u32),
    frame_count: u64,
    tex_sizes: &HashMap<String, (u32, u32)>,
) -> Vec<u8> {
    let mut b = vec![0u8; (layout.size as usize).max(16)];
    let put = |b: &mut [u8], off: usize, bytes: &[u8]| {
        if off + bytes.len() <= b.len() {
            b[off..off + bytes.len()].copy_from_slice(bytes);
        }
    };
    for f in &layout.fields {
        let o = f.offset as usize;
        match (f.name.as_str(), f.kind) {
            ("MVP", UniformFieldKind::Mat4) => {
                // ortho [0,1] → [-1,1], column-major
                #[rustfmt::skip]
                let m: [f32; 16] = [2.0,0.0,0.0,0.0, 0.0,2.0,0.0,0.0, 0.0,0.0,1.0,0.0, -1.0,-1.0,0.0,1.0];
                put(&mut b, o, f32s_bytes(&m));
            }
            ("SourceSize", _) => put(&mut b, o, f32s_bytes(&size_vec(src.0, src.1))),
            ("OriginalSize", _) => put(&mut b, o, f32s_bytes(&size_vec(orig.0, orig.1))),
            ("OutputSize", _) => put(&mut b, o, f32s_bytes(&size_vec(out.0, out.1))),
            ("FinalViewportSize", _) => {
                put(&mut b, o, f32s_bytes(&size_vec(final_vp.0, final_vp.1)))
            }
            ("FrameCount", UniformFieldKind::U32) => {
                put(&mut b, o, &(frame_count as u32).to_le_bytes())
            }
            ("FrameDirection", UniformFieldKind::I32) => put(&mut b, o, &1i32.to_le_bytes()),
            // `<Textura>Size` — history, PassOutput, feedback, LUTs, aliases.
            (name, _) if name.ends_with("Size") => {
                if let Some((w, h)) = tex_sizes.get(name.trim_end_matches("Size")) {
                    put(&mut b, o, f32s_bytes(&size_vec(*w, *h)));
                }
            }
            (name, UniformFieldKind::F32) => {
                if let Some(v) = params.get(name) {
                    put(&mut b, o, &v.to_le_bytes());
                }
            }
            _ => {}
        }
    }
    b
}

pub(super) fn new_tex(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    new_tex_fmt(device, w, h, FMT, usage)
}

pub(super) fn new_tex_fmt(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> (wgpu::Texture, wgpu::TextureView) {
    new_tex_fmt_mips(device, w, h, format, usage, 1)
}

/// Como `new_tex_fmt`, mas com `mip_level_count` explícito (>1 = aloca a
/// cadeia inteira; o conteúdo dos níveis >0 fica indefinido até alguém
/// preencher — `realize` faz isso na CPU pros LUTs, `generate_mips` faz via
/// blit na GPU pros alvos de passe).
pub(super) fn new_tex_fmt_mips(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    mip_level_count: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("etapa04 tex"),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_level_count.max(1),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let v = t.create_view(&wgpu::TextureViewDescriptor::default());
    (t, v)
}

/// `floor(log2(max(w,h))) + 1` — nº de níveis de uma cadeia de mip completa
/// (mesma fórmula do `glslang_num_miplevels` que o RetroArch usa).
pub(super) fn mip_level_count(w: u32, h: u32) -> u32 {
    32 - w.max(h).max(1).leading_zeros()
}

/// Downsample 2×2 (box filter) — aproxima o blit linear por nível que o
/// RetroArch faz na GPU (`vkCmdBlitImage` com `VK_FILTER_LINEAR`), mas
/// gerado na CPU: os LUTs só carregam 1× no load do preset, não por frame.
pub(super) fn downsample_rgba8(src: &[u8], w: u32, h: u32) -> (Vec<u8>, u32, u32) {
    let nw = (w / 2).max(1);
    let nh = (h / 2).max(1);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0u32; 4];
            for dy in 0..2 {
                for dx in 0..2 {
                    let sx = (x * 2 + dx).min(w - 1);
                    let sy = (y * 2 + dy).min(h - 1);
                    let i = ((sy * w + sx) * 4) as usize;
                    for (c, a) in acc.iter_mut().enumerate() {
                        *a += src[i + c] as u32;
                    }
                }
            }
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (acc[c] / 4) as u8;
            }
        }
    }
    (out, nw, nh)
}

/// `VkFormat` cru → `wgpu::TextureFormat` **amostrável direto** (`texture_from_raw`
/// só serve se a `VkImage` já é um formato que o wgpu conhece). `None` = precisa
/// de conversão antes (`VkBlit` faz `vkCmdBlitImage` pra RGBA8) — os formatos
/// packed 16-bit do PS1 (A1R5G5B5 = 8, R5G5B5A1 = 7, R5G6B5 = 4), que o wgpu
/// não tem.
pub(super) fn vk_format_to_wgpu(vk_format: u32) -> Option<wgpu::TextureFormat> {
    Some(match vk_format {
        37 => wgpu::TextureFormat::Rgba8Unorm, // R8G8B8A8_UNORM (vk_rendering, Beetle 32bpp)
        43 => wgpu::TextureFormat::Rgba8UnormSrgb, // R8G8B8A8_SRGB
        44 => wgpu::TextureFormat::Bgra8Unorm, // B8G8R8A8_UNORM
        50 => wgpu::TextureFormat::Bgra8UnormSrgb, // B8G8R8A8_SRGB
        64 => wgpu::TextureFormat::Rgba16Float, // R16G16B16A16_SFLOAT (Beetle HDR interno)
        97 => wgpu::TextureFormat::Rgba16Float, // (alias observado em drivers)
        // Packed 16-bit → `None`: o `bind_via_blit` converte com `vkCmdBlitImage`.
        4 | 6 | 7 | 8 => return None,
        other => {
            log::warn!("VkFormat {other} inesperado no scanout do core — assumindo Rgba8Unorm");
            wgpu::TextureFormat::Rgba8Unorm
        }
    })
}

pub(super) fn f32s_bytes(s: &[f32]) -> &[u8] {
    // SAFETY: `f32` não tem padding nem invariantes de bit.
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}
