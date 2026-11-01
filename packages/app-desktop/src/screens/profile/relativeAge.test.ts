import { describe, expect, it } from 'vitest'
import { relativeAge } from './relativeAge'

const NOW = Date.UTC(2026, 9, 31, 12) // ms
const ago = (secs: number) => Math.floor(NOW / 1000) - secs
const DAY = 86400

describe('relativeAge', () => {
  it('segundos, minutos, horas e dias como antes', () => {
    expect(relativeAge(ago(10), NOW, 'pt-BR')).toBe('agora')
    expect(relativeAge(ago(5 * 60), NOW, 'pt-BR')).toBe('há 5 minutos')
    expect(relativeAge(ago(3 * 3600), NOW, 'pt-BR')).toBe('há 3 horas')
    expect(relativeAge(ago(3 * DAY), NOW, 'pt-BR')).toBe('há 3 dias')
    expect(relativeAge(ago(29 * DAY), NOW, 'pt-BR')).toBe('há 29 dias')
  })

  it('meses e anos, sempre com o número', () => {
    expect(relativeAge(ago(45 * DAY), NOW, 'pt-BR')).toBe('há 1 mês')
    expect(relativeAge(ago(200 * DAY), NOW, 'pt-BR')).toBe('há 6 meses')
    expect(relativeAge(ago(400 * DAY), NOW, 'pt-BR')).toBe('há 1 ano')
    expect(relativeAge(ago(3 * 365 * DAY), NOW, 'en')).toBe('3 years ago')
  })

  it('um instante no futuro (relógio adiantado) vale como agora', () => {
    expect(relativeAge(ago(-120), NOW, 'pt-BR')).toBe('agora')
  })
})
