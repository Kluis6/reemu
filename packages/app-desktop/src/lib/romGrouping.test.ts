import { describe, expect, it } from 'vitest'
import i18n from '../i18n'
import {
  detectRomRegionFallback,
  formatCardGroupBadge,
  groupRoms,
  romVariantLabel,
  siblingsForRom,
} from './romGrouping'
import type { RomEntry } from './tauri'

function makeRom(overrides: Partial<RomEntry> & Pick<RomEntry, 'id' | 'title' | 'filePath'>): RomEntry {
  return {
    systemId: 'snes',
    boxart: null,
    lastPlayedAt: null,
    addedAt: 100,
    isFavorite: false,
    playTimeSecs: 0,
    ...overrides,
  }
}

describe('romGrouping', () => {
  it('groups_multiple_regional_roms_of_same_game_and_picks_preferred_region', () => {
    const jp = makeRom({
      id: 'ct-jp',
      title: 'Chrono Trigger',
      filePath: '/roms/snes/Chrono Trigger (Japan).sfc',
      groupKey: 'game:uuid-ct',
      region: 'JP',
      boxart: 'http://cover.localhost/ct-jp',
      playTimeSecs: 120,
    })
    const eu = makeRom({
      id: 'ct-eu',
      title: 'Chrono Trigger',
      filePath: '/roms/snes/Chrono Trigger (Europe).sfc',
      groupKey: 'game:uuid-ct',
      region: 'EU',
      boxart: 'http://cover.localhost/ct-eu',
      playTimeSecs: 60,
    })
    const us = makeRom({
      id: 'ct-us',
      title: 'Chrono Trigger',
      filePath: '/roms/snes/Chrono Trigger (USA).sfc',
      groupKey: 'game:uuid-ct',
      region: 'NA',
      boxart: 'http://cover.localhost/ct-us',
      playTimeSecs: 300,
    })

    const grouped = groupRoms([jp, eu, us], 'pt-BR')
    expect(grouped).toHaveLength(1)
    expect(grouped[0].id).toBe('ct-us')
    expect(grouped[0].boxart).toBe('http://cover.localhost/ct-us')
    expect(grouped[0].playTimeSecs).toBe(480)
    expect(grouped[0].regionBadges).toEqual(['USA', 'PAL', 'JP'])
    expect(formatCardGroupBadge(grouped[0], i18n.t)).toBe('USA · PAL · JP')
  })

  it('prefers_most_recently_played_regional_rom_and_its_cover', () => {
    const us = makeRom({
      id: 'ct-us',
      title: 'Chrono Trigger',
      filePath: '/roms/snes/Chrono Trigger (USA).sfc',
      groupKey: 'game:uuid-ct',
      region: 'NA',
      boxart: 'http://cover.localhost/ct-us',
      lastPlayedAt: 1000,
    })
    const jp = makeRom({
      id: 'ct-jp',
      title: 'Chrono Trigger',
      filePath: '/roms/snes/Chrono Trigger (Japan).sfc',
      groupKey: 'game:uuid-ct',
      region: 'JP',
      boxart: 'http://cover.localhost/ct-jp',
      lastPlayedAt: 5000,
    })

    const grouped = groupRoms([us, jp], 'en')
    expect(grouped).toHaveLength(1)
    expect(grouped[0].id).toBe('ct-jp')
    expect(grouped[0].boxart).toBe('http://cover.localhost/ct-jp')
  })

  it('cleans_title_and_disambiguates_chip_or_disc_variants_for_unscraped_roms', () => {
    const chip1 = makeRom({
      id: 'c1',
      systemId: 'atari2600',
      title: '128-in-1 Junior Console (Chip 1) (PAL) [!]',
      filePath: '/roms/atari2600/128-in-1 Junior Console (Chip 1) (PAL) [!].a26',
    })
    const chip2 = makeRom({
      id: 'c2',
      systemId: 'atari2600',
      title: '128-in-1 Junior Console (Chip 2) (PAL) [!]',
      filePath: '/roms/atari2600/128-in-1 Junior Console (Chip 2) (PAL) [!].a26',
    })

    expect(detectRomRegionFallback(chip1)).toBe('EU')
    const grouped = groupRoms([chip1, chip2], 'pt-BR')
    expect(grouped).toHaveLength(1)
    expect(grouped[0].title).toBe('128-in-1 Junior Console')

    const siblings = siblingsForRom([chip1, chip2], chip1, 'pt-BR')
    expect(siblings).toHaveLength(2)
    expect(romVariantLabel(siblings[0], siblings, i18n.t)).toContain('Chip 1')
    expect(romVariantLabel(siblings[1], siblings, i18n.t)).toContain('Chip 2')
  })
})
