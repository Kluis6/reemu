//! Cena 2D do preview de shader: fase de plataforma em pixel art no jeito
//! dos jogos de 16 bits, desenhada aqui em 320×240. Tem o que mais mostra a
//! diferença entre shaders: degradê com dithering, bordas duras, contornos
//! de 1 px, texto pequeno e cores chapadas lado a lado.

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
        'T' => ["###", ".#.", ".#.", ".#.", ".#."],
        'A' => [".#.", "#.#", "###", "#.#", "#.#"],
        'G' => ["###", "#..", "#.#", "#.#", "###"],
        '-' => ["...", "...", "###", "...", "..."],
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

/// Herói de capa pulando, 16×17 (`.` = transparente).
const HERO: [&str; 17] = [
    "......kkkkk.....",
    ".....kHhHHHk....",
    "....kHHHHhHHk...",
    "....kHSSSSHk....",
    "....kHSSkSEk....",
    ".....kSSSSSk....",
    "..kk..kssk......",
    ".kRRk.kBBBk.kk..",
    "kRRRRkBBBBBkSSk.",
    "kRcRRkBBBBBBkk..",
    ".kcRRkBYYYBk....",
    "..kcckbBBBbk....",
    "...kkkbbkbbk....",
    "....kbbk.kbbk...",
    "...kNNk...kNNk..",
    "...kNNNk...kNNk.",
    "....kkk.....kkk.",
];

/// Inimigo: bolinha escura com espinhos, 16×12.
const ENEMY: [&str; 12] = [
    ".....k.k.k......",
    "....kVkVkVk.....",
    "...kVVVVVVVk....",
    "..kVvVVVVVVVk...",
    ".kVvVVVVVVVVVk..",
    ".kVVVWkVVWkVVk..",
    "kVVVVWkVVWkVVVk.",
    "kVVVVVVVVVVVVVk.",
    "kVVVVVVVVVVVVVk.",
    ".kVVVVVVVVVVVk..",
    "..kVkkVVVkkVk...",
    "...k..kkk..k....",
];

/// Moeda, 8×12.
const COIN: [&str; 12] = [
    "..kkkk..", ".kYYYYk.", "kYyYYYOk", "kYyOOYOk", "kYyOYYOk", "kYyOYYOk", "kYyOYYOk", "kYyOYYOk",
    "kYyOOYOk", "kYYYYYOk", ".kOOOOk.", "..kkkk..",
];

/// Coração do HUD, 7×6.
const HEART: [&str; 6] = [
    ".RR.RR.", "RrRRRRR", "RRRRRRR", ".RRRRR.", "..RRR..", "...R...",
];

/// "?" do bloco, 6×8.
const QMARK: [&str; 8] = [
    ".####.", "##..##", "....##", "...##.", "..##..", "..##..", "......", "..##..",
];

fn pal(c: char) -> Option<Rgb> {
    Some(match c {
        'k' => (22, 18, 30),
        // herói
        'H' => (120, 62, 30),
        'h' => (176, 100, 48),
        'S' => (252, 204, 160),
        's' => (214, 150, 112),
        'E' => (20, 20, 40),
        'B' => (52, 90, 210),
        'b' => (30, 50, 140),
        'R' => (226, 52, 60),
        'r' => (255, 150, 160),
        'c' => (150, 30, 34),
        'N' => (96, 50, 26),
        // inimigo
        'V' => (36, 44, 96),
        'v' => (84, 104, 170),
        'W' => (240, 240, 250),
        // moeda e cinto
        'Y' => (250, 190, 40),
        'y' => (255, 240, 150),
        'O' => (196, 118, 24),
        _ => return None,
    })
}

fn lerp(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

/// Gerador pseudoaleatório fixo (xorshift): a cena sai sempre igual.
struct Rng(u32);

impl Rng {
    fn next(&mut self, m: i32) -> i32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 % m as u32) as i32
    }
}

/// Plataforma de terra com pedras encaixadas e grama no topo.
fn ground(cv: &mut Canvas, rng: &mut Rng, x0: i32, y0: i32, w: i32, h: i32) {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let (lx, ly) = (x - x0, y - y0);
            let off = if (ly / 9) % 2 == 0 { 0 } else { 6 };
            let (cx, cy) = ((lx + off) % 12, ly % 9);
            let corner = (cx == 1 || cx == 11) && (cy == 1 || cy == 8);
            let c = if lx == 0 || lx == w - 1 {
                (40, 20, 12)
            } else if cx == 0 || cy == 0 || corner {
                (70, 34, 18)
            } else if cy == 1 || cx == 1 {
                (178, 100, 52)
            } else if cy == 8 || cx == 11 {
                (116, 58, 28)
            } else {
                (150, 78, 38)
            };
            cv.put(x, y, c);
        }
    }
    // grama: faixa clara, faixa escura e pingos irregulares
    for x in x0..x0 + w {
        cv.put(x, y0 - 3, (30, 80, 30));
        cv.rect(x, y0 - 2, 1, 2, (120, 206, 64));
        cv.rect(x, y0, 1, 3, (70, 160, 45));
        let drip = rng.next(5);
        cv.rect(x, y0 + 3, 1, drip, (50, 128, 40));
        cv.put(x, y0 + 3 + drip, (30, 80, 30));
    }
    // tufos
    let mut x = x0 + 3 + rng.next(6);
    while x < x0 + w - 3 {
        for (dx, hgt) in [(0, 3), (1, 5), (2, 2)] {
            cv.rect(x + dx, y0 - 3 - hgt, 1, hgt, (90, 180, 56));
        }
        x += 9 + rng.next(14);
    }
}

/// Nuvem de pôr do sol: base reta, sombra embaixo e brilho em cima.
fn cloud(cv: &mut Canvas, cx: i32, base: i32, bumps: &[(i32, i32)]) {
    for (dy, c) in [
        (2, (196, 96, 140)),
        (0, (250, 150, 120)),
        (-2, (255, 196, 150)),
    ] {
        for &(bx, r) in bumps {
            let r = if dy < 0 { r - 3 } else { r };
            for y in base - r..=base {
                for x in cx + bx - r..=cx + bx + r {
                    let (ex, ey) = (x - cx - bx, y - base);
                    if ex * ex + ey * ey <= r * r && y + dy <= base {
                        cv.put(x, y + dy, c);
                    }
                }
            }
        }
    }
}

/// Bloco do cenário, 14×14: tijolo ou "?".
fn block(cv: &mut Canvas, x0: i32, y0: i32, question: bool) {
    let (base, hi) = if question {
        ((232, 140, 40), (255, 204, 96))
    } else {
        ((150, 80, 40), (186, 108, 56))
    };
    cv.rect(x0, y0, 14, 14, (70, 34, 18));
    cv.rect(x0 + 1, y0 + 1, 12, 12, base);
    cv.rect(x0 + 1, y0 + 1, 12, 1, hi);
    cv.rect(x0 + 1, y0 + 1, 1, 12, hi);
    if question {
        for (x, y) in [(2, 2), (11, 2), (2, 11), (11, 11)] {
            cv.put(x0 + x, y0 + y, (110, 56, 20));
        }
        for (j, row) in QMARK.iter().enumerate() {
            for (i, b) in row.chars().enumerate() {
                if b == '#' {
                    let (x, y) = (x0 + 4 + i as i32, y0 + 3 + j as i32);
                    cv.put(x + 1, y + 1, (110, 56, 20));
                    cv.put(x, y, (255, 236, 170));
                }
            }
        }
    } else {
        cv.rect(x0 + 1, y0 + 7, 12, 1, (90, 45, 20));
        cv.rect(x0 + 7, y0 + 1, 1, 6, (90, 45, 20));
        cv.rect(x0 + 4, y0 + 8, 1, 5, (90, 45, 20));
    }
}

/// Cena 2D do preview em XRGB8888 (B, G, R, X), 320×240: fase de plataforma
/// ao pôr do sol, a partir de uma imagem de referência do dono do projeto
/// (redesenhada aqui pixel a pixel). Céu em degradê com dithering, nuvens,
/// montanhas, castelo, lago com o reflexo do sol, árvore, plataformas de
/// terra com grama, cachoeira, ponte, blocos, moedas, herói, inimigo e o HUD
/// em cima.
pub fn sample_scene() -> Vec<u8> {
    let mut cv = Canvas::new();
    let mut rng = Rng(0x5eed_cafe);
    let horizon = 126;

    // Céu: azul-escuro em cima, roxo, rosa e laranja no horizonte.
    let sky: [Rgb; 5] = [
        (28, 30, 104),
        (60, 44, 140),
        (128, 60, 150),
        (222, 96, 112),
        (250, 156, 84),
    ];
    for y in 0..horizon {
        let f = y as f32 / horizon as f32 * (sky.len() - 1) as f32;
        let (i, t) = (f.floor() as usize, f.fract());
        for x in 0..W {
            cv.dither(x, y, sky[i], sky[(i + 1).min(sky.len() - 1)], t);
        }
    }
    // Sol com halo.
    cv.disc(238, 100, 20, (252, 188, 96));
    cv.disc(238, 100, 16, (255, 220, 110));
    cv.disc(234, 96, 9, (255, 238, 160));
    // Nuvens.
    cloud(
        &mut cv,
        104,
        68,
        &[(-26, 8), (-12, 12), (4, 15), (20, 10), (32, 6)],
    );
    cloud(&mut cv, 192, 54, &[(-20, 6), (-6, 10), (8, 12), (22, 7)]);
    cloud(&mut cv, 300, 42, &[(-18, 6), (-4, 10), (12, 12), (26, 8)]);
    cloud(&mut cv, 164, 78, &[(-10, 4), (0, 6), (10, 4)]);

    // Montanhas distantes (lado iluminado à esquerda do pico) e morros.
    for (peaks, lit, dark) in [
        (
            &[
                (50, 74, 0.9f32),
                (164, 82, 0.8),
                (190, 96, 0.7),
                (110, 98, 0.6),
            ][..],
            (128, 108, 196),
            (92, 80, 164),
        ),
        (
            &[(20, 104, 0.5f32), (110, 110, 0.45), (200, 114, 0.5)][..],
            (64, 70, 140),
            (50, 56, 122),
        ),
    ] {
        for x in 0..250i32 {
            let (mut top, mut side) = (horizon, true);
            for &(px, py, s) in peaks {
                let y = py + ((x - px).abs() as f32 * s) as i32;
                if y < top {
                    top = y;
                    side = x < px;
                }
            }
            for y in top..horizon {
                cv.put(x, y, if side { lit } else { dark });
            }
        }
    }

    // Ilha com o castelo à direita.
    for x in 252..W {
        let top = horizon - ((x - 252) as f32 * 0.7).min(30.0) as i32;
        cv.rect(x, top, 1, horizon - top, (44, 56, 104));
        cv.put(x, top, (76, 92, 136));
    }
    let castle = (66, 56, 132);
    for (x, y, w) in [(278, 78, 9), (292, 64, 12), (306, 74, 9)] {
        cv.rect(x, y, w, 100 - y, castle);
        for k in (0..w).step_by(3) {
            cv.rect(x + k, y - 2, 2, 2, castle);
        }
        cv.rect(x + w / 2 - 1, y + 6, 2, 4, (36, 30, 80));
    }
    cv.rect(278, 88, 37, 12, castle);
    cv.rect(297, 52, 1, 12, castle);
    cv.rect(298, 52, 6, 4, castle);

    // Lago: degradê, reflexo do sol e brilhos.
    for y in horizon..H {
        let t = (y - horizon) as f32 / (H - horizon) as f32;
        for x in 0..W {
            cv.dither(x, y, (44, 64, 168), (20, 30, 104), t);
        }
    }
    for k in 0..16 {
        let y = horizon + 2 + k * 3;
        let w = 26 - k + rng.next(6);
        let c = if k < 6 {
            (255, 220, 120)
        } else {
            (250, 160, 90)
        };
        cv.rect(238 - w / 2 + rng.next(5) - 2, y, w, 1, c);
    }
    for _ in 0..40 {
        let (x, y) = (rng.next(W), horizon + 2 + rng.next(H - horizon - 2));
        let base = cv.get(x, y);
        cv.rect(x, y, 4 + rng.next(6), 1, lerp(base, (140, 170, 255), 0.5));
    }
    // Margem distante com pinheiros.
    cv.rect(88, 150, 96, 10, (28, 58, 78));
    let mut x = 90;
    while x < 180 {
        let hgt = 8 + rng.next(9);
        for j in 0..hgt {
            let half = j / 3;
            cv.rect(x - half, 150 - hgt + j, half * 2 + 1, 1, (30, 72, 90));
        }
        x += 6 + rng.next(5);
    }

    // Árvore no canto superior esquerdo.
    cv.rect(4, 26, 18, 84, (110, 58, 30));
    cv.rect(4, 26, 3, 84, (150, 86, 44));
    cv.rect(19, 26, 3, 84, (70, 36, 20));
    for k in 0..6 {
        cv.rect(9 + (k % 3) * 3, 34 + k * 12, 1, 7, (70, 36, 20));
    }
    let leaves = [
        (10, 8, 22),
        (40, 6, 18),
        (62, 26, 13),
        (30, 34, 17),
        (4, 46, 13),
        (50, 46, 11),
    ];
    for &(lx, ly, r) in &leaves {
        cv.disc(lx, ly, r, (24, 80, 40));
    }
    for &(lx, ly, r) in &leaves {
        cv.disc(lx - 2, ly - 3, r - 4, (44, 124, 52));
    }
    for &(lx, ly, r) in &leaves {
        for y in ly - r..ly - r / 3 {
            for x in lx - r..lx {
                let (dx, dy) = (x - lx + 3, y - ly + 4);
                if dx * dx + dy * dy <= (r - 5) * (r - 5) {
                    let base = cv.get(x, y);
                    cv.dither(x, y, base, (96, 172, 64), 0.4);
                }
            }
        }
    }

    // Plataformas, de trás pra frente.
    ground(&mut cv, &mut rng, 0, 106, 43, 60);
    ground(&mut cv, &mut rng, 48, 122, 48, 40);
    ground(&mut cv, &mut rng, 0, 156, 77, 84);
    ground(&mut cv, &mut rng, 76, 175, 51, 65);
    ground(&mut cv, &mut rng, 172, 149, 56, 72);
    ground(&mut cv, &mut rng, 159, 187, 40, 33);

    // Cachoeira entre as plataformas do meio.
    for y in 186..220 {
        for x in 127..159 {
            let stripe = (x * 3 + y / 2) % 7;
            let c = match stripe {
                0 => (220, 240, 255),
                1 | 2 => (120, 190, 255),
                _ => (60, 140, 240),
            };
            cv.put(x, y, c);
        }
    }
    cv.rect(127, 186, 32, 2, (230, 246, 255));
    // Água da frente, com ondas e espuma.
    for y in 205..H {
        for x in 125..283 {
            cv.dither(x, y, (40, 100, 220), (24, 56, 160), (y - 205) as f32 / 35.0);
        }
    }
    for x in 125..283 {
        let wave = (x / 4) % 2;
        cv.put(x, 205 - wave, (200, 230, 255));
        cv.put(x, 206, (100, 170, 255));
    }
    for (x, w) in [(127, 32)] {
        for k in 0..w {
            let y = 219 + (k % 3);
            cv.put(x + k, y, (230, 246, 255));
        }
    }

    // Ponte de madeira.
    cv.rect(227, 166, 57, 6, (150, 85, 40));
    cv.rect(227, 166, 57, 1, (196, 124, 64));
    cv.rect(227, 171, 57, 1, (70, 36, 20));
    for x in (231..283).step_by(7) {
        cv.rect(x, 167, 1, 4, (90, 45, 22));
    }
    for x in [230, 279] {
        cv.rect(x, 158, 3, 8, (120, 66, 30));
        cv.put(x + 1, 158, (176, 108, 56));
    }
    for k in 0..12 {
        cv.rect(232 + k, 172 + k, 2, 1, (110, 58, 28));
        cv.rect(278 - k, 172 + k, 2, 1, (110, 58, 28));
    }
    ground(&mut cv, &mut rng, 283, 158, 37, 82);

    // Placa com a seta.
    cv.rect(59, 141, 3, 15, (110, 58, 30));
    cv.rect(50, 129, 22, 13, (70, 34, 18));
    cv.rect(51, 130, 20, 11, (164, 96, 48));
    cv.rect(51, 130, 20, 1, (200, 130, 70));
    cv.rect(55, 134, 9, 3, (246, 214, 170));
    for k in 0..4 {
        cv.rect(64 + k, 132 + k, 1, 7 - k * 2, (246, 214, 170));
    }

    // Blocos, moedas, inimigo e herói.
    block(&mut cv, 117, 124, false);
    block(&mut cv, 131, 124, true);
    block(&mut cv, 145, 124, false);
    for (x, y) in [(122, 94), (139, 98), (156, 103), (290, 139)] {
        cv.sprite(&COIN, x, y, 1, pal);
    }
    cv.sprite(&ENEMY, 195, 137, 1, pal);
    cv.sprite(&HERO, 88, 84, 1, pal);

    // HUD: retrato e vidas, corações, pontos, moedas e fase.
    cv.rect(7, 7, 20, 20, (236, 236, 244));
    cv.rect(8, 8, 18, 18, (52, 104, 196));
    cv.sprite(&HERO[..9], 9, 12, 1, pal);
    let white = (255, 255, 255);
    text(&mut cv, "x03", 31, 12, 2, white);
    for k in 0..3 {
        cv.sprite(&HEART, 70 + k * 15, 11, 2, pal);
    }
    text(&mut cv, "SCORE", 138, 4, 2, white);
    text(&mut cv, "000420", 134, 17, 2, white);
    cv.sprite(&COIN, 205, 10, 1, pal);
    text(&mut cv, "x12", 216, 12, 2, white);
    text(&mut cv, "STAGE 1-2", 247, 12, 2, white);

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
        if let Ok(path) = std::env::var("REEMU_DUMP_SCENE") {
            let mut out = format!("P6\n{} {}\n255\n", super::W, super::H).into_bytes();
            for p in px.chunks(4) {
                out.extend_from_slice(&[p[2], p[1], p[0]]);
            }
            std::fs::write(path, out).unwrap();
        }
    }

    /// Todo sprite precisa ser retangular (linhas do mesmo tamanho).
    #[test]
    fn sprites_are_rectangular() {
        let all: [&[&str]; 5] = [
            &super::HERO,
            &super::ENEMY,
            &super::COIN,
            &super::HEART,
            &super::QMARK,
        ];
        for rows in all {
            assert!(rows.iter().all(|r| r.len() == rows[0].len()), "{rows:?}");
        }
    }
}
