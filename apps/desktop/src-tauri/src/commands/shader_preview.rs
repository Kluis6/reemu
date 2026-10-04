//! Preview de shader (Configurações › Vídeo): o preset ativo aplicado a uma
//! cena de exemplo desenhada pelo app (2D em `shader_preview_2d`, 3D em
//! `shader_preview_3d` — sem arte de terceiros), num
//! `FrameProcessor` só do preview — o do jogo não muda de preset nem de
//! viewport.

use super::*;
use domain::frame_source::{Frame, FrameMetadata, FrameOrigin, SoftwarePixelFormat};

/// Resolução da cena: 320×240, 4:3 como a maioria dos consoles 8/16 bits.
pub const PREVIEW_W: u32 = 320;
pub const PREVIEW_H: u32 = 240;

/// Cena escolhida na prévia: `"3d"` = paisagem 3D (`shader_preview_3d`);
/// qualquer outra coisa = a cena 2D de pixel art.
fn scene_pixels(scene: &str) -> Vec<u8> {
    if scene == "3d" {
        super::sample_scene_3d()
    } else {
        super::sample_scene()
    }
}

fn scene_frame(scene: &str) -> Frame {
    Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data: scene_pixels(scene),
            pitch: PREVIEW_W * 4,
            format: SoftwarePixelFormat::Xrgb8888,
        },
        metadata: FrameMetadata {
            native_width: PREVIEW_W,
            native_height: PREVIEW_H,
            aspect_ratio: PREVIEW_W as f32 / PREVIEW_H as f32,
            rotation_degrees: 0,
        },
    }
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(rgba).map_err(|e| e.to_string())?;
    wr.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

/// A cena original, sem shader (320×240, PNG) — o "antes" do comparador.
#[tauri::command]
pub fn shader_preview_source(scene: String) -> Result<tauri::ipc::Response, String> {
    let bgrx = scene_pixels(&scene);
    let mut rgba = Vec::with_capacity(bgrx.len());
    for p in bgrx.chunks_exact(4) {
        rgba.extend_from_slice(&[p[2], p[1], p[0], 0xFF]);
    }
    encode_png(PREVIEW_W, PREVIEW_H, &rgba).map(tauri::ipc::Response::new)
}

/// A cena com o preset ativo do jogo e os mesmos valores de parâmetro,
/// renderizada em `width`×`height` (PNG). `async` pra não travar a janela
/// enquanto um preset pesado compila.
#[tauri::command]
pub async fn render_shader_preview(
    state: State<'_, AppState>,
    width: u32,
    height: u32,
    scene: String,
) -> Result<tauri::ipc::Response, String> {
    let (source, params) = {
        let guard = state.gpu.lock().unwrap_or_else(|p| p.into_inner());
        let fp = guard.as_ref().ok_or("sem GPU — shader indisponível")?;
        let params: Vec<(String, f32)> = fp
            .shader_param_meta()
            .iter()
            .filter_map(|m| Some((m.name.clone(), fp.shader_param_value(&m.name)?)))
            .collect();
        (fp.preset_source().to_string(), params)
    };

    let mut guard = state
        .shader_preview
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if guard.is_none() {
        *guard = crate::gpu::FrameProcessor::new();
    }
    let fp = guard.as_mut().ok_or("sem GPU — shader indisponível")?;
    if fp.preset_source() != source {
        fp.set_preset(&source)?;
    }
    for (name, value) in &params {
        fp.set_shader_param(name, *value);
    }
    let (w, h, rgba) = fp
        .render_still(&scene_frame(&scene), width.clamp(64, 2048), height.clamp(48, 2048), 4)
        .ok_or("shader: o preview não gerou imagem")?;
    encode_png(w, h, &rgba).map(tauri::ipc::Response::new)
}
