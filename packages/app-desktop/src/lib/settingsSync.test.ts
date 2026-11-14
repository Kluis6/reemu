// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest'
import i18n from '../i18n'
import type { AppearanceDoc } from './appearanceGroup'
import {
  APPEARANCE_UNDO_KEY,
  clearAppearanceUndo,
  readAppearanceUndo,
  saveAppearanceUndo,
  settingsGroupErrorText,
} from './settingsSync'

describe('settingsGroupErrorText', () => {
  const t = i18n.getFixedT('pt-BR')

  it('a recusa da nuvem diz o grupo, não "save"', () => {
    const big = settingsGroupErrorText(t, 'core-options', 'too_large')
    expect(big).toBe('O grupo Opções dos cores passa do tamanho que a nuvem aceita e não foi enviado.')
    const refused = settingsGroupErrorText(t, 'video', 'rejected')
    expect(refused).toBe('A nuvem recusou o grupo Vídeo, que não foi enviado.')
    expect(`${big} ${refused}`).not.toMatch(/save/i)
  })

  it('o que parou o envio todo usa o texto de sync.err', () => {
    expect(settingsGroupErrorText(t, 'audio', 'no_tier')).toBe(t('sync.err.no_tier'))
    expect(settingsGroupErrorText(t, 'audio', 'quota')).toBe(t('sync.err.quota'))
    expect(settingsGroupErrorText(t, 'audio', 'codigo_que_nao_existe')).toBe(t('sync.err.failed'))
  })

  it('cada grupo tem nome nos três idiomas', () => {
    const groups = ['core-options', 'shaders-decorations', 'hotkeys-keyboard', 'audio', 'appearance', 'video'] as const
    for (const lang of ['pt-BR', 'en', 'es']) {
      const tl = i18n.getFixedT(lang)
      for (const g of groups) {
        const text = settingsGroupErrorText(tl, g, 'rejected')
        expect(text).not.toContain('sync.settingsGroup')
        expect(text).not.toContain('{{group}}')
      }
    }
  })
})

describe('appearance undo in localStorage', () => {
  const DOC: AppearanceDoc = {
    v: 1,
    themeSelection: '{"selection":{"id":"dark"}}',
    uiScale: 1,
    overscan: 0,
    uiSounds: true,
    reduceMotion: true,
  }

  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  it('comes back only for the copy it was saved with', () => {
    saveAppearanceUndo(DOC, 1_700_000_000)
    expect(readAppearanceUndo(1_700_000_000)).toEqual(DOC)
    expect(readAppearanceUndo(1_700_000_001)).toBeNull()
    clearAppearanceUndo()
    expect(readAppearanceUndo(1_700_000_000)).toBeNull()
  })

  it('a document without its copy (an older version) never comes back', () => {
    localStorage.setItem(APPEARANCE_UNDO_KEY, JSON.stringify(DOC))
    expect(readAppearanceUndo(1_700_000_000)).toBeNull()
    localStorage.setItem(APPEARANCE_UNDO_KEY, '{ nao e json')
    expect(readAppearanceUndo(1_700_000_000)).toBeNull()
  })

  it('a localStorage that throws never breaks the screen', () => {
    const boom = () => {
      throw new Error('negado')
    }
    vi.stubGlobal('localStorage', { getItem: boom, setItem: boom, removeItem: boom })
    expect(() => saveAppearanceUndo(DOC, 1)).not.toThrow()
    expect(readAppearanceUndo(1)).toBeNull()
    expect(() => clearAppearanceUndo()).not.toThrow()
  })
})
