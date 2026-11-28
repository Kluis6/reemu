import type { RomEntry } from '../../lib/tauri'
import { contrastRatio } from '../../styles/themes'

/** Números locais do perfil. `games` conta toda a biblioteca. */
export function profileStats(roms: RomEntry[]): {
  playTimeSecs: number
  games: number
  systems: number
  favorites: number
} {
  return {
    playTimeSecs: roms.reduce((s, r) => s + (r.playTimeSecs ?? 0), 0),
    games: roms.length,
    systems: new Set(roms.map((r) => r.systemId)).size,
    favorites: roms.filter((r) => r.isFavorite).length,
  }
}

/** Mesma regra da Home: só quem tem `lastPlayedAt`, o mais recente primeiro. */
export function recentlyPlayed(roms: RomEntry[], limit?: number): RomEntry[] {
  return roms
    .filter((r) => r.lastPlayedAt != null)
    .sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0))
    .slice(0, limit)
}

/** Só quem tem tempo de jogo, o maior primeiro (independe de `lastPlayedAt`). */
export function mostPlayed(roms: RomEntry[], limit: number): RomEntry[] {
  return roms
    .filter((r) => r.playTimeSecs > 0)
    .sort((a, b) => b.playTimeSecs - a.playTimeSecs)
    .slice(0, limit)
}

/** `pt` para `pt-BR`, `en` para o resto, depois qualquer valor presente. */
export function pickText(map: Record<string, string> | undefined, lang: string): string | null {
  if (!map) return null
  const preferred = lang.toLowerCase().startsWith('pt') ? 'pt' : 'en'
  for (const key of [preferred, 'en']) {
    if (map[key]) return map[key]
  }
  return Object.values(map).find((v) => !!v) ?? null
}

/** Tinta (preto ou branco) com o maior contraste sobre `bg` (`#rrggbb`). */
export function readableInk(bg: string): '#000000' | '#ffffff' {
  return contrastRatio(bg, '#000000') >= contrastRatio(bg, '#ffffff') ? '#000000' : '#ffffff'
}

/** O véu preto sob a legenda branca de "Seus momentos" (opacidade no topo e
 *  embaixo). O topo sozinho, sobre uma captura toda branca, já dá 4,5:1. */
export const CAPTION_SCRIM = { top: 0.55, bottom: 0.85 } as const
