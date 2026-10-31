import { describe, expect, it } from 'vitest'
import { OVERSCAN_ROWS, gridColumns, gridWindow, letterIndex, titleLetter } from './virtualGrid'

describe('trecho da grade', () => {
  it('no topo, começa do primeiro item', () => {
    const w = gridWindow(1000, 5, 100, 0, 500)
    expect(w.start).toBe(0)
    expect(w.padTop).toBe(0)
    expect(w.end).toBe((5 + OVERSCAN_ROWS) * 5)
  })

  it('a altura total não muda com a rolagem', () => {
    const total = (top: number) => {
      const w = gridWindow(1000, 5, 100, top, 500)
      return w.padTop + w.padBottom + (Math.ceil((w.end - w.start) / 5)) * 100
    }
    expect(total(0)).toBe(20000)
    expect(total(7300)).toBe(20000)
    expect(total(19500)).toBe(20000)
  })

  it('começa sempre no início de uma fileira', () => {
    expect(gridWindow(1000, 5, 100, 3333, 500).start % 5).toBe(0)
  })

  it('rolagem além do fim ainda mostra a última fileira', () => {
    const w = gridWindow(12, 5, 100, 99999, 500)
    expect(w.end).toBe(12)
    expect(w.start).toBeLessThan(12)
  })
})

describe('colunas', () => {
  it('segue o auto-fill e nunca fica sem coluna', () => {
    expect(gridColumns(1118, 184, 20)).toBe(5)
    expect(gridColumns(100, 184, 20)).toBe(1)
  })
})

describe('letras', () => {
  it('ignora acento e agrupa o resto em #', () => {
    expect(titleLetter('Ágata')).toBe('A')
    expect(titleLetter('1941')).toBe('#')
  })

  it('aponta para o primeiro título de cada letra', () => {
    expect(letterIndex(['1941', 'Aladdin', 'Asterix', 'Bomberman'])).toEqual([
      { letter: '#', index: 0 },
      { letter: 'A', index: 1 },
      { letter: 'B', index: 3 },
    ])
  })
})
