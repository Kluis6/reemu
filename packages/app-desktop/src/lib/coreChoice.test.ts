import { describe, expect, it } from 'vitest'
import { rankCores } from './coreChoice'
import type { InstalledCore } from './tauri'

const core = (coreId: string, name: string, extensions: string[], systems: string[]): InstalledCore => ({
  coreId,
  name,
  version: '',
  extensions,
  renderBackend: null,
  systems,
})

const cores = [
  core('a5200_libretro', 'a5200', ['a52', 'bin'], ['atari5200']),
  core('genesis_plus_gx_libretro', 'Genesis Plus GX', ['md', 'bin', 'gen'], ['megadrive', 'mastersystem']),
  core('stella_libretro', 'Stella', ['a26', 'bin'], ['atari2600']),
  core('fceumm_libretro', 'FCEUmm', ['nes'], ['nes']),
]

describe('rankCores', () => {
  it('prefere o core do sistema do jogo, não o 1º em ordem alfabética', () => {
    expect(rankCores(cores, 'megadrive', 'bin')[0].coreId).toBe('genesis_plus_gx_libretro')
    expect(rankCores(cores, 'atari2600', 'bin')[0].coreId).toBe('stella_libretro')
    expect(rankCores(cores, 'atari5200', 'bin')[0].coreId).toBe('a5200_libretro')
  })

  it('sem core do sistema, cai na extensão e depois no nome', () => {
    const r = rankCores(cores, 'psx', 'bin').map((c) => c.coreId)
    expect(r.slice(0, 3)).toEqual(['a5200_libretro', 'genesis_plus_gx_libretro', 'stella_libretro'])
    expect(r[3]).toBe('fceumm_libretro')
  })
})
