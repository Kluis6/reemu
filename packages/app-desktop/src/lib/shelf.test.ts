import { describe, expect, it } from 'vitest'
import { CARD_MAX, CARD_MIN, SHELF_GAP, cardWidthPx, shelfCapacity, shelfFillWidth } from './shelf'

describe('cardWidthPx', () => {
  it('respeita piso e teto', () => {
    expect(cardWidthPx(320)).toBe(CARD_MIN)
    expect(cardWidthPx(10_000)).toBe(CARD_MAX)
  })

  it('escala com a janela entre os dois', () => {
    expect(cardWidthPx(1920)).toBe(Math.round(1920 * 0.116))
  })
})

describe('shelfCapacity', () => {
  it('nunca é menor que 1', () => {
    expect(shelfCapacity(50, 1920)).toBe(1)
  })

  it('conta os gaps entre os cards', () => {
    const card = cardWidthPx(1920)
    const exato = 5 * card + 4 * SHELF_GAP
    expect(shelfCapacity(exato, 1920)).toBe(5)
    expect(shelfCapacity(exato - 1, 1920)).toBe(4)
  })
})

describe('shelfFillWidth', () => {
  it('enche a prateleira sem nunca passar dela (senão ela rola ao focar)', () => {
    for (const [shelf, vw] of [[1300, 1366], [1800, 1920], [3700, 3840], [1535.2, 1536]]) {
      const cap = shelfCapacity(shelf, vw)
      const w = shelfFillWidth(shelf, vw, cap)
      const total = cap * w + (cap - 1) * SHELF_GAP
      expect(total).toBeLessThanOrEqual(shelf + 1e-9)
      expect(shelf - total).toBeLessThan(0.01 * cap + 1e-9)
    }
  })

  it('nunca encolhe abaixo do nominal', () => {
    expect(shelfFillWidth(100, 1920, 3)).toBe(cardWidthPx(1920))
  })

  it('cap inválido devolve o nominal', () => {
    expect(shelfFillWidth(1000, 1920, 0)).toBe(cardWidthPx(1920))
  })
})
