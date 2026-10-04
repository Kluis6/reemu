//! Cena 2D do preview de shader: pixel art no jeito dos jogos de 16 bits,
//! desenhada aqui (sem material de terceiros) em 320×240. Tem o que mais
//! mostra a diferença entre shaders: degradê com dithering, bordas duras,
//! contornos de 1 px, texto pequeno e cores chapadas lado a lado.

use super::shader_preview::{PREVIEW_H, PREVIEW_W};

type Rgb = (u8, u8, u8);

const W: i32 = PREVIEW_W as i32;
const H: i32 = PREVIEW_H as i32;

/// Matriz de Bayer 4×4 (dithering ordenado, como nos consoles de 16 bits).
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

struct Canvas {
    px: Vec<Rgb>,
}

impl Canvas {
    fn new() -> Self {
        Self { px: vec![(0, 0, 0); (W * H) as usize] }
    }
    fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            self.px[(y * W + x) as usize] = c;
        }
    }
    fn get(&self, x: i32, y: i32) -> Rgb {
        self.px[(y.clamp(0, H - 1) * W + x.clamp(0, W - 1)) as usize]
    }
    fn rect(&mut self, x0: i32, y0: i32, w: i32, h: i32, c: Rgb) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.put(x, y, c);
            }
        }
    }
    /// Escolhe `a` ou `b` por pixel, na proporção `t` (0..1), com Bayer.
    fn dither(&mut self, x: i32, y: i32, a: Rgb, b: Rgb, t: f32) {
        let th = (BAYER[(y & 3) as usize][(x & 3) as usize] as f32 + 0.5) / 16.0;
        self.put(x, y, if t > th { b } else { a });
    }
    fn disc(&mut self, cx: i32, cy: i32, r: i32, c: Rgb) {
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                if (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r {
                    self.put(x, y, c);
                }
            }
        }
    }
    fn sprite(&mut self, rows: &[&str], ox: i32, oy: i32, scale: i32, pal: fn(char) -> Option<Rgb>) {
        for (j, row) in rows.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                if let Some(c) = pal(ch) {
                    self.rect(ox + i as i32 * scale, oy + j as i32 * scale, scale, scale, c);
                }
            }
        }
    }
}

/// Fonte 3×5 do HUD.
fn glyph(c: char) -> [&'static str; 5] {
    match c {
        'S' => ["###", "#..", "###", "..#", "###"],
        'C' => ["###", "#..", "#..", "#..", "###"],
        'O' | '0' => ["###", "#.#", "#.#", "#.#", "###"],
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'E' => ["###", "#..", "##.", "#..", "###"],
        'M' => ["#.#", "###", "###", "#.#", "#.#"],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        '1' => [".#.", "##.", ".#.", ".#.", "###"],
        '2' => ["###", "..#", "###", "#..", "###"],
        '3' => ["###", "..#", ".##", "..#", "###"],
        '4' => ["#.#", "#.#", "###", "..#", "..#"],
        '5' => ["###", "#..", "###", "..#", "###"],
        '6' => ["###", "#..", "###", "#.#", "###"],
        '7' => ["###", "..#", ".#.", ".#.", ".#."],
        '8' => ["###", "#.#", "###", "#.#", "###"],
        '9' => ["###", "#.#", "###", "..#", "###"],
        'x' => ["...", "#.#", ".#.", "#.#", "..."],
        _ => ["...", "...", "...", "...", "..."],
    }
}

fn text(cv: &mut Canvas, s: &str, x: i32, y: i32, scale: i32, c: Rgb) {
    for (k, ch) in s.chars().enumerate() {
        for (j, row) in glyph(ch).iter().enumerate() {
            for (i, b) in row.chars().enumerate() {
                if b == '#' {
                    let (px, py) = (x + (k as i32 * 4 + i as i32) * scale, y + j as i32 * scale);
                    cv.rect(px + scale / 2 + 1, py + scale / 2 + 1, scale, scale, (16, 12, 32));
                    cv.rect(px, py, scale, scale, c);
                }
            }
        }
    }
}

/// O robô do avatar do ReEmu, 12×16, com sombreado (`.` = transparente).
const HERO: [&str; 16] = [
    ".....Yy.....",
    ".....gg.....",
    "..kkkkkkkk..",
    ".kWWWWWWWwk.",
    ".kWDDDDDDwk.",
    ".kWDCDDCDwk.",
    ".kWDDDDDDwk.",
    ".kWWWWWWwwk.",
    "..kkkkkkkk..",
    ".kWWWWWWWwk.",
    "gkWWWRrWWwkg",
    "gkWWWrrWWwkg",
    ".kwWWWWWwwk.",
    "..kkkkkkkk..",
    "..kgGk.kgGk.",
    "..kkkk.kkkk.",
];

/// Baú 16×12.
const CHEST: [&str; 12] = [
    "..kkkkkkkkkkkk..",
    ".kOOOOOOOOOOOOk.",
    "kOoOOOOOOOOOOoOk",
    "kOOOOOOOOOOOOOOk",
    "kYYYYYYYYYYYYYYk",
    "kooooooYYooooook",
    "kOOOOOYkkYOOOOOk",
    "kOOOOOOYYOOOOOOk",
    "kOOOOOOOOOOOOOOk",
    "kYYYYYYYYYYYYYYk",
    "kooooooooooooook",
    "kkkkkkkkkkkkkkkk",
];

/// Coração do HUD, 7×6.
const HEART: [&str; 6] = [".RR.RR.", "RrRRRRR", "RRRRRRR", ".RRRRR.", "..RRR..", "...R..."];

/// Moeda genérica com brilho, 8×8.
const COIN: [&str; 8] = [
    "..kkkk..", ".kYYYYk.", "kYyYYYYk", "kYyOOYYk", "kYyOYYYk", "kYYOYYYk", ".kYYYYk.", "..kkkk..",
];

fn pal(c: char) -> Option<Rgb> {
    Some(match c {
        'W' => (244, 247, 250),
        'w' => (190, 200, 214),
        'D' => (18, 24, 44),
        'C' => (40, 216, 242),
        'Y' => (252, 214, 64),
        'y' => (255, 246, 190),
        'G' => (148, 160, 184),
        'g' => (100, 110, 132),
        'R' => (232, 56, 84),
        'r' => (255, 150, 160),
        'O' => (158, 86, 34),
        'o' => (112, 58, 22),
        'k' => (22, 18, 30),
        _ => return None,
    })
}

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

/// Cena 2D em XRGB8888 (B, G, R, X), 320×240.
pub fn sample_scene() -> Vec<u8> {
    let mut cv = Canvas::new();
    let horizon = 176;

    // Céu: degradê do anil ao pêssego em faixas com dithering entre elas.
    let sky: [Rgb; 7] = [
        (22, 18, 64),
        (40, 34, 104),
        (72, 52, 140),
        (122, 74, 160),
        (186, 100, 150),
        (236, 142, 128),
        (252, 196, 140),
    ];
    for y in 0..horizon {
        let f = y as f32 / horizon as f32 * (sky.len() - 1) as f32;
        let (i, t) = (f.floor() as usize, f.fract());
        let (a, b) = (sky[i], sky[(i + 1).min(sky.len() - 1)]);
        for x in 0..W {
            cv.dither(x, y, a, b, t);
        }
    }
    // Estrelas no alto (algumas maiores, em cruz).
    for (x, y, big) in [(18, 14, true), (44, 30, false), (96, 10, false), (132, 36, true), (170, 16, false), (292, 20, true), (306, 44, false), (230, 8, false)] {
        cv.put(x, y, (255, 255, 255));
        if big {
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                cv.put(x + dx, y + dy, (200, 200, 255));
            }
        }
    }
    // Sol com halo em dithering.
    let (sx, sy) = (250, 64);
    for y in sy - 34..=sy + 34 {
        for x in sx - 34..=sx + 34 {
            let d = (((x - sx) * (x - sx) + (y - sy) * (y - sy)) as f32).sqrt();
            if d <= 16.0 {
                cv.put(x, y, if d <= 12.0 { (255, 244, 200) } else { (255, 222, 140) });
            } else if d <= 34.0 {
                let base = cv.get(x, y);
                cv.dither(x, y, base, (255, 200, 140), (1.0 - (d - 16.0) / 18.0) * 0.55);
            }
        }
    }
    // Nuvens com luz por cima e sombra por baixo.
    for (cx, cy, s) in [(56, 46, 1.0f32), (150, 30, 0.8), (200, 92, 0.7)] {
        let blobs = [(-14, 2, 9), (-4, -4, 12), (8, -1, 10), (18, 3, 7)];
        for (dx, dy, r) in blobs {
            let r = (r as f32 * s) as i32;
            let (bx, by) = (cx + (dx as f32 * s) as i32, cy + (dy as f32 * s) as i32);
            for y in by - r..=by + r {
                for x in bx - r..=bx + r {
                    if (x - bx) * (x - bx) + (y - by) * (y - by) <= r * r {
                        let shade = if y > by + r / 3 { (196, 186, 214) } else { (250, 248, 255) };
                        cv.put(x, y, shade);
                    }
                }
            }
        }
    }
    // Montanhas ao fundo: face iluminada à esquerda, sombra à direita, neve.
    let peaks = [(30, 96, 60), (96, 84, 70), (170, 100, 56), (300, 90, 72)];
    for x in 0..W {
        let mut top = horizon;
        let mut lit = false;
        for &(px, py, half) in &peaks {
            let h = py + ((x - px).abs() * (horizon - py)) / half;
            if h < top {
                top = h;
                lit = x < px;
            }
        }
        for y in top..horizon {
            let snow = y < top + 6 && top < 112;
            let c = if snow {
                if lit { (240, 240, 252) } else { (186, 190, 220) }
            } else if lit {
                (118, 96, 150)
            } else {
                (86, 68, 120)
            };
            cv.put(x, y, c);
        }
    }
    // Colinas com pinheiros.
    let hill = |x: i32| 150 + ((x as f32 / 26.0).sin() * 9.0 + (x as f32 / 11.0).cos() * 3.0) as i32;
    for x in 0..W {
        let top = hill(x);
        for y in top..horizon {
            cv.put(x, y, if y < top + 2 { (88, 168, 96) } else { (48, 116, 72) });
        }
    }
    for tx in (6..W).step_by(19) {
        let base = hill(tx) + 4;
        for j in 0..18 {
            let half = j / 2 + 1;
            for i in -half..=half {
                let c = if i < 0 { (34, 98, 60) } else { (22, 72, 46) };
                cv.put(tx + i, base - 18 + j, c);
            }
        }
        cv.rect(tx - 1, base, 3, 4, (92, 58, 30));
    }
    // Lago à direita: água com brilho e o reflexo do sol.
    for y in horizon..H {
        for x in 212..W {
            let depth = (y - horizon) as f32 / (H - horizon) as f32;
            let c = lerp((70, 120, 190), (28, 54, 120), depth);
            cv.put(x, y, c);
        }
    }
    for k in 0..26 {
        let y = horizon + 3 + k * 2;
        let w = 18 - k / 2;
        let x = sx - w / 2 + if k % 2 == 0 { 0 } else { 3 };
        if y < H && w > 1 {
            cv.rect(x, y, w, 1, (255, 226, 150));
        }
    }
    for (x, y, w) in [(222, 188, 10), (290, 196, 12), (236, 214, 8), (300, 226, 9), (226, 232, 6)] {
        cv.rect(x, y, w, 1, (150, 196, 240));
    }
    // Chão: grama com tufos, terra com pedras e camadas.
    for y in horizon..H {
        for x in 0..212 {
            let c = if y < horizon + 5 {
                if y == horizon { (150, 224, 96) } else { (76, 168, 64) }
            } else {
                let band = ((y - horizon) / 14) % 2 == 0;
                if band { (156, 92, 48) } else { (138, 80, 42) }
            };
            cv.put(x, y, c);
        }
    }
    for x in (2..210).step_by(7) {
        cv.put(x, horizon - 1, (150, 224, 96));
        cv.put(x + 1, horizon - 2, (150, 224, 96));
    }
    for (x, y, r) in [(20, 196, 3), (64, 214, 2), (110, 204, 3), (150, 224, 2), (186, 200, 3), (40, 230, 2), (130, 236, 3)] {
        cv.disc(x, y, r, (110, 100, 96));
        cv.put(x - 1, y - 1, (170, 160, 150));
    }
    // margem do lago
    for y in horizon..H {
        cv.put(212, y, (96, 66, 40));
        cv.put(213, y, (120, 82, 48));
    }
    // Flores na grama.
    for (x, c) in [(28, (255, 90, 120)), (36, (255, 220, 80)), (118, (255, 90, 120)), (196, (255, 220, 80)), (204, (180, 120, 255))] {
        cv.put(x, horizon + 1, c);
        cv.put(x, horizon + 2, (40, 120, 50));
    }
    // Plataforma de tijolos flutuando, com as moedas em cima.
    let (px0, py0, pw) = (104, 120, 64);
    for y in py0..py0 + 10 {
        for x in px0..px0 + pw {
            let row = (y - py0) / 5;
            let off = if row % 2 == 0 { 0 } else { 4 };
            let mortar = (y - py0) % 5 == 4 || (x - px0 + off) % 8 == 7;
            let c = if mortar {
                (90, 40, 24)
            } else if (y - py0) % 5 == 0 {
                (236, 140, 84)
            } else {
                (200, 100, 56)
            };
            cv.put(x, y, c);
        }
    }
    cv.rect(px0, py0 + 10, pw, 1, (40, 20, 16));
    for k in 0..4 {
        cv.sprite(&COIN, px0 + 6 + k * 15, py0 - 14, 1, pal);
    }
    // Robô e baú no chão.
    cv.sprite(&HERO, 52, horizon - 32, 2, pal);
    cv.sprite(&CHEST, 150, horizon - 24, 2, pal);
    // sombras no chão
    for x in 52..76 {
        let p = cv.get(x, horizon + 1);
        cv.put(x, horizon + 1, lerp(p, (20, 40, 20), 0.5));
    }
    // HUD: vidas e pontos.
    for k in 0..3 {
        cv.sprite(&HEART, 8 + k * 10, 8, 1, pal);
    }
    text(&mut cv, "x3", 40, 9, 1, (255, 255, 255));
    text(&mut cv, "SCORE 012340", 218, 8, 2, (255, 255, 255));

    let mut out = Vec::with_capacity((W * H * 4) as usize);
    for (r, g, b) in cv.px {
        out.extend_from_slice(&[b, g, r, 0xFF]);
    }
    out
}
