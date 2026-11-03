import type { AlvanistaTierColors } from './tauri'

const HEX = /^#[0-9a-f]{6}$/i

/** O anel do plano pronto para o CSS. */
export interface TierRing {
  background: string
  /** Cor do brilho (`rgba`), ou `null` sem brilho. */
  glow: string | null
}

const isHex = (v: unknown): v is string => typeof v === 'string' && HEX.test(v)

/**
 * O anel do plano a partir das cores do `/me`. Só cor `#rrggbb` e ângulo
 * numérico entram no CSS; qualquer outra coisa e não há anel (`null`).
 */
export function tierRing(colors: AlvanistaTierColors | null | undefined): TierRing | null {
  const ring = colors?.ring
  if (!ring || (ring.kind !== 'linear' && ring.kind !== 'conic')) return null
  const { angle, stops } = ring
  if (typeof angle !== 'number' || !Number.isFinite(angle)) return null
  if (!Array.isArray(stops) || stops.length < 2 || !stops.every(isHex)) return null
  const list = stops.join(', ')
  const background =
    ring.kind === 'linear' ? `linear-gradient(${angle}deg, ${list})` : `conic-gradient(from ${angle}deg, ${list})`
  return { background, glow: glowOf(colors?.glow) }
}

function glowOf(glow: AlvanistaTierColors['glow']): string | null {
  const alpha = glow?.alpha
  if (!isHex(glow?.color) || typeof alpha !== 'number' || alpha < 0 || alpha > 1) return null
  const n = parseInt(glow.color.slice(1), 16)
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`
}

/** A cor do chip do plano, ou `null` se não for `#rrggbb`. */
export function tierAccent(colors: AlvanistaTierColors | null | undefined): string | null {
  return isHex(colors?.accent) ? colors.accent : null
}
