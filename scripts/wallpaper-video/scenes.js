// Cenas originais para a geração dos vídeos. t é a fase normalizada do loop.
// Use ciclos inteiros e desvaneça os elementos que voltam ao início do trajeto.
export const W = 1920
export const H = 1080

const TAU = Math.PI * 2
const frac = (x) => x - Math.floor(x)
const wave = (t, cycles = 1, phase = 0) => Math.sin(TAU * (t * cycles + phase))
const fade = (t) => Math.sin(Math.PI * t) ** 2

function rng(seed) {
  let state = seed
  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let n = Math.imul(state ^ (state >>> 15), state | 1)
    n ^= n + Math.imul(n ^ (n >>> 7), n | 61)
    return ((n ^ (n >>> 14)) >>> 0) / 4294967296
  }
}

function layer(paint, width = W, height = H) {
  const surface = new OffscreenCanvas(width, height)
  paint(surface.getContext('2d'))
  return surface
}

function linear(ctx, x0, y0, x1, y1, colors) {
  const gradient = ctx.createLinearGradient(x0, y0, x1, y1)
  colors.forEach((color, i) => gradient.addColorStop(i / (colors.length - 1), color))
  return gradient
}

function path(points, closed = false) {
  const shape = new Path2D()
  shape.moveTo(...points[0])
  for (const p of points.slice(1)) shape.lineTo(...p)
  if (closed) shape.closePath()
  return shape
}

function disc(ctx, x, y, radius, color) {
  ctx.fillStyle = color
  ctx.beginPath()
  ctx.arc(x, y, radius, 0, TAU)
  ctx.fill()
}

function box(ctx, x, y, width, height, radius, fill, stroke) {
  ctx.beginPath()
  ctx.roundRect(x, y, width, height, radius)
  if (fill) {
    ctx.fillStyle = fill
    ctx.fill()
  }
  if (stroke) {
    ctx.strokeStyle = stroke
    ctx.stroke()
  }
}

function halo(ctx, x, y, rx, ry, color, angle = 0) {
  ctx.save()
  ctx.translate(x, y)
  ctx.rotate(angle)
  ctx.scale(rx, ry)
  const light = ctx.createRadialGradient(0, 0, 0, 0, 0, 1)
  light.addColorStop(0, color)
  light.addColorStop(1, '#0000')
  ctx.fillStyle = light
  ctx.fillRect(-1, -1, 2, 2)
  ctx.restore()
}

function atmosphere(ctx, colors, light, x, y) {
  ctx.fillStyle = linear(ctx, 0, 0, W, H, colors)
  ctx.fillRect(0, 0, W, H)
  halo(ctx, x, y, 940, 660, light)
}

function shade(ctx) {
  ctx.fillStyle = linear(ctx, 0, H * 0.59, 0, H, ['#0000', '#0007'])
  ctx.fillRect(0, 0, W, H)
}

function trace(points) {
  const segments = points.slice(1).map(([x, y], i) => {
    const [fromX, fromY] = points[i]
    const dx = x - fromX
    const dy = y - fromY
    return { fromX, fromY, dx, dy, length: Math.hypot(dx, dy) }
  })
  return { points, segments, length: segments.reduce((sum, part) => sum + part.length, 0) }
}

function pointAt(route, phase) {
  let distance = phase * route.length
  for (const { fromX, fromY, dx, dy, length } of route.segments) {
    if (distance <= length) return [fromX + dx * distance / length, fromY + dy * distance / length]
    distance -= length
  }
  return route.points.at(-1)
}

// Recorta a nuvem com bordas suaves para a moldura da sacada ficar fixa.
function cloudBank(artwork, x, y, width, height) {
  return layer((ctx) => {
    ctx.drawImage(artwork, x, y, width, height, 0, 0, width, height)
    ctx.globalCompositeOperation = 'destination-in'
    ctx.translate(width / 2, height / 2)
    ctx.scale(width / 2, height / 2)
    const mask = ctx.createRadialGradient(0, 0, 0, 0, 0, 1)
    mask.addColorStop(0.61, '#fff')
    mask.addColorStop(1, '#fff0')
    ctx.fillStyle = mask
    ctx.fillRect(-1, -1, 2, 2)
  }, width, height)
}

function foliagePatch(artwork, points) {
  const padding = 16
  const xs = points.map(([x]) => x)
  const ys = points.map(([, y]) => y)
  const x = Math.max(0, Math.min(...xs) - padding)
  const y = Math.max(0, Math.min(...ys) - padding)
  const width = Math.min(artwork.naturalWidth, Math.max(...xs) + padding) - x
  const height = Math.max(...ys) + padding - y
  const outline = path(points.map(([px, py]) => [px - x, py - y]), true)
  const mask = layer((ctx) => {
    ctx.filter = 'blur(4px)'
    ctx.fill(outline)
  }, width, height)
  const texture = layer((ctx) => {
    ctx.drawImage(artwork, x, y, width, height, 0, 0, width, height)
    ctx.globalCompositeOperation = 'destination-in'
    ctx.drawImage(mask, 0, 0)
  }, width, height)
  return { texture, x, y }
}

// Circuito. A câmera fixa enquadra as nuvens, o sol e a cidade.
async function circuit() {
  const artwork = new Image()
  artwork.src = new URL('./circuito-lofi.png', import.meta.url).href
  await artwork.decode()

  // Coordenadas na ilustração de origem, antes da escala de saída.
  const clouds = [
    [358, 55, 535, 195, 10, 1.7, 0.08],
    [1090, 37, 515, 204, 14, 1.9, 0.33],
    [746, 121, 477, 121, 7, 1.1, 0.62],
  ].map(([x, y, width, height, drift, lift, phase]) => ({
    x, y, drift, lift, phase, texture: cloudBank(artwork, x, y, width, height),
  }))
  const plants = [
    // Flores perto da luminária, jardineira, flores da direita e folhagem alta.
    [[[101, 588], [146, 580], [157, 545], [195, 537], [252, 553], [273, 586],
      [288, 633], [274, 663], [246, 643], [155, 643], [109, 631]], 645, 0.052, 0.09],
    [[[273, 680], [306, 639], [435, 630], [469, 643], [502, 670], [550, 681],
      [576, 712], [555, 734], [504, 728], [450, 722], [380, 727], [276, 731]], 730, 0.052, 0.21],
    [[[1228, 766], [1260, 736], [1280, 717], [1282, 682], [1333, 677], [1375, 685],
      [1402, 728], [1418, 758], [1400, 787], [1293, 790], [1262, 805], [1229, 797]], 785, 0.051, 0.34],
    [[[1450, 683], [1472, 653], [1526, 621], [1550, 601], [1537, 577], [1567, 543],
      [1538, 501], [1572, 464], [1606, 457], [1623, 420], [1672, 415], [1672, 719],
      [1633, 749], [1612, 779], [1552, 780], [1485, 775], [1450, 749]], 782, 0.024, 0.45],
  ].map(([outline, rootY, strength, phase]) => ({
    ...foliagePatch(artwork, outline), rootY, strength, phase,
  }))
  const reflections = [
    // Tampo do processador, trilha de cobre e borda metálica da sacada.
    [[[938, 472], [1039, 419], [1238, 424], [1192, 482]], 1058, 451, 71, 25, -1.25],
    [[[975, 630], [1030, 656], [1143, 669], [1141, 675], [1025, 664], [972, 637]], 1038, 663, 72, 7, 0.12],
    [[[815, 785], [1156, 790], [1156, 806], [815, 799]], 1000, 796, 125, 12, 0.03],
  ].map(([outline, x, y, rx, ry, angle]) => ({ outline: path(outline, true), x, y, rx, ry, angle }))
  const streets = [
    [[1030, 554], [925, 636], [909, 648], [873, 687]],
    [[1056, 562], [979, 633], [1023, 660], [1164, 678]],
    [[1249, 531], [1271, 540], [1327, 547], [1348, 555], [1354, 579]],
    [[511, 646], [572, 655], [621, 662], [640, 661]],
    [[884, 746], [944, 750], [1007, 717], [1160, 722]],
  ].map(trace)
  const windows = [
    [580, 429], [975, 498], [1003, 500], [1113, 507], [1202, 495],
    [1056, 402], [1304, 617], [1456, 532],
  ]
  const rand = rng(619)
  const dust = Array.from({ length: 30 }, (_, i) => {
    const byLamp = i < 11
    return {
      x: byLamp ? 90 + 170 * rand() : 359 + 1145 * rand(),
      y: byLamp ? 464 + 187 * rand() : 76 + 396 * rand(),
      radius: 0.55 + rand() ** 2 * 1.35,
      drift: 13 + 43 * rand(),
      opacity: (byLamp ? 0.69 : 0.49) * (0.35 + 0.65 * rand()),
      phase: rand(),
    }
  })

  return (ctx, t) => {
    ctx.save()
    ctx.scale(W / artwork.naturalWidth, H / artwork.naturalHeight)
    ctx.drawImage(artwork, 0, 0)

    for (const { x, y, drift, lift, phase, texture } of clouds) {
      ctx.drawImage(texture, x + drift * wave(t, 1, phase), y + lift * wave(t, 1, phase + 0.25))
    }

    for (const { texture, x, y, rootY, strength, phase } of plants) {
      const tilt = strength * (0.76 * wave(t, 3, phase) + 0.24 * wave(t, 5, phase + 0.2))
      ctx.save()
      // A base fica parada e o deslocamento cresce em direção às pontas.
      ctx.transform(1, 0, -tilt, 1, tilt * rootY, 0)
      ctx.drawImage(texture, x, y)
      ctx.restore()
    }

    ctx.save()
    ctx.globalCompositeOperation = 'screen'
    const exposure = 0.72 + 0.28 * wave(t, 1, 0.16)
    ctx.globalAlpha = 0.10 * exposure
    halo(ctx, 1028, 290, 170, 89, '#ffac5f')
    ctx.globalAlpha = 0.20 * exposure
    for (const { outline, x, y, rx, ry, angle } of reflections) {
      ctx.save()
      ctx.clip(outline)
      halo(ctx, x, y, rx, ry, '#ffd092', angle)
      ctx.restore()
    }
    ctx.restore()

    for (const [i, street] of streets.entries()) {
      const phase = frac(t * (i % 2 ? 2 : 1) + i * 0.239)
      const copper = i === 2 ? '#a6ffdb' : '#ffe3a7'
      for (let tail = 3; tail >= 0; tail--) {
        const progress = phase - tail * 0.008
        if (progress <= 0) continue
        const [x, y] = pointAt(street, progress)
        ctx.globalAlpha = fade(progress) * (0.79 - 0.19 * tail)
        halo(ctx, x, y, 9, 7, copper)
        disc(ctx, x, y, 1.55 - 0.22 * tail, copper)
      }
    }

    for (const [i, [x, y]] of windows.entries()) {
      const pulse = 0.5 + 0.5 * wave(t, 1 + i % 2, i * 0.27)
      ctx.globalAlpha = 0.07 + 0.32 * pulse ** 3
      halo(ctx, x, y, 12, 8, '#ffc286')
      disc(ctx, x, y, 1.2, '#ffdb9f')
    }

    ctx.globalAlpha = 0.10 + 0.04 * wave(t, 1, 0.2)
    halo(ctx, 137, 516, 62, 78, '#ffc37f', 0.3)
    for (const mote of dust) {
      const life = frac(t + mote.phase)
      const x = mote.x + mote.drift * (life - 0.5) + 5 * wave(t, 2, mote.phase)
      const y = mote.y - mote.drift * 0.65 * (life - 0.5)
      ctx.globalAlpha = fade(life) * mote.opacity
      halo(ctx, x, y, mote.radius * 3.7, mote.radius * 3.7, '#ffbd8f')
      disc(ctx, x, y, mote.radius, '#ffe2bc')
    }
    ctx.restore()
  }
}

function symbol(kind, size) {
  const shape = new Path2D()
  if (kind === 'ring') shape.arc(0, 0, size, 0, TAU)
  if (kind === 'square') shape.roundRect(-size, -size, size * 2, size * 2, size * 0.16)
  if (kind === 'triangle') return path([[0, -size], [size, size * 0.82], [-size, size * 0.82]], true)
  if (kind === 'cross') {
    const arm = size * 0.84
    shape.moveTo(-arm, -arm)
    shape.lineTo(arm, arm)
    shape.moveTo(-arm, arm)
    shape.lineTo(arm, -arm)
  }
  return shape
}

function controlSprite(kind, color) {
  const outline = symbol(kind, 48)
  return layer((ctx) => {
    ctx.translate(96, 96)
    ctx.lineJoin = 'round'
    ctx.lineCap = 'round'
    ctx.strokeStyle = color
    for (const [width, opacity] of [[31, 0.05], [20, 0.11], [13, 0.27], [7, 1]]) {
      ctx.globalAlpha = opacity
      ctx.lineWidth = width
      ctx.stroke(outline)
    }
    ctx.globalAlpha = 0.8
    ctx.lineWidth = 2
    ctx.strokeStyle = '#f9e7ff'
    ctx.stroke(outline)
  }, 192, 192)
}

// Controles. Símbolos e rastros avançam de um ponto de fuga em profundidade.
function controls() {
  const sprites = [
    controlSprite('triangle', '#b582fa'),
    controlSprite('ring', '#ef82c2'),
    controlSprite('cross', '#82aef2'),
    controlSprite('square', '#f1b0f5'),
  ]
  const base = layer((ctx) => {
    atmosphere(ctx, ['#110d27', '#321b48', '#230f2e'], '#9d3e9a82', 1120, 515)
    halo(ctx, 1120, 515, 370, 235, '#ca73ce39')
  })
  const rand = rng(824)
  const marks = Array.from({ length: 80 }, (_, i) => ({
    sprite: sprites[i % sprites.length],
    angle: i * 2.39996 + rand() * 0.35,
    spread: 240 + rand() * 440,
    size: 20 + rand() * 49,
    roll: (rand() - 0.5) * 0.55,
    offset: (i + rand()) / 80,
  }))
  const streaks = Array.from({ length: 112 }, (_, i) => ({
    angle: rand() * TAU,
    spread: 180 + rand() * 750,
    offset: rand(),
    length: i < 14 ? 150 + rand() * 140 : 8 + rand() * 28,
    width: i < 14 ? 3.4 : 0.75 + rand() * 1.2,
    opacity: i < 14 ? 0.92 : 0.62,
    color: i % 2 ? '#c6a4f4' : '#f1b0e2',
  }))
  const perspective = (depth) => 1 / (0.24 + 3.4 * depth)
  const visibility = (depth) => Math.min(1, depth * 16, (1 - depth) * 5)

  return (ctx, t) => {
    ctx.drawImage(base, 0, 0)
    const centerX = 1060 + 43 * wave(t)
    const centerY = 519 + 35 * wave(t, 2, 0.16)
    const roll = 0.33 * wave(t, 2) + 0.10 * wave(t, 3, 0.2)
    const travel = 5 * t + 0.08 * wave(t, 2)
    ctx.lineCap = 'round'

    for (const streak of streaks) {
      const depth = frac(streak.offset - travel)
      const scale = perspective(depth)
      const angle = streak.angle + roll
      const dx = Math.cos(angle)
      const dy = Math.sin(angle)
      const distance = streak.spread * scale
      const tail = Math.min(distance * 0.84, streak.length * scale ** 1.45)
      ctx.beginPath()
      ctx.moveTo(centerX + dx * distance, centerY + dy * distance)
      ctx.lineTo(centerX + dx * (distance - tail), centerY + dy * (distance - tail))
      ctx.strokeStyle = streak.color
      ctx.lineWidth = Math.max(0.65, streak.width * scale)
      ctx.globalAlpha = visibility(depth) * streak.opacity * (1 - 0.58 * depth)
      ctx.stroke()
    }
    ctx.globalAlpha = 1

    const layers = marks.map(mark => ({ mark, depth: frac(mark.offset - travel) }))
    layers.sort((a, b) => b.depth - a.depth)
    for (const { mark, depth } of layers) {
      const scale = perspective(depth)
      const angle = mark.angle + roll
      const x = centerX + Math.cos(angle) * mark.spread * scale
      const y = centerY + Math.sin(angle) * mark.spread * scale * 0.82
      const radius = mark.size * scale
      if (x < -2 * radius || x > W + 2 * radius || y < -2 * radius || y > H + 2 * radius) continue

      ctx.save()
      ctx.translate(x, y)
      ctx.rotate(mark.roll + 0.66 * roll + 0.10 * wave(t, 2, mark.offset))
      ctx.globalAlpha = visibility(depth) * (0.36 + 0.64 * (1 - depth))
      ctx.drawImage(mark.sprite, -2 * radius, -2 * radius, 4 * radius, 4 * radius)
      ctx.restore()
    }
    shade(ctx)
  }
}

function pixelShape(ctx, color, points) {
  ctx.fillStyle = color
  ctx.fill(path(points, true))
}

function forestPine(colors, seed) {
  const rand = rng(seed)
  return layer((ctx) => {
    const [shadow, needles, lit, bark] = colors
    ctx.fillStyle = bark
    ctx.fillRect(22, 32, 5, 47)
    ctx.fillStyle = lit
    ctx.fillRect(22, 58, 1, 18)
    for (let level = 5; level >= 0; level--) {
      const y = 6 + level * 8
      const left = 4 + level * 3 + Math.floor(rand() * 4)
      const right = 4 + level * 3 + Math.floor(rand() * 4)
      const end = y + 18 + level
      pixelShape(ctx, shadow, [
        [24, y], [29, y + 7], [24 + right - 2, end - 7], [24 + right, end],
        [25 + Math.floor(right / 2), end - 2], [28, end + 2], [17, end],
        [24 - left, end + 1], [27 - left, end - 6], [20, y + 8],
      ])
      pixelShape(ctx, needles, [
        [24, y + 2], [25, y + 9], [22, end - 3], [15, end - 2],
        [13, end - 4], [25 - left, end - 1], [29 - left, end - 8], [21, y + 9],
      ])
      ctx.fillStyle = lit
      const tip = 25 - left + Math.floor(rand() * 4)
      ctx.fillRect(tip, end - 4, 2 + Math.floor(rand() * (2 + level)), 1)
    }
  }, 50, 80)
}

// Floresta 8-bit. A paisagem é composta em uma grade de 400×225.
function forest() {
  const w = 400
  const h = 225
  const tile = (width, paint) => layer((ctx) => {
    ctx.imageSmoothingEnabled = false
    for (const x of [-width, 0, width]) {
      ctx.save()
      ctx.translate(x, 0)
      paint(ctx)
      ctx.restore()
    }
  }, width, h)
  const sky = layer((ctx) => {
    const bands = ['#122840', '#1a394e', '#224b58', '#2c605f', '#2d716d', '#548478', '#8d9b72', '#c3ac6b']
    const dither = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
    for (let i = 0; i < bands.length; i++) {
      const y = i * 22
      ctx.fillStyle = bands[i]
      ctx.fillRect(0, y, w, h - y)
      if (!i) continue
      for (let dy = 0; dy < 8; dy++) {
        for (let x = 0; x < w; x++) {
          if (dither[dy % 4][x % 4] < 2 * (dy + 1)) ctx.fillRect(x, y - 8 + dy, 1, 1)
        }
      }
    }
    halo(ctx, 280, 70, 98, 62, '#f3d98053')
    for (let dy = -24; dy <= 24; dy++) {
      const half = Math.floor(Math.sqrt(24 ** 2 - dy ** 2))
      ctx.fillStyle = dy > 13 ? '#efad68' : dy > 4 ? '#f2bf72' : '#f7d98c'
      ctx.fillRect(280 - half, 70 + dy, half * 2 + 1, 1)
      if (dy === 4 || dy === 13) {
        ctx.fillStyle = dy === 13 ? '#efad68' : '#f2bf72'
        for (let x = 281 - half; x < 280 + half; x += 2) ctx.fillRect(x, 70 + dy, 1, 1)
      }
    }
  }, w, h)
  const clouds = tile(260, (ctx) => {
    for (const [x, y, width, height] of [[16, 55, 0.77, 0.61], [113, 31, 1.06, 1.13], [222, 74, 0.64, 0.83]]) {
      ctx.save()
      ctx.translate(x, y)
      ctx.scale(width, height)
      pixelShape(ctx, '#507b76', [
        [0, 13], [10, 9], [17, 9], [23, 5], [36, 3], [41, -2], [52, -2], [58, 0],
        [65, 0], [75, 7], [83, 8], [87, 12], [100, 15], [80, 17], [34, 16],
      ])
      pixelShape(ctx, '#a7a88b', [
        [8, 12], [19, 8], [30, 6], [39, 3], [42, -3], [52, -3], [61, 1],
        [68, 1], [74, 6], [80, 8], [91, 13], [72, 12], [65, 9], [52, 11], [41, 10], [27, 12],
      ])
      pixelShape(ctx, '#d6c7a0', [[33, 6], [42, -3], [52, -3], [61, 1], [68, 1], [74, 6], [62, 5], [52, 2], [45, 2]])
      ctx.restore()
    }
  })
  const mountains = tile(432, (ctx) => {
    pixelShape(ctx, '#4d716e', [
      [0, 134], [23, 121], [62, 91], [75, 88], [100, 109], [145, 131], [187, 117],
      [222, 89], [235, 81], [258, 101], [290, 120], [332, 128], [370, 100],
      [385, 105], [411, 126], [432, 134], [432, 186], [0, 186],
    ])
    pixelShape(ctx, '#3a6164', [
      [22, 163], [40, 148], [55, 131], [75, 120], [89, 105], [104, 111], [114, 105],
      [130, 120], [148, 127], [165, 146], [194, 164], [208, 175], [22, 175],
    ])
    pixelShape(ctx, '#708776', [
      [89, 105], [104, 111], [114, 105], [130, 120], [148, 127], [164, 144],
      [143, 135], [130, 133], [109, 119], [113, 129], [97, 123], [90, 115], [75, 120],
    ])
    pixelShape(ctx, '#507676', [[44, 150], [72, 129], [67, 143], [85, 147], [72, 155], [55, 154], [41, 163]])
    for (const [x, y, size] of [[236, 118, 0.58], [300, 83, 1]]) {
      ctx.save()
      ctx.translate(x, y)
      ctx.scale(size, size)
      pixelShape(ctx, '#30585e', [
        [-151, 90], [-118, 74], [-83, 55], [-50, 24], [-34, 24], [-12, 8], [0, 0],
        [21, 7], [46, 36], [64, 46], [82, 64], [134, 89], [134, 121], [-151, 121],
      ])
      pixelShape(ctx, '#607c6c', [
        [0, 0], [21, 7], [46, 36], [64, 46], [82, 64], [114, 79], [87, 73], [63, 67],
        [51, 56], [25, 43], [34, 59], [8, 54], [-7, 39], [0, 18], [-15, 19],
      ])
      pixelShape(ctx, '#89977c', [
        [0, 0], [21, 7], [39, 29], [20, 22], [14, 15], [18, 27], [2, 21], [-8, 24],
        [-4, 12], [-15, 18],
      ])
      pixelShape(ctx, '#426b69', [
        [-88, 58], [-60, 45], [-41, 29], [-51, 50], [-36, 57], [-59, 65],
        [-47, 72], [-75, 74], [-81, 67], [-116, 80],
      ])
      pixelShape(ctx, '#3e6968', [[-32, 79], [0, 63], [12, 68], [40, 75], [60, 89], [42, 89], [24, 85], [0, 88]])
      ctx.restore()
    }
  })
  const trees = [
    forestPine(['#244b47', '#305c4d', '#446452', '#29433d'], 710),
    forestPine(['#1d3936', '#2a5149', '#4b7060', '#243b35'], 761),
    forestPine(['#122a2b', '#254440', '#385749', '#172d2b'], 342),
    forestPine(['#122a2b', '#21413d', '#395d4e', '#172d2b'], 892),
  ]
  const woods = tile(512, (ctx) => {
    const rand = rng(375)
    for (let x = 4; x < 512; x += 5 + Math.floor(rand() * 6)) {
      const size = 12 + Math.floor(rand() * 25)
      ctx.drawImage(trees[0], x - 12, 172 - size, Math.round(size * 0.67), size)
    }
    for (let x = 15; x < 512; x += 15 + Math.floor(rand() * 17)) {
      const y = 173 + Math.round(4 * wave(x / 512, 3))
      const size = 28 + Math.floor(rand() * 33)
      ctx.drawImage(trees[1], x - 16, y - size, Math.round(size * 0.59), size)
    }
    ctx.fillStyle = '#244b47'
    ctx.fillRect(0, 171, 512, 6)
  })
  const bank = tile(640, (ctx) => {
    const rand = rng(249)
    const ground = x => Math.round(207 + 5 * wave(x / 640, 3, 0.19) + 2 * wave(x / 640, 10, 0.3))
    for (let x = 0; x < 640; x++) {
      const y = ground(x)
      ctx.fillStyle = '#112c2d'
      ctx.fillRect(x, y, 1, h - y)
      ctx.fillStyle = '#315346'
      ctx.fillRect(x, y, 1, 3)
      if (rand() < 0.57) {
        ctx.fillStyle = rand() < 0.29 ? '#78a06a' : '#42684d'
        ctx.fillRect(x, y - Math.floor(rand() * 5), 1, 3)
      }
      if (rand() < 0.36) {
        ctx.fillStyle = '#244139'
        ctx.fillRect(x, y + 5 + Math.floor(rand() * 9), 2, 1)
      }
    }
    const pines = [[22, 115], [73, 78], [187, 62], [352, 103], [398, 77], [523, 91], [588, 69]]
    for (const [i, [x, size]] of pines.entries()) {
      const y = ground(x) + 4
      const width = Math.round(size * 0.64)
      ctx.drawImage(trees[2 + i % 2], x - Math.floor(width / 2), y - size, width, size)
    }
    for (let x = 9; x < 640; x += 9 + Math.floor(rand() * 21)) {
      const y = ground(x)
      pixelShape(ctx, '#2c594c', [[x - 4, y], [x, y - 4], [x + 5, y - 6], [x + 9, y - 4], [x + 13, y + 1]])
      ctx.fillStyle = '#668b60'
      ctx.fillRect(x + 3, y - 5, 3, 1)
    }
  })
  const water = layer((ctx) => {
    ctx.fillStyle = linear(ctx, 0, 170, 0, 215, ['#385e55', '#4d887e', '#24465a'])
    ctx.fillRect(0, 170, w, 48)
    halo(ctx, 280, 179, 71, 39, '#c3ac6b48')
  }, w, h)
  const rand = rng(345)
  const ripples = Array.from({ length: 37 }, () => ({
    x: rand() * w, y: 174 + Math.floor(rand() * 34), length: 3 + Math.floor(rand() ** 2 * 38), phase: rand(),
  }))
  const flies = Array.from({ length: 24 }, () => [rand() * w, 158 + rand() * 61, rand()])
  const frame = new OffscreenCanvas(w, h)
  const pixel = frame.getContext('2d')
  pixel.imageSmoothingEnabled = false
  const scroll = (surface, t, initial) => {
    const offset = Math.round(frac(t + initial) * surface.width) % surface.width
    for (let x = -offset; x < w; x += surface.width) pixel.drawImage(surface, x, 0)
  }
  return (ctx, t) => {
    pixel.drawImage(sky, 0, 0)
    scroll(clouds, t, 0.09)
    scroll(mountains, t, 0.02)
    scroll(woods, t, 0.13)
    pixel.drawImage(water, 0, 0)
    for (const { x, y, length, phase } of ripples) {
      const left = Math.floor(frac(x / w - t * 2) * w)
      pixel.fillStyle = '#b4d5b6'
      pixel.globalAlpha = 0.10 + 0.10 * (0.5 + 0.5 * wave(t, 3, phase))
      pixel.fillRect(left, y, length, 1)
      pixel.fillRect(left - w, y, length, 1)
    }
    pixel.fillStyle = '#f3cf8c'
    for (let i = 0; i < 13; i++) {
      const length = 3 + i * 3 + Math.floor(3 * wave(t, 2, i * 0.31))
      const x = Math.floor(280 - length / 2 + (2 + i / 2) * wave(t, 2, i * 0.37))
      pixel.globalAlpha = (0.14 + 0.1 * wave(t, 3, i * 0.19)) * (1 - i / 15)
      pixel.fillRect(x, 174 + 2 * i, length, 1)
    }
    pixel.globalAlpha = 1
    scroll(bank, t, 0)
    for (const [x, y, phase] of flies) {
      const px = Math.floor(x + 13 * wave(t, 1, phase))
      const py = Math.floor(y + 5 * wave(t, 2, phase + 0.3))
      pixel.globalAlpha = 0.18 + 0.72 * (0.5 + 0.5 * wave(t, 3, phase)) ** 2
      halo(pixel, px, py, 4, 4, '#efffa68a')
      pixel.fillStyle = '#f8f0a7'
      pixel.fillRect(px, py, 1, 1)
    }
    pixel.globalAlpha = 1

    ctx.save()
    ctx.imageSmoothingEnabled = false
    ctx.drawImage(frame, 0, 0, W, H)
    ctx.restore()
  }
}

function spaceNebula(rand) {
  const frequencies = [5, 10, 21, 43, 86]
  const octaves = frequencies.map((frequency) => {
    const columns = frequency * 2 + 2
    const cells = Float32Array.from({ length: columns * (frequency + 2) }, rand)
    return (u, v) => {
      const x = u * frequency * 2
      const y = v * frequency
      const column = Math.floor(x)
      const row = Math.floor(y)
      const index = row * columns + column
      const dx = x - column
      const dy = y - row
      const sx = dx * dx * (3 - 2 * dx)
      const sy = dy * dy * (3 - 2 * dy)
      const top = cells[index] + (cells[index + 1] - cells[index]) * sx
      const bottom = cells[index + columns] + (cells[index + columns + 1] - cells[index + columns]) * sx
      return top + (bottom - top) * sy
    }
  })

  return layer((ctx) => {
    const { width, height } = ctx.canvas
    const pixels = ctx.createImageData(width, height)
    for (let y = 0; y < height; y++) {
      const v = y / height
      for (let x = 0; x < width; x++) {
        const u = x / width
        let cloud = 0
        for (const [i, sample] of octaves.entries()) cloud += sample(u, v) * 0.52 ** i
        cloud = Math.max(0, cloud - 0.46) ** 2
        const violet = cloud * Math.exp(-((v - 0.56 + 0.30 * Math.sin(5 * u)) ** 2) / 0.076)
        const blue = cloud * Math.exp(-((v - 0.15 - 0.55 * (1 - u) ** 2) ** 2) / 0.046)
        const vignette = 1 - 0.34 * ((u - 0.5) ** 2 * 2 + (v - 0.47) ** 2)
        const grain = rand() * 1.6
        const index = (y * width + x) * 4
        pixels.data[index] = (6 + 73 * violet + 18 * blue) * vignette + grain
        pixels.data[index + 1] = (3 + 18 * violet + 49 * blue) * vignette + grain
        pixels.data[index + 2] = (13 + 140 * violet + 132 * blue) * vignette + grain
        pixels.data[index + 3] = 255
      }
    }
    ctx.putImageData(pixels, 0, 0)
  }, 768, 432)
}

// Espaço. Nebulosa, estrelas e feixes sobre uma grade em perspectiva.
function space() {
  const rand = rng(558)
  const nebula = spaceNebula(rand)
  const horizon = H * 0.638
  const vanishingX = W * 0.49
  const sky = layer((ctx) => {
    ctx.drawImage(nebula, 0, 0, W, H)
    halo(ctx, W * 0.27, horizon - 85, W * 0.40, H * 0.20, '#5c129042', -0.18)
    halo(ctx, W * 0.72, H * 0.30, W * 0.39, H * 0.19, '#202ea44a', -0.66)
    ctx.fillStyle = linear(ctx, 0, horizon - 46, 0, H, ['#06030c00', '#080412df', '#050309'])
    ctx.fillRect(0, horizon - 46, W, H - horizon + 46)

    ctx.lineWidth = 1.35
    ctx.strokeStyle = linear(ctx, 0, horizon, 0, H, ['#34115500', '#7328b54d', '#8f36ccb3'])
    ctx.beginPath()
    for (let i = -62; i <= 62; i++) {
      ctx.moveTo(vanishingX, horizon - 50)
      ctx.lineTo(vanishingX + 46 * i, H)
    }
    ctx.stroke()

    for (let i = 0; i < 620; i++) {
      const x = rand() * W
      const y = rand() * H
      const size = 0.43 + rand() ** 5 * 1.55
      ctx.globalAlpha = (0.20 + rand() * 0.62) * (y < horizon ? 1 : 0.55)
      disc(ctx, x, y, size, i % 5 ? '#e2e7ff' : '#dcaffd')
    }
  })
  const twinkles = Array.from({ length: 43 }, () => ({
    x: 25 + rand() * (W - 50),
    y: 25 + rand() * (H - 67),
    size: 0.8 + rand() ** 2 * 1.75,
    drift: 5 + rand() * 18,
    phase: rand(),
  }))
  const beams = [
    [W * 0.26, horizon + 25, W * 0.47, 17, '#ae28fc', 0.82, 0.12],
    [W * 0.09, horizon + 13, W * 0.29, 10, '#6b57ff', 0.66, 0.28],
    [W * 0.41, horizon - 8, W * 0.21, 8, '#346cff', 0.74, 0.50],
    [W * 0.89, H * 0.522, W * 0.26, 14, '#a02bfa', 0.78, 0.65],
  ]

  return (ctx, t) => {
    ctx.drawImage(sky, 0, 0)
    const rows = 29
    const travel = frac(t * 4)
    ctx.lineWidth = 1.1
    for (let i = 0; i < rows; i++) {
      const distance = (H - horizon) * 1.18 ** (i + travel + 1 - rows)
      const y = horizon + distance
      if (y >= H) continue
      const opacity = 0.60 * (distance / (H - horizon)) ** 0.68 * Math.min(1, (H - y) / 24)
      ctx.strokeStyle = 'rgba(142,58,199,' + opacity + ')'
      ctx.beginPath()
      ctx.moveTo(0, y)
      ctx.lineTo(W, y)
      ctx.stroke()
    }

    for (const [x, y, width, height, tint, brightness, phase] of beams) {
      const cx = x + 58 * wave(t, 1, phase)
      const cy = y + 5 * wave(t, 2, phase)
      ctx.globalAlpha = brightness * (0.86 + 0.14 * wave(t, 2, phase))
      halo(ctx, cx, cy, width, height * 3.6, tint + '35')
      halo(ctx, cx, cy, width, height, tint + 'b8')
      halo(ctx, cx, cy, width * 0.61, height * 0.29, tint)
      halo(ctx, cx, cy, width * 0.12, height * 0.20, '#dbc7ff8c')
    }

    for (const { x, y, size, drift, phase } of twinkles) {
      const cx = x + drift * wave(t, 1, phase)
      const cy = y + drift * 0.58 * wave(t, 1, phase + 0.19)
      ctx.globalAlpha = 0.30 + 0.58 * (0.5 + 0.5 * wave(t, 2, phase)) ** 2
      halo(ctx, cx, cy, size * 5, size * 5, '#a8a9ff65')
      disc(ctx, cx, cy, size, '#f0e7ff')
    }
    ctx.globalAlpha = 1
  }
}

// ReEmu. Feixes de partículas em três profundidades, com a paleta de branding/.
function reemuParticles() {
  const rand = rng(613)
  const colors = ['#3ddc2f', '#27b8c9', '#1e8bff']
  const backdrop = layer((ctx) => {
    ctx.fillStyle = '#030605'
    ctx.fillRect(0, 0, W, H)
    halo(ctx, 257, 545, 989, 704, '#195d2acd', -0.2)
    halo(ctx, 582, 565, 744, 550, '#13695862', 0.18)
    halo(ctx, 1136, 608, 826, 468, '#0c3b785c', -0.12)
    halo(ctx, 321, 479, 484, 304, '#3ddc2f33', -0.34)
    halo(ctx, 946, 638, 668, 301, '#1e8bff25', 0.22)

    // A névoa é pré-composta em manchas de tamanhos diferentes.
    for (let i = 0; i < 122; i++) {
      const x = W * (rand() ** 1.5 * 0.91 - 0.03)
      const y = H * (0.17 + rand() * 0.73)
      const size = 15 + rand() ** 2 * 95
      const tint = x < W * 0.29 ? colors[0] : x < W * 0.52 ? colors[1] : colors[2]
      halo(ctx, x, y, size * 1.65, size, tint + (i % 3 ? '12' : '1b'), rand() * 2 - 1)
    }
  })

  const sprites = colors.map(tint => [
    [[0, '#edfffa'], [0.22, '#d9fff5'], [0.39, tint], [0.68, tint + '6d'], [1, tint + '00']],
    [[0, tint + 'e0'], [0.28, tint + 'ab'], [0.63, tint + '37'], [1, tint + '00']],
    [[0, tint + 'bc'], [0.50, tint + '89'], [0.64, tint + 'b8'], [0.82, tint + '33'], [1, tint + '00']],
  ].map(stops => layer((ctx) => {
    const light = ctx.createRadialGradient(32, 32, 0, 32, 32, 32)
    for (const [at, color] of stops) light.addColorStop(at, color)
    ctx.fillStyle = light
    ctx.fillRect(0, 0, 64, 64)
  }, 64, 64)))

  const streams = [0.17, 0.31, 0.47, 0.66, 0.83]
  const clusters = [0.07, 0.14, 0.22, 0.31, 0.42, 0.54]
  const particles = [
    { count: 1840, size: 1.15, spread: 2.45, travel: 166, sway: 16, alpha: 0.93, sprite: 0 },
    { count: 220, size: 2.4, spread: 5.7, travel: 228, sway: 33, alpha: 0.66, sprite: 1 },
    { count: 52, size: 5.5, spread: 22, travel: 348, sway: 49, alpha: 0.74, sprite: 2 },
  ].flatMap(({ count, size, spread, travel, sway, alpha, sprite }) =>
    Array.from({ length: count }, (_, index) => {
      const clustered = sprite === 0 && index % 3 !== 0
      const x = clustered
        ? W * (clusters[Math.floor(rand() * clusters.length)] + (rand() - rand()) * 0.18)
        : W * (rand() ** 1.55 * 1.13 - 0.10)
      const stream = index % streams.length
      const arc = 0.091 * wave(x / W, 0.64, stream * 0.21)
      const scatter = (rand() + rand() + rand() - 1.5) * (clustered ? 0.077 : 0.205)
      const y = H * (streams[stream] + arc + scatter + 0.084 * x / W)
      const radius = size + spread * rand() ** 2
      const hue = x / W + (rand() - 0.5) * 0.34
      const color = hue < 0.28 ? 0 : hue < 0.56 ? 1 : 2
      return {
        x, y, radius, phase: rand(),
        sprite: sprites[color][sprite === 2 && index % 3 === 0 ? 1 : sprite],
        cycles: sprite === 2 && index % 3 === 1 ? 2 : 1,
        drift: travel * (0.54 + rand() * 0.85),
        sway: sway * (0.6 + rand() * 0.6),
        intensity: alpha * (0.25 + rand() * 0.75),
      }
    }),
  )

  return (ctx, t) => {
    ctx.drawImage(backdrop, 0, 0)
    halo(ctx, 332 + 39 * wave(t, 1), 530, 587, 300, '#34b83d17', -0.18)
    halo(ctx, 1000, 565 + 38 * wave(t, 1, 0.35), 724, 377, '#1e8bff18', 0.2)
    for (const { x, y, radius, phase, sprite, cycles, drift, sway, intensity } of particles) {
      const life = frac(t * cycles + phase)
      const opacity = Math.min(1, 3 * fade(life))
      const px = x + (life - 0.5) * drift
      const py = y + (life - 0.5) * drift * 0.24 + sway * wave(life, 1, phase)
      ctx.globalAlpha = intensity * opacity * (0.84 + 0.16 * wave(t, 2, phase))
      ctx.drawImage(sprite, px - radius, py - radius, 2 * radius, 2 * radius)
    }
    ctx.globalAlpha = 1
  }
}

export const SCENES = {
  '1': circuit,
  '2': controls,
  '3': forest,
  '5': space,
  '6': reemuParticles,
}
