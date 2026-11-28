// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import type { AlvanistaHomeItem, AlvanistaStatus, RomEntry } from '../lib/tauri'
import { HeroCarousel, isHeroPlayingNowFresh, type HeroAlvanistaProps } from './HeroCarousel'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const NOW = 1_760_000_000

const GAME_1: RomEntry = {
  id: 'rom-1',
  title: 'Chrono Trigger',
  systemId: 'snes',
  filePath: '/roms/ct.sfc',
  addedAt: NOW - 100,
  lastPlayedAt: NOW - 50,
  playTimeSecs: 3600,
  isFavorite: true,
  boxart: 'https://cdn.example/ct.jpg',
}

const DISCONNECTED_STATUS: AlvanistaStatus = {
  connected: false,
  username: null,
  avatar_url: null,
  hero_url: null,
  bio: null,
  profile_url: null,
  offline: false,
  ended: false,
  card: null,
  fetched_at: null,
  connected_at: null,
  device_label: null,
  save_sync: null,
  scopes: [],
}

const NEWS: AlvanistaHomeItem[] = [
  {
    id: 'art-1',
    kind: 'article',
    title: 'Os classicos de 16 bits',
    summary: 'Um passeio pela geracao dourada dos RPGs.',
    imageUrl: 'https://cdn.alvanista.com/art-1.jpg',
  },
  {
    id: 'rev-1',
    kind: 'review',
    title: 'Analise: Super Metroid',
    summary: 'Design de mapa e atmosfera inigualaveis.',
    imageUrl: 'https://cdn.alvanista.com/rev-1.jpg',
  },
  {
    id: 'art-extra',
    kind: 'article',
    title: 'Terceiro artigo excedente',
    summary: 'Nao deve aparecer por causa do limite de 2.',
    imageUrl: 'https://cdn.alvanista.com/art-3.jpg',
  },
]

let host: HTMLDivElement | null = null
let cleanupRoot: (() => Promise<void>) | null = null

async function renderHero({
  items = [GAME_1],
  onOpen = vi.fn(),
  alvanista,
}: {
  items?: readonly RomEntry[]
  onOpen?: (id: string) => void
  alvanista?: HeroAlvanistaProps
}) {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () => {
    root.render(
      <FluentProvider theme={webDarkTheme}>
        <HeroCarousel items={items} onOpen={onOpen} alvanista={alvanista} />
      </FluentProvider>,
    )
  })
  cleanupRoot = () => act(async () => root.unmount())
  return { onOpen }
}

afterEach(async () => {
  if (cleanupRoot) await cleanupRoot()
  cleanupRoot = null
  host?.remove()
  host = null
})

describe('HeroCarousel with Alvanista highlights', () => {
  it('shows only library game slides when highlights are disabled', async () => {
    await renderHero({
      alvanista: {
        enabled: false,
        status: DISCONNECTED_STATUS,
        news: NEWS,
        nowSecs: NOW,
      },
    })
    const slideButtons = host!.querySelectorAll('button[data-slide-kind]')
    expect(slideButtons).toHaveLength(1)
    expect(host!.textContent).toContain('Chrono Trigger')
    expect(host!.textContent).not.toContain('Entrar com Alvanista')
    expect(host!.textContent).not.toContain('Os classicos de 16 bits')
  })

  it('disconnected: renders invite slide plus at most 2 curated news slides and triggers callbacks', async () => {
    const onConnectClick = vi.fn()
    const onOpenNews = vi.fn()
    await renderHero({
      alvanista: {
        enabled: true,
        status: DISCONNECTED_STATUS,
        news: NEWS,
        nowSecs: NOW,
        onConnectClick,
        onOpenNews,
      },
    })

    const slideButtons = [...host!.querySelectorAll<HTMLButtonElement>('button[data-slide-kind]')]
    // 1 jogo + 1 convite + 2 novidades (o 3º é descartado) = 4 slides
    expect(slideButtons).toHaveLength(4)
    expect(host!.textContent).toContain('Entrar com Alvanista')
    expect(host!.textContent).toContain('Artigo no Alvanista')
    expect(host!.textContent).toContain('Review no Alvanista')
    expect(host!.textContent).not.toContain('Terceiro artigo excedente')

    await act(async () => slideButtons[1].click())
    expect(onConnectClick).toHaveBeenCalledTimes(1)

    await act(async () => slideButtons[2].click())
    expect(onOpenNews).toHaveBeenCalledWith('art-1')

    await act(async () => slideButtons[3].click())
    expect(onOpenNews).toHaveBeenCalledWith('rev-1')
  })

  it('connected with fresh playing_now (<= 30 min): shows Jogando agora slide and navigates to profile', async () => {
    const onProfileClick = vi.fn()
    await renderHero({
      alvanista: {
        enabled: true,
        nowSecs: NOW,
        status: {
          ...DISCONNECTED_STATUS,
          connected: true,
          username: 'kluis',
          fetched_at: NOW - 600,
          card: {
            playing_now: {
              title: 'Castlevania: Symphony of the Night',
              cover_url: 'https://cdn.alvanista.com/sotn.jpg',
            },
            badges: {
              count: 3,
              recent: [{ name: 'Pioneiro', icon_url: 'https://cdn.alvanista.com/b1.png' }],
            },
          },
        },
        onProfileClick,
      },
    })

    const slideButtons = [...host!.querySelectorAll<HTMLButtonElement>('button[data-slide-kind]')]
    expect(slideButtons).toHaveLength(2)
    expect(host!.textContent).toContain('Jogando agora')
    expect(host!.textContent).toContain('Castlevania: Symphony of the Night')
    expect(host!.querySelector('img[src="https://cdn.alvanista.com/sotn.jpg"]')).toBeTruthy()

    await act(async () => slideButtons[1].click())
    expect(onProfileClick).toHaveBeenCalledTimes(1)
  })

  it('connected with stale playing_now (> 30 min): falls back to recent badge slide', async () => {
    const onProfileClick = vi.fn()
    await renderHero({
      alvanista: {
        enabled: true,
        nowSecs: NOW,
        status: {
          ...DISCONNECTED_STATUS,
          connected: true,
          username: 'kluis',
          fetched_at: NOW - 2400, // 40 min atrás (> 1800s)
          card: {
            playing_now: {
              title: 'Jogo antigo da sessao passada',
              cover_url: 'https://cdn.alvanista.com/old.jpg',
            },
            badges: {
              count: 4,
              recent: [
                {
                  name: 'Collector',
                  icon_url: 'https://cdn.alvanista.com/badge.png',
                  names: { pt: 'Colecionador' },
                },
              ],
            },
          },
        },
        onProfileClick,
      },
    })

    expect(host!.textContent).not.toContain('Jogo antigo da sessao passada')
    expect(host!.textContent).toContain('Nova badge no Alvanista')
    expect(host!.textContent).toContain('Colecionador')
    expect(host!.querySelector('img[src="https://cdn.alvanista.com/badge.png"]')).toBeTruthy()
  })

  it('validates the 30-minute playing_now freshness helper', () => {
    expect(
      isHeroPlayingNowFresh(
        { playing_now: { title: 'Zelda' } },
        NOW - 1800,
        NOW,
      ),
    ).toBe(true)
    expect(
      isHeroPlayingNowFresh(
        { playing_now: { title: 'Zelda' } },
        NOW - 1801,
        NOW,
      ),
    ).toBe(false)
    expect(
      isHeroPlayingNowFresh(
        {
          playing_now: { title: 'Zelda' },
          computed_at: new Date((NOW - 3600) * 1000).toISOString(),
        },
        NOW - 10,
        NOW,
      ),
    ).toBe(false)
  })
})
