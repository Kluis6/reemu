import type { CoreOption } from './bindings/CoreOption'

/** Valores de cada predefinição; Normal é o padrão do core, sem valores próprios. */
export interface CorePreset {
  performance: Record<string, string>
  quality: Record<string, string>
  /** Sem predefinição pronta: muda só a resolução. */
  resolutionOnly?: boolean
}

export type PresetLevel = 'performance' | 'normal' | 'quality' | 'custom'

const MELONDS_PRESET: CorePreset = {
  performance: {
    melonds_jit_enable: 'enabled',
    melonds_threaded_renderer: 'enabled',
    melonds_opengl_renderer: 'disabled',
    melonds_opengl_resolution: '1x native (256x192)',
    melonds_opengl_filtering: 'nearest',
  },
  quality: {
    melonds_jit_enable: 'enabled',
    melonds_threaded_renderer: 'enabled',
    melonds_opengl_renderer: 'enabled',
    melonds_opengl_resolution: '4x native (1024x768)',
    melonds_opengl_filtering: 'linear',
  },
}

/**
 * Por core (`crates/core-catalog/src/catalog.rs`). Chaves, valores e efeitos
 * conferidos nos `libretro_core_options.h` de cada core e em docs.libretro.com
 * (`docs/ai-context/21-opcoes-cores-por-cenario.md`).
 *
 * - PPSSPP: `mulitsample` é a grafia da chave no core; `skip_buffer_effects`
 *   fica de fora porque alguns jogos podem não desenhar nada.
 * - PCSX ReARMed: `gpu_unai_blending` e `gpu_unai_lighting` ficam de fora
 *   porque causam erros graves de imagem.
 * - Flycast: inclui os prefixos `reicast_*` (usado pelo core atual) e
 *   `flycast_*` (usado na documentação); chaves ausentes no core são ignoradas.
 */
const PRESETS: Record<string, CorePreset> = {
  ppsspp_libretro: {
    performance: {
      ppsspp_internal_resolution: '480x272',
      ppsspp_mulitsample_level: 'Disabled',
      ppsspp_auto_frameskip: 'enabled',
      ppsspp_frameskip: '2',
      ppsspp_texture_scaling_level: 'disabled',
      ppsspp_texture_anisotropic_filtering: 'disabled',
    },
    quality: {
      ppsspp_internal_resolution: '1920x1088',
      ppsspp_mulitsample_level: 'x4',
      ppsspp_auto_frameskip: 'disabled',
      ppsspp_frameskip: 'disabled',
      ppsspp_texture_anisotropic_filtering: '16x',
      ppsspp_texture_filtering: 'Auto max quality',
    },
  },
  mednafen_psx_hw_libretro: {
    performance: {
      beetle_psx_hw_renderer: 'hardware',
      beetle_psx_hw_internal_resolution: '1x(native)',
      beetle_psx_hw_pgxp_mode: 'disabled',
      beetle_psx_hw_filter: 'nearest',
      beetle_psx_hw_dither_mode: '1x(native)',
      beetle_psx_hw_cpu_dynarec: 'enabled',
    },
    quality: {
      beetle_psx_hw_renderer: 'hardware',
      beetle_psx_hw_internal_resolution: '4x',
      beetle_psx_hw_pgxp_mode: 'memory only',
      beetle_psx_hw_filter: 'bilinear',
      beetle_psx_hw_dither_mode: 'internal resolution',
      beetle_psx_hw_cpu_dynarec: 'enabled',
    },
  },
  swanstation_libretro: {
    performance: {
      swanstation_GPU_ResolutionScale: '1',
      swanstation_GPU_TrueColor: 'false',
      swanstation_GPU_TextureFilter: 'Nearest',
      swanstation_GPU_PGXPEnable: 'false',
      swanstation_GPU_MSAA: '1',
      swanstation_CPU_ExecutionMode: 'Recompiler',
    },
    quality: {
      swanstation_GPU_ResolutionScale: '4',
      swanstation_GPU_TrueColor: 'true',
      swanstation_GPU_TextureFilter: 'Bilinear',
      swanstation_GPU_PGXPEnable: 'true',
      swanstation_GPU_MSAA: '4',
      swanstation_CPU_ExecutionMode: 'Recompiler',
    },
  },
  pcsx_rearmed_libretro: {
    performance: {
      pcsx_rearmed_drc: 'enabled',
      pcsx_rearmed_frameskip_type: 'auto',
      pcsx_rearmed_gpu_thread_rendering: 'auto',
      pcsx_rearmed_dithering: 'disabled',
      pcsx_rearmed_neon_enhancement_enable: 'disabled',
    },
    quality: {
      pcsx_rearmed_drc: 'enabled',
      pcsx_rearmed_frameskip_type: 'disabled',
      pcsx_rearmed_gpu_thread_rendering: 'auto',
      pcsx_rearmed_dithering: 'enabled',
      pcsx_rearmed_neon_enhancement_enable: 'enabled',
    },
  },
  mupen64plus_next_libretro: {
    performance: {
      'mupen64plus-cpucore': 'dynamic_recompiler',
      'mupen64plus-43screensize': '320x240',
      'mupen64plus-169screensize': '640x360',
      'mupen64plus-MultiSampling': '0',
      'mupen64plus-FXAA': '0',
    },
    quality: {
      'mupen64plus-cpucore': 'dynamic_recompiler',
      'mupen64plus-43screensize': '1280x960',
      'mupen64plus-169screensize': '1920x1080',
      'mupen64plus-MultiSampling': '4',
      'mupen64plus-FXAA': '1',
    },
  },
  flycast_libretro: {
    performance: {
      reicast_internal_resolution: '640x480',
      flycast_internal_resolution: '640x480',
      reicast_threaded_rendering: 'enabled',
      flycast_threaded_rendering: 'enabled',
      reicast_auto_skip_frame: 'some',
      flycast_auto_skip_frame: 'some',
      reicast_alpha_sorting: 'per-strip (fast, least accurate)',
      flycast_alpha_sorting: 'per-strip (fast, least accurate)',
      reicast_anisotropic_filtering: 'off',
      flycast_anisotropic_filtering: 'off',
    },
    quality: {
      reicast_internal_resolution: '1920x1440',
      flycast_internal_resolution: '1920x1440',
      reicast_threaded_rendering: 'enabled',
      flycast_threaded_rendering: 'enabled',
      reicast_auto_skip_frame: 'disabled',
      flycast_auto_skip_frame: 'disabled',
      reicast_alpha_sorting: 'per-pixel (accurate)',
      flycast_alpha_sorting: 'per-pixel (accurate)',
      reicast_anisotropic_filtering: '16',
      flycast_anisotropic_filtering: '16',
    },
  },
  melonds_libretro: MELONDS_PRESET,
  melondsds_libretro: {
    performance: {
      ...MELONDS_PRESET.performance,
      melonds_render_mode: 'software',
      melonds_opengl_scale_factor: '1',
    },
    quality: {
      ...MELONDS_PRESET.quality,
      melonds_render_mode: 'opengl',
      melonds_opengl_scale_factor: '4',
    },
  },
}

/** Variantes equivalentes entre versões dos cores. */
const FALLBACK_CHOICES: Record<string, readonly string[]> = {
  bilinear: ['xBR'],
  xBR: ['bilinear'],
  some: ['enabled'],
  off: ['disabled'],
  disabled: ['off'],
  '16': ['16x'],
  '16x': ['16'],
  x4: ['4x', '4'],
  '1x(native)': ['1x (native)', '1x'],
  'per-strip (fast, least accurate)': ['per-strip'],
  'per-pixel (accurate)': ['per-pixel'],
}

/** Escala de resolução dos cores 3D. Fica de fora quem só muda o tamanho da tela 2D
 *  (`atari800_resolution`, `fbneo-resolution`). */
const RESOLUTION_KEY =
  /internal_resolution|opengl_resolution|screensize|resolutionscale|scale_factor|parallel-rdp-upscaling|upscale_multiplier/i
/** Largura e fator-alvo da Qualidade, cerca de 4x nos consoles 3D de 240p. */
const QUALITY_WIDTH = 1920
const QUALITY_FACTOR = 4

/** Tamanho comparável de um valor: largura de `640x480`, ou o fator de `4x`, `1x(native)` e `4`. */
function resolutionSize(value: string): { kind: 'width' | 'factor'; size: number } | null {
  const pixels = /^(\d+)x\d+/i.exec(value)
  if (pixels) return { kind: 'width', size: Number(pixels[1]) }
  const factor = /^(\d+(?:\.\d+)?)x?(?:[\s(]|$)/i.exec(value)
  return factor ? { kind: 'factor', size: Number(factor[1]) } : null
}

/** O maior valor até o alvo; sem nenhum, o padrão do core. */
function qualityResolution(option: CoreOption): string {
  let best = option.defaultValue
  let bestSize = -1
  for (const value of option.choices) {
    const parsed = resolutionSize(value)
    if (!parsed) continue
    const limit = parsed.kind === 'width' ? QUALITY_WIDTH : QUALITY_FACTOR
    if (parsed.size <= limit && parsed.size > bestSize) {
      best = value
      bestSize = parsed.size
    }
  }
  return best
}

/** O menor valor de resolução oferecido pelo core (nativa). */
function nativeResolution(option: CoreOption): string | null {
  let best: string | null = null
  let bestSize = Infinity
  for (const value of option.choices) {
    const parsed = resolutionSize(value)
    if (!parsed) continue
    const limit = parsed.kind === 'width' ? 640 : 1
    if (parsed.size <= limit && parsed.size < bestSize) {
      best = value
      bestSize = parsed.size
    }
  }
  return best
}

/** Casa o valor pedido com o que esta versão do core realmente oferece em `choices`,
 *  usada tanto em `detectLevel` quanto em `presetChanges` para manter os dois idênticos. */
function resolveChoice(option: CoreOption, wanted: string | undefined): string | null {
  if (wanted === undefined) return null
  if (option.choices.includes(wanted)) return wanted
  const lower = wanted.toLowerCase()
  const caseMatch = option.choices.find((c) => c.toLowerCase() === lower)
  if (caseMatch !== undefined) return caseMatch

  for (const alt of FALLBACK_CHOICES[wanted] ?? []) {
    if (option.choices.includes(alt)) return alt
    const altMatch = option.choices.find((c) => c.toLowerCase() === alt.toLowerCase())
    if (altMatch !== undefined) return altMatch
  }

  if (RESOLUTION_KEY.test(option.key)) {
    const target = resolutionSize(wanted)
    if (!target) return null
    const sameSize = option.choices.find((c) => {
      const parsed = resolutionSize(c)
      return parsed !== null && parsed.kind === target.kind && parsed.size === target.size
    })
    if (sameSize !== undefined) return sameSize

    const isUpscaled =
      (target.kind === 'factor' && target.size > 1) ||
      (target.kind === 'width' && target.size >= 960)
    if (isUpscaled) {
      const best = qualityResolution(option)
      const parsed = resolutionSize(best)
      const minUpscaled = parsed?.kind === 'width' ? 640 : 1
      if (option.choices.includes(best) && parsed && parsed.size > minUpscaled) {
        return best
      }
    } else {
      return nativeResolution(option)
    }
  }

  return null
}

/** Por enquanto só resolução: Desempenho no padrão do core, em geral a nativa, e
 *  Qualidade perto de 4x. */
function resolutionPreset(options: readonly CoreOption[]): CorePreset {
  const performance: Record<string, string> = {}
  const quality: Record<string, string> = {}
  for (const option of options) {
    if (!RESOLUTION_KEY.test(option.key)) continue
    performance[option.key] = option.defaultValue
    quality[option.key] = qualityResolution(option)
  }
  return { performance, quality, resolutionOnly: true }
}

/** A predefinição completa do core ou, sem ela, a só de resolução. */
export function corePreset(coreId: string, options: readonly CoreOption[]): CorePreset {
  return PRESETS[coreId] ?? resolutionPreset(options)
}

/** `false` se o core não oferece nenhuma opção que a predefinição possa mudar. */
export function presetHasOptions(preset: CorePreset): boolean {
  return Object.keys(preset.quality).length > 0 || Object.keys(preset.performance).length > 0
}

/** O nível que os valores gravados no jogo representam. Só contam as chaves das
 *  predefinições: ajuste em outra opção fica como está e não muda o nível. */
export function detectLevel(preset: CorePreset, options: readonly CoreOption[]): PresetLevel {
  const known = new Set([...Object.keys(preset.performance), ...Object.keys(preset.quality)])
  const relevant = options.filter((o) => known.has(o.key))
  if (relevant.every((o) => o.romValue == null)) return 'normal'
  // Valor que esta versão do core não oferece não é gravado, então não entra na conta.
  const matches = (values: Record<string, string>) =>
    relevant.every((o) => {
      const expected = resolveChoice(o, values[o.key])
      return (o.romValue ?? null) === expected
    })
  if (matches(preset.performance)) return 'performance'
  if (matches(preset.quality)) return 'quality'
  return 'custom'
}

/** O que gravar para chegar ao nível: `''` limpa o valor do jogo. Só usa opção e
 *  valor que a versão instalada do core oferece. Ajustes em outras chaves ficam. */
export function presetChanges(
  preset: CorePreset,
  level: 'performance' | 'normal' | 'quality',
  options: readonly CoreOption[],
): { key: string; value: string }[] {
  const target = level === 'normal' ? {} : preset[level]
  const known = new Set([...Object.keys(preset.performance), ...Object.keys(preset.quality)])
  const changes: { key: string; value: string }[] = []
  for (const option of options) {
    if (!known.has(option.key)) continue
    const expected = level === 'normal' ? null : resolveChoice(option, target[option.key])
    const value = expected ?? ''
    if ((option.romValue ?? '') !== value) changes.push({ key: option.key, value })
  }
  return changes
}
