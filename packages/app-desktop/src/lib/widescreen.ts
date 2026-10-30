import type { CoreOption } from './tauri'

/**
 * Opção de widescreen de verdade (o core desenha mais cena) de cada core:
 * chave e os valores "ligado", do preferido ao último recurso. O interruptor
 * só aparece se o core declarar a chave e um desses valores — a lista vem do
 * que ele declara, não da documentação (a do Flycast chama de `flycast_*`, o
 * core instalado usa `reicast_*`).
 *
 * Fora daqui de propósito: opções que só esticam (`virtualjaguar_widescreen`,
 * `gearsystem_aspect_ratio`), que a Proporção da imagem já cobre.
 */
const WIDESCREEN: Record<string, { key: string; on: string[]; conflicts?: string[] }> = {
  mednafen_psx_hw_libretro: { key: 'beetle_psx_hw_widescreen_hack', on: ['enabled'] },
  mednafen_psx_libretro: { key: 'beetle_psx_widescreen_hack', on: ['enabled'] },
  swanstation_libretro: { key: 'swanstation_GPU_WidescreenHack', on: ['true', 'enabled'] },
  // Flycast: o patch por jogo (`widescreen_cheats`, core/cheats.cpp). O hack
  // genérico NÃO entra junto: os dois se somam (medido no Jet Set Radio,
  // 2026-10-30: AR 1,787 com o patch e 2,382 com os dois). Por isso ligar o
  // patch desliga o hack. O ReEmu não sabe quais jogos têm patch; nos outros,
  // o hack fica à mão na lista de opções do core.
  flycast_libretro: {
    key: 'reicast_widescreen_cheats',
    on: ['enabled'],
    conflicts: ['reicast_widescreen_hack'],
  },
  mupen64plus_next_libretro: { key: 'mupen64plus-aspect', on: ['16:9 adjusted', '16:9'] },
  parallel_n64_libretro: { key: 'parallel-n64-aspectratiohint', on: ['widescreen'] },
  bsnes_hd_beta_libretro: { key: 'bsnes_mode7_widescreen', on: ['16:9', 'ON', 'enabled'] },
  pcsx2_libretro: { key: 'pcsx2_widescreen_hint', on: ['enabled', 'true'] },
}

/** A opção de widescreen do core, o valor que a liga e as que não podem ficar
 *  ligadas junto com ela. */
export interface WidescreenOption {
  option: CoreOption
  on: string
  conflicts: CoreOption[]
}

/** A opção de widescreen que o core declara; `null` = não tem. */
export function widescreenOption(
  coreId: string,
  options: CoreOption[] | undefined,
): WidescreenOption | null {
  const entry = WIDESCREEN[coreId]
  const option = entry && options?.find((o) => o.key === entry.key)
  const on = option && entry.on.find((v) => option.choices.includes(v))
  if (!option || !on) return null
  const conflicts = (options ?? []).filter((o) => entry.conflicts?.includes(o.key))
  return { option, on, conflicts }
}

/** Ligada de verdade: a opção no valor certo e nenhum conflito ativo. */
export function widescreenActive(w: WidescreenOption): boolean {
  return w.option.value === w.on && w.conflicts.every((c) => c.value === c.defaultValue)
}
