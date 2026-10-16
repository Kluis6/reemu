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
        Self {
            px: vec![(0, 0, 0); (W * H) as usize],
        }
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
    fn sprite(
        &mut self,
        rows: &[&str],
        ox: i32,
        oy: i32,
        scale: i32,
        pal: fn(char) -> Option<Rgb>,
    ) {
        for (j, row) in rows.iter().enumerate() {
            for (i, ch) in row.chars().enumerate() {
                if let Some(c) = pal(ch) {
                    self.rect(
                        ox + i as i32 * scale,
                        oy + j as i32 * scale,
                        scale,
                        scale,
                        c,
                    );
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
                    cv.rect(
                        px + scale / 2 + 1,
                        py + scale / 2 + 1,
                        scale,
                        scale,
                        (16, 12, 32),
                    );
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

/// Os outros avatares do ReEmu em pixel art, mesma paleta dos SVGs
/// (`lib/avatars.ts`): slime (preset 3), controle (2), joystick de
/// fliperama (4) e nave (5). O robô (1) é o `HERO`.
const SLIME: [&str; 11] = [
    "....kkkkkk....",
    "..kkLLLLLLkk..",
    ".kLLlLLLLLLLk.",
    ".kLlLLLLLLLLk.",
    "kLLLLLLLLLLLLk",
    "kLLLeLLLLeLLLk",
    "kLLLeLLLLeLLLk",
    "kLpLLLLLLLLpLk",
    "kLLLLLmmLLLLLk",
    "kLLLLLLLLLLLLk",
    ".kkkkkkkkkkkk.",
];

const PAD: [&str; 8] = [
    "..kkkkkkkkkkkk..",
    ".kFFFFFFFFFFFFk.",
    "kFFPFFFFFFFFaFFk",
    "kFPPPFFFFFFbFcFk",
    "kFFPFFFQQFFFdFFk",
    "kFFFFFkkkkFFFFFk",
    ".kFFFk....kFFFk.",
    "..kkk......kkk..",
];

const STICK: [&str; 14] = [
    "....kkkk....",
    "...kJJJJk...",
    "..kJjJJJJk..",
    "..kJJJJJJk..",
    "..kJJJJJJk..",
    "...kJJJJk...",
    "....kTTk....",
    "....kTTk....",
    "....kTTk....",
    ".kkkkTTkkkk.",
    "kBBBBBBBBBBk",
    "kBqBBBBBBvBk",
    "kBBBBBBBBBBk",
    ".kkkkkkkkkk.",
];

const ROCKET: [&str; 15] = [
    "....kk....",
    "...kWWk...",
    "..kWWWWk..",
    "..kWWWWk..",
    "..kWzzWk..",
    "..kWzzWk..",
    "..kWWWWk..",
    "..kWWWWk..",
    ".kRkWWkRk.",
    "kRRkWWkRRk",
    "kRRkkkkRRk",
    "kkk.gg.kkk",
    "....YY....",
    "....Yf....",
    ".....f....",
];

/// Coração do HUD, 7×6.
const HEART: [&str; 6] = [
    ".RR.RR.", "RrRRRRR", "RRRRRRR", ".RRRRR.", "..RRR..", "...R...",
];

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
        // slime
        'L' => (217, 249, 157),
        'l' => (236, 252, 203),
        'e' => (20, 83, 45),
        'm' => (20, 83, 45),
        'p' => (251, 113, 133),
        // controle
        'F' => (253, 244, 255),
        'P' => (59, 7, 100),
        'Q' => (216, 180, 254),
        'a' => (34, 197, 94),
        'b' => (59, 130, 246),
        'c' => (239, 68, 68),
        'd' => (234, 179, 8),
        // joystick de fliperama
        'J' => (253, 224, 71),
        'j' => (255, 255, 255),
        'T' => (229, 231, 235),
        'B' => (31, 41, 55),
        'q' => (34, 211, 238),
        'v' => (163, 230, 53),
        // nave
        'z' => (56, 189, 248),
        'f' => (249, 115, 22),
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
    for (x, y, big) in [
        (18, 14, true),
        (44, 30, false),
        (96, 10, false),
        (132, 36, true),
        (170, 16, false),
        (292, 20, true),
        (306, 44, false),
        (230, 8, false),
    ] {
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
                cv.put(
                    x,
                    y,
                    if d <= 12.0 {
                        (255, 244, 200)
                    } else {
                        (255, 222, 140)
                    },
                );
            } else if d <= 34.0 {
                let base = cv.get(x, y);
                cv.dither(
                    x,
                    y,
                    base,
                    (255, 200, 140),
                    (1.0 - (d - 16.0) / 18.0) * 0.55,
                );
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
                        let shade = if y > by + r / 3 {
                            (196, 186, 214)
                        } else {
                            (250, 248, 255)
                        };
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
                if lit {
                    (240, 240, 252)
                } else {
                    (186, 190, 220)
                }
            } else if lit {
                (118, 96, 150)
            } else {
                (86, 68, 120)
            };
            cv.put(x, y, c);
        }
    }
    // Colinas com pinheiros.
    let hill =
        |x: i32| 150 + ((x as f32 / 26.0).sin() * 9.0 + (x as f32 / 11.0).cos() * 3.0) as i32;
    for x in 0..W {
        let top = hill(x);
        for y in top..horizon {
            cv.put(
                x,
                y,
                if y < top + 2 {
                    (88, 168, 96)
                } else {
                    (48, 116, 72)
                },
            );
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
    for (x, y, w) in [
        (222, 188, 10),
        (290, 196, 12),
        (236, 214, 8),
        (300, 226, 9),
        (226, 232, 6),
    ] {
        cv.rect(x, y, w, 1, (150, 196, 240));
    }
    // Chão: grama com tufos, terra com pedras e camadas.
    for y in horizon..H {
        for x in 0..212 {
            let c = if y < horizon + 5 {
                if y == horizon {
                    (150, 224, 96)
                } else {
                    (76, 168, 64)
                }
            } else {
                let band = ((y - horizon) / 14) % 2 == 0;
                if band {
                    (156, 92, 48)
                } else {
                    (138, 80, 42)
                }
            };
            cv.put(x, y, c);
        }
    }
    for x in (2..210).step_by(7) {
        cv.put(x, horizon - 1, (150, 224, 96));
        cv.put(x + 1, horizon - 2, (150, 224, 96));
    }
    for (x, y, r) in [
        (20, 196, 3),
        (64, 214, 2),
        (110, 204, 3),
        (150, 224, 2),
        (186, 200, 3),
        (40, 230, 2),
        (130, 236, 3),
    ] {
        cv.disc(x, y, r, (110, 100, 96));
        cv.put(x - 1, y - 1, (170, 160, 150));
    }
    // margem do lago
    for y in horizon..H {
        cv.put(212, y, (96, 66, 40));
        cv.put(213, y, (120, 82, 48));
    }
    // Flores na grama.
    for (x, c) in [
        (28, (255, 90, 120)),
        (36, (255, 220, 80)),
        (118, (255, 90, 120)),
        (196, (255, 220, 80)),
        (204, (180, 120, 255)),
    ] {
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
    // moedas nas pontas e o joystick de fliperama (avatar 4) no meio
    for k in [0, 3] {
        cv.sprite(&COIN, px0 + 6 + k * 15, py0 - 14, 1, pal);
    }
    cv.sprite(&STICK, px0 + 20, py0 - 28, 2, pal);
    // Nave (avatar 5) voando entre as nuvens, com rastro.
    cv.sprite(&ROCKET, 112, 40, 2, pal);
    for (k, y) in (0..4).zip([72, 76, 80, 84]) {
        cv.put(121 + (k % 2), y, (255, 226, 150));
    }
    // Controle (avatar 2) flutuando como item, com brilho em volta.
    cv.sprite(&PAD, 12, 112, 2, pal);
    for (x, y) in [(8, 108), (48, 110), (30, 132)] {
        cv.put(x, y, (255, 255, 255));
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            cv.put(x + dx, y + dy, (255, 236, 160));
        }
    }
    // Robô (avatar 1), slime (avatar 3) e baú no chão.
    cv.sprite(&HERO, 52, horizon - 32, 2, pal);
    cv.sprite(&SLIME, 98, horizon - 22, 2, pal);
    cv.sprite(&CHEST, 150, horizon - 24, 2, pal);
    // sombras no chão
    for x in (52..76).chain(100..124) {
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

/// Gira um sprite 90° no sentido horário (a nave do avatar aponta pra cima;
/// na cena espacial ela voa pra direita).
fn rotate_cw(rows: &[&str]) -> Vec<String> {
    let grid: Vec<Vec<char>> = rows.iter().map(|r| r.chars().collect()).collect();
    (0..grid[0].len())
        .map(|x| grid.iter().rev().map(|row| row[x]).collect())
        .collect()
}

/// Segunda cena 2D do preview: um shoot 'em up de rolagem lateral no jeito de
/// Gradius/R-Type (16 bits), com os avatares do ReEmu em papéis do gênero — a
/// nave é o jogador, os slimes a esquadrilha inimiga, o robô o chefe, e o
/// controle e o joystick de fliperama os power-ups. Espaço com nebulosa em
/// dithering, estrelas em 3 camadas, planeta com anel e o HUD numa faixa
/// embaixo, como nos shmups horizontais. Desenhada aqui, sem material de
/// terceiros.
pub fn sample_scene_space() -> Vec<u8> {
    let mut cv = Canvas::new();
    let hud = 220;

    // Espaço: degradê do azul quase preto ao roxo, com dithering.
    let deep: [Rgb; 4] = [(6, 6, 20), (12, 10, 36), (26, 14, 56), (40, 18, 70)];
    for y in 0..hud {
        let f = y as f32 / hud as f32 * (deep.len() - 1) as f32;
        let (i, t) = (f.floor() as usize, f.fract());
        let (a, b) = (deep[i], deep[(i + 1).min(deep.len() - 1)]);
        for x in 0..W {
            cv.dither(x, y, a, b, t);
        }
    }
    // Nebulosa: manchas magenta e azul em dithering.
    for (cx, cy, r, c, k) in [
        (90, 70, 70, (150, 40, 140), 0.45f32),
        (170, 150, 60, (40, 70, 170), 0.4),
        (40, 180, 40, (110, 40, 150), 0.35),
    ] {
        for y in (cy - r).max(0)..(cy + r).min(hud) {
            for x in (cx - r).max(0)..(cx + r).min(W) {
                let d = (((x - cx) * (x - cx) + (y - cy) * (y - cy)) as f32).sqrt() / r as f32;
                if d < 1.0 {
                    let base = cv.get(x, y);
                    cv.dither(x, y, base, c, (1.0 - d) * k);
                }
            }
        }
    }
    // Estrelas em 3 camadas: distantes, médias e próximas (com rastro).
    let mut seed: u32 = 0x5eed_cafe;
    let mut rnd = |m: i32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % m as u32) as i32
    };
    for _ in 0..90 {
        let (x, y) = (rnd(W), rnd(hud));
        cv.put(x, y, (110, 110, 160));
    }
    for _ in 0..30 {
        let (x, y) = (rnd(W), rnd(hud));
        cv.put(x, y, (220, 220, 255));
    }
    for _ in 0..8 {
        let (x, y) = (rnd(W), rnd(hud - 4) + 2);
        cv.rect(x - 7, y, 6, 1, (80, 80, 130));
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            cv.put(x + dx, y + dy, (170, 180, 255));
        }
        cv.put(x, y, (255, 255, 255));
    }
    // Planeta com faixas e anel, sombreado em dithering (luz da esquerda).
    let (px, py, pr) = (262, 52, 34);
    let ring = |x: i32, k: i32| py + 6 + ((x - px) as f32 / 52.0 * 10.0) as i32 + k;
    // metade de trás do anel, antes do planeta
    for k in 0..2 {
        for x in px..=px + 52 {
            cv.put(x, ring(x, k) - 4, (150, 110, 100));
        }
    }
    for y in py - pr..=py + pr {
        for x in px - pr..=px + pr {
            let (dx, dy) = ((x - px) as f32, (y - py) as f32);
            if (dx * dx + dy * dy).sqrt() <= pr as f32 {
                let base = if ((y - py + pr) / 9) % 2 == 0 {
                    (224, 132, 88)
                } else {
                    (196, 104, 70)
                };
                let shade = ((dx + dy * 0.4) / pr as f32 + 0.2).clamp(0.0, 1.0);
                cv.dither(x, y, base, (70, 30, 50), shade);
            }
        }
    }
    for k in 0..2 {
        for x in px - 52..=px + 52 {
            let c = if k == 0 {
                (250, 220, 170)
            } else {
                (170, 130, 110)
            };
            cv.put(x, ring(x, k), c);
        }
    }
    // Asteroides.
    for (ax, ay, r) in [(150, 30, 6), (200, 200, 5), (28, 36, 4)] {
        cv.disc(ax, ay, r, (96, 84, 92));
        cv.disc(ax - 1, ay - 1, r - 2, (132, 118, 124));
        cv.put(ax + 1, ay + 1, (60, 52, 60));
    }

    // Power-ups em cápsulas: o controle e o joystick de fliperama.
    for (cx, cy) in [(132, 180), (206, 96)] {
        cv.disc(cx, cy, 14, (60, 90, 170));
        cv.disc(cx, cy, 12, (16, 20, 48));
        cv.rect(cx - 8, cy - 9, 2, 2, (210, 235, 255));
    }
    cv.sprite(&PAD, 124, 176, 1, pal);
    cv.sprite(&STICK, 200, 89, 1, pal);

    // Esquadrilha de slimes em onda.
    for (k, (x, y)) in [(150, 60), (178, 84), (150, 122), (178, 146)]
        .into_iter()
        .enumerate()
    {
        cv.sprite(&SLIME, x, y, if k % 2 == 0 { 2 } else { 1 }, pal);
    }
    // Explosão do slime que levou o tiro.
    let (ex, ey) = (124, 100);
    for (r, c) in [
        (9, (255, 120, 40)),
        (6, (255, 200, 80)),
        (3, (255, 250, 220)),
    ] {
        cv.disc(ex, ey, r, c);
    }
    for (dx, dy) in [(-13, -4), (12, 6), (-4, 13), (6, -13), (14, -9)] {
        cv.rect(ex + dx, ey + dy, 2, 2, (255, 180, 60));
    }

    // Chefe: o robô gigante atrás de um escudo de energia, carregando o raio.
    cv.sprite(&HERO, 248, 112, 4, pal);
    for y in 104..186 {
        for x in 238..241 {
            let base = cv.get(x, y);
            cv.put(
                x,
                y,
                lerp(base, (90, 220, 255), 0.25 + 0.2 * (x - 238) as f32),
            );
        }
    }
    cv.disc(230, 160, 5, (90, 220, 255));
    cv.disc(230, 160, 2, (230, 250, 255));

    // Jogador: a nave virada pra direita, com a chama do motor e os tiros.
    let ship = rotate_cw(&ROCKET);
    let ship: Vec<&str> = ship.iter().map(String::as_str).collect();
    let (sx, sy) = (30, 96);
    cv.sprite(&ship, sx, sy, 3, pal);
    for (k, c) in [(220, 80, 40), (255, 170, 60), (255, 240, 160)]
        .into_iter()
        .enumerate()
    {
        let k = k as i32;
        cv.rect(sx - 14 + k * 4, sy + 12 + k, 14 - k * 4, 6 - k * 2, c);
    }
    for (bx, by) in [(84, 106), (104, 106), (84, 115)] {
        cv.rect(bx, by, 8, 2, (255, 246, 160));
        cv.put(bx + 8, by, (255, 255, 255));
    }

    // HUD embaixo: vidas, barra de power e pontos.
    cv.rect(0, hud, W, H - hud, (14, 12, 28));
    cv.rect(0, hud, W, 1, (90, 80, 140));
    for k in 0..3 {
        cv.sprite(&HEART, 8 + k * 10, hud + 7, 1, pal);
    }
    for k in 0..6 {
        let c = if k < 4 { (90, 220, 255) } else { (50, 50, 80) };
        cv.rect(54 + k * 9, hud + 7, 7, 6, c);
    }
    text(&mut cv, "SCORE 098760", 210, hud + 5, 2, (255, 255, 255));

    let mut out = Vec::with_capacity((W * H * 4) as usize);
    for (r, g, b) in cv.px {
        out.extend_from_slice(&[b, g, r, 0xFF]);
    }
    out
}

#[cfg(test)]
mod tests {
    /// Grava a cena em PPM quando `REEMU_DUMP_SCENE` aponta pra um arquivo
    /// (pra conferir o desenho a olho); sem a variável, só confere o tamanho.
    #[test]
    fn sample_scene_has_the_expected_size() {
        let px = super::sample_scene();
        assert_eq!(px.len(), (super::W * super::H * 4) as usize);
        let space = super::sample_scene_space();
        assert_eq!(space.len(), px.len());
        if let Ok(path) = std::env::var("REEMU_DUMP_SPACE") {
            let mut out = format!("P6\n{} {}\n255\n", super::W, super::H).into_bytes();
            for p in space.chunks(4) {
                out.extend_from_slice(&[p[2], p[1], p[0]]);
            }
            std::fs::write(path, out).unwrap();
        }
        if let Ok(path) = std::env::var("REEMU_DUMP_SCENE") {
            let mut out = format!("P6\n{} {}\n255\n", super::W, super::H).into_bytes();
            for p in px.chunks(4) {
                out.extend_from_slice(&[p[2], p[1], p[0]]);
            }
            std::fs::write(path, out).unwrap();
        }
    }
}
