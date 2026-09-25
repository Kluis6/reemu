// Formata os campos de metadados pra gaveta "Informações do jogo", seguindo o
// que cada provedor documenta:
//  - ScreenScraper (webapi2.php): datas `aaaa-mm-jj`, e "xxxx-01-01" quando
//    só o ano é conhecido; gêneros com o principal primeiro (o scraper grava
//    "Plataforma, Ação"); textos por idioma.
//  - TheGamesDB (API v1): `release_date` `AAAA-MM-DD`, `overview` em texto.

import i18n from '../i18n'

// Datas no idioma da interface via Intl (UTC: a data do provedor não tem
// fuso, e o fuso local podia jogar o dia pra trás).
const dateText = (y: number, month: number, day: number) =>
  new Date(Date.UTC(y, month - 1, day || 1)).toLocaleDateString(i18n.language, {
    timeZone: 'UTC',
    year: 'numeric',
    month: 'long',
    ...(day ? { day: 'numeric' } : {}),
  })

/** "1995-08-11" → "11 de agosto de 1995"; "1991-01-01" (só o ano, pela doc
 *  do ScreenScraper) → "1991"; "1994-11" → "novembro de 1994"; "1993" →
 *  "1993". Formato desconhecido volta como veio. */
export function formatReleaseDate(raw: string | null | undefined): string | null {
  const t = raw?.trim()
  if (!t) return null
  const m = t.match(/^(\d{4})(?:-(\d{1,2})(?:-(\d{1,2}))?)?$/)
  if (!m) return t
  const [, y, mo, d] = m
  const month = mo ? Number(mo) : 0
  const day = d ? Number(d) : 0
  if (!month || month > 12) return y
  if (month === 1 && day === 1) return y // convenção "só o ano"
  return dateText(Number(y), month, day)
}

/** "Plataforma, Ação" / "Action / Platform" → ["Plataforma", "Ação"]. */
export function splitGenres(raw: string | null | undefined): string[] {
  if (!raw) return []
  const out: string[] = []
  for (const g of raw.split(/\s*[,/;|]\s*/)) {
    const v = g.trim()
    if (v && !out.includes(v)) out.push(v)
  }
  return out
}

/** Decodifica entidades HTML (os textos vêm de sites) sem interpretar tags. */
function decodeEntities(s: string): string {
  return s
    .replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(Number(n)))
    .replace(/&#x([0-9a-f]+);/gi, (_, n) => String.fromCodePoint(parseInt(n, 16)))
    .replace(/&quot;/g, '"')
    .replace(/&apos;|&#39;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
}

/** Descrição em parágrafos: entidades decodificadas, `<br>` e quebras de
 *  linha viram divisões, espaços repetidos somem. */
export function descriptionParagraphs(raw: string | null | undefined): string[] {
  if (!raw) return []
  const text = decodeEntities(raw)
    .replace(/\r/g, '')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]+>/g, '')
  return text
    .split(/\n+/)
    .map((p) => p.replace(/[ \t]+/g, ' ').trim())
    .filter(Boolean)
}

const PROVIDERS: Record<string, string> = {
  screenscraper: 'ScreenScraper',
  thegamesdb: 'TheGamesDB',
}

export function providerLabel(raw: string | null | undefined): string | null {
  if (!raw) return null
  const k = raw.toLowerCase()
  return k === 'manual' ? i18n.t('format.manual') : (PROVIDERS[k] ?? raw)
}

/** Epoch em segundos → "25 de set. de 2026, 06:30". */
export function formatDateTime(epochSec: number): string {
  return new Date(epochSec * 1000).toLocaleString(i18n.language, {
    dateStyle: 'medium',
    timeStyle: 'short',
  })
}

/** Caminho → nome do arquivo e pasta (Windows ou Unix). */
export function splitPath(path: string): { name: string; dir: string } {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return i < 0 ? { name: path, dir: '' } : { name: path.slice(i + 1), dir: path.slice(0, i) }
}
