import { describe, expect, it } from 'vitest'
import type { RomEntry } from '../../lib/tauri'
import { contrastRatio } from '../../styles/themes'
import {
  CAPTION_SCRIM,
  mostPlayed,
  pickText,
  profileStats,
  readableInk,
  recentlyPlayed,
} from './profileData'

const rom = (id: string, o: Partial<RomEntry> = {}): RomEntry => ({
  id,
  title: id,
  systemId: 'nes',
  filePath: `/${id}`,
  boxart: null,
  lastPlayedAt: null,
  addedAt: 0,
  isFavorite: false,
  playTimeSecs: 0,
  ...o,
})

const fixture = [
  rom('a', { playTimeSecs: 0 }),
  rom('b', { playTimeSecs: 90, systemId: 'snes', isFavorite: true, lastPlayedAt: 10 }),
  rom('c', { playTimeSecs: 3600, lastPlayedAt: 20 }),
  rom('d', { playTimeSecs: 0, systemId: 'snes' }),
]

describe('profileStats', () => {
  it('soma tempo, jogos, sistemas e favoritos', () => {
    expect(profileStats(fixture)).toEqual({ playTimeSecs: 3690, games: 4, systems: 2, favorites: 1 })
  })
})

describe('listas', () => {
  it('ROM com tempo e sem lastPlayedAt entra em mostPlayed e não em recentlyPlayed', () => {
    const roms = [rom('x', { playTimeSecs: 500, lastPlayedAt: null }), ...fixture]
    expect(mostPlayed(roms, 10).map((r) => r.id)).toEqual(['c', 'x', 'b'])
    expect(recentlyPlayed(roms, 10).map((r) => r.id)).toEqual(['c', 'b'])
  })
  it('respeita o limite', () => {
    expect(mostPlayed(fixture, 1).map((r) => r.id)).toEqual(['c'])
    expect(recentlyPlayed(fixture, 1).map((r) => r.id)).toEqual(['c'])
  })
})

describe('pickText', () => {
  it('escolhe o idioma, cai para en e depois para qualquer valor', () => {
    expect(pickText({ pt: 'Colecionador', en: 'Collector' }, 'es')).toBe('Collector')
    expect(pickText({ pt: 'Colecionador', en: 'Collector' }, 'pt-BR')).toBe('Colecionador')
    expect(pickText({ ja: 'x' }, 'pt-BR')).toBe('x')
    expect(pickText(undefined, 'en')).toBeNull()
  })
})

describe('readableInk', () => {
  it('escolhe a tinta com contraste mínimo de 4.5:1', () => {
    expect(readableInk('#AD9F85')).toBe('#000000')
    expect(readableInk('#1B2F13')).toBe('#ffffff')
  })
})

describe('legenda de "Seus momentos"', () => {
  it('texto branco sobre o véu, numa captura toda branca, fica em 4,5:1', () => {
    // o ponto mais claro do véu: preto no topo sobre branco
    const v = Math.round(255 * (1 - CAPTION_SCRIM.top))
    const hex = `#${v.toString(16).padStart(2, '0').repeat(3)}`
    expect(contrastRatio('#ffffff', hex)).toBeGreaterThanOrEqual(4.5)
    expect(CAPTION_SCRIM.bottom).toBeGreaterThan(CAPTION_SCRIM.top)
  })
})
