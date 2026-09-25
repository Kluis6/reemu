import { describe, expect, it } from 'vitest'
import en from './locales/en'
import es from './locales/es'
import ptBR from './locales/pt-BR'
import { matchLanguage, systemLanguage } from './index'

/** Todas as chaves-folha (`a.b.c`) de um objeto de mensagens. */
function keys(o: object, prefix = ''): string[] {
  return Object.entries(o).flatMap(([k, v]) =>
    typeof v === 'string' ? [`${prefix}${k}`] : keys(v as object, `${prefix}${k}.`),
  )
}

/** Variáveis de interpolação `{{x}}` de um texto. */
const vars = (s: string) => [...s.matchAll(/\{\{(\w+)\}\}/g)].map((m) => m[1]).sort()

function get(o: object, path: string): string {
  return path.split('.').reduce((a: any, k) => a[k], o) as string
}

describe('locales', () => {
  const base = keys(ptBR).sort()
  it.each([
    ['en', en],
    ['es', es],
  ])('%s tem exatamente as chaves do pt-BR e as mesmas variáveis', (_, msgs) => {
    expect(keys(msgs).sort()).toEqual(base)
    for (const k of base) {
      expect(vars(get(msgs, k)), k).toEqual(vars(get(ptBR, k)))
      expect(get(msgs, k).trim(), k).not.toBe('')
    }
  })
})

describe('idioma do sistema', () => {
  it('casa pela língua-base', () => {
    expect(matchLanguage('pt-PT')).toBe('pt-BR')
    expect(matchLanguage('es-MX')).toBe('es')
    expect(matchLanguage('en_GB')).toBe('en')
    expect(matchLanguage('fr-FR')).toBeNull()
  })
  it('pega o primeiro suportado e cai no pt-BR', () => {
    expect(systemLanguage(['fr-FR', 'es-AR', 'en-US'])).toBe('es')
    expect(systemLanguage(['de-DE'])).toBe('pt-BR')
    expect(systemLanguage([])).toBe('pt-BR')
  })
})
