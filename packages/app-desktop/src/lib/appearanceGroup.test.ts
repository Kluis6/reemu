// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import i18n, { setLanguagePreference } from '../i18n'
import { useThemeStore } from '../stores/useThemeStore'
import { DEFAULT_CUSTOM_HUE, DEFAULT_THEME_ID } from '../styles/themes'
import { applyAppearance, exportAppearance } from './appearanceGroup'
import { setReduceMotion, setUiSounds } from './uiPrefs'
import { setOverscan, setUiScale } from './uiScale'

// O Rust de mentira entra antes dos imports: `lib/tauri.ts` decide se está no
// Tauri quando o módulo carrega.
const rust = vi.hoisted(() => {
  const state = { theme: '', calls: [] as string[] }
  ;(window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: async (cmd: string, args?: { json?: string }) => {
      state.calls.push(cmd)
      if (cmd === 'get_theme_selection') return state.theme
      if (cmd === 'set_theme_selection') state.theme = args?.json ?? ''
    },
  }
  return state
})

const THEME = JSON.stringify({
  selection: { kind: 'custom', hue: 200, mode: 'light' },
  customDraft: { hue: 200, mode: 'light' },
})

/** Uma máquina de fábrica. */
async function factory() {
  localStorage.clear()
  rust.theme = ''
  useThemeStore.setState({
    selection: { kind: 'preset', id: DEFAULT_THEME_ID },
    customDraft: { hue: DEFAULT_CUSTOM_HUE, mode: 'dark' },
  })
  document.documentElement.removeAttribute('data-reduce-motion')
  await i18n.changeLanguage('pt-BR')
}

/** Esta máquina, com tudo trocado. */
async function configured() {
  rust.theme = THEME
  await setUiScale(1.3)
  setOverscan(5)
  setUiSounds(false)
  setReduceMotion(true)
  await setLanguagePreference('es')
}

describe('appearanceGroup', () => {
  beforeEach(async () => {
    await factory()
    rust.calls.length = 0
  })
  afterEach(factory)

  it('round trip: exportado aqui e aplicado numa máquina de fábrica, volta igual', async () => {
    await configured()
    const doc = await exportAppearance()
    expect(doc).toEqual({
      v: 1,
      themeSelection: THEME,
      uiScale: 1.3,
      overscan: 5,
      uiSounds: false,
      reduceMotion: true,
      language: 'es',
    })

    await factory()
    expect(await exportAppearance()).not.toEqual(doc)
    // como volta da nuvem: texto
    await applyAppearance(JSON.parse(JSON.stringify(doc)))
    expect(await exportAppearance()).toEqual(doc)
    expect(useThemeStore.getState().selection).toEqual({ kind: 'custom', hue: 200, mode: 'light' })
    expect(document.documentElement.hasAttribute('data-reduce-motion')).toBe(true)
    expect(i18n.language).toBe('es')
  })

  it('o papel de parede não vai: é um arquivo desta máquina', async () => {
    const doc = await exportAppearance()
    expect(Object.keys(doc).some((k) => /wallpaper/i.test(k))).toBe(false)
    await applyAppearance(doc)
    expect(rust.calls.some((c) => c.includes('wallpaper'))).toBe(false)
  })

  it('versão desconhecida é recusada e nada muda', async () => {
    await configured()
    const before = await exportAppearance()
    rust.calls.length = 0
    const newer: Record<string, unknown> = { ...before, v: 2, uiScale: 0.8, language: 'en' }
    await expect(applyAppearance(newer)).rejects.toThrow('settings_version')
    delete newer.v
    await expect(applyAppearance(newer)).rejects.toThrow('settings_version')
    expect(await exportAppearance()).toEqual(before)
    expect(rust.calls).not.toContain('set_theme_selection')
  })

  it('formato errado é recusado antes de mudar qualquer coisa', async () => {
    await configured()
    const before = await exportAppearance()
    await expect(applyAppearance({ ...before, uiSounds: 'sim' })).rejects.toThrow('settings_invalid')
    await expect(
      applyAppearance({ ...before, uiScale: 0.8, themeSelection: '{nao-e-json' }),
    ).rejects.toThrow('settings_invalid')
    await expect(applyAppearance(null)).rejects.toThrow('settings_invalid')
    expect(await exportAppearance()).toEqual(before)
  })

  it('valor que esta versão não oferece deixa o desta máquina', async () => {
    await configured()
    const before = await exportAppearance()
    await applyAppearance({ ...before, uiScale: 1.5, overscan: 7, language: 'fr', uiSounds: true })
    expect(await exportAppearance()).toEqual({ ...before, uiSounds: true })
  })
})
