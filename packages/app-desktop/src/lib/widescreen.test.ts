import { describe, expect, it } from 'vitest'
import type { CoreOption } from './tauri'
import { widescreenActive, widescreenOption } from './widescreen'

const opt = (key: string, choices: string[]): CoreOption => ({
  key,
  displayName: key,
  choices,
  defaultValue: choices[0],
  value: choices[0],
  coreValue: null,
  romValue: null,
})

describe('widescreenOption', () => {
  const cheats = opt('reicast_widescreen_cheats', ['disabled', 'enabled'])
  const hack = opt('reicast_widescreen_hack', ['disabled', 'enabled'])

  it('no Flycast é o patch, com o hack como conflito', () => {
    const w = widescreenOption('flycast_libretro', [hack, cheats])
    expect(w?.option.key).toBe('reicast_widescreen_cheats')
    expect(w?.conflicts.map((c) => c.key)).toEqual(['reicast_widescreen_hack'])
  })

  it('só conta como ligado com o patch ligado e o hack desligado', () => {
    const on = { ...cheats, value: 'enabled' }
    expect(widescreenActive(widescreenOption('flycast_libretro', [on, hack])!)).toBe(true)
    // os dois juntos se somam (AR 2,38): aparece desligado pra corrigir
    const both = [on, { ...hack, value: 'enabled' }]
    expect(widescreenActive(widescreenOption('flycast_libretro', both)!)).toBe(false)
    expect(widescreenActive(widescreenOption('flycast_libretro', [cheats, hack])!)).toBe(false)
  })

  it('prefere o "adjusted" do Mupen; sem ele, o 16:9 puro', () => {
    expect(
      widescreenOption('mupen64plus_next_libretro', [
        opt('mupen64plus-aspect', ['4:3', '16:9', '16:9 adjusted']),
      ])?.on,
    ).toBe('16:9 adjusted')
    expect(
      widescreenOption('mupen64plus_next_libretro', [opt('mupen64plus-aspect', ['4:3', '16:9'])])?.on,
    ).toBe('16:9')
  })

  it('sem a chave, sem o valor ou sem o core na lista: nada', () => {
    expect(widescreenOption('flycast_libretro', [])).toBeNull()
    expect(widescreenOption('flycast_libretro', undefined)).toBeNull()
    expect(
      widescreenOption('flycast_libretro', [opt('reicast_widescreen_cheats', ['off', 'on'])]),
    ).toBeNull()
    expect(widescreenOption('snes9x_libretro', [cheats])).toBeNull()
  })
})
