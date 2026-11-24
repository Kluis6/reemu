// Paisagem synthwave do Fliperama. A cena é desenhada só no gerador de vídeo.
const TAU = 2 * Math.PI
const COURSE = 2400
const ROW = 48
const ROAD = 70
const EYE = 45
const FAR = 1920
const OFFSETS = [83, 116, 159, 217, 298, 405, 550, 747, 992, 1310, 1740]

const wrap = (value, range) => ((value % range) + range) % range
const mix = (a, b, fraction) => a + (b - a) * fraction
function smooth(from, to, value) {
  const t = Math.max(0, Math.min(1, (value - from) / (to - from)))
  return t * t * (3 - 2 * t)
}

function random(seed) {
  return () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0
    return seed / 4294967296
  }
}

function polygon(ctx, points) {
  ctx.beginPath()
  ctx.moveTo(...points[0])
  for (const point of points.slice(1)) ctx.lineTo(...point)
  ctx.closePath()
}

function radial(ctx, x, y, rx, ry, stops) {
  ctx.save()
  ctx.translate(x, y)
  ctx.scale(rx, ry)
  const gradient = ctx.createRadialGradient(0, 0, 0, 0, 0, 1)
  for (const [offset, color] of stops) gradient.addColorStop(offset, color)
  ctx.fillStyle = gradient
  ctx.fillRect(-1, -1, 2, 2)
  ctx.restore()
}

function reflectSun(ctx, { center, sunX, horizon, width, height, focal, near, travel }) {
  const reflectionX = mix(center, sunX, 0.52)
  radial(ctx, reflectionX, horizon + height * 0.16, width * 0.13, height * 0.31, [
    [0, 'rgba(227,94,166,0.26)'],
    [0.36, 'rgba(183,68,143,0.15)'],
    [1, 'rgba(153,63,136,0)'],
  ])
  radial(ctx, reflectionX, horizon + height * 0.10, width * 0.055, height * 0.24, [
    [0, 'rgba(255,145,186,0.24)'],
    [0.47, 'rgba(228,104,163,0.11)'],
    [1, 'rgba(168,71,145,0)'],
  ])

  // Faixas de luz sobre o asfalto. As posições repetem após uma volta da pista.
  const spacing = 16
  for (let row = Math.floor((travel + 830) / spacing); row * spacing > travel + near; row--) {
    const rng = random(307 + wrap(row, COURSE / spacing) * 1151)
    const depth = row * spacing - travel + rng() * 12
    const scale = focal / depth
    const y = horizon + EYE * scale
    const spread = (5 + rng() * 14) * scale
    const x = reflectionX + (rng() - 0.5) * 13 * scale
    const opacity = (0.10 + rng() * 0.13) * smooth(horizon, horizon + height * 0.12, y)
      * (1 - smooth(height * 0.78, height * 1.08, y))
    const thickness = Math.max(0.65, EYE * scale * (2 + rng() * 4) / depth)
    const streak = ctx.createLinearGradient(x - spread, y, x + spread, y)
    streak.addColorStop(0, 'rgba(229,114,128,0)')
    streak.addColorStop(0.43, 'rgba(250,163,109,' + opacity + ')')
    streak.addColorStop(1, 'rgba(229,114,128,0)')
    ctx.fillStyle = streak
    ctx.fillRect(x - spread, y, 2 * spread, thickness)
  }
}

function makeSky(width, height, horizon) {
  const sky = document.createElement('canvas')
  sky.width = width
  sky.height = height
  const ctx = sky.getContext('2d')
  const backdrop = ctx.createLinearGradient(0, 0, 0, horizon + height * 0.1)
  backdrop.addColorStop(0, '#190a20')
  backdrop.addColorStop(0.44, '#401333')
  backdrop.addColorStop(0.77, '#5d2152')
  backdrop.addColorStop(1, '#855184')
  ctx.fillStyle = backdrop
  ctx.fillRect(0, 0, width, height)
  radial(ctx, width * 0.51, height * 0.45, width * 0.52, height * 0.44, [
    [0, 'rgba(234,34,134,0.28)'], [0.39, 'rgba(196,26,118,0.15)'], [1, 'rgba(164,29,116,0)'],
  ])
  const rng = random(79)
  for (let i = 0; i < 155; i++) {
    const x = rng() * width
    const y = rng() * horizon * 0.75
    ctx.fillStyle = 'rgba(220,193,249,' + (0.12 + rng() * 0.39) + ')'
    const size = rng() > 0.94 ? 1.95 : 0.65 + rng() * 1.1
    ctx.fillRect(x, y, size, size)
  }
  return sky
}

function makeSun(height) {
  const radius = height * 0.35
  const center = Math.ceil(radius * 1.81)
  const sun = document.createElement('canvas')
  sun.width = sun.height = center * 2
  const ctx = sun.getContext('2d')

  radial(ctx, center, center, radius * 1.8, radius * 1.8, [
    [0.48, '#ff3897a1'], [0.63, '#f5228f6e'], [0.82, '#c81c8734'], [1, '#aa1b7a00'],
  ])
  radial(ctx, center, center, radius * 1.15, radius * 1.15, [
    [0.82, '#ff88c49c'], [0.89, '#ff63b66e'], [1, '#f647ab00'],
  ])

  const light = ctx.createLinearGradient(0, center - radius, 0, center + radius)
  light.addColorStop(0, '#ffe6ac')
  light.addColorStop(0.24, '#ffb783')
  light.addColorStop(0.47, '#ff78ac')
  light.addColorStop(0.66, '#f650c4')
  light.addColorStop(1, '#cb30a9')
  ctx.fillStyle = light
  ctx.beginPath()
  ctx.arc(center, center, radius, 0, TAU)
  ctx.fill()
  return sun
}

function makeScanlines(height, horizon) {
  const scanlines = document.createElement('canvas')
  scanlines.width = 1
  scanlines.height = height
  const ctx = scanlines.getContext('2d')
  // A textura fixa imita o CRT sem cintilação ou recortes no disco solar.
  const pitch = height / 108
  for (let y = 0; y < height; y++) {
    const trough = (0.5 + 0.5 * Math.cos(TAU * (y + 0.5) / pitch)) ** 2
    const strength = mix(0.32, 0.10, smooth(horizon, height * 0.88, y))
    ctx.fillStyle = 'rgba(30,8,38,' + (strength * trough) + ')'
    ctx.fillRect(0, y, 1, 1)
  }
  return scanlines
}

function terrainHeight(offset, row, side) {
  const z = row * ROW / COURSE
  const crest = 340 + 175 * Math.sin(TAU * (2 * z + side * 0.2))
  const ridge = Math.exp(-Math.pow((offset - crest) / 195, 2))
  const outerRidge = Math.exp(-Math.pow((offset - 870 + 135 * Math.sin(TAU * (z + side * 0.19))) / 290, 2))
  const jagged = Math.sin(TAU * (9 * z + offset * 0.0023 + side * 0.37))
    * Math.cos(TAU * (4 * z - offset * 0.0041))
  return smooth(125, 265, offset) * (ridge * 91 + outerRidge * 116)
    * (0.73 + 0.23 * Math.sin(TAU * (5 * z + side * 0.09)) + 0.21 * jagged)
}

function makeSkyline(width, height) {
  const rng = random(712)
  const canvas = document.createElement('canvas')
  canvas.width = Math.round(width * 0.56)
  canvas.height = Math.round(height * 0.19)
  const ctx = canvas.getContext('2d')
  ctx.scale(canvas.width, canvas.height)

  // Bairros distantes com perfis mais claros que os edifícios à frente.
  ctx.fillStyle = '#714662'
  for (let x = 0.04; x < 0.96;) {
    const span = 0.012 + rng() * 0.025
    const density = Math.sin(Math.PI * x) ** 2
    const top = 0.95 - (0.10 + rng() * 0.40) * density
    ctx.fillRect(x, top, span, 1 - top)
    ctx.fillRect(x + span * 0.20, top - 0.025 - rng() * 0.045, span * 0.62, 0.07)
    x += span + 0.003 + rng() * 0.009
  }

  const facade = ctx.createLinearGradient(0, 0, 0, 1)
  facade.addColorStop(0, '#291933')
  facade.addColorStop(0.72, '#3a2440')
  facade.addColorStop(1, '#512e46')
  ctx.fillStyle = facade
  for (let x = 0.07; x < 0.93;) {
    const span = 0.015 + rng() * 0.025
    const density = Math.sin(Math.PI * x) ** 1.4
    const top = 0.98 - (0.12 + rng() * 0.28) * density
    ctx.fillRect(x, top, span, 1 - top)
    ctx.fillRect(x + span * 0.18, top - 0.032, span * 0.53, 0.036)
    x += span + 0.002 + rng() * 0.005
  }

  // Torres desenhadas com recuos e coberturas diferentes para marcar o horizonte.
  const towers = [
    [0.205, 0.030, 0.62, 0],
    [0.281, 0.039, 0.46, 0.10],
    [0.352, 0.047, 0.32, 0],
    [0.432, 0.039, 0.50, 0.13],
    [0.509, 0.049, 0.20, 0.16],
    [0.578, 0.029, 0.40, 0],
    [0.635, 0.050, 0.37, 0.08],
    [0.719, 0.034, 0.48, 0],
    [0.796, 0.024, 0.64, 0.08],
  ]
  for (const [x, span, top, mast] of towers) {
    const right = x + span
    const shoulder = top + span * 1.1
    ctx.fillStyle = facade
    polygon(ctx, [
      [x, 1], [x, shoulder + 0.045], [x + span * 0.20, shoulder + 0.045],
      [x + span * 0.20, shoulder], [x + span * 0.40, top], [right - span * 0.15, top],
      [right - span * 0.15, shoulder], [right, shoulder], [right, 1],
    ])
    ctx.fill()
    if (mast) ctx.fillRect(x + span * 0.60, top - mast, 0.0022, mast + 0.008)
    ctx.fillStyle = 'rgba(190,118,143,0.16)'
    ctx.fillRect(right - span * 0.15, shoulder + 0.012, span * 0.085, (1 - shoulder) * 0.92)

    ctx.fillStyle = 'rgba(251,180,139,0.45)'
    for (let y = shoulder + 0.072; y < 0.93; y += 0.046) {
      for (let col = 1; col <= 4; col++) {
        if (rng() > 0.63) ctx.fillRect(x + span * col * 0.18, y, span * 0.071, 0.010)
      }
    }
  }

  // A névoa suaviza só a base, preservando a silhueta diante do sol.
  ctx.globalCompositeOperation = 'destination-in'
  const haze = ctx.createLinearGradient(0, 0, 0, 1)
  haze.addColorStop(0.84, '#000')
  haze.addColorStop(1, 'rgba(0,0,0,0.20)')
  ctx.fillStyle = haze
  ctx.fillRect(0, 0, 1, 1)
  return canvas
}

export function createFliperama(width, height) {
  const horizon = height * 0.535
  const focal = height * 0.92
  const sky = makeSky(width, height, horizon)
  const sun = makeSun(height)
  const sunX = width * 0.512
  const scanlines = makeScanlines(height, horizon)
  const skyline = makeSkyline(width, height)
  const rng = random(314)
  const clouds = Array.from({ length: 12 }, () => ({
    x: rng() * width, y: height * (0.22 + 0.29 * rng()),
    width: width * (0.14 + 0.19 * rng()), phase: rng(),
  }))

  const rows = COURSE / ROW
  const terrain = [-1, 1].map(side => Array.from({ length: rows }, (_, row) =>
    OFFSETS.map(offset => terrainHeight(offset, row, side))
  ))

  return {
    draw(ctx, t) {
      const phase = wrap(t, 1)
      const angle = TAU * phase
      const travel = COURSE * phase + 12 * Math.sin(angle)
      const center = width * (0.5 + 0.013 * Math.sin(angle) + 0.004 * Math.sin(2 * angle))
      const project = (x, y, z) => [center + focal * x / z, horizon + focal * (EYE - y) / z]
      const near = 46
      ctx.drawImage(sky, 0, 0)
      ctx.drawImage(sun, sunX - sun.width / 2, height * 0.443 - sun.height / 2)
      for (const cloud of clouds) {
        radial(ctx, cloud.x + width * 0.045 * Math.sin(angle + cloud.phase * TAU), cloud.y,
          cloud.width, height * 0.009, [[0, 'rgba(103,42,113,0.12)'], [1, 'rgba(90,42,97,0)']])
      }

      const ground = ctx.createLinearGradient(0, horizon - height * 0.04, 0, height)
      ground.addColorStop(0, '#653252')
      ground.addColorStop(0.44, '#281d3c')
      ground.addColorStop(1, '#151427')
      ctx.fillStyle = ground
      ctx.fillRect(0, horizon, width, height - horizon)

      // Cordilheiras estáveis ao longe. As malhas mais próximas avançam em perspectiva.
      for (const [level, color] of [[0, '#69405d'], [1, '#472944']]) {
        ctx.fillStyle = color
        ctx.beginPath()
        ctx.moveTo(0, horizon + 2)
        for (let i = 0; i <= 72; i++) {
          const u = i / 72
          const valley = smooth(0.035, 0.32, Math.abs(u - 0.51))
          const peak = Math.abs(Math.sin(u * 18 + level * 0.7)) ** 1.7
            + 0.25 * Math.sin(u * 57 + level) + 0.15 * Math.cos(u * 149)
          const y = horizon - height * valley * (0.044 + peak * (0.065 - level * 0.012))
          ctx.lineTo(u * width, y)
        }
        ctx.lineTo(width, horizon + 2)
        ctx.fill()
      }

      // A cidade cobre o sol e o pé das montanhas antes do terreno da pista.
      const cityX = (width - skyline.width) / 2 + (center - width * 0.5) * 0.28
      ctx.drawImage(skyline, cityX, horizon - skyline.height + 2)

      for (let row = Math.floor((travel + FAR) / ROW); row * ROW - travel > near - ROW; row--) {
        const rowZ = row * ROW - travel
        const z1 = Math.max(near, rowZ)
        const z2 = Math.min(FAR, rowZ + ROW)
        if (z2 <= z1) continue
        const a = wrap(row, rows)
        const b = (a + 1) % rows
        const faded = smooth(260, FAR, z2)
        const front = (z1 - rowZ) / ROW
        const back = (z2 - rowZ) / ROW
        ctx.lineWidth = (height / 900) * mix(1.65, 0.85, faded)
        const lineOpacity = mix(0.58, 0.18, faded)
        for (const [index, side] of [[0, -1], [1, 1]]) {
          const tint = side === -1 ? '248,116,203' : '135,168,247'
          ctx.strokeStyle = 'rgba(' + tint + ',' + lineOpacity + ')'
          const p = terrain[index][a]
          const q = terrain[index][b]
          for (let j = 0; j < OFFSETS.length - 1; j++) {
            const x1 = OFFSETS[j] * side
            const x2 = OFFSETS[j + 1] * side
            const vertices = [
              project(x1, mix(p[j], q[j], front), z1),
              project(x2, mix(p[j + 1], q[j + 1], front), z1),
              project(x2, mix(p[j + 1], q[j + 1], back), z2),
              project(x1, mix(p[j], q[j], back), z2),
            ]
            if (side === -1 && vertices.every(([x]) => x < -2)) break
            if (side === 1 && vertices.every(([x]) => x > width + 2)) break
            polygon(ctx, vertices)
            const heightShade = (p[j] + q[j]) / 21
            const r = Math.round(mix(32 + heightShade * 4, 88, faded))
            const g = Math.round(mix(23 + heightShade, 45, faded))
            const blue = Math.round(mix(43 + heightShade * 3, 76, faded))
            ctx.fillStyle = 'rgb(' + r + ',' + g + ',' + blue + ')'
            ctx.fill()
            ctx.stroke()
          }
        }
      }

      const asphalt = ctx.createLinearGradient(0, horizon, 0, height)
      asphalt.addColorStop(0, '#805061')
      asphalt.addColorStop(0.30, '#23192d')
      asphalt.addColorStop(1, '#100f20')
      const roadShape = (x1, x2, z1, z2) =>
        polygon(ctx, [project(x1, 0, z1), project(x2, 0, z1), project(x2, 0, z2), project(x1, 0, z2)])
      roadShape(-OFFSETS[0] - 0.5, OFFSETS[0] + 0.5, near, FAR * 7)
      ctx.fillStyle = asphalt
      ctx.fill()
      ctx.save()
      roadShape(-ROAD, ROAD, near, FAR * 7)
      ctx.clip()
      reflectSun(ctx, { center, sunX, horizon, width, height, focal, near, travel })
      ctx.restore()

      for (const side of [-1, 1]) {
        roadShape(side * ROAD, side * OFFSETS[0], near, FAR)
        ctx.fillStyle = side === -1 ? '#362336' : '#282638'
        ctx.fill()
        const tint = side === -1 ? '242,119,165' : '125,194,218'
        // Faixas em perspectiva, com luz concentrada no contorno.
        for (const [radius, peak] of [[5.4, 0.05], [3.2, 0.10], [1.65, 0.16]]) {
          roadShape(side * (ROAD - radius), side * (ROAD + radius), near, FAR)
          const halo = ctx.createLinearGradient(0, horizon, 0, height)
          halo.addColorStop(0, 'rgba(' + tint + ',0)')
          halo.addColorStop(0.44, 'rgba(' + tint + ',' + peak + ')')
          halo.addColorStop(1, 'rgba(' + tint + ',' + peak * 0.72 + ')')
          ctx.fillStyle = halo
          ctx.fill()
        }
        roadShape(side * (ROAD - 0.55), side * (ROAD + 0.55), near, FAR)
        const edge = ctx.createLinearGradient(0, horizon, 0, height)
        edge.addColorStop(0, '#804859')
        edge.addColorStop(0.35, side === -1 ? '#eaa2b9' : '#a8ccd8')
        edge.addColorStop(1, side === -1 ? '#b77291' : '#7aabb9')
        ctx.fillStyle = edge
        ctx.fill()
      }
      for (let i = Math.floor((travel + FAR) / 80); i >= Math.floor((travel + near) / 80); i--) {
        const start = Math.max(i * 80 - travel, near)
        const end = Math.min(i * 80 - travel + 31, FAR)
        if (end <= start) continue
        roadShape(-0.8, 0.8, start, end)
        ctx.fillStyle = 'rgba(229,171,154,' + (0.43 * (1 - smooth(300, 1400, end))) + ')'
        ctx.fill()
      }

      radial(ctx, center, horizon - height * 0.005, width * 0.31, height * 0.067, [
        [0, '#bd6ac75c'], [0.36, '#986fc632'], [1, '#6a6ac000'],
      ])
      radial(ctx, center, horizon, width * 0.28, height * 0.025, [
        [0, '#73caff6b'], [0.28, '#6aaef93f'], [1, '#5682ea00'],
      ])
      ctx.drawImage(scanlines, 0, 0, width, height)
    },
  }
}
