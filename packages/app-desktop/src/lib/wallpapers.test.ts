import { describe, expect, it } from 'vitest'
import { WALLPAPER_IDS, wallpaperIdOf, wallpaperSvg } from './wallpapers'

// Mesmas regras de `preset_svg_ok` (apps/desktop/src-tauri/src/appearance.rs):
// o backend recusa o que não passar.
describe('papéis de parede embutidos', () => {
  it.each(WALLPAPER_IDS)('o %s passa na validação do backend e se reconhece', (id) => {
    const svg = wallpaperSvg(id)
    const lower = svg.toLowerCase()
    expect(lower.startsWith('<svg')).toBe(true)
    expect(svg.length).toBeLessThan(512 * 1024)
    expect(lower).not.toContain('<script')
    expect(lower).not.toContain(' on')
    expect(lower).not.toContain('javascript:')
    expect(wallpaperIdOf(svg)).toBe(id)
  })

  it('o mesmo desenho a cada chamada', () => {
    expect(wallpaperSvg('3')).toBe(wallpaperSvg('3'))
  })

  it('imagem da pessoa não é embutida', () => {
    expect(wallpaperIdOf('<svg xmlns="http://www.w3.org/2000/svg"></svg>')).toBeNull()
  })
})
