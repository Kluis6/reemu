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
  it('escolhe o core LaserDisc pela mídia e descarta extensões de outros sistemas', () => {
    const dirksimple = core('dirksimple_libretro', 'DirkSimple', ['ogv', 'dirksimple'], ['laserdisc'])
    const hypseus = core('hypseushw_libretro', 'HypseusHW', ['txt', 'zip'], ['laserdisc'])
    const arcade = core('arcade_libretro', 'Arcade', ['zip', 'txt'], ['arcade'])
    const players = [dirksimple, hypseus, arcade]
    expect(rankCores(players, 'laserdisc', 'txt')).toEqual([hypseus])
    expect(rankCores(players, 'laserdisc', 'ogv')).toEqual([dirksimple])
    expect(rankCores([dirksimple, arcade], 'laserdisc', 'txt')).toEqual([])
  })

  it('prioriza os cores validados no carregamento comum de Satellaview e Sufami Turbo', () => {
    const snes = [
      core('bsnes-jg_libretro', 'bsnes-jg', ['bs', 'st', 'sfc'], ['snes']),
      core('mesen-s_libretro', 'Mesen-S', ['bs', 'sfc'], ['snes']),
      core('mesen2_libretro', 'Mesen2', ['sfc'], ['snes']),
      core('snes9x_libretro', 'Snes9x', ['bs', 'sfc'], ['snes']),
    ]
    expect(rankCores(snes, 'snes', 'bs')[0].coreId).toBe('snes9x_libretro')
    expect(rankCores(snes.slice(0, 3), 'snes', 'bs')[0].coreId).toBe('mesen-s_libretro')
    expect(rankCores(snes, 'snes', 'st')[0].coreId).toBe('snes9x_libretro')
    expect(rankCores(snes.slice(0, 3), 'snes', 'st')[0].coreId).toBe('mesen2_libretro')
  })

  it('escolhe o core pelo sistema ao abrir fitas TZX', () => {
    const eightyone = core('81_libretro', 'EightyOne', ['p', 'tzx', 't81'], ['zx81'])
    const fuse = core('fuse_libretro', 'Fuse', ['tzx', 'tap'], ['zxspectrum'])
    expect(rankCores([eightyone, fuse], 'zx81', 'tzx')).toEqual([eightyone])
    expect(rankCores([eightyone, fuse], 'zxspectrum', 'tzx')).toEqual([fuse])
    expect(rankCores([fuse], 'zx81', 'tzx')).toEqual([])
    expect(rankCores([eightyone], 'zxspectrum', 'tzx')).toEqual([])
  })

  it('oferece os dois cores Arduboy para HEX e seleciona Ardens para pacotes', () => {
    const ardens = core('ardens_libretro', 'Ardens', ['hex', 'arduboy'], ['arduboy'])
    const arduous = core('arduous_libretro', 'Arduous', ['hex'], ['arduboy'])
    expect(rankCores([arduous, ardens], 'arduboy', 'hex')).toEqual([ardens, arduous])
    expect(rankCores([arduous, ardens], 'arduboy', 'arduboy')).toEqual([ardens])
    expect(rankCores([arduous], 'arduboy', 'arduboy')).toEqual([])
  })

  it('prefere o core do sistema do jogo, não o 1º em ordem alfabética', () => {
    expect(rankCores(cores, 'megadrive', 'bin')[0].coreId).toBe('genesis_plus_gx_libretro')
    expect(rankCores(cores, 'atari2600', 'bin')[0].coreId).toBe('stella_libretro')
    expect(rankCores(cores, 'atari5200', 'bin')[0].coreId).toBe('a5200_libretro')
  })

  it('oferece as variantes CPS para ZIP de Arcade sem mudar a escolha automática existente', () => {
    const cps1 = core('fbalpha2012_cps1_libretro', 'FB Alpha 2012 CPS-1', ['zip'], ['arcade'])
    const cps2 = core('fbalpha2012_cps2_libretro', 'FB Alpha 2012 CPS-2', ['zip'], ['arcade'])
    const cps3 = core('fbalpha2012_cps3_libretro', 'FB Alpha 2012 CPS-3', ['iso', 'zip'], ['arcade'])
    const fbneo = core('fbneo_libretro', 'FinalBurn Neo', ['zip'], ['arcade'])
    const mame = core('mame_libretro', 'MAME', ['zip'], ['arcade'])

    expect(rankCores([cps3, cps2, cps1, mame, fbneo], 'arcade', 'zip')).toEqual([fbneo, mame, cps1, cps2, cps3])
    expect(rankCores([cps2, mame], 'arcade', 'zip')).toEqual([mame, cps2])
    expect(rankCores([cps2], 'arcade', 'zip')).toEqual([cps2])
    expect(rankCores([cps3], 'arcade', 'zip')).toEqual([cps3])
  })

  it('escolhe Uzem para Uzebox e exige um core compatível', () => {
    const uzem = core('uzem_libretro', 'Uzem', ['uze'], ['uzebox'])
    expect(rankCores([...cores, uzem], 'uzebox', 'uze')).toEqual([uzem])
    expect(rankCores(cores, 'uzebox', 'uze')).toEqual([])
  })

  it('usa somente um core VaporSpec para programas VAPORBIN', () => {
    const vaporspec = core('vaporspec_libretro', 'VaporSpec', ['vaporbin'], ['vaporspec'])
    expect(rankCores([...cores, vaporspec], 'vaporspec', 'vaporbin')).toEqual([vaporspec])
    expect(rankCores(cores, 'vaporspec', 'vaporbin')).toEqual([])
  })

  it('seleciona SameDuck para DUCK e BIN do Mega Duck', () => {
    const sameDuck = core('sameduck_libretro', 'SameDuck', ['bin', 'duck'], ['megaduck'])
    expect(rankCores([...cores, sameDuck], 'megaduck', 'duck')).toEqual([sameDuck])
    expect(rankCores([...cores, sameDuck], 'megaduck', 'bin')).toEqual([sameDuck])
    expect(rankCores(cores, 'megaduck', 'bin')).toEqual([])
  })

  it('seleciona AmiArcadia pelo sistema mesmo com outros cores que aceitam BIN', () => {
    const amiArcadia = core('amiarcadia_libretro', 'AmiArcadia', ['bin', 'tvc'], ['arcadia', 'vc4000', 'elektor'])
    const installed = [...cores, amiArcadia]
    expect(rankCores(installed, 'arcadia', 'bin')).toEqual([amiArcadia])
    expect(rankCores(installed, 'vc4000', 'bin')).toEqual([amiArcadia])
    expect(rankCores(installed, 'elektor', 'tvc')).toEqual([amiArcadia])
    expect(rankCores(cores, 'vc4000', 'bin')).toEqual([])
    expect(rankCores(installed, 'atari2600', 'bin')[0].coreId).toBe('stella_libretro')
  })

  it('sem core do sistema, cai na extensão e depois no nome', () => {
    const r = rankCores(cores, 'psx', 'bin').map((c) => c.coreId)
    expect(r.slice(0, 3)).toEqual(['a5200_libretro', 'genesis_plus_gx_libretro', 'stella_libretro'])
    expect(r[3]).toBe('fceumm_libretro')
  })
})
