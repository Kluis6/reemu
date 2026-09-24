import { describe, expect, it } from 'vitest'
import { customBgPalette, THEMES } from './themes'

/** Razão de contraste WCAG 2.x entre duas cores `#rrggbb`. */
function contrast(a: string, b: string): number {
  const lum = (hex: string) => {
    const n = hex.replace('#', '')
    const [r, g, bl] = [0, 2, 4].map((i) => {
      const c = parseInt(n.slice(i, i + 2), 16) / 255
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl
  }
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

describe('tema de alto contraste', () => {
  const t = THEMES['alto-contraste'].theme
  const bg = t.colorNeutralBackground1

  it.each([
    ['texto principal', t.colorNeutralForeground1, bg],
    ['texto secundário', t.colorNeutralForeground2, bg],
    ['texto de marca', t.reemuBrandText, bg],
    ['texto sobre a marca', t.reemuOnBrand, t.reemuBrandSolid],
    ['item selecionado', t.reemuActiveFg, t.reemuActiveBg],
  ])('%s passa AAA (7:1)', (_, fg, back) => {
    expect(contrast(fg, back)).toBeGreaterThanOrEqual(7)
  })

  it('fundo sem as manchas de cor do tema', () => {
    expect([t.reemuBg1, t.reemuBg2, t.reemuBg3, t.reemuBg4]).toEqual([bg, bg, bg, bg])
  })
})

describe('paletas do fundo', () => {
  it('todo tema tem as 4 cores do fundo', () => {
    for (const [id, { theme }] of Object.entries(THEMES)) {
      for (const c of [theme.reemuBg1, theme.reemuBg2, theme.reemuBg3, theme.reemuBg4]) {
        expect(c, id).toMatch(/^#[0-9a-fA-F]{6}$/)
      }
    }
  })

  it('o Personalizado deriva 4 matizes distintos', () => {
    const p = customBgPalette(205)
    expect(new Set([p.bg1, p.bg2, p.bg3, p.bg4]).size).toBe(4)
  })
})
