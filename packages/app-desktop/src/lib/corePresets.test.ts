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

function applyChanges(
  options: readonly CoreOption[],
  changes: readonly { key: string; value: string }[],
): CoreOption[] {
  const byKey = new Map(changes.map((c) => [c.key, c.value]))
  return options.map((o) => {
    if (!byKey.has(o.key)) return o
    const next = byKey.get(o.key)!
    const romValue = next === '' ? null : next
    return { ...o, romValue, value: romValue ?? o.defaultValue }
  })
}

describe('corePresets', () => {
  it('PPSSPP has a complete preset', () => {
    const psp = corePreset('ppsspp_libretro', [])
    expect(psp.resolutionOnly).toBeUndefined()
    expect(psp.quality.ppsspp_internal_resolution).toBe('1920x1088')
  })

  it.each([
    [
      'ppsspp_libretro',
      [
        option('ppsspp_internal_resolution', ['480x272', '960x544', '1920x1088']),
        option('ppsspp_mulitsample_level', ['Disabled', 'x2', 'x4']),
        option('ppsspp_auto_frameskip', ['disabled', 'enabled']),
        option('ppsspp_frameskip', ['disabled', '1', '2']),
        option('ppsspp_texture_scaling_level', ['disabled', '2x', '4x']),
        option('ppsspp_texture_anisotropic_filtering', ['disabled', '4x', '16x']),
        option('ppsspp_texture_filtering', ['Auto', 'Nearest', 'Auto max quality']),
      ],
    ],
    [
      'mednafen_psx_hw_libretro',
      [
        option('beetle_psx_hw_renderer', ['hardware', 'software']),
        option('beetle_psx_hw_internal_resolution', ['1x(native)', '2x', '4x', '8x']),
        option('beetle_psx_hw_pgxp_mode', ['disabled', 'memory only', 'memory + CPU']),
        option('beetle_psx_hw_filter', ['nearest', 'xBR', 'bilinear', 'JINC2']),
        option('beetle_psx_hw_dither_mode', ['1x(native)', 'internal resolution', 'disabled']),
        option('beetle_psx_hw_cpu_dynarec', ['disabled', 'enabled']),
      ],
    ],
    [
      'swanstation_libretro',
      [
        option('swanstation_GPU_ResolutionScale', ['1', '2', '4', '8']),
        option('swanstation_GPU_TrueColor', ['false', 'true']),
        option('swanstation_GPU_TextureFilter', ['Nearest', 'Bilinear', 'xBR']),
        option('swanstation_GPU_PGXPEnable', ['false', 'true']),
        option('swanstation_GPU_MSAA', ['1', '2', '4', '8']),
        option('swanstation_CPU_ExecutionMode', ['Interpreter', 'Recompiler']),
      ],
    ],
    [
      'pcsx_rearmed_libretro',
      [
        option('pcsx_rearmed_drc', ['enabled', 'disabled']),
        option('pcsx_rearmed_frameskip_type', ['disabled', 'auto', 'fixed_interval']),
        option('pcsx_rearmed_gpu_thread_rendering', ['disabled', 'sync', 'auto']),
        option('pcsx_rearmed_dithering', ['enabled', 'disabled']),
        option('pcsx_rearmed_neon_enhancement_enable', ['disabled', 'enabled']),
      ],
    ],
    [
      'mupen64plus_next_libretro',
      [
        option('mupen64plus-cpucore', ['dynamic_recompiler', 'cached_interpreter']),
        option('mupen64plus-43screensize', ['320x240', '640x480', '1280x960', '1920x1440']),
        option('mupen64plus-169screensize', ['640x360', '1280x720', '1920x1080']),
        option('mupen64plus-MultiSampling', ['0', '2', '4', '8']),
        option('mupen64plus-FXAA', ['0', '1']),
      ],
    ],
    [
      'flycast_libretro',
      [
        option('reicast_internal_resolution', ['640x480', '1280x960', '1920x1440']),
        option('reicast_threaded_rendering', ['enabled', 'disabled']),
        option('reicast_auto_skip_frame', ['disabled', 'some', 'more']),
        option('reicast_alpha_sorting', [
          'per-triangle (normal)',
          'per-pixel (accurate)',
          'per-strip (fast, least accurate)',
        ]),
        option('reicast_anisotropic_filtering', ['off', '2', '4', '8', '16']),
      ],
    ],
    [
      'melonds_libretro',
      [
        option('melonds_jit_enable', ['enabled', 'disabled']),
        option('melonds_threaded_renderer', ['disabled', 'enabled']),
        option('melonds_opengl_renderer', ['disabled', 'enabled']),
        option('melonds_opengl_resolution', [
          '1x native (256x192)',
          '2x native (512x384)',
          '4x native (1024x768)',
        ]),
        option('melonds_opengl_filtering', ['nearest', 'linear']),
      ],
    ],
    [
      'melondsds_libretro',
      [
        option('melonds_jit_enable', ['enabled', 'disabled']),
        option('melonds_threaded_renderer', ['disabled', 'enabled']),
        option('melonds_render_mode', ['software', 'opengl']),
        option('melonds_opengl_scale_factor', ['1', '2', '4', '8']),
      ],
    ],
  ])('applies and detects performance, quality, and normal round-trip for %s', (coreId, initial) => {
    const p = corePreset(coreId, initial)
    expect(p.resolutionOnly).toBeUndefined()
    expect(detectLevel(p, initial)).toBe('normal')

    const afterPerf = applyChanges(initial, presetChanges(p, 'performance', initial))
    expect(detectLevel(p, afterPerf)).toBe('performance')
    expect(presetChanges(p, 'performance', afterPerf)).toEqual([])

    const afterQual = applyChanges(afterPerf, presetChanges(p, 'quality', afterPerf))
    expect(detectLevel(p, afterQual)).toBe('quality')
    expect(presetChanges(p, 'quality', afterQual)).toEqual([])

    const afterNorm = applyChanges(afterQual, presetChanges(p, 'normal', afterQual))
    expect(detectLevel(p, afterNorm)).toBe('normal')
  })

  it('PCSX ReARMed excludes destructive gpu_unai options', () => {
    const pcsx = corePreset('pcsx_rearmed_libretro', [])
    const keys = [...Object.keys(pcsx.performance), ...Object.keys(pcsx.quality)]
    expect(keys.some((k) => k.includes('gpu_unai_blending'))).toBe(false)
    expect(keys.some((k) => k.includes('gpu_unai_lighting'))).toBe(false)
  })

  it('Flycast also works with flycast_* option prefix and 1440x1080 max choice', () => {
    const flyOptions = [
      option('flycast_internal_resolution', ['640x480', '1280x960', '1440x1080']),
      option('flycast_threaded_rendering', ['enabled', 'disabled']),
      option('flycast_auto_skip_frame', ['disabled', 'some']),
      option('flycast_alpha_sorting', ['per-strip (fast, least accurate)', 'per-pixel (accurate)']),
      option('flycast_anisotropic_filtering', ['off', '4', '16']),
    ]
    const fly = corePreset('flycast_libretro', flyOptions)
    const afterQual = applyChanges(flyOptions, presetChanges(fly, 'quality', flyOptions))
    expect(afterQual.find((o) => o.key === 'flycast_internal_resolution')?.romValue).toBe('1440x1080')
    expect(detectLevel(fly, afterQual)).toBe('quality')

    const afterPerf = applyChanges(afterQual, presetChanges(fly, 'performance', afterQual))
    expect(detectLevel(fly, afterPerf)).toBe('performance')
  })

  it('matches resolution choices with extra labels across core versions', () => {
    const beetleVariant = [
      option('beetle_psx_hw_internal_resolution', ['1x (native)', '2x (640x480)', '4x (1280x960)']),
      option('beetle_psx_hw_pgxp_mode', ['disabled', 'memory only']),
      option('beetle_psx_hw_filter', ['nearest', 'xBR']),
    ]
    const p = corePreset('mednafen_psx_hw_libretro', beetleVariant)
    const afterQual = applyChanges(beetleVariant, presetChanges(p, 'quality', beetleVariant))
    expect(afterQual[0].romValue).toBe('4x (1280x960)')
    expect(afterQual[2].romValue).toBe('xBR')
    expect(detectLevel(p, afterQual)).toBe('quality')

    const afterPerf = applyChanges(afterQual, presetChanges(p, 'performance', afterQual))
    expect(afterPerf[0].romValue).toBe('1x (native)')
    expect(detectLevel(p, afterPerf)).toBe('performance')
  })

  it('other cores change only the resolution, from the default to about 4x', () => {
    const n64 = corePreset('parallel_n64_libretro', [
      {
        ...option('parallel-n64-screensize', ['320x240', '640x480', '1920x1440', '2560x1920']),
        defaultValue: '640x480',
      },
      option('parallel-n64-filtering', ['automatic', 'nearest']),
    ])
    expect(n64.resolutionOnly).toBe(true)
    expect(n64.performance).toEqual({ 'parallel-n64-screensize': '640x480' })
    expect(n64.quality).toEqual({ 'parallel-n64-screensize': '1920x1440' })
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
