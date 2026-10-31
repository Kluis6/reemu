import { describe, expect, it } from 'vitest'
import { fitZoom } from './uiScale'

describe('zoom', () => {
  it('tela maior que a lógica escala por igual', () => {
    expect(fitZoom(1920, 1080, 1)).toBe(1.5)
    expect(fitZoom(3840, 2160, 1)).toBe(3)
  })

  it('janela menor não encolhe: a tela se reorganiza', () => {
    expect(fitZoom(800, 600, 1)).toBe(1)
    expect(fitZoom(1280, 720, 1)).toBe(1)
  })

  it('a preferência multiplica', () => {
    expect(fitZoom(800, 600, 1.3)).toBe(1.3)
    expect(fitZoom(1920, 1080, 0.8)).toBeCloseTo(1.2, 3)
  })
})
