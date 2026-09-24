// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
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

/** Espera as gravações "dispare e esqueça" (sem `await`) chegarem ao mock. */
const flush = () => new Promise((r) => setTimeout(r, 0))

describe('sincronização com o Rust', () => {
  let rustValue = ''
  const calls: [string, unknown][] = []

  beforeEach(() => {
    localStorage.clear()
    rustValue = ''
    calls.length = 0
    ;(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: { json?: string }) => {
        calls.push([cmd, args])
        if (cmd === 'get_theme_selection') return rustValue
        if (cmd === 'set_theme_selection') rustValue = args.json ?? ''
      },
    }
  })
  afterEach(() => {
    delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  it('o valor do Rust prevalece sobre o localStorage', async () => {
    rustValue = JSON.stringify({ selection: { kind: 'preset', id: 'alto-contraste' } })
    const s = await freshStore()
    const { syncFromRust } = await import('./useThemeStore')
    await syncFromRust()
    expect(s.getState().selection).toEqual({ kind: 'preset', id: 'alto-contraste' })
    expect(localStorage.getItem(KEY)).toContain('alto-contraste')
  })

  it('Rust vazio recebe o que estava no localStorage (migração)', async () => {
    localStorage.setItem(KEY, JSON.stringify({ selection: { kind: 'custom', hue: 90, mode: 'light' } }))
    await freshStore()
    const { syncFromRust } = await import('./useThemeStore')
    await syncFromRust()
    await flush()
    expect(JSON.parse(rustValue).selection).toEqual({ kind: 'custom', hue: 90, mode: 'light' })
  })

  it('trocar de tema grava no Rust', async () => {
    const s = await freshStore()
    s.getState().setPreset('alto-contraste')
    await flush()
    expect(calls.some(([c]) => c === 'set_theme_selection')).toBe(true)
    expect(JSON.parse(rustValue).selection).toEqual({ kind: 'preset', id: 'alto-contraste' })
  })
})
