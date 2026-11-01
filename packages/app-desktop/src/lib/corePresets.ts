import type { CoreOption } from './bindings/CoreOption'

/** Valores de cada predefinição; Normal é o padrão do core, sem valores próprios. */
export interface CorePreset {
  performance: Record<string, string>
  quality: Record<string, string>
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

export function corePreset(coreId: string): CorePreset | undefined {
  return PRESETS[coreId]
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
