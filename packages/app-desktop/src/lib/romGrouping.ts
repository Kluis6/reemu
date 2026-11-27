import type { TFunction } from 'i18next'
import { cleanSeedQuery } from '../components/catalog/cleanSeedQuery'
import type { RomEntry } from './tauri'

export type CanonicalRegion = 'NA' | 'EU' | 'JP' | 'BR' | 'KR' | 'AU' | 'CN' | 'WORLD'

export interface GroupedRomEntry extends RomEntry {
  variants: RomEntry[]
  regionBadges: string[]
}

const PAREN_OR_BRACKET_RE = /\(([^()]+)\)|\[([^[\]]+)\]/g
const DISC_CHIP_RE = /^(?:(?:disc|disk|cd|dvd|gd|umd|side|chip)\s*(?:\d+|[a-d])(?:\s*of\s*\d+)?)$/i
const REV_RE = /^(?:rev\.?\s*(?:\d+|[a-c])|v\d+(?:\.\d+)+[a-z]?|ver\.?\s*\d+(?:\.\d+)*)$/i
const EDITION_RE =
  /^(?:beta(?:\s*\d+)?|proto(?:\s*\d+)?|prototype|demo|sample|promo|kiosk|arcade|virtual console|unl|pirate|t[+-][a-z]{2,5}.*)$/i
const NON_FINAL_RE = /\b(?:beta|proto|prototype|demo|sample|promo|kiosk)\b/i

function normalizeSingleRegion(token: string, allowSingleLetter: boolean): CanonicalRegion | null {
  const lower = token.trim().toLowerCase()
  switch (lower) {
    case 'na':
    case 'usa':
    case 'us':
    case 'u.s.a.':
    case 'north america':
    case 'canada':
    case 'ca':
    case 'cus':
    case 'ntsc-u':
    case 'ntsc':
    case 'americas':
      return 'NA'
    case 'eu':
    case 'eur':
    case 'europe':
    case 'pal':
    case 'pal-e':
    case 'pal-m':
    case 'uk':
    case 'united kingdom':
    case 'england':
    case 'great britain':
    case 'germany':
    case 'ger':
    case 'de':
    case 'deu':
    case 'france':
    case 'fra':
    case 'fr':
    case 'spain':
    case 'esp':
    case 'es':
    case 'italy':
    case 'ita':
    case 'it':
    case 'netherlands':
    case 'nl':
    case 'holland':
    case 'sweden':
    case 'swe':
    case 'se':
    case 'scandinavia':
    case 'portugal':
    case 'pt':
      return 'EU'
    case 'jp':
    case 'jpn':
    case 'jap':
    case 'japan':
    case 'ja':
    case 'ntsc-j':
      return 'JP'
    case 'br':
    case 'bra':
    case 'brazil':
    case 'brasil':
      return 'BR'
    case 'kr':
    case 'kor':
    case 'korea':
    case 'south korea':
    case 'ko':
      return 'KR'
    case 'au':
    case 'aus':
    case 'australia':
      return 'AU'
    case 'cn':
    case 'chn':
    case 'china':
    case 'asia':
    case 'taiwan':
    case 'tw':
    case 'hong kong':
    case 'hk':
      return 'CN'
    case 'world':
    case 'wor':
    case 'jue':
    case 'ue':
    case 'ju':
    case 'uj':
      return 'WORLD'
    default:
      break
  }
  if (allowSingleLetter) {
    switch (lower) {
      case 'u':
        return 'NA'
      case 'e':
      case 'f':
      case 'g':
      case 's':
      case 'i':
        return 'EU'
      case 'j':
        return 'JP'
      case 'b':
        return 'BR'
      case 'k':
        return 'KR'
      case 'a':
        return 'AU'
      case 'w':
        return 'WORLD'
      default:
        break
    }
  }
  return null
}

export function normalizeRegionCode(raw?: string | null): CanonicalRegion | null {
  const trimmed = raw?.trim()
  if (!trimmed) return null
  if (trimmed.includes(',')) {
    const codes: CanonicalRegion[] = []
    for (const part of trimmed.split(',')) {
      const code = normalizeSingleRegion(part, false)
      if (code && !codes.includes(code)) codes.push(code)
    }
    if (codes.length === 0) return null
    if (codes.length === 1) return codes[0]
    if (codes.every((c) => c === 'EU' || c === 'AU')) return 'EU'
    if (
      (codes.includes('NA') && codes.includes('EU')) ||
      (codes.includes('NA') && codes.includes('JP')) ||
      (codes.includes('EU') && codes.includes('JP'))
    ) {
      return 'WORLD'
    }
    return codes[0]
  }
  return normalizeSingleRegion(trimmed, true)
}

function fileStemFromPath(filePath: string): string {
  const base = filePath.split(/[/\\]/).pop() ?? filePath
  const dot = base.lastIndexOf('.')
  return dot > 0 ? base.slice(0, dot) : base
}

export function detectRomRegionFallback(rom: RomEntry): CanonicalRegion | null {
  const direct = normalizeRegionCode(rom.region)
  if (direct) return direct
  const base = fileStemFromPath(rom.filePath)
  PAREN_OR_BRACKET_RE.lastIndex = 0
  let m: RegExpExecArray | null
  while ((m = PAREN_OR_BRACKET_RE.exec(base)) !== null) {
    const inner = (m[1] ?? m[2] ?? '').trim()
    if (!inner) continue
    if (m[2] !== undefined && inner.length === 1) {
      if (inner === 'U') return 'NA'
      if (inner === 'E') return 'EU'
      if (inner === 'J') return 'JP'
      continue
    }
    const code = normalizeRegionCode(inner)
    if (code) return code
  }
  return null
}

function extractQualifierFallback(rom: RomEntry): string | null {
  if (rom.versionQualifier?.trim()) return rom.versionQualifier.trim()
  const base = fileStemFromPath(rom.filePath)
  const parts: string[] = []
  PAREN_OR_BRACKET_RE.lastIndex = 0
  let m: RegExpExecArray | null
  while ((m = PAREN_OR_BRACKET_RE.exec(base)) !== null) {
    const inner = (m[1] ?? m[2] ?? '').trim()
    if (!inner || inner === '!') continue
    if (DISC_CHIP_RE.test(inner) || REV_RE.test(inner) || EDITION_RE.test(inner)) {
      if (!parts.some((p) => p.toLowerCase() === inner.toLowerCase())) {
        parts.push(inner)
      }
    }
  }
  return parts.length > 0 ? parts.join(' · ') : null
}

function canonicalTitleKey(raw: string): string {
  const cleaned = cleanSeedQuery(raw) || raw
  return cleaned
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/['\u2018\u2019`]/g, '')
    .replace(/&/g, ' and ')
    .replace(/[^a-z0-9]+/g, ' ')
    .trim()
}

export function effectiveGroupKey(rom: RomEntry): string {
  if (rom.groupKey?.trim()) return `${rom.systemId}::${rom.groupKey.trim()}`
  if (rom.userTitle?.trim()) {
    const k = canonicalTitleKey(rom.userTitle)
    return `${rom.systemId}::user:${k || rom.id}`
  }
  const k = canonicalTitleKey(rom.cleanTitle ?? rom.title)
  return `${rom.systemId}::${k ? `title:${k}` : `rom:${rom.id}`}`
}

export function regionShortBadge(region?: string | null): string | null {
  switch (normalizeRegionCode(region)) {
    case 'NA':
      return 'USA'
    case 'EU':
      return 'PAL'
    case 'JP':
      return 'JP'
    case 'BR':
      return 'BR'
    case 'KR':
      return 'KR'
    case 'AU':
      return 'AU'
    case 'CN':
      return 'ASIA'
    case 'WORLD':
      return 'WORLD'
    default:
      return null
  }
}

export function regionDisplayLabel(region: string | null | undefined, t: TFunction): string {
  const norm = normalizeRegionCode(region)
  return norm ? t(`game.regions.${norm}`) : t('game.regions.unknown')
}

function preferredRegionOrder(uiLanguage?: string): readonly CanonicalRegion[] {
  const lang = (uiLanguage ?? 'pt').toLowerCase()
  if (lang.startsWith('pt')) {
    return ['BR', 'NA', 'WORLD', 'EU', 'AU', 'JP', 'KR', 'CN']
  }
  if (lang.startsWith('es')) {
    return ['EU', 'NA', 'WORLD', 'BR', 'AU', 'JP', 'KR', 'CN']
  }
  return ['NA', 'WORLD', 'EU', 'AU', 'BR', 'JP', 'KR', 'CN']
}

function regionRank(rom: RomEntry, order: readonly CanonicalRegion[]): number {
  const code = detectRomRegionFallback(rom)
  if (!code) return order.length + 1
  const idx = order.indexOf(code)
  return idx >= 0 ? idx : order.length
}

function isNonFinalBuild(rom: RomEntry): boolean {
  const q = extractQualifierFallback(rom)
  return q ? NON_FINAL_RE.test(q) : NON_FINAL_RE.test(rom.filePath)
}

function sortVariants(variants: readonly RomEntry[], uiLanguage?: string): RomEntry[] {
  const order = preferredRegionOrder(uiLanguage)
  return [...variants].sort((a, b) => {
    const playedDiff = (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0)
    if (playedDiff !== 0) return playedDiff
    if (a.isFavorite !== b.isFavorite) return a.isFavorite ? -1 : 1
    const nonFinalA = isNonFinalBuild(a)
    const nonFinalB = isNonFinalBuild(b)
    if (nonFinalA !== nonFinalB) return nonFinalA ? 1 : -1
    const regDiff = regionRank(a, order) - regionRank(b, order)
    if (regDiff !== 0) return regDiff
    const hasArtA = a.boxart ? 0 : 1
    const hasArtB = b.boxart ? 0 : 1
    if (hasArtA !== hasArtB) return hasArtA - hasArtB
    return a.filePath.localeCompare(b.filePath)
  })
}

export function groupRoms(roms: readonly RomEntry[], uiLanguage?: string): GroupedRomEntry[] {
  const buckets = new Map<string, RomEntry[]>()
  for (const rom of roms) {
    const key = effectiveGroupKey(rom)
    const list = buckets.get(key)
    if (list) list.push(rom)
    else buckets.set(key, [rom])
  }

  const out: GroupedRomEntry[] = []
  for (const rawVariants of buckets.values()) {
    const variants = sortVariants(rawVariants, uiLanguage)
    const primary = variants[0]
    const regionBadges: string[] = []
    for (const v of variants) {
      const badge = regionShortBadge(detectRomRegionFallback(v))
      if (badge && !regionBadges.includes(badge)) {
        regionBadges.push(badge)
      }
    }

    let displayTitle = primary.title
    if (variants.length > 1 && !primary.userTitle) {
      const candidateClean = primary.cleanTitle ?? cleanSeedQuery(primary.title)
      if (candidateClean && /[([]/.test(primary.title)) {
        displayTitle = candidateClean
      }
    }

    let maxPlayed: number | null = null
    let maxAdded = primary.addedAt
    let totalPlayTime = 0
    let anyFav = false
    let fallbackBoxart = primary.boxart
    for (const v of variants) {
      if (v.lastPlayedAt != null && (maxPlayed == null || v.lastPlayedAt > maxPlayed)) {
        maxPlayed = v.lastPlayedAt
      }
      if (v.addedAt > maxAdded) maxAdded = v.addedAt
      totalPlayTime += v.playTimeSecs
      if (v.isFavorite) anyFav = true
      if (!fallbackBoxart && v.boxart) fallbackBoxart = v.boxart
    }

    out.push({
      ...primary,
      title: displayTitle,
      boxart: primary.boxart ?? fallbackBoxart,
      isFavorite: anyFav,
      playTimeSecs: totalPlayTime,
      lastPlayedAt: maxPlayed,
      addedAt: maxAdded,
      variants,
      regionBadges,
    })
  }
  return out
}

export function siblingsForRom(
  roms: readonly RomEntry[],
  current: RomEntry | undefined,
  uiLanguage?: string,
): RomEntry[] {
  if (!current) return []
  const targetKey = effectiveGroupKey(current)
  const matches = roms.filter((r) => effectiveGroupKey(r) === targetKey)
  return sortVariants(matches, uiLanguage)
}

export function romVariantLabel(
  rom: RomEntry,
  siblings: readonly RomEntry[],
  t: TFunction,
): string {
  const buildBase = (item: RomEntry) => {
    const reg = detectRomRegionFallback(item)
    const qualifier = extractQualifierFallback(item)
    const regText = regionDisplayLabel(reg, t)
    return qualifier ? `${regText} · ${qualifier}` : regText
  }
  const base = buildBase(rom)
  const collides = siblings.some((other) => other.id !== rom.id && buildBase(other) === base)
  if (!collides) return base
  return `${base} (${fileStemFromPath(rom.filePath)})`
}

export function formatCardGroupBadge(group: GroupedRomEntry, t: TFunction): string | undefined {
  if (group.variants.length <= 1) return undefined
  if (group.regionBadges.length >= 2) {
    return group.regionBadges.slice(0, 3).join(' · ')
  }
  return t('library.versionsBadge', { count: group.variants.length })
}
