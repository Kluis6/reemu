// Cenas dos papéis de parede em vídeo do ReEmu (pedido do dono do projeto,
// 2026-11-14): arte original em canvas, nas cores dos avatares
// (`packages/app-desktop/src/lib/avatars.ts`, AVATAR_GRADIENTS) e SEM os
// personagens; a 6ª é a do tema principal (Neon Circuit, `styles/themes.ts`).
// Ideias de tema vindas de bancos de imagens (Freepik, Vecteezy), nada copiado.
//
// `draw(ctx, t)` desenha o quadro na fase `t` ∈ [0, 1) da volta. Todo
// movimento usa um número INTEIRO de ciclos por volta: o último quadro emenda
// no primeiro sem salto.

export const W = 1920
export const H = 1080
const TAU = Math.PI * 2

/** Gerador determinístico (mulberry32): o mesmo vídeo a cada geração. */
function rng(seed) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}
const wave = (t, cycles, phase = 0) => 0.5 + 0.5 * Math.sin(TAU * (t * cycles + phase))
const frac = (x) => x - Math.floor(x)

function gradientBg(ctx, a, b) {
  const g = ctx.createLinearGradient(0, 0, W, H)
  g.addColorStop(0, a)
  g.addColorStop(1, b)
  ctx.fillStyle = g
  ctx.fillRect(0, 0, W, H)
}
function glowBlob(ctx, x, y, r, color, alpha) {
  const g = ctx.createRadialGradient(x, y, 0, x, y, r)
  g.addColorStop(0, color)
  g.addColorStop(1, 'rgba(0,0,0,0)')
  ctx.globalAlpha = alpha
  ctx.fillStyle = g
  ctx.fillRect(x - r, y - r, r * 2, r * 2)
  ctx.globalAlpha = 1
}
/** Escurecido embaixo: o texto da Início fica legível. */
function shade(ctx) {
  const g = ctx.createLinearGradient(0, 0, 0, H)
  g.addColorStop(0.5, 'rgba(0,0,0,0)')
  g.addColorStop(1, 'rgba(0,0,0,0.4)')
  ctx.fillStyle = g
  ctx.fillRect(0, 0, W, H)
}

// 1. Circuito (robô: azul → ciano): trilhas de placa com pulsos de luz
// correndo por elas, LEDs piscando e chips com o contorno aceso.
function circuit() {
  const rand = rng(11)
  const G = 40
  const traces = []
  for (let i = 0; i < 52; i++) {
    let x = Math.round(rand() * 48) * G
    let y = Math.round(rand() * 27) * G
    const pts = [[x, y]]
    for (let s = 0; s < 4; s++) {
      const len = (2 + Math.floor(rand() * 6)) * G
      const dir = Math.floor(rand() * 4)
      if (dir === 0) x += len
      else if (dir === 1) y += len
      else if (dir === 2) { x += len / 2; y += len / 2 }
      else { x += len / 2; y -= len / 2 }
      pts.push([x, y])
    }
    let total = 0
    const segs = []
    for (let k = 1; k < pts.length; k++) {
      const l = Math.hypot(pts[k][0] - pts[k - 1][0], pts[k][1] - pts[k - 1][1])
      segs.push(l)
      total += l
    }
    traces.push({ pts, segs, total, speed: 1 + Math.floor(rand() * 2), phase: rand(), lit: rand() < 0.55 })
  }
  const leds = Array.from({ length: 22 }, () => ({
    x: Math.round(rand() * 48) * G, y: Math.round(rand() * 27) * G,
    cyc: 1 + Math.floor(rand() * 4), ph: rand(),
  }))
  const chips = [[200, 160, 200, 120], [760, 720, 160, 160], [1240, 120, 220, 110], [1500, 640, 180, 140]]
  const pointAt = (tr, d) => {
    for (let k = 0; k < tr.segs.length; k++) {
      if (d <= tr.segs[k]) {
        const [x0, y0] = tr.pts[k]
        const [x1, y1] = tr.pts[k + 1]
        const f = d / tr.segs[k]
        return [x0 + (x1 - x0) * f, y0 + (y1 - y0) * f]
      }
      d -= tr.segs[k]
    }
    return tr.pts[tr.pts.length - 1]
  }
  return (ctx, t) => {
    gradientBg(ctx, '#1d4ed8', '#22d3ee')
    glowBlob(ctx, 1400 + 120 * Math.sin(TAU * t), 420 + 80 * Math.cos(TAU * t), 700, '#0ea5e9', 0.35)
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'
    ctx.strokeStyle = 'rgba(224,242,254,0.16)'
    ctx.lineWidth = 4
    for (const tr of traces) {
      ctx.beginPath()
      ctx.moveTo(...tr.pts[0])
      for (const p of tr.pts.slice(1)) ctx.lineTo(...p)
      ctx.stroke()
      const [ex, ey] = tr.pts[tr.pts.length - 1]
      ctx.beginPath()
      ctx.arc(ex, ey, 9, 0, TAU)
      ctx.stroke()
    }
    for (const [x, y, w, h] of chips) {
      ctx.fillStyle = 'rgba(15,23,42,0.25)'
      ctx.strokeStyle = `rgba(103,232,249,${0.2 + 0.25 * wave(t, 2, x / W)})`
      ctx.lineWidth = 3
      ctx.beginPath()
      ctx.roundRect(x, y, w, h, 12)
      ctx.fill()
      ctx.stroke()
    }
    // pulsos: uma luz por trilha acesa, `speed` voltas por ciclo
    for (const tr of traces) {
      if (!tr.lit) continue
      const [x, y] = pointAt(tr, frac(t * tr.speed + tr.phase) * tr.total)
      glowBlob(ctx, x, y, 26, '#67e8f9', 0.9)
      ctx.fillStyle = '#ecfeff'
      ctx.beginPath()
      ctx.arc(x, y, 4, 0, TAU)
      ctx.fill()
    }
    for (const l of leds) {
      const a = 0.25 + 0.75 * wave(t, l.cyc, l.ph)
      glowBlob(ctx, l.x, l.y, 22, '#22d3ee', 0.35 * a)
      ctx.globalAlpha = a
      ctx.fillStyle = '#22d3ee'
      ctx.beginPath()
      ctx.arc(l.x, l.y, 6, 0, TAU)
      ctx.fill()
      ctx.globalAlpha = 1
    }
    shade(ctx)
  }
}

// 2. Controles (controle: roxo → rosa): padrão de ícones de jogo em traço que
// balançam e respiram no lugar, com alguns acendendo em sequência.
function doodles() {
  const rand = rng(22)
  const icons = [
    (c) => { c.moveTo(-10, -30); c.lineTo(10, -30); c.lineTo(10, -10); c.lineTo(30, -10); c.lineTo(30, 10); c.lineTo(10, 10); c.lineTo(10, 30); c.lineTo(-10, 30); c.lineTo(-10, 10); c.lineTo(-30, 10); c.lineTo(-30, -10); c.lineTo(-10, -10); c.closePath() },
    (c) => { for (const [x, y] of [[0, -22], [22, 0], [-22, 0], [0, 22]]) { c.moveTo(x + 9, y); c.arc(x, y, 9, 0, TAU) } },
    (c) => { c.moveTo(-34, -14); c.lineTo(34, -14); c.quadraticCurveTo(48, -14, 50, 2); c.lineTo(52, 16); c.quadraticCurveTo(50, 30, 38, 22); c.lineTo(28, 12); c.lineTo(-28, 12); c.lineTo(-38, 22); c.quadraticCurveTo(-50, 30, -52, 16); c.lineTo(-50, 2); c.quadraticCurveTo(-48, -14, -34, -14); c.closePath() },
    (c) => { c.moveTo(0, -20); c.lineTo(20, 14); c.lineTo(-20, 14); c.closePath() },
    (c) => { c.roundRect(-17, -17, 34, 34, 4) },
    (c) => { c.moveTo(18, 0); c.arc(0, 0, 18, 0, TAU) },
    (c) => { c.moveTo(-16, -16); c.lineTo(16, 16); c.moveTo(16, -16); c.lineTo(-16, 16) },
    (c) => { const s = [[0, -24], [7, -9], [23, -7], [11, 4], [14, 20], [0, 12], [-14, 20], [-11, 4], [-23, -7], [-7, -9]]; c.moveTo(...s[0]); for (const p of s.slice(1)) c.lineTo(...p); c.closePath() },
  ]
  const cells = []
  for (let row = 0; row < 9; row++) {
    for (let col = 0; col < 14; col++) {
      cells.push({
        x: col * 145 + (row % 2) * 72 + (rand() - 0.5) * 30,
        y: row * 130 + 40 + (rand() - 0.5) * 30,
        icon: icons[Math.floor(rand() * icons.length)],
        rot: (rand() - 0.5) * 0.9,
        sc: 0.8 + rand() * 0.6,
        ph: rand(),
        cyc: 1 + Math.floor(rand() * 2),
      })
    }
  }
  return (ctx, t) => {
    gradientBg(ctx, '#6d28d9', '#ec4899')
    glowBlob(ctx, 1300 + 200 * Math.cos(TAU * t), 500 + 120 * Math.sin(TAU * t * 2), 760, '#f0abfc', 0.3)
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'
    ctx.lineWidth = 5
    for (const c of cells) {
      // um "brilho" que passa em diagonal pela tela uma vez por volta
      const sweep = frac(t - (c.x + c.y) / (W + H))
      const lit = Math.max(0, 1 - Math.abs(sweep - 0.5) * 8)
      ctx.save()
      ctx.translate(c.x, c.y + 8 * Math.sin(TAU * (t * c.cyc + c.ph)))
      ctx.rotate(c.rot + 0.12 * Math.sin(TAU * (t * c.cyc + c.ph + 0.25)))
      ctx.scale(c.sc, c.sc)
      ctx.strokeStyle = `rgba(253,244,255,${0.16 + 0.5 * lit})`
      ctx.beginPath()
      c.icon(ctx)
      ctx.stroke()
      ctx.restore()
    }
    shade(ctx)
  }
}

// 3. Floresta 8-bit (slime: verde → lima): colinas de pixel em paralaxe,
// nuvens, sol que pulsa e vaga-lumes.
function pixelLand() {
  const rand = rng(33)
  const P = 12
  // camada: altura por coluna de P px, repetindo a cada `period` px
  const layer = (period, top, amp) => {
    const n = period / P
    const hs = []
    let y = 0
    for (let i = 0; i < n; i++) {
      y += Math.round((rand() - 0.5) * 2)
      y = Math.max(-amp, Math.min(amp, y))
      hs.push(y)
    }
    // emenda o fim no começo
    for (let i = 0; i < 6; i++) hs[n - 1 - i] = Math.round(hs[n - 1 - i] * (i / 6))
    return { period, top, hs }
  }
  const layers = [
    { ...layer(480, 620, 8), color: '#065f46', alpha: 0.55, k: 1 },
    { ...layer(960, 730, 7), color: '#047857', alpha: 0.8, k: 1 },
    { ...layer(960, 850, 6), color: '#14532d', alpha: 0.95, k: 2 },
  ]
  const trees = Array.from({ length: 6 }, () => ({ x: Math.floor(rand() * 80) * P, ph: rand() }))
  const flies = Array.from({ length: 26 }, () => ({
    x: rand() * W, y: 620 + rand() * 380, r: 10 + rand() * 30, cyc: 1 + Math.floor(rand() * 3), ph: rand(),
  }))
  const clouds = [[180, 150, 1], [760, 100, 2], [1240, 210, 1], [1600, 120, 2]]
  const drawLayer = (ctx, L, t) => {
    const off = (t * L.k * L.period) % L.period
    ctx.fillStyle = L.color
    ctx.globalAlpha = L.alpha
    for (let x = -L.period; x < W + L.period; x += P) {
      const i = Math.floor(((x + off) % L.period + L.period) % L.period / P)
      const y = L.top + L.hs[i] * P
      ctx.fillRect(Math.round(x), y, P, H - y)
    }
    ctx.globalAlpha = 1
    return off
  }
  return (ctx, t) => {
    gradientBg(ctx, '#047857', '#84cc16')
    // sol de pixels, pulsando
    const pulse = Math.round(wave(t, 2) * 2) * P
    ctx.fillStyle = 'rgba(254,240,138,0.9)'
    ctx.fillRect(1560 - pulse / 2, 120 - pulse / 2, P * 12 + pulse, P * 12 + pulse)
    ctx.fillRect(1560 - P * 2, 120 + P * 2, P * 16, P * 8)
    ctx.fillRect(1560 + P * 2, 120 - P * 2, P * 8, P * 16)
    // nuvens: vão e voltam um pouco (ciclo inteiro)
    ctx.fillStyle = 'rgba(236,252,203,0.38)'
    for (const [x, y, d] of clouds) {
      const dx = Math.round(Math.sin(TAU * t * d) * 6) * P
      ctx.fillRect(x + dx, y, P * 16, P * 4)
      ctx.fillRect(x + dx + P * 4, y - P * 2, P * 8, P * 2)
    }
    drawLayer(ctx, layers[0], t)
    const off1 = drawLayer(ctx, layers[1], t)
    // árvores na camada do meio (andam junto com ela)
    for (const tr of trees) {
      for (const rep of [0, 960, 1920]) {
        const x = Math.round((tr.x + rep - off1) / P) * P
        if (x < -P * 6 || x > W) continue
        const i = Math.floor((((tr.x + rep) % 960) + 960) % 960 / P)
        const base = layers[1].top + layers[1].hs[i] * P
        const sway = Math.round(Math.sin(TAU * (t * 2 + tr.ph))) * P
        ctx.fillStyle = '#713f12'
        ctx.fillRect(x + P * 2, base - P * 4, P * 2, P * 4)
        ctx.fillStyle = '#166534'
        ctx.fillRect(x + sway / 2, base - P * 10, P * 6, P * 6)
        ctx.fillRect(x + P * 2 + sway, base - P * 12, P * 2, P * 2)
      }
    }
    drawLayer(ctx, layers[2], t)
    // vaga-lumes
    for (const f of flies) {
      const a = wave(t, f.cyc, f.ph)
      const x = Math.round((f.x + f.r * Math.cos(TAU * (t + f.ph))) / 4) * 4
      const y = Math.round((f.y + f.r * Math.sin(TAU * (t * 2 + f.ph))) / 4) * 4
      glowBlob(ctx, x, y, 18, '#fef08a', 0.5 * a)
      ctx.globalAlpha = a
      ctx.fillStyle = '#fef9c3'
      ctx.fillRect(x - 3, y - 3, 6, 6)
      ctx.globalAlpha = 1
    }
    shade(ctx)
  }
}

// 4. Fliperama (joystick: laranja → vermelho): synthwave — sol listrado com as
// faixas descendo, montanhas, estrelas e a grade vindo em direção à tela.
function synthwave() {
  const rand = rng(44)
  const horizon = 700
  const stars = Array.from({ length: 90 }, () => ({ x: rand() * W, y: rand() * 420, cyc: 1 + Math.floor(rand() * 3), ph: rand() }))
  return (ctx, t) => {
    gradientBg(ctx, '#ea580c', '#be123c')
    const sky = ctx.createLinearGradient(0, 0, 0, horizon)
    sky.addColorStop(0, 'rgba(30,6,40,0.55)')
    sky.addColorStop(1, 'rgba(30,6,40,0)')
    ctx.fillStyle = sky
    ctx.fillRect(0, 0, W, horizon)
    for (const s of stars) {
      ctx.globalAlpha = 0.25 + 0.75 * wave(t, s.cyc, s.ph)
      ctx.fillStyle = '#fff7ed'
      ctx.fillRect(s.x, s.y, 3, 3)
    }
    ctx.globalAlpha = 1
    // sol com faixas que descem (uma faixa por volta)
    ctx.save()
    ctx.beginPath()
    ctx.arc(960, 560, 260, 0, TAU)
    ctx.clip()
    const sun = ctx.createLinearGradient(0, 300, 0, 820)
    sun.addColorStop(0, '#fde047')
    sun.addColorStop(1, '#f43f5e')
    ctx.fillStyle = sun
    ctx.fillRect(700, 300, 520, 520)
    ctx.globalCompositeOperation = 'destination-out'
    const step = 38
    for (let i = -1; i < 9; i++) {
      const y = 470 + (i + t) * step
      const h = Math.max(0, (y - 470) / 20)
      ctx.fillRect(700, y, 520, h)
    }
    ctx.restore()
    glowBlob(ctx, 960, 560, 420, '#fb923c', 0.35)
    // montanhas
    ctx.fillStyle = 'rgba(76,5,25,0.85)'
    ctx.beginPath()
    ctx.moveTo(0, horizon); ctx.lineTo(180, 560); ctx.lineTo(330, 640); ctx.lineTo(520, 470); ctx.lineTo(720, 620); ctx.lineTo(860, 560); ctx.lineTo(960, horizon)
    ctx.moveTo(960, horizon); ctx.lineTo(1080, 580); ctx.lineTo(1260, 460); ctx.lineTo(1420, 600); ctx.lineTo(1600, 500); ctx.lineTo(1780, 610); ctx.lineTo(1920, 540); ctx.lineTo(1920, horizon)
    ctx.fill()
    // chão e grade
    ctx.fillStyle = 'rgba(28,7,16,0.88)'
    ctx.fillRect(0, horizon, W, H - horizon)
    ctx.strokeStyle = 'rgba(251,146,60,0.55)'
    ctx.lineWidth = 3
    ctx.beginPath()
    for (let i = -16; i <= 16; i++) {
      ctx.moveTo(960 + i * 40, horizon)
      ctx.lineTo(960 + i * 260, H)
    }
    // linhas horizontais em perspectiva: z vai de longe a perto; uma linha
    // inteira de avanço por volta
    const N = 14
    for (let i = 0; i < N; i++) {
      const z = (i + 1 - frac(t)) / N // 1 = longe, ~0 = perto
      const y = horizon + (H - horizon) * (0.04 / (z + 0.04)) * 0.98
      if (y > H) continue
      ctx.moveTo(0, y)
      ctx.lineTo(W, y)
    }
    ctx.stroke()
    const haze = ctx.createLinearGradient(0, horizon - 40, 0, horizon + 60)
    haze.addColorStop(0, 'rgba(251,146,60,0)')
    haze.addColorStop(0.5, 'rgba(251,146,60,0.35)')
    haze.addColorStop(1, 'rgba(251,146,60,0)')
    ctx.fillStyle = haze
    ctx.fillRect(0, horizon - 40, W, 100)
    shade(ctx)
  }
}

// 5. Espaço (nave: índigo → roxo): estrelas piscando, nebulosa que respira,
// planeta com anel flutuando, lua e duas estrelas cadentes por volta.
function space() {
  const rand = rng(55)
  const stars = Array.from({ length: 260 }, () => ({
    x: rand() * W, y: rand() * H, r: rand() < 0.08 ? 2.6 : rand() < 0.4 ? 1.8 : 1.1,
    cyc: 1 + Math.floor(rand() * 3), ph: rand(),
  }))
  return (ctx, t) => {
    gradientBg(ctx, '#312e81', '#9333ea')
    glowBlob(ctx, 580, 430, 620, '#ec4899', 0.25 + 0.12 * wave(t, 1))
    glowBlob(ctx, 1500, 760, 520, '#6366f1', 0.25 + 0.1 * wave(t, 1, 0.5))
    for (const s of stars) {
      ctx.globalAlpha = 0.3 + 0.7 * wave(t, s.cyc, s.ph)
      ctx.fillStyle = '#ffffff'
      ctx.beginPath()
      ctx.arc(s.x, s.y, s.r, 0, TAU)
      ctx.fill()
    }
    ctx.globalAlpha = 1
    // estrelas cadentes: duas por volta, cada uma num trecho curto
    for (const [start, x0, y0] of [[0.1, 1200, 80], [0.6, 700, 140]]) {
      const p = (frac(t - start) ) / 0.12
      if (p >= 0 && p <= 1) {
        const x = x0 + p * 420
        const y = y0 + p * 240
        const g = ctx.createLinearGradient(x - 160, y - 92, x, y)
        g.addColorStop(0, 'rgba(255,255,255,0)')
        g.addColorStop(1, `rgba(255,255,255,${0.9 * (1 - Math.abs(p - 0.5) * 2) + 0.1})`)
        ctx.strokeStyle = g
        ctx.lineWidth = 3
        ctx.beginPath()
        ctx.moveTo(x - 160, y - 92)
        ctx.lineTo(x, y)
        ctx.stroke()
      }
    }
    // planeta com anel, flutuando
    const py = 300 + 14 * Math.sin(TAU * t)
    ctx.save()
    ctx.translate(380, py)
    const ring = (front) => {
      ctx.save()
      ctx.rotate(-0.32)
      if (front) { ctx.beginPath(); ctx.rect(-260, 0, 520, 70); ctx.clip() }
      ctx.strokeStyle = front ? 'rgba(253,230,138,0.85)' : 'rgba(253,230,138,0.4)'
      ctx.lineWidth = 10
      ctx.beginPath()
      ctx.ellipse(0, 0, 230, 46, 0, 0, TAU)
      ctx.stroke()
      ctx.restore()
    }
    ring(false)
    const pl = ctx.createLinearGradient(-130, -130, 130, 130)
    pl.addColorStop(0, '#fbbf24')
    pl.addColorStop(1, '#c2410c')
    ctx.fillStyle = pl
    ctx.beginPath()
    ctx.arc(0, 0, 130, 0, TAU)
    ctx.fill()
    ctx.fillStyle = 'rgba(255,255,255,0.12)'
    ctx.beginPath()
    ctx.arc(0, 0, 130, Math.PI, 0)
    ctx.fill()
    ring(true)
    ctx.restore()
    // lua em órbita curta
    const a = TAU * t
    const mx = 1500 + 90 * Math.cos(a)
    const my = 260 + 30 * Math.sin(a)
    const moon = ctx.createLinearGradient(mx - 60, my - 60, mx + 60, my + 60)
    moon.addColorStop(0, '#67e8f9')
    moon.addColorStop(1, '#1e40af')
    ctx.fillStyle = moon
    ctx.beginPath()
    ctx.arc(mx, my, 60, 0, TAU)
    ctx.fill()
    shade(ctx)
  }
}

// 6. ReEmu (tema principal, Neon Circuit): fundo quase preto com os brilhos do
// tema em órbita lenta, grade em perspectiva no verde da marca e formas de
// botão de jogo em néon girando e flutuando.
function reemu() {
  const rand = rng(66)
  // uma forma por célula de uma grade 6×3 (sorteada dentro da célula): elas
  // se espalham pela tela sem se amontoar
  const shapes = []
  for (let row = 0; row < 3; row++) {
    for (let col = 0; col < 6; col++) {
      if (rand() < 0.2) continue
      shapes.push({
        kind: Math.floor(rand() * 4), x: 110 + col * 320 + rand() * 160, y: 90 + row * 210 + rand() * 90,
        s: 24 + rand() * 30, ph: rand(), dir: rand() < 0.5 ? -1 : 1,
      })
    }
  }
  const horizon = 760
  const blob = (x, y, r, c, a) => glowBlob(ctx_, x, y, r, c, a)
  let ctx_ = null
  return (ctx, t) => {
    ctx_ = ctx
    const bg = ctx.createLinearGradient(0, 0, 0, H)
    bg.addColorStop(0, '#08100C')
    bg.addColorStop(0.45, '#030605')
    bg.addColorStop(1, '#030605')
    ctx.fillStyle = bg
    ctx.fillRect(0, 0, W, H)
    const a = TAU * t
    blob(420 + 160 * Math.cos(a), 300 + 90 * Math.sin(a), 760, '#1B6516', 0.75)
    blob(1500 + 140 * Math.cos(a + 2), 260 + 100 * Math.sin(a * 2), 700, '#0E3C78', 0.75)
    blob(1300 + 120 * Math.sin(a), 860 + 60 * Math.cos(a), 640, '#13508F', 0.55)
    blob(300 + 100 * Math.sin(a * 2), 900, 560, '#0C3A18', 0.7)
    // grade no chão
    ctx.strokeStyle = 'rgba(45,192,32,0.32)'
    ctx.lineWidth = 2
    ctx.beginPath()
    for (let i = -18; i <= 18; i++) {
      ctx.moveTo(960 + i * 30, horizon)
      ctx.lineTo(960 + i * 220, H)
    }
    const N = 10
    for (let i = 0; i < N; i++) {
      const z = (i + 1 - frac(t)) / N
      const y = horizon + (H - horizon) * (0.05 / (z + 0.05))
      if (y > H) continue
      ctx.moveTo(0, y)
      ctx.lineTo(W, y)
    }
    ctx.stroke()
    // a grade some no horizonte, sem faixa: o fade começa transparente acima
    const fade = ctx.createLinearGradient(0, horizon - 60, 0, horizon + 160)
    fade.addColorStop(0, 'rgba(3,6,5,0)')
    fade.addColorStop(0.3, 'rgba(3,6,5,0.85)')
    fade.addColorStop(1, 'rgba(3,6,5,0)')
    ctx.fillStyle = fade
    ctx.fillRect(0, horizon - 60, W, 220)
    // formas em néon: giram um passo de simetria por volta e flutuam
    ctx.lineJoin = 'round'
    ctx.lineCap = 'round'
    for (const sh of shapes) {
      const sym = [TAU / 3, TAU / 4, TAU, TAU / 4][sh.kind]
      const rot = sh.dir * sym * t + sh.ph * sym
      const y = sh.y + 16 * Math.sin(TAU * (t + sh.ph))
      const glow = 0.45 + 0.55 * wave(t, 2, sh.ph)
      ctx.save()
      ctx.translate(sh.x, y)
      ctx.rotate(rot)
      ctx.shadowColor = 'rgba(45,192,32,0.9)'
      ctx.shadowBlur = 18
      ctx.strokeStyle = `rgba(45,192,32,${0.35 + 0.45 * glow})`
      ctx.lineWidth = 4
      const s = sh.s
      ctx.beginPath()
      if (sh.kind === 0) { ctx.moveTo(0, -s); ctx.lineTo(s * 0.87, s * 0.5); ctx.lineTo(-s * 0.87, s * 0.5); ctx.closePath() }
      else if (sh.kind === 1) { ctx.rect(-s * 0.75, -s * 0.75, s * 1.5, s * 1.5) }
      else if (sh.kind === 2) { ctx.arc(0, 0, s * 0.8, 0, TAU) }
      else { ctx.moveTo(-s * 0.7, -s * 0.7); ctx.lineTo(s * 0.7, s * 0.7); ctx.moveTo(s * 0.7, -s * 0.7); ctx.lineTo(-s * 0.7, s * 0.7) }
      ctx.stroke()
      ctx.restore()
    }
    shade(ctx)
  }
}

export const SCENES = {
  '1': circuit,
  '2': doodles,
  '3': pixelLand,
  '4': synthwave,
  '5': space,
  '6': reemu,
}
