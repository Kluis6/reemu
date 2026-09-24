// @vitest-environment happy-dom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { DEFAULT_THEME_ID } from '../styles/themes'

const KEY = 'reemu.theme'

// A store lê o `localStorage` na criação do módulo — cada teste reimporta.
async function freshStore() {
  vi.resetModules()
  return (await import('./useThemeStore')).useThemeStore
}

describe('useThemeStore', () => {
  beforeEach(() => localStorage.clear())

  it('sem nada salvo usa o tema padrão', async () => {
    const s = await freshStore()
    expect(s.getState().selection).toEqual({ kind: 'preset', id: DEFAULT_THEME_ID })
  })

  it('lê o formato antigo (só o id cru)', async () => {
    localStorage.setItem(KEY, DEFAULT_THEME_ID)
    const s = await freshStore()
    expect(s.getState().selection).toEqual({ kind: 'preset', id: DEFAULT_THEME_ID })
  })

  it('JSON inválido ou lixo cai no padrão', async () => {
    localStorage.setItem(KEY, '{nao-e-json')
    expect((await freshStore()).getState().selection.kind).toBe('preset')
    localStorage.setItem(KEY, JSON.stringify({ selection: { kind: 'custom', hue: 'x' } }))
    expect((await freshStore()).getState().selection.kind).toBe('preset')
  })

  it('personalizado persiste e sobrevive ao recarregar', async () => {
    const s = await freshStore()
    s.getState().setCustomHue(200)
    s.getState().setCustomMode('light')
    const again = await freshStore()
    expect(again.getState().selection).toEqual({ kind: 'custom', hue: 200, mode: 'light' })
  })

  it('voltar pra preset lembra o ajuste do personalizado', async () => {
    const s = await freshStore()
    s.getState().setCustomHue(42)
    s.getState().setPreset(DEFAULT_THEME_ID)
    s.getState().activateCustom()
    expect(s.getState().selection).toEqual({ kind: 'custom', hue: 42, mode: 'dark' })
  })
})
