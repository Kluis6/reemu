import { describe, expect, it } from 'vitest'
import i18n from '../i18n'
import { settingsGroupErrorText } from './settingsSync'

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
