import { describe, expect, it } from 'vitest'
import type { CoreOption } from './bindings/CoreOption'
import { corePreset, detectLevel, presetChanges, presetHasOptions, type CorePreset } from './corePresets'

const preset: CorePreset = {
  performance: { res: '1x', skip: 'on' },
  quality: { res: '4x', aa: 'x4' },
}

const option = (key: string, choices: string[], romValue: string | null = null): CoreOption => ({
  key,
  displayName: key,
  choices,
  defaultValue: choices[0],
  value: romValue ?? choices[0],
  coreValue: null,
  romValue,
})

const base = (res: string | null, skip: string | null, aa: string | null) => [
  option('res', ['1x', '4x'], res),
  option('skip', ['off', 'on'], skip),
  option('aa', ['off', 'x4'], aa),
  option('outra', ['a', 'b']),
]

describe('corePresets', () => {
  it('PPSSPP has a complete preset', () => {
    const psp = corePreset('ppsspp_libretro', [])
    expect(psp.resolutionOnly).toBeUndefined()
    expect(psp.quality.ppsspp_internal_resolution).toBe('1920x1088')
  })

  it('other cores change only the resolution, from the default to about 4x', () => {
    const fly = corePreset('flycast_libretro', [
      { ...option('reicast_internal_resolution', ['320x240', '640x480', '1920x1440', '2560x1920']), defaultValue: '640x480' },
      option('reicast_texupscale', ['1', '2', '4']),
    ])
    expect(fly.resolutionOnly).toBe(true)
    expect(fly.performance).toEqual({ reicast_internal_resolution: '640x480' })
    expect(fly.quality).toEqual({ reicast_internal_resolution: '1920x1440' })
  })

  it('reads factor and plain number scales', () => {
    const quality = (key: string, choices: string[]) => corePreset('x', [option(key, choices)]).quality[key]
    expect(quality('beetle_psx_hw_internal_resolution', ['1x', '2x', '4x', '8x'])).toBe('4x')
    expect(quality('swanstation_GPU_ResolutionScale', ['1', '2', '4', '8'])).toBe('4')
    expect(quality('virtualjaguar_internal_resolution', ['1x', '2x'])).toBe('2x')
  })

  it('a 2D core has nothing to change', () => {
    const preset2d = corePreset('fbneo_libretro', [option('fbneo-resolution', ['640x480', '1920x1440'])])
    expect(presetHasOptions(preset2d)).toBe(false)
  })

  it('detects the level from the stored values', () => {
    expect(detectLevel(preset, base(null, null, null))).toBe('normal')
    expect(detectLevel(preset, base('1x', 'on', null))).toBe('performance')
    expect(detectLevel(preset, base('4x', null, 'x4'))).toBe('quality')
  })

  it('a different value in a preset key is custom', () => {
    expect(detectLevel(preset, base('4x', 'on', null))).toBe('custom')
    expect(detectLevel(preset, base('1x', null, null))).toBe('custom')
  })

  it('a saved option outside the presets does not change the level', () => {
    for (const [values, level] of [
      [base(null, null, null), 'normal'],
      [base('1x', 'on', null), 'performance'],
      [base('4x', null, 'x4'), 'quality'],
    ] as const) {
      const list = [...values]
      list[3] = option('outra', ['a', 'b'], 'b')
      expect(detectLevel(preset, list)).toBe(level)
    }
  })

  it('a value this core version lacks does not break detection', () => {
    const old = [option('res', ['1x', '2x'], '1x'), option('skip', ['off', 'on'], 'on')]
    expect(detectLevel(preset, old)).toBe('performance')
  })

  it('writes only what changes and clears the other level keys', () => {
    expect(presetChanges(preset, 'quality', base('1x', 'on', null))).toEqual([
      { key: 'res', value: '4x' },
      { key: 'skip', value: '' },
      { key: 'aa', value: 'x4' },
    ])
    expect(presetChanges(preset, 'quality', base('4x', null, 'x4'))).toEqual([])
  })

  it('normal clears the preset keys and keeps other adjustments', () => {
    const list = base('1x', 'on', null)
    list[3] = option('outra', ['a', 'b'], 'b')
    expect(presetChanges(preset, 'normal', list)).toEqual([
      { key: 'res', value: '' },
      { key: 'skip', value: '' },
    ])
  })

  it('skips an option or value this core version does not offer', () => {
    const old = [option('res', ['1x', '2x']), option('skip', ['off', 'on'])]
    expect(presetChanges(preset, 'quality', old)).toEqual([])
    expect(presetChanges(preset, 'performance', old)).toEqual([
      { key: 'res', value: '1x' },
      { key: 'skip', value: 'on' },
    ])
  })
})
