import type { CoreOption } from './bindings/CoreOption'

/** Valores de cada predefinição; Normal é o padrão do core, sem valores próprios. */
export interface CorePreset {
  performance: Record<string, string>
  quality: Record<string, string>
  /** Sem predefinição pronta: muda só a resolução. */
  resolutionOnly?: boolean
}

export type PresetLevel = 'performance' | 'normal' | 'quality' | 'custom'

/**
 * Por core. Chaves, valores e efeitos conferidos em `libretro/libretro_core_options.h`
 * do PPSSPP e em docs.libretro.com/library/ppsspp. `mulitsample` é a grafia da chave
 * no core. `skip_buffer_effects` fica de fora: alguns jogos podem não desenhar nada.
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
}

/** Escala de resolução dos cores 3D. Fica de fora quem só muda o tamanho da tela 2D
 *  (`atari800_resolution`, `fbneo-resolution`). */
const RESOLUTION_KEY =
  /internal_resolution|opengl_resolution|screensize|resolutionscale|parallel-rdp-upscaling|upscale_multiplier/i
/** Largura e fator-alvo da Qualidade, cerca de 4x nos consoles 3D de 240p. */
const QUALITY_WIDTH = 1920
const QUALITY_FACTOR = 4

/** Tamanho comparável de um valor: largura de `640x480`, ou o fator de `4x` e `4`. */
function resolutionSize(value: string): { kind: 'width' | 'factor'; size: number } | null {
  const pixels = /^(\d+)x\d+/.exec(value)
  if (pixels) return { kind: 'width', size: Number(pixels[1]) }
  const factor = /^(\d+(?:\.\d+)?)x?(?:\s|$)/.exec(value)
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
      const wanted = values[o.key]
      const expected = wanted !== undefined && o.choices.includes(wanted) ? wanted : null
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
    const wanted = target[option.key]
    const value = wanted !== undefined && option.choices.includes(wanted) ? wanted : ''
    if ((option.romValue ?? '') !== value) changes.push({ key: option.key, value })
  }
  return changes
}
