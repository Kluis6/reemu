//! Preview de shader (Configurações › Vídeo): o preset ativo aplicado a uma
//! cena de pixel art desenhada aqui mesmo (sem arte de terceiros), num
//! `FrameProcessor` só do preview — o do jogo não muda de preset nem de
//! viewport.

use super::*;
use domain::frame_source::{Frame, FrameMetadata, FrameOrigin, SoftwarePixelFormat};

/// Resolução da cena: 320×240, 4:3 como a maioria dos consoles 8/16 bits.
pub const PREVIEW_W: u32 = 320;
pub const PREVIEW_H: u32 = 240;

type Rgb = (u8, u8, u8);

/// Fonte 3×5 só com as letras usadas.
fn glyph(c: char) -> [&'static str; 5] {
    match c {
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'E' => ["###", "#..", "##.", "#..", "###"],
        'M' => ["#.#", "###", "###", "#.#", "#.#"],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        _ => ["...", "...", "...", "...", "..."],
    }
}

/// Herói 12×16 (`.` = transparente): o robozinho do avatar do ReEmu.
const HERO: [&str; 16] = [
    ".....YY.....",
    ".....GG.....",
    "..kkkkkkkk..",
    ".kWWWWWWWWk.",
    ".kWDDDDDDWk.",
    ".kWDCDDCDWk.",
    ".kWDDDDDDWk.",
    ".kWWWWWWWWk.",
    "..kkkkkkkk..",
    ".kWWWWWWWWk.",
    "GkWWWRRWWWkG",
    "GkWWWRRWWWkG",
    ".kWWWWWWWWk.",
    "..kkkkkkkk..",
    "..kGGk.kGGk.",
    "..kkkk.kkkk.",
];

/// Baú 16×12.
const BLOCK: [&str; 12] = [
    "..kkkkkkkkkkkk..",
    ".kOOOOOOOOOOOOk.",
    "kOOOOOOOOOOOOOOk",
    "kOOOOOOOOOOOOOOk",
    "kYYYYYYYYYYYYYYk",
    "kOOOOOOYYOOOOOOk",
    "kOOOOOYkkYOOOOOk",
    "kOOOOOOYYOOOOOOk",
    "kOOOOOOOOOOOOOOk",
    "kYYYYYYYYYYYYYYk",
    "kOOOOOOOOOOOOOOk",
    "kkkkkkkkkkkkkkkk",
];

fn palette(c: char) -> Option<Rgb> {
    Some(match c {
        'W' => (240, 244, 248),
        'D' => (15, 23, 42),
        'C' => (34, 211, 238),
        'Y' => (252, 216, 60),
        'G' => (148, 163, 184),
        'R' => (244, 63, 94),
        'O' => (150, 82, 30),
        'k' => (24, 20, 28),
        _ => return None,
    })
}

/// Cena de teste em XRGB8888 (B, G, R, X): céu em faixas, sol, montanhas,
/// chão de tijolos, um robô, um baú, nuvem e o texto "REEMU" — bordas
/// duras, cores chapadas e degradê em faixas, o que mostra bem o efeito de
/// CRT, suavização e scanlines.
pub fn sample_scene() -> Vec<u8> {
    let (w, h) = (PREVIEW_W as i32, PREVIEW_H as i32);
    let mut px = vec![(0u8, 0u8, 0u8); (w * h) as usize];
    let mut put = |x: i32, y: i32, c: Rgb| {
        if (0..w).contains(&x) && (0..h).contains(&y) {
            px[(y * w + x) as usize] = c;
        }
    };

    // Céu: 10 faixas do azul-escuro ao laranja.
    let sky: [Rgb; 10] = [
        (24, 20, 72), (36, 28, 96), (52, 40, 120), (76, 52, 140), (108, 64, 152),
        (148, 76, 152), (188, 92, 140), (220, 116, 120), (240, 148, 104), (248, 184, 96),
    ];
    for y in 0..176 {
        let band = sky[(y * sky.len() as i32 / 176) as usize];
        for x in 0..w {
            put(x, y, band);
        }
    }
    // Estrelas no alto.
    for (x, y) in [(20, 30), (58, 12), (97, 40), (140, 18), (176, 34), (300, 14), (282, 46)] {
        put(x, y, (240, 240, 255));
    }
    // Sol com anel.
    for y in 40..110 {
        for x in 200..290 {
            let d = ((x - 245) * (x - 245) + (y - 75) * (y - 75)) as f32;
            if d < 22.0 * 22.0 {
                put(x, y, (252, 236, 140));
            } else if d < 26.0 * 26.0 && (x + y) % 2 == 0 {
                put(x, y, (252, 200, 110));
            }
        }
    }
    // Montanhas em duas camadas.
    for x in 0..w {
        let far = 120 + ((x as f32 / 28.0).sin() * 14.0 + (x as f32 / 9.0).sin() * 4.0) as i32;
        for y in far..176 {
            put(x, y, (92, 56, 120));
        }
        let near = 140 + (((x + 40) as f32 / 22.0).cos() * 12.0) as i32;
        for y in near..176 {
            put(x, y, if y == near { (60, 140, 92) } else { (40, 96, 72) });
        }
    }
    // Nuvem.
    for (cx, cy, r) in [(60, 64, 10), (74, 58, 13), (90, 64, 10)] {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                if (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r {
                    put(x, y, (248, 248, 252));
                }
            }
        }
    }
    // Chão: grama e tijolos 16×8 com rejunte.
    for y in 176..h {
        for x in 0..w {
            let c = if y < 180 {
                if y == 176 { (124, 220, 92) } else { (60, 168, 56) }
            } else {
                let row = (y - 180) / 8;
                let off = if row % 2 == 0 { 0 } else { 8 };
                let mortar = (y - 180) % 8 == 7 || (x + off) % 16 == 15;
                if mortar { (88, 40, 16) } else { (200, 96, 40) }
            };
            put(x, y, c);
        }
    }
    // Sprites.
    let mut sprite = |rows: &[&str], ox: i32, oy: i32, scale: i32| {
        for (j, row) in rows.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                if let Some(c) = palette(ch) {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            put(ox + i as i32 * scale + dx, oy + j as i32 * scale + dy, c);
                        }
                    }
                }
            }
        }
    };
    sprite(&BLOCK, 176, 152, 2);
    sprite(&HERO, 72, 144, 2);
    // "REEMU": letras 3×5 em blocos de 4 px, com sombra.
    for (k, ch) in "REEMU".chars().enumerate() {
        let g = glyph(ch);
        for (j, row) in g.iter().enumerate() {
            for (i, bit) in row.chars().enumerate() {
                if bit != '#' {
                    continue;
                }
                for dy in 0..4 {
                    for dx in 0..4 {
                        let x = 16 + k as i32 * 16 + i as i32 * 4 + dx;
                        let y = 16 + j as i32 * 4 + dy;
                        put(x + 2, y + 2, (20, 12, 40));
                        put(x, y, (255, 255, 255));
                    }
                }
            }
        }
    }

    let mut out = Vec::with_capacity(px.len() * 4);
    for (r, g, b) in px {
        out.extend_from_slice(&[b, g, r, 0xFF]);
    }
    out
}

fn scene_frame() -> Frame {
    Frame {
        origin: FrameOrigin::SoftwareRawBuffer {
            data: sample_scene(),
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
pub fn shader_preview_source() -> Result<tauri::ipc::Response, String> {
    let bgrx = sample_scene();
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
        .render_still(&scene_frame(), width.clamp(64, 2048), height.clamp(48, 2048), 4)
        .ok_or("shader: o preview não gerou imagem")?;
    encode_png(w, h, &rgba).map(tauri::ipc::Response::new)
}
