//! Cena 3D do preview de shader: um rasterizador de triângulos em software
//! (z-buffer, iluminação direcional, neblina) que desenha uma paisagem no
//! jeito dos jogos 3D de PS1/N64 — chão quadriculado em perspectiva com
//! sombras, paredes de tijolo, pilares, árvores, lago, pirâmide e montanhas
//! sob um céu com sol, com saída em 15 bits e dithering como no PS1. Arte gerada aqui, sem material
//! de terceiros. Mesma resolução da cena 2D (320×240).

use super::shader_preview::{PREVIEW_H, PREVIEW_W};

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}
fn mix(a: V3, b: V3, t: f32) -> V3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Como a superfície é pintada.
#[derive(Clone, Copy)]
enum Paint {
    Flat(V3),
    /// Chão: quadriculado de 1 m com rejunte, pelas coordenadas x/z do mundo.
    Floor,
    /// Parede de tijolos na cor dada (textura pelas coordenadas do mundo).
    Brick(V3),
    /// Lago: azul com o céu refletido e brilho em ondas.
    Water,
}

struct Tri {
    v: [V3; 3],
    paint: Paint,
}

fn quad(out: &mut Vec<Tri>, a: V3, b: V3, c: V3, d: V3, paint: Paint) {
    out.push(Tri { v: [a, b, c], paint });
    out.push(Tri { v: [a, c, d], paint });
}

/// Caixa alinhada aos eixos (sem a face de baixo).
fn boxy(out: &mut Vec<Tri>, min: V3, max: V3, p: Paint) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    quad(out, [x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0], p); // topo
    quad(out, [x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0], p); // frente
    quad(out, [x1, y0, z1], [x1, y1, z1], [x0, y1, z1], [x0, y0, z1], p); // trás
    quad(out, [x0, y0, z1], [x0, y1, z1], [x0, y1, z0], [x0, y0, z0], p); // esquerda
    quad(out, [x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1], p); // direita
}

fn pyramid(out: &mut Vec<Tri>, c: V3, half: f32, h: f32, color: V3) {
    let [cx, cy, cz] = c;
    let top = [cx, cy + h, cz];
    let a = [cx - half, cy, cz - half];
    let b = [cx + half, cy, cz - half];
    let d = [cx + half, cy, cz + half];
    let e = [cx - half, cy, cz + half];
    let p = Paint::Flat(color);
    for (s, t) in [(a, b), (b, d), (d, e), (e, a)] {
        out.push(Tri { v: [s, top, t], paint: p });
    }
}

/// Cone de `sides` lados (copa de árvore).
fn cone(out: &mut Vec<Tri>, c: V3, r: f32, h: f32, sides: usize, color: V3) {
    let top = [c[0], c[1] + h, c[2]];
    for k in 0..sides {
        let a0 = k as f32 / sides as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / sides as f32 * std::f32::consts::TAU;
        let p0 = [c[0] + r * a0.cos(), c[1], c[2] + r * a0.sin()];
        let p1 = [c[0] + r * a1.cos(), c[1], c[2] + r * a1.sin()];
        out.push(Tri { v: [p1, top, p0], paint: Paint::Flat(color) });
    }
}

fn tree(out: &mut Vec<Tri>, x: f32, z: f32, s: f32) {
    boxy(out, [x - 0.15 * s, 0.0, z - 0.15 * s], [x + 0.15 * s, 0.9 * s, z + 0.15 * s], Paint::Flat([0.42, 0.26, 0.14]));
    cone(out, [x, 0.7 * s, z], 1.0 * s, 1.6 * s, 7, [0.16, 0.48, 0.24]);
    cone(out, [x, 1.6 * s, z], 0.75 * s, 1.4 * s, 7, [0.20, 0.58, 0.28]);
}

/// Sombras no chão (x, z, raio): uma mancha escura sob cada objeto,
/// deslocada um pouco pro lado oposto da luz.
const SHADOWS: &[(f32, f32, f32)] = &[
    (-4.5, 5.6, 1.8),
    (-4.5, 8.0, 1.8),
    (-0.3, 5.7, 0.8),
    (6.9, 13.4, 1.8),
    (-7.7, 14.4, 1.5),
    (-10.6, 18.4, 1.8),
    (-6.7, 23.4, 1.5),
    (9.4, 16.4, 1.5),
    (12.6, 19.4, 1.8),
    (6.4, 21.4, 1.4),
];

fn scene() -> Vec<Tri> {
    let mut t = Vec::new();
    quad(
        &mut t,
        [-40.0, 0.0, -10.0],
        [-40.0, 0.0, 80.0],
        [40.0, 0.0, 80.0],
        [40.0, 0.0, -10.0],
        Paint::Floor,
    );
    // Lago à direita do caminho (um tico acima do chão pra ganhar no z).
    quad(
        &mut t,
        [3.2, 0.02, 3.0],
        [3.2, 0.02, 11.0],
        [14.0, 0.02, 11.0],
        [14.0, 0.02, 3.0],
        Paint::Water,
    );
    // Montanhas no horizonte.
    for (x, half, h, c) in [
        (-38.0, 16.0, 16.0, [0.42, 0.36, 0.56]),
        (-14.0, 20.0, 22.0, [0.48, 0.40, 0.62]),
        (14.0, 18.0, 18.0, [0.44, 0.38, 0.58]),
        (40.0, 16.0, 20.0, [0.46, 0.40, 0.60]),
    ] {
        pyramid(&mut t, [x, 0.0, 78.0], half, h, c);
    }
    // Degraus de tijolo à esquerda, pilares ao longo do caminho, pirâmide ao
    // fundo e uma torre de tijolos à direita.
    boxy(&mut t, [-6.0, 0.0, 4.0], [-3.5, 1.2, 6.5], Paint::Brick([0.86, 0.40, 0.26]));
    boxy(&mut t, [-6.0, 0.0, 6.5], [-3.5, 2.4, 9.0], Paint::Brick([0.90, 0.56, 0.24]));
    boxy(&mut t, [-1.0, 0.0, 5.0], [0.0, 1.0, 6.0], Paint::Flat([0.95, 0.80, 0.25]));
    for i in 0..5 {
        let z = 8.0 + i as f32 * 6.0;
        let stone = Paint::Brick([0.72, 0.74, 0.80]);
        boxy(&mut t, [-2.6, 0.0, z], [-2.0, 3.2, z + 0.6], stone);
        boxy(&mut t, [2.0, 0.0, z], [2.6, 3.2, z + 0.6], stone);
        boxy(&mut t, [-2.7, 3.2, z - 0.1], [-1.9, 3.5, z + 0.7], Paint::Flat([0.85, 0.86, 0.90]));
        boxy(&mut t, [1.9, 3.2, z - 0.1], [2.7, 3.5, z + 0.7], Paint::Flat([0.85, 0.86, 0.90]));
    }
    boxy(&mut t, [5.5, 0.0, 12.0], [7.8, 4.8, 14.3], Paint::Brick([0.36, 0.56, 0.86]));
    boxy(&mut t, [6.0, 4.8, 12.5], [7.3, 5.9, 13.8], Paint::Flat([0.95, 0.85, 0.30]));
    pyramid(&mut t, [-1.0, 0.0, 40.0], 7.0, 9.0, [0.88, 0.72, 0.46]);
    for (x, z, sc) in [(-8.0, 14.0, 1.4), (-11.0, 18.0, 1.7), (-7.0, 23.0, 1.4), (9.0, 16.0, 1.4), (12.0, 19.0, 1.7), (6.0, 21.0, 1.3)] {
        tree(&mut t, x, z, sc);
    }
    t
}

/// Ruído barato e estável por célula (variação de cor da grama/pedra).
fn hash(x: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (z as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h & 0xffff) as f32 / 65535.0
}

fn brick_color(base: V3, u: f32, v: f32) -> V3 {
    // tijolos de 0,5 × 0,25 m, fileiras alternadas
    let row = (v / 0.25).floor();
    let uu = u + if row as i32 % 2 == 0 { 0.0 } else { 0.25 };
    let (fu, fv) = ((uu / 0.5).fract().abs(), (v / 0.25).fract().abs());
    if fu < 0.06 || fv < 0.12 {
        return [base[0] * 0.45, base[1] * 0.45, base[2] * 0.45];
    }
    let k = 0.85 + 0.25 * hash((uu / 0.5).floor() as i32, row as i32);
    [base[0] * k, base[1] * k, base[2] * k]
}

fn floor_color(x: f32, z: f32) -> V3 {
    let (fx, fz) = (x.rem_euclid(1.0), z.rem_euclid(1.0));
    if fx < 0.05 || fz < 0.05 {
        // rejunte: escuro no caminho de pedra, só um tom abaixo na grama
        return if x.abs() < 1.5 { [0.30, 0.29, 0.28] } else { [0.21, 0.44, 0.22] };
    }
    // caminho de pedra no meio, grama quadriculada dos lados
    let n = 0.9 + 0.2 * hash((x * 3.0).floor() as i32, (z * 3.0).floor() as i32);
    let c = if x.abs() < 1.5 {
        if (x.floor() as i32 + z.floor() as i32) % 2 == 0 {
            [0.62, 0.60, 0.58]
        } else {
            [0.52, 0.50, 0.49]
        }
    } else if (x.floor() as i32 + z.floor() as i32).rem_euclid(2) == 0 {
        [0.30, 0.62, 0.30]
    } else {
        [0.24, 0.52, 0.26]
    };
    let mut k = n;
    for &(sx, sz, r) in SHADOWS {
        let d = ((x - sx) * (x - sx) + (z - sz) * (z - sz)).sqrt() / r;
        if d < 1.0 {
            k *= 0.55 + 0.45 * d * d;
        }
    }
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Câmera: posição, inclinação para baixo e campo de visão vertical.
const EYE: V3 = [0.0, 2.6, -6.0];
const PITCH: f32 = 0.16;
const FOV_Y: f32 = 1.05; // ~60°
const NEAR: f32 = 0.1;

const SKY_TOP: V3 = [0.16, 0.30, 0.62];
const SKY_HORIZON: V3 = [0.92, 0.72, 0.58];
const SUN_DIR: V3 = [-0.35, 0.18, 0.92];

fn to_camera(p: V3) -> V3 {
    let d = sub(p, EYE);
    let (s, c) = PITCH.sin_cos();
    // gira em torno de X pra olhar um pouco pra baixo
    [d[0], d[1] * c + d[2] * s, -d[1] * s + d[2] * c]
}

/// Recorta o triângulo (em espaço de câmera) no plano z = NEAR.
fn clip_near(v: [V3; 3]) -> Vec<V3> {
    let mut out = Vec::with_capacity(4);
    for i in 0..3 {
        let a = v[i];
        let b = v[(i + 1) % 3];
        let (ina, inb) = (a[2] >= NEAR, b[2] >= NEAR);
        if ina {
            out.push(a);
        }
        if ina != inb {
            let t = (NEAR - a[2]) / (b[2] - a[2]);
            out.push(mix(a, b, t));
        }
    }
    out
}

fn sky(dir_world_y: f32, dir: V3) -> V3 {
    let t = (dir_world_y * 3.0).clamp(0.0, 1.0).powf(0.7);
    let mut c = mix(SKY_HORIZON, SKY_TOP, t);
    let s = dot(norm(dir), norm(SUN_DIR));
    if s > 0.9985 {
        c = [1.0, 0.95, 0.75];
    } else if s > 0.995 {
        c = mix(c, [1.0, 0.85, 0.55], 0.6);
    }
    c
}

/// Cena 3D em XRGB8888 (B, G, R, X), 320×240.
pub fn sample_scene_3d() -> Vec<u8> {
    let (w, h) = (PREVIEW_W as usize, PREVIEW_H as usize);
    let f = (h as f32 / 2.0) / (FOV_Y / 2.0).tan();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let (ps, pc) = PITCH.sin_cos();

    // Céu pixel a pixel (direção do raio de volta pro mundo).
    let mut color = vec![[0f32; 3]; w * h];
    for y in 0..h {
        for x in 0..w {
            let dc = [(x as f32 - cx) / f, -(y as f32 - cy) / f, 1.0];
            // desfaz o giro da câmera
            let dw = [dc[0], dc[1] * pc - dc[2] * ps, dc[1] * ps + dc[2] * pc];
            color[y * w + x] = sky(norm(dw)[1], dw);
        }
    }

    let light = norm([-0.45, 0.85, -0.35]);
    let mut depth = vec![f32::INFINITY; w * h];
    for tri in scene() {
        let n = norm(cross(sub(tri.v[1], tri.v[0]), sub(tri.v[2], tri.v[0])));
        let shade = 0.38 + 0.62 * dot(n, light).max(0.0);
        let cam = [to_camera(tri.v[0]), to_camera(tri.v[1]), to_camera(tri.v[2])];
        let poly = clip_near(cam);
        if poly.len() < 3 {
            continue;
        }
        // cada vértice: tela (x, y), 1/z e a posição no mundo / z (pra
        // textura com perspectiva correta)
        let proj: Vec<(f32, f32, f32, V3)> = poly
            .iter()
            .map(|p| {
                let iz = 1.0 / p[2];
                // volta pro mundo só pra textura do chão
                let wx = p[0] + EYE[0];
                let wy = (p[1] * pc - p[2] * ps) + EYE[1];
                let wz = (p[1] * ps + p[2] * pc) + EYE[2];
                (cx + f * p[0] * iz, cy - f * p[1] * iz, iz, [wx * iz, wy * iz, wz * iz])
            })
            .collect();
        for k in 1..proj.len() - 1 {
            let (a, b, c) = (proj[0], proj[k], proj[k + 1]);
            let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            if area.abs() < 1e-6 {
                continue;
            }
            let minx = a.0.min(b.0).min(c.0).floor().max(0.0) as usize;
            let maxx = a.0.max(b.0).max(c.0).ceil().min(w as f32 - 1.0) as usize;
            let miny = a.1.min(b.1).min(c.1).floor().max(0.0) as usize;
            let maxy = a.1.max(b.1).max(c.1).ceil().min(h as f32 - 1.0) as usize;
            for y in miny..=maxy {
                for x in minx..=maxx {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) / area;
                    let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let iz = w0 * a.2 + w1 * b.2 + w2 * c.2;
                    let z = 1.0 / iz;
                    let i = y * w + x;
                    if z >= depth[i] {
                        continue;
                    }
                    depth[i] = z;
                    let wx = (w0 * a.3[0] + w1 * b.3[0] + w2 * c.3[0]) / iz;
                    let wy = (w0 * a.3[1] + w1 * b.3[1] + w2 * c.3[1]) / iz;
                    let wz = (w0 * a.3[2] + w1 * b.3[2] + w2 * c.3[2]) / iz;
                    let base = match tri.paint {
                        Paint::Flat(col) => col,
                        Paint::Floor => floor_color(wx, wz),
                        Paint::Brick(col) => {
                            if n[1].abs() > 0.5 {
                                col
                            } else if n[0].abs() > 0.5 {
                                brick_color(col, wz, wy)
                            } else {
                                brick_color(col, wx, wy)
                            }
                        }
                        Paint::Water => {
                            let reflect = mix([0.10, 0.22, 0.48], SKY_HORIZON, (z / 40.0).clamp(0.0, 0.7));
                            let wave = ((wz * 5.0 + (wx * 1.7).sin() * 2.0).sin() * 0.5 + 0.5).powf(8.0);
                            mix(reflect, [0.95, 0.90, 0.80], wave * 0.6)
                        }
                    };
                    let k = if matches!(tri.paint, Paint::Water) { 1.0 } else { shade };
                    let lit = [base[0] * k, base[1] * k, base[2] * k];
                    // neblina na direção do horizonte
                    let fog = ((z - 10.0) / 50.0).clamp(0.0, 0.62);
                    color[i] = mix(lit, SKY_HORIZON, fog);
                }
            }
        }
    }

    // Saída em 15 bits (5 por canal) com dithering ordenado 4×4 — o jeito
    // do PS1, que deixa o degradê do céu e a neblina "granulados".
    const BAYER: [[f32; 4]; 4] = [
        [0.0, 8.0, 2.0, 10.0],
        [12.0, 4.0, 14.0, 6.0],
        [3.0, 11.0, 1.0, 9.0],
        [15.0, 7.0, 13.0, 5.0],
    ];
    let mut out = Vec::with_capacity(w * h * 4);
    for (i, c) in color.iter().enumerate() {
        let (x, y) = (i % w, i / w);
        let d = (BAYER[y & 3][x & 3] + 0.5) / 16.0 - 0.5;
        let q = |v: f32| {
            let v5 = (v.clamp(0.0, 1.0) * 31.0 + d).round().clamp(0.0, 31.0);
            (v5 * 255.0 / 31.0).round() as u8
        };
        out.extend_from_slice(&[q(c[2]), q(c[1]), q(c[0]), 0xFF]);
    }
    out
}
