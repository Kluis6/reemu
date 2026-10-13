import { describe, expect, it } from 'vitest'
import { customBgPalette, familyOf, THEME_FAMILIES, THEMES, type ThemeId } from './themes'

describe('THEME_FAMILIES', () => {
  it('cada tema está em exatamente uma família, no modo certo', () => {
    const ids = THEME_FAMILIES.flatMap((f) => [f.dark, ...(f.light ? [f.light] : [])])
    expect([...ids].sort()).toEqual(Object.keys(THEMES).sort())
    for (const f of THEME_FAMILIES) {
      expect(familyOf(f.dark)).toEqual({ family: f, mode: f.lightOnly ? 'light' : 'dark' })
      if (f.light) expect(familyOf(f.light as ThemeId)).toEqual({ family: f, mode: 'light' })
    }
  })
})

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

describe('tema Alva', () => {
  it.each(['alva', 'alva-claro'] as const)('%s: texto legível (AA) sobre os fundos', (id) => {
    const t = THEMES[id].theme
    for (const back of [t.colorNeutralBackground1, t.colorNeutralBackground2]) {
      expect(contrast(t.colorNeutralForeground1, back)).toBeGreaterThanOrEqual(4.5)
      expect(contrast(t.reemuBrandText, back)).toBeGreaterThanOrEqual(4.5)
    }
    expect(contrast(t.reemuOnBrand, t.reemuBrandSolid)).toBeGreaterThanOrEqual(3)
    expect(contrast(t.reemuActiveFg, t.reemuActiveBg)).toBeGreaterThanOrEqual(3)
  })

  it('usa o azul do Alvanista como marca', () => {
    // preenchimento sólido de marca = tom 80 da rampa = #0094D3 do site
    expect(THEMES.alva.theme.reemuBrandSolid.toLowerCase()).toBe('#0094d3')
    expect(THEMES['alva-claro'].theme.reemuActiveBg.toLowerCase()).toBe('#0094d3')
  })
})

describe('temas de fundo simples', () => {
  it.each(['simple-vibes', 'serene'] as const)('%s: fundo de uma cor só', (id) => {
    const t = THEMES[id].theme
    const bg = t.reemuPage
    expect([t.reemuBg1, t.reemuBg2, t.reemuBg3, t.reemuBg4, t.reemuGlowVeil]).toEqual([bg, bg, bg, bg, bg])
  })

  it.each(['simple-vibes', 'serene'] as const)('%s: texto legível (AA)', (id) => {
    const t = THEMES[id].theme
    for (const back of [t.colorNeutralBackground1, t.colorNeutralBackground2]) {
      expect(contrast(t.colorNeutralForeground1, back)).toBeGreaterThanOrEqual(4.5)
      expect(contrast(t.colorNeutralForeground2, back)).toBeGreaterThanOrEqual(4.5)
      expect(contrast(t.colorNeutralForeground3, back)).toBeGreaterThanOrEqual(4.5)
      expect(contrast(t.reemuBrandText, back)).toBeGreaterThanOrEqual(4.5)
    }
    expect(contrast(t.reemuOnBrand, t.reemuBrandSolid)).toBeGreaterThanOrEqual(4.5)
    expect(contrast(t.reemuActiveFg, t.reemuActiveBg)).toBeGreaterThanOrEqual(4.5)
    expect(contrast(t.colorNeutralForegroundOnBrand, t.colorBrandBackground)).toBeGreaterThanOrEqual(4.5)
  })

  it('Serene: branco como destaque e superfícies um degrau acima da página', () => {
    const t = THEMES.serene.theme
    expect(t.colorBrandBackground.toLowerCase()).toBe('#e9e9e9')
    expect(t.colorBrandStroke1.toLowerCase()).toBe('#f2f2f3')
    expect(contrast(t.colorNeutralForegroundOnBrand, t.colorBrandBackgroundPressed)).toBeGreaterThanOrEqual(4.5)
    // Xbox: página #1A1B1E, superfície #25282C (~1,2:1). Card e linhas das
    // Configurações sobem um degrau cada.
    const page = t.reemuPage
    expect(contrast(t.colorNeutralBackground2, page)).toBeGreaterThanOrEqual(1.18)
    expect(contrast(t.reemuSurface1, page)).toBeGreaterThanOrEqual(1.18)
    expect(contrast(t.reemuSurface2, t.reemuSurface1)).toBeGreaterThanOrEqual(1.15)
    expect(contrast(t.reemuSurface3, t.reemuSurface2)).toBeGreaterThanOrEqual(1.15)
    // texto secundário legível (AA) em todas as camadas
    for (const back of [t.reemuSurface1, t.reemuSurface2, t.reemuSurface3]) {
      expect(contrast(t.colorNeutralForeground3, back)).toBeGreaterThanOrEqual(4.5)
    }
  })

  it('Simple Vibes é só claro e Serene só escuro', () => {
    expect(familyOf('simple-vibes').mode).toBe('light')
    expect(familyOf('simple-vibes').family.light).toBeUndefined()
    expect(familyOf('serene').mode).toBe('dark')
    expect(familyOf('serene').family.light).toBeUndefined()
  })
})

describe('destaque de ícones e indicadores', () => {
  it.each(Object.keys(THEMES) as ThemeId[])('%s: 3:1 com página e superfícies', (id) => {
    const t = THEMES[id].theme
    for (const bg of [t.colorNeutralBackground1, t.colorNeutralBackground2, t.colorNeutralBackground3]) {
      if (!/^#[0-9a-fA-F]{6}$/.test(bg)) continue
      expect(contrast(t.reemuBrandAccent, bg), `${id} sobre ${bg}`).toBeGreaterThanOrEqual(3)
    }
    // os indicadores do Fluent seguem o mesmo destaque
    if (id !== 'alto-contraste') {
      expect(t.colorCompoundBrandBackground).toBe(t.reemuBrandAccent)
      expect(t.colorBrandStroke1).toBe(t.reemuBrandAccent)
    }
  })

  it('usa o tom primário (80) quando ele já tem contraste', () => {
    expect(THEMES.serene.theme.reemuBrandAccent).toBe(THEMES.serene.theme.reemuBrandSolid)
    expect(THEMES.reemu.theme.reemuBrandAccent).toBe(THEMES.reemu.theme.reemuBrandSolid)
  })
})

describe('tema Dream Sky', () => {
  it.each(['dream', 'dream-claro'] as const)('%s: texto legível (AA) sobre os fundos', (id) => {
    const t = THEMES[id].theme
    for (const back of [t.colorNeutralBackground1, t.colorNeutralBackground2]) {
      expect(contrast(t.colorNeutralForeground1, back)).toBeGreaterThanOrEqual(4.5)
      expect(contrast(t.colorNeutralForeground2, back)).toBeGreaterThanOrEqual(4.5)
      expect(contrast(t.colorNeutralForeground3, back)).toBeGreaterThanOrEqual(4.5)
      expect(contrast(t.reemuBrandText, back)).toBeGreaterThanOrEqual(4.5)
    }
    expect(contrast(t.reemuOnBrand, t.reemuBrandSolid)).toBeGreaterThanOrEqual(3)
    expect(contrast(t.reemuActiveFg, t.reemuActiveBg)).toBeGreaterThanOrEqual(3)
  })
})

describe('superfície dos cards', () => {
  it.each(Object.keys(THEMES).filter((id) => id !== 'alto-contraste') as ThemeId[])(
    '%s: card se separa da página',
    (id) => {
      const t = THEMES[id].theme
      const page = t.reemuPage
      // escuro: topo ≥ 1,3:1 e base ≥ 1,15:1; claro: superfície branca (sem
      // traço, como no app Xbox)
      const dark = contrast(t.colorNeutralForeground1, '#000000') > contrast(t.colorNeutralForeground1, '#ffffff')
      if (dark) {
        expect(contrast(t.reemuCardTop, page), id).toBeGreaterThanOrEqual(1.3)
        expect(contrast(t.reemuCardBottom, page), id).toBeGreaterThanOrEqual(1.15)
      } else {
        expect(contrast(t.reemuCardTop, page), id).toBeGreaterThanOrEqual(1.08)
        expect(t.reemuCardStroke).toBe('transparent')
      }
    },
  )
})

describe('elevação (Fluent: Background1 = superfície acima da página)', () => {
  it.each(Object.keys(THEMES).filter((id) => id !== 'alto-contraste') as ThemeId[])(
    '%s: Background1 um degrau acima da página',
    (id) => {
      const t = THEMES[id].theme
      // escuro: superfície mais CLARA que a página (≥ 1,15:1, o Xbox tem ~1,2);
      // claro: superfície mais clara (branca) que a página (≥ 1,08:1)
      expect(contrast(t.colorNeutralBackground1, t.reemuPage), id).toBeGreaterThanOrEqual(1.08)
      const lum = (h: string) => parseInt(h.slice(1, 3), 16) + parseInt(h.slice(3, 5), 16) + parseInt(h.slice(5, 7), 16)
      expect(lum(t.colorNeutralBackground1), id).toBeGreaterThan(lum(t.reemuPage))
      // texto continua legível na superfície
      expect(contrast(t.colorNeutralForeground1, t.colorNeutralBackground1), id).toBeGreaterThanOrEqual(4.5)
    },
  )
})

describe('escada de elevação', () => {
  const lum = (h: string) => parseInt(h.slice(1, 3), 16) + parseInt(h.slice(3, 5), 16) + parseInt(h.slice(5, 7), 16)
  it.each(Object.keys(THEMES).filter((id) => id !== 'alto-contraste') as ThemeId[])(
    '%s: página → superfície → realces em ordem',
    (id) => {
      const t = THEMES[id].theme
      const p = t.reemuPage
      // superfície única (Background1 = Background2)
      expect(t.colorNeutralBackground2, id).toBe(t.colorNeutralBackground1)
      const dark = lum(t.colorNeutralForeground1) > lum(p)
      if (dark) {
        // escuro: cada degrau mais claro
        expect(lum(t.colorNeutralBackground1)).toBeGreaterThan(lum(p))
        expect(lum(t.colorNeutralBackground3)).toBeGreaterThan(lum(t.colorNeutralBackground1))
        expect(lum(t.colorNeutralBackground4)).toBeGreaterThan(lum(t.colorNeutralBackground3))
      } else {
        // claro: superfície mais clara que a página; realces abaixo dela
        expect(lum(t.colorNeutralBackground1)).toBeGreaterThan(lum(p))
        expect(lum(t.colorNeutralBackground3)).toBeLessThan(lum(t.colorNeutralBackground1))
      }
      // o realce maior aparece sobre a página (hover de aba) e sobre a superfície
      expect(contrast(t.colorNeutralBackground4, p), id).toBeGreaterThanOrEqual(1.08)
      expect(contrast(t.colorNeutralBackground3, t.colorNeutralBackground1), id).toBeGreaterThanOrEqual(1.08)
      // texto legível em todos os degraus
      for (const bg of [t.colorNeutralBackground1, t.colorNeutralBackground3, t.colorNeutralBackground4]) {
        expect(contrast(t.colorNeutralForeground1, bg), id).toBeGreaterThanOrEqual(4.5)
      }
    },
  )
})
