import { describe, expect, it } from 'vitest'
import {
  descriptionParagraphs,
  formatReleaseDate,
  providerLabel,
  splitGenres,
  splitPath,
} from './metadataFormat'

describe('formatReleaseDate', () => {
  it.each([
    ['1995-08-11', '11 de agosto de 1995'],
    ['1991-01-01', '1991'], // ScreenScraper: "xxxx-01-01" = só o ano
    ['1994-11', 'novembro de 1994'],
    ['1993', '1993'],
    ['  2001-12-3 ', '3 de dezembro de 2001'],
    ['Q3 1996', 'Q3 1996'],
  ])('%s → %s', (raw, out) => {
    expect(formatReleaseDate(raw)).toBe(out)
  })

  it('vazio vira null', () => {
    expect(formatReleaseDate('')).toBeNull()
    expect(formatReleaseDate(null)).toBeNull()
  })
})

describe('splitGenres', () => {
  it('separa por vírgula, barra e ponto e vírgula, sem repetir', () => {
    expect(splitGenres('Plataforma, Ação')).toEqual(['Plataforma', 'Ação'])
    expect(splitGenres('Action / Platform;Action')).toEqual(['Action', 'Platform'])
    expect(splitGenres(null)).toEqual([])
  })
})

describe('descriptionParagraphs', () => {
  it('decodifica entidades, quebra em parágrafos e tira tags', () => {
    expect(
      descriptionParagraphs('Sonic &amp; Tails&#39; aventura.\r\n\r\nSegundo <b>parágrafo</b>.<br/>Fim'),
    ).toEqual(["Sonic & Tails' aventura.", 'Segundo parágrafo.', 'Fim'])
  })
})

describe('providerLabel e splitPath', () => {
  it('nome do provedor como a marca escreve', () => {
    expect(providerLabel('screenscraper')).toBe('ScreenScraper')
    expect(providerLabel('thegamesdb')).toBe('TheGamesDB')
  })

  it('separa nome e pasta nos dois sistemas', () => {
    expect(splitPath('E:\\roms\\md\\Sonic.md')).toEqual({ name: 'Sonic.md', dir: 'E:\\roms\\md' })
    expect(splitPath('/home/l/roms/Sonic.md')).toEqual({ name: 'Sonic.md', dir: '/home/l/roms' })
  })
})
