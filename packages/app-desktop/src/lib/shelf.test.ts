import { describe, expect, it } from 'vitest'
import { CARD_W } from '../styles/metrics'
import { SHELF_GAP, shelfCapacity, shelfFillWidth } from './shelf'

describe('CARD_W', () => {
  it('cabem 6 cards numa fileira da tela de referência 16:9 (regra dos 6 cliques)', () => {
    // 1366 − rail (68) − paddings (47 + 47) = 1204 epx, a área dos cards do
    // app Xbox em 1366×768
    expect(shelfCapacity(1204)).toBe(6)
    expect(shelfFillWidth(1204, 6)).toBe(184)
  })
})

describe('shelfCapacity', () => {
  it('nunca devolve menos que 1', () => {
    expect(shelfCapacity(50)).toBe(1)
  })

  it('conta exata nas bordas', () => {
    const exato = 5 * CARD_W + 4 * SHELF_GAP
    expect(shelfCapacity(exato)).toBe(5)
    expect(shelfCapacity(exato - 1)).toBe(4)
  })
})

describe('shelfFillWidth', () => {
  it('a fileira enche a prateleira sem passar dela', () => {
    for (const shelf of [818, 1204, 1234.5, 2000]) {
      const cap = shelfCapacity(shelf)
      const w = shelfFillWidth(shelf, cap)
      const total = cap * w + (cap - 1) * SHELF_GAP
      expect(total).toBeLessThanOrEqual(shelf)
      expect(shelf - total).toBeLessThan(cap * 0.01 + 1e-9)
    }
  })

  it('nunca encolhe abaixo do nominal', () => {
    expect(shelfFillWidth(100, 3)).toBe(CARD_W)
  })

  it('sem capacidade devolve o nominal', () => {
    expect(shelfFillWidth(1000, 0)).toBe(CARD_W)
  })
})
