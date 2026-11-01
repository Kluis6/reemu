import type { AlvanistaCard, RomEntry } from '../../lib/tauri'
import { contrastRatio } from '../../styles/themes'

/** Números locais do perfil. `games` conta toda a biblioteca. */
export function profileStats(roms: RomEntry[]): {
  playTimeSecs: number
  games: number
  systems: number
  favorites: number
} {
  return {
    playTimeSecs: roms.reduce((s, r) => s + (r.playTimeSecs ?? 0), 0),
    games: roms.length,
    systems: new Set(roms.map((r) => r.systemId)).size,
    favorites: roms.filter((r) => r.isFavorite).length,
  }
}

/** Mesma regra da Home: só quem tem `lastPlayedAt`, o mais recente primeiro. */
export function recentlyPlayed(roms: RomEntry[], limit: number): RomEntry[] {
  return roms
    .filter((r) => r.lastPlayedAt != null)
    .sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0))
    .slice(0, limit)
}

/** Só quem tem tempo de jogo, o maior primeiro (independe de `lastPlayedAt`). */
export function mostPlayed(roms: RomEntry[], limit: number): RomEntry[] {
  return roms
    .filter((r) => r.playTimeSecs > 0)
    .sort((a, b) => b.playTimeSecs - a.playTimeSecs)
    .slice(0, limit)
}

/** `pt` para `pt-BR`, `en` para o resto, depois qualquer valor presente. */
export function pickText(map: Record<string, string> | undefined, lang: string): string | null {
  if (!map) return null
  const preferred = lang.toLowerCase().startsWith('pt') ? 'pt' : 'en'
  for (const key of [preferred, 'en']) {
    if (map[key]) return map[key]
  }
  return Object.values(map).find((v) => !!v) ?? null
}

/** Cor `#rrggbb`: a única que o chip do plano aceita para se tingir. */
export function isHexColor(v: string | undefined): v is string {
  return !!v && /^#[0-9a-f]{6}$/i.test(v)
}

/** Tinta (preto ou branco) com o maior contraste sobre `bg` (`#rrggbb`). */
export function readableInk(bg: string): '#000000' | '#ffffff' {
  return contrastRatio(bg, '#000000') >= contrastRatio(bg, '#ffffff') ? '#000000' : '#ffffff'
}

/** O nome de cada rede de troféus que o servidor manda; outra fica como veio. */
const NETWORKS: Record<string, string> = {
  psn: 'PlayStation',
  xbox: 'Xbox',
  steam: 'Steam',
  gog: 'GOG',
  retroachievements: 'RetroAchievements',
  epic: 'Epic',
}

export function networkLabel(slug: string): string {
  return NETWORKS[slug.toLowerCase()] ?? slug
}

/** A partir de quanto o cartão mais velho que a leitura mostra a própria idade (s). */
export const CARD_AGE_SHOWN_AFTER = 5 * 60
/** A partir de quanto "jogando agora" não é mais agora (s). */
export const PLAYING_NOW_STALE_AFTER = 30 * 60

/** O `computed_at` do cartão (RFC 3339) em segundos Unix; `null` sem uma data
 *  válida (o servidor manda o instante zero quando não calculou). */
export function cardComputedAt(card: AlvanistaCard): number | null {
  if (!card.computed_at) return null
  const ms = Date.parse(card.computed_at)
  return Number.isFinite(ms) && ms > 0 ? Math.floor(ms / 1000) : null
}

/** Quanto o cartão é mais velho que a leitura do status (s); `null` sem as
 *  duas datas. Um `/me` sem cartão mantém o guardado com a leitura nova. */
export function cardLag(card: AlvanistaCard, fetchedAt: number | null | undefined): number | null {
  const at = cardComputedAt(card)
  return at == null || fetchedAt == null ? null : Math.max(0, fetchedAt - at)
}

/** "Jogando agora", só enquanto o cartão é recente. */
export function playingNow(card: AlvanistaCard, fetchedAt: number | null | undefined): AlvanistaCard['playing_now'] {
  const lag = cardLag(card, fetchedAt)
  return lag != null && lag > PLAYING_NOW_STALE_AFTER ? undefined : card.playing_now
}

/** Um número de "No Alvanista". O servidor omite os zeros: o que falta não entra. */
export interface CardTile {
  key: 'collection' | 'trophies' | 'hours' | 'finished' | 'checkins' | 'gamers'
  value: number
  platforms?: number
  networks?: string[]
}

export function cardTiles(card: AlvanistaCard): CardTile[] {
  const out: CardTile[] = []
  const collection = card.proofs?.collection
  if (collection) out.push({ key: 'collection', value: collection.games, platforms: collection.platforms })
  const trophies = card.proofs?.trophies
  if (trophies) out.push({ key: 'trophies', value: trophies.count, networks: trophies.networks ?? [] })
  for (const key of ['hours', 'finished', 'checkins', 'gamers'] as const) {
    const value = card.ledger?.[key]
    if (value != null) out.push({ key, value })
  }
  return out
}

/** `true` se "No Alvanista" tem o que mostrar: um número, um selo ou o jogo
 *  de agora. Sem nada, a seção some inteira. */
export function hasCardContent(card: AlvanistaCard, fetchedAt: number | null | undefined): boolean {
  return (
    cardTiles(card).length > 0 ||
    (card.badges?.recent?.length ?? 0) > 0 ||
    (card.badges?.count ?? 0) > 0 ||
    !!playingNow(card, fetchedAt)
  )
}

/** O véu preto sob a legenda branca de "Seus momentos" (opacidade no topo e
 *  embaixo). O topo sozinho, sobre uma captura toda branca, já dá 4,5:1. */
export const CAPTION_SCRIM = { top: 0.55, bottom: 0.85 } as const
