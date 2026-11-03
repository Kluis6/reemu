import { describe, expect, it } from 'vitest'
import { tierAccent, tierRing } from './tierRing'

// O que o Alvanista manda para o Collector (2026-11-01).
const COLLECTOR = {
  ring: { kind: 'linear', angle: 140, stops: ['#d1fae5', '#34d399', '#059669'] },
  glow: { color: '#34d399', alpha: 0.55 },
  accent: '#22d3ee',
}

describe('tierRing', () => {
  it('linear: o degradê do CSS e o brilho em rgba', () => {
    expect(tierRing(COLLECTOR)).toEqual({
      background: 'linear-gradient(140deg, #d1fae5, #34d399, #059669)',
      glow: 'rgba(52, 211, 153, 0.55)',
    })
  })

  it('conic começa no ângulo', () => {
    const r = tierRing({ ...COLLECTOR, ring: { kind: 'conic', angle: 0, stops: ['#f5c542', '#34d399', '#f5c542'] } })
    expect(r?.background).toBe('conic-gradient(from 0deg, #f5c542, #34d399, #f5c542)')
  })

  it('sem cores, ou com algo que não dá para usar, não há anel', () => {
    expect(tierRing(null)).toBeNull()
    expect(tierRing(undefined)).toBeNull()
    expect(tierRing({})).toBeNull()
    for (const ring of [
      { kind: 'radial', angle: 0, stops: ['#000000', '#ffffff'] },
      { kind: 'linear', angle: Number.NaN, stops: ['#000000', '#ffffff'] },
      { kind: 'linear', angle: 140, stops: ['#000000'] },
      { kind: 'linear', angle: 140, stops: ['red', '#ffffff'] },
      { kind: 'linear', angle: 140, stops: ['#fff', '#ffffff'] },
      { kind: 'linear', angle: 140, stops: ['#000000);background:url(x)', '#ffffff'] },
      { kind: 'linear', angle: '140', stops: ['#000000', '#ffffff'] },
    ]) {
      expect(tierRing({ ...COLLECTOR, ring: ring as never }), JSON.stringify(ring)).toBeNull()
    }
  })

  it('um brilho que não dá para usar tira só o brilho', () => {
    for (const glow of [undefined, { color: 'green', alpha: 0.5 }, { color: '#34d399', alpha: 2 }, { color: '#34d399' }]) {
      const r = tierRing({ ...COLLECTOR, glow: glow as never })
      expect(r?.background).toBe('linear-gradient(140deg, #d1fae5, #34d399, #059669)')
      expect(r?.glow).toBeNull()
    }
  })
})

describe('tierAccent', () => {
  it('só uma cor #rrggbb tinge o chip', () => {
    expect(tierAccent(COLLECTOR)).toBe('#22d3ee')
    expect(tierAccent({ accent: 'cyan' })).toBeNull()
    expect(tierAccent(null)).toBeNull()
  })
})
