/**
 * 5 papéis de parede embutidos, um por avatar (`avatars.ts`): o mesmo
 * degradê e o mesmo personagem, cada um no cenário dele. Arte original em
 * SVG (1920×1080), sem material de terceiros; as referências de tema vieram
 * de bancos de imagens (Freepik, Vecteezy: placa de circuito, padrão de
 * controles em traço, paisagem 8-bit, synthwave com grade, espaço), só como
 * ideia (pedido do usuário, 2026-11-14).
 *
 * Escolher um grava o SVG como `wallpaper.svg` (`set_wallpaper_preset`); o
 * atributo `data-reemu-wallpaper` na raiz diz qual é, pra tela marcar o
 * escolhido.
 */
import { AVATAR_GRADIENTS, presetArt, PRESET_IDS, type PresetId } from './avatars'

export const WALLPAPER_IDS = PRESET_IDS
export type WallpaperId = PresetId

const W = 1920
const H = 1080

/** Gerador determinístico (mulberry32): o mesmo desenho a cada build. */
function rng(seed: number) {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const r1 = (n: number) => Math.round(n * 10) / 10

/** O personagem do avatar `id` com o centro em (cx, cy) e `size` px de lado. */
function hero(id: PresetId, cx: number, cy: number, size: number) {
  const k = size / 64
  return `<g transform="translate(${r1(cx - size / 2)} ${r1(cy - size / 2)}) scale(${r1(k * 100) / 100})">${presetArt(id)}</g>`
}

/** Fundo comum: o degradê do avatar, uma luz suave atrás do personagem e
 *  um escurecido embaixo (o texto da Início fica legível). */
function base(id: PresetId, glowX: number, glowY: number, glow: string) {
  const [a, b] = AVATAR_GRADIENTS[id]
  return (
    `<defs>` +
    `<linearGradient id="bg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/></linearGradient>` +
    `<radialGradient id="glow" cx="${glowX / W}" cy="${glowY / H}" r="0.45"><stop offset="0" stop-color="${glow}" stop-opacity="0.45"/><stop offset="1" stop-color="${glow}" stop-opacity="0"/></radialGradient>` +
    `<linearGradient id="shade" x1="0" y1="0" x2="0" y2="1"><stop offset="0.45" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.45"/></linearGradient>` +
    `</defs>` +
    `<rect width="${W}" height="${H}" fill="url(#bg)"/>`
  )
}
const glowLayer = `<rect width="${W}" height="${H}" fill="url(#glow)"/>`
const shadeLayer = `<rect width="${W}" height="${H}" fill="url(#shade)"/>`

// 1. Robô: placa de circuito — trilhas com curvas de 45°, ilhas e LEDs.
function circuit(): string {
  const rand = rng(11)
  let out = '<g fill="none" stroke="#e0f2fe" stroke-opacity="0.16" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">'
  const dots: string[] = []
  for (let i = 0; i < 46; i++) {
    let x = Math.round(rand() * 48) * 40
    let y = Math.round(rand() * 27) * 40
    let d = `M${x} ${y}`
    for (let s = 0; s < 4; s++) {
      const len = (2 + Math.floor(rand() * 6)) * 40
      const dir = Math.floor(rand() * 4)
      if (dir === 0) x += len
      else if (dir === 1) y += len
      else if (dir === 2) { x += len / 2; y += len / 2 }
      else { x += len / 2; y -= len / 2 }
      d += `L${x} ${y}`
    }
    out += `<path d="${d}"/>`
    dots.push(`<circle cx="${x}" cy="${y}" r="9"/>`)
  }
  out += '</g>'
  out += `<g fill="none" stroke="#e0f2fe" stroke-opacity="0.22" stroke-width="4">${dots.join('')}</g>`
  // LEDs acesos
  let leds = '<g fill="#22d3ee">'
  for (let i = 0; i < 14; i++) {
    const x = Math.round(rand() * 48) * 40
    const y = Math.round(rand() * 27) * 40
    leds += `<circle cx="${x}" cy="${y}" r="6" opacity="0.8"/><circle cx="${x}" cy="${y}" r="16" opacity="0.15"/>`
  }
  leds += '</g>'
  // chips
  let chips = '<g fill="#0f172a" fill-opacity="0.25" stroke="#e0f2fe" stroke-opacity="0.2" stroke-width="3">'
  for (const [x, y, w, h] of [[200, 160, 200, 120], [760, 720, 160, 160], [1240, 120, 220, 110]]) {
    chips += `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="12"/>`
  }
  chips += '</g>'
  return out + chips + leds
}

// 2. Controle: padrão de ícones de jogo em traço (cruz, botões, controles).
function doodles(): string {
  const rand = rng(22)
  const icons = [
    // direcional
    '<path d="M-10 -30h20v20h20v20h-20v20h-20v-20h-20v-20h20z"/>',
    // quatro botões
    '<circle cx="0" cy="-22" r="9"/><circle cx="22" cy="0" r="9"/><circle cx="-22" cy="0" r="9"/><circle cx="0" cy="22" r="9"/>',
    // controle pequeno
    '<path d="M-34 -14h68c8 0 13 6 14 13l3 16c1 9-9 14-15 8l-8-8h-56l-8 8c-6 6-16 1-15-8l3-16c1-7 6-13 14-13z"/>',
    // triângulo, quadrado, círculo, xis
    '<path d="M0 -20l20 34h-40z"/>',
    '<rect x="-17" y="-17" width="34" height="34" rx="4"/>',
    '<circle r="18"/>',
    '<path d="M-16 -16l32 32M16 -16l-32 32"/>',
    // estrela
    '<path d="M0 -24l7 15 16 2-12 11 3 16-14-8-14 8 3-16-12-11 16-2z"/>',
  ]
  let out = '<g fill="none" stroke="#fdf4ff" stroke-opacity="0.16" stroke-width="5" stroke-linecap="round" stroke-linejoin="round">'
  for (let row = 0; row < 9; row++) {
    for (let col = 0; col < 14; col++) {
      const x = col * 145 + (row % 2) * 72 + (rand() - 0.5) * 30
      const y = row * 130 + 40 + (rand() - 0.5) * 30
      const icon = icons[Math.floor(rand() * icons.length)]
      const rot = Math.round((rand() - 0.5) * 50)
      const sc = r1(0.8 + rand() * 0.6)
      out += `<g transform="translate(${r1(x)} ${r1(y)}) rotate(${rot}) scale(${sc})">${icon}</g>`
    }
  }
  return out + '</g>'
}

// 3. Slime: paisagem 8-bit — sol de pixels, colinas em degraus, árvores e
// outros slimes pequenos.
function pixelLand(): string {
  const rand = rng(33)
  const P = 24 // tamanho do "pixel"
  let out = ''
  // nuvens
  out += '<g fill="#ecfccb" fill-opacity="0.35">'
  for (const [x, y] of [[180, 140], [760, 90], [1240, 200]]) {
    out += `<rect x="${x}" y="${y}" width="${P * 8}" height="${P * 2}"/><rect x="${x + P * 2}" y="${y - P}" width="${P * 4}" height="${P}"/>`
  }
  out += '</g>'
  // sol
  out += `<g fill="#fef08a" fill-opacity="0.9"><rect x="1560" y="120" width="${P * 6}" height="${P * 6}"/><rect x="${1560 - P}" y="${120 + P}" width="${P * 8}" height="${P * 4}"/><rect x="${1560 + P}" y="${120 - P}" width="${P * 4}" height="${P * 8}"/></g>`
  // colinas: três camadas em degraus
  const layers: [number, string, number][] = [
    [600, '#065f46', 0.55],
    [720, '#047857', 0.75],
    [840, '#14532d', 0.9],
  ]
  for (const [top, color, op] of layers) {
    let d = `M0 ${H}`
    let y = top
    for (let x = 0; x <= W; x += P * 3) {
      y = Math.max(top - P * 5, Math.min(top + P * 4, y + Math.round((rand() - 0.5) * 3) * P))
      d += `L${x} ${y}L${x + P * 3} ${y}`
    }
    d += `L${W} ${H}z`
    out += `<path d="${d}" fill="${color}" fill-opacity="${op}"/>`
    // árvores de pixel na camada do meio
    if (top === 720) {
      for (let i = 0; i < 7; i++) {
        const tx = Math.round((80 + rand() * 1700) / P) * P
        const ty = top - P * 3
        out +=
          `<g><rect x="${tx + P}" y="${ty + P * 3}" width="${P}" height="${P * 2}" fill="#713f12"/>` +
          `<rect x="${tx}" y="${ty}" width="${P * 3}" height="${P * 3}" fill="#166534"/>` +
          `<rect x="${tx + P}" y="${ty - P}" width="${P}" height="${P}" fill="#166534"/></g>`
      }
    }
  }
  // slimes pequenos
  for (const [x, y, s] of [[340, 800, 90], [760, 880, 70], [1080, 790, 60]]) {
    out += `<g opacity="0.9">${hero('3', x, y, s)}</g>`
  }
  return out
}

// 4. Fliperama: synthwave — sol listrado, montanhas e grade em perspectiva.
function synthwave(): string {
  const horizon = 700
  let out = ''
  // sol listrado
  out += '<defs><linearGradient id="sun" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fde047"/><stop offset="1" stop-color="#f43f5e"/></linearGradient>'
  out += '<clipPath id="sunCut">'
  for (let i = 0; i < 12; i++) {
    const y = 300 + i * 34
    const gap = i < 5 ? 0 : (i - 4) * 4
    out += `<rect x="560" y="${y}" width="800" height="${34 - gap}"/>`
  }
  out += '</clipPath></defs>'
  out += `<circle cx="960" cy="560" r="260" fill="url(#sun)" clip-path="url(#sunCut)" opacity="0.9"/>`
  // montanhas
  out += `<path d="M0 ${horizon}L180 560L330 640L520 470L720 620L860 560L960 ${horizon}z" fill="#4c0519" fill-opacity="0.75"/>`
  out += `<path d="M960 ${horizon}L1080 580L1260 460L1420 600L1600 500L1780 610L1920 540L1920 ${horizon}z" fill="#4c0519" fill-opacity="0.75"/>`
  // chão
  out += `<rect y="${horizon}" width="${W}" height="${H - horizon}" fill="#1c0710" fill-opacity="0.75"/>`
  let grid = '<g stroke="#fb923c" stroke-opacity="0.5" stroke-width="3">'
  for (let i = -16; i <= 16; i++) {
    grid += `<line x1="${960 + i * 40}" y1="${horizon}" x2="${960 + i * 260}" y2="${H}"/>`
  }
  let y = horizon
  let step = 10
  while (y < H) {
    grid += `<line x1="0" y1="${r1(y)}" x2="${W}" y2="${r1(y)}"/>`
    y += step
    step *= 1.35
  }
  grid += '</g>'
  return out + grid
}

// 5. Nave: espaço — estrelas, nebulosa, dois planetas e o rastro da nave.
function space(): string {
  const rand = rng(55)
  let out =
    '<defs><radialGradient id="neb" cx="0.3" cy="0.4" r="0.5"><stop offset="0" stop-color="#ec4899" stop-opacity="0.35"/><stop offset="1" stop-color="#ec4899" stop-opacity="0"/></radialGradient>' +
    '<linearGradient id="pl1" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#fbbf24"/><stop offset="1" stop-color="#c2410c"/></linearGradient>' +
    '<linearGradient id="pl2" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#67e8f9"/><stop offset="1" stop-color="#1e40af"/></linearGradient>' +
    '<linearGradient id="trail" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#fbbf24" stop-opacity="0"/><stop offset="1" stop-color="#fbbf24" stop-opacity="0.55"/></linearGradient></defs>'
  out += `<rect width="${W}" height="${H}" fill="url(#neb)"/>`
  out += '<g fill="#fff">'
  for (let i = 0; i < 220; i++) {
    const r = rand() < 0.08 ? 3 : rand() < 0.4 ? 2 : 1.2
    out += `<circle cx="${r1(rand() * W)}" cy="${r1(rand() * H)}" r="${r}" opacity="${r1(0.3 + rand() * 0.7)}"/>`
  }
  out += '</g>'
  // planeta com anel
  // planeta com anel: o anel inteiro atrás e só a metade da frente por cima
  const ring = '<ellipse rx="230" ry="46" fill="none" stroke="#fde68a" stroke-width="10"/>'
  out +=
    '<defs><clipPath id="ringFront"><rect x="-260" y="0" width="520" height="60"/></clipPath></defs>' +
    '<g transform="translate(360 300)">' +
    `<g transform="rotate(-18)" stroke-opacity="0.45">${ring}</g>` +
    '<circle r="130" fill="url(#pl1)"/>' +
    '<path d="M-118 -40a130 130 0 0 1 236 0" fill="#fff" fill-opacity="0.12"/>' +
    `<g transform="rotate(-18)" clip-path="url(#ringFront)">${ring}</g>` +
    '</g>'
  out += '<circle cx="1000" cy="860" r="70" fill="url(#pl2)" opacity="0.85"/>'
  out += '<circle cx="1640" cy="150" r="34" fill="#c4b5fd" opacity="0.6"/>'
  // rastro da nave (diagonal até ela)
  out += '<path d="M780 980L1380 560L1440 600L840 1000z" fill="url(#trail)" transform="rotate(0)"/>'
  return out
}

const SCENES: Record<WallpaperId, { scene: () => string; hero: [number, number, number, number?]; glow: string }> = {
  '1': { scene: circuit, hero: [1480, 560, 420], glow: '#22d3ee' },
  '2': { scene: doodles, hero: [1460, 560, 520], glow: '#f0abfc' },
  '3': { scene: pixelLand, hero: [1500, 760, 380], glow: '#fef08a' },
  '4': { scene: synthwave, hero: [1560, 800, 360], glow: '#fb923c' },
  '5': { scene: space, hero: [1460, 520, 380, 40], glow: '#a78bfa' },
}

/** SVG completo do papel de parede `id` (1920×1080). */
export function wallpaperSvg(id: WallpaperId): string {
  const sc = SCENES[id]
  const [x, y, size, rot] = sc.hero
  const h = hero(id, x, y, size)
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" preserveAspectRatio="xMidYMid slice" data-reemu-wallpaper="${id}">` +
    base(id, x, y, sc.glow) +
    sc.scene() +
    glowLayer +
    (rot ? `<g transform="rotate(${rot} ${x} ${y})">${h}</g>` : h) +
    shadeLayer +
    `</svg>`
  )
}

/** `data:` URI do SVG — miniatura na tela de Plano de fundo. */
export function wallpaperDataUri(id: WallpaperId): string {
  return `data:image/svg+xml,${encodeURIComponent(wallpaperSvg(id))}`
}

/** Qual papel de parede embutido é este SVG (`null` = imagem da pessoa). */
export function wallpaperIdOf(svg: string): WallpaperId | null {
  const m = /data-reemu-wallpaper="([1-5])"/.exec(svg.slice(0, 400))
  return m ? (m[1] as WallpaperId) : null
}
