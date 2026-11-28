// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useToastStore } from '../stores/useToastStore'
import type { AlvanistaCard, AlvanistaStatus, RecentSaveState, RomEntry } from '../lib/tauri'
import { readableInk } from './profile/profileData'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true
// O happy-dom rejeita a animação do Fluent quando o menu fecha durante o teste.
;(Element.prototype as { animate?: unknown }).animate = undefined

const api = vi.hoisted(() => ({
  listRoms: vi.fn(),
  listRecentSaveStates: vi.fn(),
  saveThumbnailUrl: vi.fn(),
  getProfile: vi.fn(),
  alvanistaStatus: vi.fn(),
  alvanistaCachedStatus: vi.fn(),
  alvanistaOpenProfile: vi.fn(),
  alvanistaOpenSite: vi.fn(),
  alvanistaOpenUser: vi.fn(),
  alvanistaOpenGame: vi.fn(),
  alvanistaProfileCheckins: vi.fn(),
  alvanistaFriends: vi.fn(),
  alvanistaCollectionStatus: vi.fn(),
  alvanistaDisconnect: vi.fn(),
  alvanistaConnect: vi.fn(),
  alvanistaCancel: vi.fn(),
  raStatus: vi.fn(),
  raGameAchievements: vi.fn(),
}))

vi.mock('../lib/tauri', async (orig) => ({
  ...(await orig<typeof import('../lib/tauri')>()),
  ...api,
}))

import { ProfilePage } from './ProfilePage'

const OFF: AlvanistaStatus = {
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
const ON: AlvanistaStatus = {
  ...OFF,
  connected: true,
  username: 'toy',
  avatar_url: 'https://cdn.alvanista.com/a/toy.png',
  hero_url: 'https://cdn.alvanista.com/h/toy.png',
  bio: 'bio do Alvanista',
  profile_url: 'https://www.alvanista.com/users/toy',
}

const rom = (id: string, over: Partial<RomEntry> = {}): RomEntry => ({
  id,
  title: `Jogo ${id}`,
  systemId: 'snes',
  filePath: `/r/${id}`,
  boxart: null,
  lastPlayedAt: null,
  addedAt: 1,
  isFavorite: false,
  playTimeSecs: 0,
  ...over,
})

let host: HTMLDivElement
let unmount: () => Promise<void>

async function settle() {
  for (let i = 0; i < 5; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0))
    })
  }
}

async function mount() {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  await act(async () =>
    root.render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/perfil']}>
          <Routes>
            <Route path="/perfil" element={<ProfilePage />} />
            <Route path="/settings/perfil" element={<div>tela de ajustes</div>} />
            <Route path="/rom/:romId" element={<div>tela do jogo</div>} />
            <Route path="/conquistas" element={<div>tela de conquistas</div>} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

const byText = (text: string) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent?.includes(text))
const section = (id: string) => host.querySelector(`[data-testid="${id}"]`)

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.listRoms.mockResolvedValue([])
  api.listRecentSaveStates.mockResolvedValue([])
  api.saveThumbnailUrl.mockResolvedValue(null)
  api.getProfile.mockResolvedValue({ name: 'Local', bio: 'bio local', avatar: 'preset:2', onboarded: true })
  api.alvanistaStatus.mockResolvedValue(OFF)
  api.alvanistaCachedStatus.mockResolvedValue(OFF)
  api.alvanistaOpenProfile.mockResolvedValue(undefined)
  api.alvanistaOpenSite.mockResolvedValue(undefined)
  api.alvanistaOpenUser.mockResolvedValue(undefined)
  api.alvanistaOpenGame.mockResolvedValue(undefined)
  api.alvanistaProfileCheckins.mockResolvedValue({ items: [], nextPage: null })
  api.alvanistaFriends.mockResolvedValue([])
  api.alvanistaCollectionStatus.mockResolvedValue([])
  api.alvanistaDisconnect.mockResolvedValue(undefined)
  api.alvanistaCancel.mockResolvedValue(undefined)
  api.raStatus.mockResolvedValue({ connected: false })
  useToastStore.setState({ queue: [] })
})
afterEach(async () => {
  await unmount()
  host.remove()
})

describe('ProfilePage', () => {
  it('local profile: theme banner, local name and bio, Editar perfil goes to /settings/perfil, no chips', async () => {
    await mount()
    const banner = section('profile-banner') as HTMLElement
    expect(banner).toBeTruthy()
    expect(banner.style.backgroundImage).toBe('')
    expect(host.textContent).toContain('Local')
    expect(host.textContent).toContain('bio local')
    expect(section('profile-chips')).toBeNull()
    expect(host.querySelector('[data-testid="tier-ring"]')).toBeNull()
    expect(byText('Ver no Alvanista')).toBeUndefined()
    await act(async () => byText('Editar perfil')!.click())
    await settle()
    expect(document.body.textContent).toContain('tela de ajustes')
  })

  it('connected without card: Alvanista name, avatar and bio, Ver no Alvanista and Editar no Alvanista, no chips', async () => {
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    api.alvanistaStatus.mockResolvedValue(ON)
    await mount()
    expect(host.textContent).toContain('toy')
    expect(host.textContent).toContain('bio do Alvanista')
    expect(host.textContent).not.toContain('bio local')
    expect(host.querySelector('img[src="https://cdn.alvanista.com/a/toy.png"]')).toBeTruthy()
    expect((section('profile-banner') as HTMLElement).style.backgroundImage).toContain('h/toy.png')
    expect(section('profile-chips')).toBeNull()
    await act(async () => byText('Ver no Alvanista')!.click())
    expect(api.alvanistaOpenProfile).toHaveBeenCalledTimes(1)
    await act(async () => byText('Editar no Alvanista')!.click())
    expect(api.alvanistaOpenSite).toHaveBeenCalledWith('edit')
    expect(byText('Editar perfil')).toBeUndefined()
  })

  it('connected without Alvanista bio shows no bio even with a local bio', async () => {
    const st = { ...ON, bio: null }
    api.alvanistaCachedStatus.mockResolvedValue(st)
    api.alvanistaStatus.mockResolvedValue(st)
    await mount()
    expect(host.textContent).toContain('toy')
    expect(host.textContent).not.toContain('bio local')
  })

  it('banner follows the card hero position', async () => {
    const st = { ...ON, card: { identity: { hero_position_x: 20, hero_position_y: 70 } } }
    api.alvanistaCachedStatus.mockResolvedValue(st)
    api.alvanistaStatus.mockResolvedValue(st)
    await mount()
    expect((section('profile-banner') as HTMLElement).style.backgroundPosition).toBe('20% 70%')
  })

  it('No ReEmu shows the four numbers', async () => {
    api.listRoms.mockResolvedValue([
      rom('a', { playTimeSecs: 3600, isFavorite: true }),
      rom('b', { playTimeSecs: 1800, systemId: 'n64' }),
      rom('c', { systemId: 'n64' }),
    ])
    await mount()
    const s = section('profile-numbers')!
    expect(s.textContent).toContain('1 h 30 min')
    expect(s.textContent).toContain('Jogos na biblioteca')
    expect(s.textContent).toContain('Sistemas')
    expect(s.textContent).toContain('Favoritos')
    expect(s.textContent).toContain('Tempo jogado')
    const stat = (k: string) => s.querySelector(`[data-stat="${k}"]`)?.textContent
    expect(stat('games')).toBe('3')
    expect(stat('systems')).toBe('2')
    expect(stat('favorites')).toBe('1')
  })

  it('a library with no play time shows 0 min, not less than a minute', async () => {
    api.listRoms.mockResolvedValue([rom('a'), rom('b')])
    await mount()
    const time = section('profile-numbers')!.querySelector('[data-stat="time"]')!.textContent
    expect(time).toBe('0 min')
  })

  it('an empty library shows the invitation line and hides the shelves', async () => {
    await mount()
    const s = section('profile-numbers')!
    expect(s.textContent).toContain('Jogue algo para ver seus números aqui.')
    expect(s.querySelector('[data-stat]')).toBeNull()
    expect(section('profile-recent')).toBeNull()
    expect(section('profile-most-played')).toBeNull()
    expect(section('profile-moments')).toBeNull()
  })

  it('recent and most played follow profileData', async () => {
    api.listRoms.mockResolvedValue([
      rom('old', { title: 'Antigo', lastPlayedAt: 100, playTimeSecs: 9000 }),
      rom('new', { title: 'Novo', lastPlayedAt: 900, playTimeSecs: 60 }),
    ])
    await mount()
    const first = (id: string) => section(id)!.querySelector('button, [role="button"]')!
    // as capas, da mais recente
    const covers = () => section('profile-recent')!.querySelectorAll<HTMLElement>('.fui-Card')
    expect(covers()[0].getAttribute('aria-label')).toBe('Novo')
    // a linha de "Mais jogados" se nomeia pelo conteúdo: título e tempo, só spans
    const row = first('profile-most-played')
    expect(row.getAttribute('aria-label')).toBeNull()
    expect(row.textContent).toContain('Antigo')
    expect(row.textContent).toContain('2 h 30 min')
    expect(section('profile-most-played')!.querySelector('button div')).toBeNull()
    expect(api.raGameAchievements).not.toHaveBeenCalled()
    await act(async () => covers()[0].click())
    await settle()
    expect(document.body.textContent).toContain('tela do jogo')
  })

  it('moments hide when there are no saves with thumbnails', async () => {
    api.listRoms.mockResolvedValue([rom('a', { playTimeSecs: 60 })])
    await mount()
    expect(section('profile-moments')).toBeNull()
  })

  it('opens all recent covers in a dialog and keeps the nine-cover preview on close', async () => {
    api.listRoms.mockResolvedValue(
      Array.from({ length: 12 }, (_, i) => rom(String(i), { lastPlayedAt: i + 1 })),
    )
    await mount()
    const recent = section('profile-recent')!
    const toggle = byText('Mostrar tudo')!
    expect(recent.querySelectorAll('.fui-Card')).toHaveLength(9)
    expect(toggle.getAttribute('aria-haspopup')).toBe('dialog')
    await act(async () => toggle.click())
    const dialog = document.querySelector('[role="dialog"]')!
    expect(dialog.querySelectorAll('.fui-Card')).toHaveLength(12)
    const close = dialog.querySelector<HTMLButtonElement>('button[aria-label="Fechar"]')!
    await act(async () => close.click())
    await settle()
    expect(document.querySelector('[role="dialog"]')).toBeNull()
    expect(recent.querySelectorAll('.fui-Card')).toHaveLength(9)
  })

  it('shows the connected RetroAchievements scores and opens the achievements page', async () => {
    api.raStatus.mockResolvedValue({ connected: true, score: 1350, scoreSoftcore: 320 })
    await mount()
    expect(host.textContent).toContain('1.350 pts Hardcore')
    expect(host.textContent).toContain('320 pts Softcore')
    await act(async () => byText('Ver conquistas')!.click())
    expect(host.textContent).toContain('tela de conquistas')
  })

  it('shows verified achievement counts by game, including zero unlocked', async () => {
    api.raStatus.mockResolvedValue({ connected: true, score: 1350, scoreSoftcore: 320 })
    api.listRoms.mockResolvedValue([
      rom('complete', { playTimeSecs: 60 }),
      rom('started', { playTimeSecs: 9000 }),
      rom('zero', { playTimeSecs: 100 }),
      rom('unidentified', { playTimeSecs: 200 }),
    ])
    api.raGameAchievements.mockImplementation(async (id: string) => {
      if (id === 'unidentified') return { status: 'unidentified', totalAchievements: 0 }
      const unlockedAchievements = { complete: 10, started: 8, zero: 0 }[id]
      return { status: 'loaded', totalAchievements: 10, unlockedAchievements }
    })
    await mount()
    const rows = [...section('profile-most-played')!.querySelectorAll('button')]
    expect(rows[0].textContent).toContain('8/10')
    expect(rows[0].textContent).toContain('80%')
    expect(rows[0].querySelector('[aria-label="8 de 10 conquistas desbloqueadas"]')).toBeTruthy()
    expect(rows[1].textContent).not.toContain('0/0')
    expect(rows[1].querySelector('[role="img"]')).toBeNull()
    expect(rows[2].textContent).toContain('0/10')
    expect(rows[2].textContent).toContain('0%')
    expect(rows[3].textContent).toContain('100%')
  })

  it('moments show title and slot', async () => {
    const saves: RecentSaveState[] = [
      { id: 's1', romId: 'a', romTitle: 'Kuon', slot: 2, createdAt: 1, playTimeAtSave: 3600 },
      { id: 's2', romId: 'b', romTitle: 'Killzone 2', slot: null, createdAt: 2, playTimeAtSave: null },
    ]
    api.listRoms.mockResolvedValue([rom('a', { playTimeSecs: 60 })])
    api.listRecentSaveStates.mockResolvedValue(saves)
    await mount()
    const s = section('profile-moments')!
    expect(s.textContent).toContain('Kuon')
    expect(s.textContent).toContain('Slot 2')
    expect(s.textContent).toContain('Killzone 2')
    expect(s.textContent).toContain('Auto')
  })

  it('offline shows the badge and the last profile', async () => {
    const st = { ...ON, offline: true, fetched_at: 1 }
    api.alvanistaCachedStatus.mockResolvedValue(st)
    api.alvanistaStatus.mockResolvedValue(st)
    await mount()
    expect(host.textContent).toContain('Sem conexão: mostrando o último perfil salvo')
    expect(host.textContent).toContain('toy')
  })

  it('Atualizado is hidden without fetched_at and the refresh button refetches the status', async () => {
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    api.alvanistaStatus.mockResolvedValue(ON)
    await mount()
    expect(host.textContent).not.toContain('Atualizado')
    await act(async () => byText('Mais opções')!.click())
    const refresh = document.querySelector<HTMLElement>('[role="menuitem"]')!
    expect(refresh.textContent).toContain('Atualizar agora')
    const before = api.alvanistaStatus.mock.calls.length
    await act(async () => refresh.click())
    await settle()
    expect(api.alvanistaStatus.mock.calls.length).toBe(before + 1)
  })

  it('the full status with another identity replaces the cached one in the hero', async () => {
    api.alvanistaCachedStatus.mockResolvedValueOnce(ON)
    const next = { ...ON, username: 'novo', bio: 'nova bio' }
    api.alvanistaCachedStatus.mockResolvedValue(next)
    api.alvanistaStatus.mockResolvedValue(next)
    await mount()
    expect(host.textContent).toContain('novo')
    expect(host.textContent).toContain('nova bio')
    expect(host.textContent).not.toContain('bio do Alvanista')
  })

  it('ended: falls back to the local profile and pushes the ended toast', async () => {
    api.alvanistaCachedStatus.mockResolvedValueOnce(ON)
    api.alvanistaCachedStatus.mockResolvedValue(OFF)
    api.alvanistaStatus.mockResolvedValue({ ...OFF, ended: true })
    await mount()
    expect(host.textContent).toContain('Local')
    expect(byText('Ver no Alvanista')).toBeUndefined()
    const messages = useToastStore.getState().queue.map((x) => x.message)
    expect(messages).toContain('Conexão com o Alvanista encerrada')
  })

  it('ended: the hero drops the Alvanista identity at once, even if the cached re-read never lands', async () => {
    api.alvanistaCachedStatus.mockResolvedValueOnce(ON)
    api.alvanistaCachedStatus.mockReturnValue(new Promise(() => {}))
    api.alvanistaStatus.mockResolvedValue({ ...OFF, ended: true })
    await mount()
    const hero = section('profile-hero')!
    expect(hero.textContent).toContain('Local')
    expect(hero.textContent).not.toContain('toy')
    expect(byText('Ver no Alvanista')).toBeUndefined()
    const ended = useToastStore.getState().queue.filter((x) => x.message === 'Conexão com o Alvanista encerrada')
    expect(ended.length).toBe(1)
  })

  it('a library read error is not an empty library: no invitation, a neutral line instead', async () => {
    api.listRoms.mockRejectedValue('db')
    await mount()
    const s = section('profile-numbers')!
    expect(s.textContent).not.toContain('Jogue algo para ver seus números aqui.')
    expect(s.textContent).toContain('Não foi possível ler a biblioteca.')
    expect(s.querySelector('[data-stat]')).toBeNull()
    expect(section('profile-recent')).toBeNull()
    expect(section('profile-most-played')).toBeNull()
  })

  it('a failed local profile read while disconnected shows a minimal local hero, not a spinner', async () => {
    api.getProfile.mockRejectedValue('db')
    await mount()
    const hero = section('profile-hero')!
    expect(hero).toBeTruthy()
    expect(hero.textContent).toContain('Jogador')
    expect(byText('Editar perfil')).toBeTruthy()
    expect(section('profile-alvanista-invite')).toBeTruthy()
  })

  it('without a database at all the page still renders: local hero, neutral library line, no invitation', async () => {
    api.getProfile.mockRejectedValue('no db')
    api.alvanistaCachedStatus.mockRejectedValue('no db')
    api.alvanistaStatus.mockRejectedValue('no db')
    api.listRoms.mockRejectedValue('no db')
    api.listRecentSaveStates.mockRejectedValue('no db')
    await mount()
    expect(section('profile-hero')!.textContent).toContain('Jogador')
    expect(section('profile-numbers')!.textContent).toContain('Não foi possível ler a biblioteca.')
    expect(section('profile-alvanista-invite')).toBeNull()
    expect(section('profile-sidebar')).toBeNull()
  })

  describe('Alvanista card', () => {
    const FULL: AlvanistaCard = {
      member_since: 2010,
      identity: {
        location: 'Newcastle Upon Tyne',
        level: 19,
        tier: {
          slug: 'collector',
          rank: 3,
          name: { pt: 'Colecionador', en: 'Collector' },
          colors: {
            ring: { kind: 'linear', angle: 140, stops: ['#d1fae5', '#34d399', '#059669'] },
            glow: { color: '#34d399', alpha: 0.55 },
            accent: '#22d3ee',
          },
        },
      },
      reino_title_text: { pt: 'Conde de Lola', en: 'Count of Lola' },
      palette: { base: '#111111', mid: '#e0b84a', accent: '#ffffff', source: 'cover' },
      proofs: { collection: { games: 2047, platforms: 44 }, trophies: { count: 3943, networks: ['psn', 'xbox', 'nova-rede'] } },
      ledger: { hours: 614, finished: 23, checkins: 244, gamers: 173 },
      badges: {
        count: 27,
        recent: [1, 2, 3, 4, 5].map((n) => ({ name: `Badge ${n}`, icon_url: `https://cdn.gamekave.net/b${n}.webp` })),
      },
      playing_now: { title: 'Silver Pines', cover_url: 'https://cdn.gamekave.net/sp.jpg' },
    }
    const connect = (over: Partial<AlvanistaStatus> = {}, card: AlvanistaCard | null = FULL) => {
      const st = {
        ...ON,
        card,
        connected_at: 1793000000,
        device_label: 'ReEmu (Windows)',
        profile_url: 'https://www.alvanista.com/u/toy',
        ...over,
      }
      api.alvanistaCachedStatus.mockResolvedValue(st)
      api.alvanistaStatus.mockResolvedValue(st)
      return st
    }

    it('card shows level, tier, Reino title in the UI language, location and member since', async () => {
      connect()
      await mount()
      const chips = section('profile-chips')!
      expect(chips.textContent).toContain('Nível 19')
      expect(chips.textContent).toContain('Colecionador')
      expect(chips.textContent).not.toContain('Collector')
      expect(chips.textContent).toContain('Conde de Lola')
      expect(chips.textContent).toContain('Alvanista')
      const meta = section('profile-meta')!
      expect(meta.textContent).toContain('Newcastle Upon Tyne')
      expect(meta.textContent).toContain('Membro desde 2010')
    })

    it('replaces the summary with dedicated account galleries', async () => {
      connect()
      await mount()
      expect(section('profile-alvanista')).toBeNull()
      expect(section('profile-alvanista-badges')!.querySelectorAll('img').length).toBe(5)
      expect(section('profile-alvanista-finished')).toBeTruthy()
      expect(section('profile-alvanista-collection')).toBeTruthy()
      expect(section('profile-alvanista-checkins')).toBeTruthy()
    })

    it('loads the remaining linked games in batches when the Finished dialog opens', async () => {
      connect({ scopes: ['collection'] })
      const gameId = (index: number) => `11111111-0000-0000-0000-${String(index).padStart(12, '0')}`
      api.listRoms.mockResolvedValue(
        Array.from({ length: 201 }, (_, index) => rom(String(index), { gameUuid: gameId(index) })),
      )
      api.alvanistaCollectionStatus.mockImplementation(async (ids: string[]) => ids.map((gameId) => ({
        gameId,
        status: gameId.endsWith('000') ? 'finished' : gameId.endsWith('200') ? 'completed' : 'played',
      })))
      await mount()
      const section = host.querySelector('[data-testid="profile-alvanista-finished"]')!
      const toggle = section.querySelector('button')!
      expect(api.alvanistaCollectionStatus.mock.calls.map(([ids]) => ids.length)).toEqual([200])
      expect(section.querySelectorAll('.fui-Card')).toHaveLength(1)
      await act(async () => toggle.click())
      await settle()
      expect(api.alvanistaCollectionStatus.mock.calls.map(([ids]) => ids.length)).toEqual([200, 1])
      const dialog = document.querySelector('[role="dialog"]')!
      expect([...dialog.querySelectorAll('.fui-Card')].map((card) => card.getAttribute('aria-label')))
        .toEqual(['Jogo 0', 'Jogo 200'])
    })

    it('shows public check-ins and finished games without any matching ROM or collection entry', async () => {
      connect()
      const id = '11111111-0000-0000-0000-000000000001'
      const entry = {
        id: 'checkin-1', gameId: id, title: 'Remote game', coverUrl: null,
        status: 'finished', createdAt: '2026-11-03T12:00:00Z',
      }
      api.alvanistaProfileCheckins.mockImplementation(async (page: number) => page === 1
        ? { items: [entry], nextPage: 2 }
        : { items: [{ ...entry, id: 'checkin-2', gameId: null, title: 'Another game', status: 'playing' }], nextPage: null })
      await mount()
      const finished = section('profile-alvanista-finished')!
      expect(finished.querySelector('[aria-label="Remote game"]')).toBeTruthy()
      await act(async () => finished.querySelector<HTMLElement>('[aria-label="Remote game"]')!.click())
      expect(api.alvanistaOpenGame).toHaveBeenCalledWith(id)
      expect(api.alvanistaCollectionStatus).not.toHaveBeenCalled()
      await act(async () => section('profile-alvanista-checkins')!.querySelector('button')!.click())
      await settle()
      expect(api.alvanistaProfileCheckins.mock.calls.map(([page]) => page)).toEqual([1, 2])
      expect(document.querySelector('[role="dialog"] [aria-label="Another game"]')).toBeTruthy()
      expect(finished.querySelector('[aria-label="Another game"]')).toBeNull()
    })

    it('shows games explicitly marked in the collection and links to the full collection', async () => {
      connect({ scopes: ['collection'] })
      const gameUuid = '11111111-0000-0000-0000-000000000003'
      api.listRoms.mockResolvedValue([rom('owned', { gameUuid })])
      api.alvanistaCollectionStatus.mockResolvedValue([{ gameId: gameUuid, inCollection: true, status: 'unplayed' }])
      await mount()
      const collection = section('profile-alvanista-collection')!
      expect(collection.querySelector('[aria-label="Jogo owned"]')).toBeTruthy()
      await act(async () => collection.querySelector('button')!.click())
      const openSite = [...document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')]
        .find((button) => button.textContent?.includes('Abrir coleção no Alvanista'))!
      await act(async () => openSite.click())
      expect(api.alvanistaOpenSite).toHaveBeenCalledWith('collection')
    })

    it('uses the authorized Gamers list and opens profiles by username', async () => {
      connect({ scopes: ['friends:read'] })
      api.alvanistaFriends.mockResolvedValue([{ id: '1', username: 'Ada', avatarUrl: null }])
      await mount()
      const gamers = section('profile-alvanista-gamers')!
      await act(async () => gamers.querySelector('button')!.click())
      const card = document.querySelector<HTMLElement>('[role="dialog"] .fui-Card')!
      await act(async () => card.click())
      expect(api.alvanistaOpenUser).toHaveBeenCalledWith('Ada')
      expect(api.alvanistaCollectionStatus).not.toHaveBeenCalled()
    })

    it('offers consent in the dialog for a connected account without collection access', async () => {
      connect()
      await mount()
      const toggle = section('profile-alvanista-finished')!.querySelector('button')!
      await act(async () => toggle.click())
      expect(document.querySelector('[role="dialog"]')!.textContent).toContain('Conectar à minha Coleção')
      expect(api.alvanistaCollectionStatus).not.toHaveBeenCalled()
      expect(api.alvanistaFriends).not.toHaveBeenCalled()
    })

    it('a card with nothing for No Alvanista hides the whole block, header included', async () => {
      connect({}, { identity: { level: 3 } })
      await mount()
      expect(section('profile-alvanista')).toBeNull()
      expect(host.textContent).not.toContain('No Alvanista')
      expect(section('profile-chips')!.textContent).toContain('Nível 3')
    })

    it('a connected status with a null card hides No Alvanista', async () => {
      connect({}, null)
      await mount()
      expect(section('profile-alvanista')).toBeNull()
      expect(section('profile-alvanista-invite')).toBeNull()
    })

    it('member_since is a year: shown as is, hidden when it is not a plausible year', async () => {
      connect({}, { ...FULL, member_since: 2010 })
      await mount()
      expect(section('profile-meta')!.textContent).toContain('Membro desde 2010')
      await unmount()
      host.remove()
      connect({}, { ...FULL, member_since: 5 })
      await mount()
      expect(section('profile-meta')!.textContent).not.toContain('Membro desde')
      expect(section('profile-meta')!.textContent).toContain('Newcastle')
    })

    it('without a database (cached state errors) the invitation is not offered', async () => {
      api.alvanistaCachedStatus.mockRejectedValue('no db')
      await mount()
      expect(section('profile-alvanista-invite')).toBeNull()
      expect(byText('Entrar com Alvanista')).toBeUndefined()
    })

    it('a broken badge image is hidden', async () => {
      connect()
      await mount()
      const img = section('profile-alvanista-badges')!.querySelector('img') as HTMLImageElement
      await act(async () => {
        img.dispatchEvent(new Event('error'))
      })
      expect(section('profile-alvanista-badges')!.querySelectorAll('img').length).toBe(4)
    })


    it('local state shows the invitation with the Entrar com Alvanista button', async () => {
      await mount()
      const inv = section('profile-alvanista-invite')!
      expect(inv).toBeTruthy()
      expect(inv.textContent).toContain('Entrar com Alvanista')
      expect(section('profile-alvanista')).toBeNull()
      expect(api.alvanistaConnect).not.toHaveBeenCalled()
    })

    it('the invitation button runs the connect flow and the page turns connected', async () => {
      const st = connect()
      api.alvanistaCachedStatus.mockResolvedValue(OFF)
      api.alvanistaStatus.mockResolvedValue(OFF)
      api.alvanistaConnect.mockResolvedValue(st)
      await mount()
      api.alvanistaCachedStatus.mockResolvedValue(st)
      await act(async () => byText('Entrar com Alvanista')!.click())
      await settle()
      expect(api.alvanistaConnect).toHaveBeenCalledTimes(1)
      expect(section('profile-alvanista-collection')).toBeTruthy()
    })

    it('a failed connect shows the outcome in the invitation', async () => {
      api.alvanistaConnect.mockRejectedValue('denied')
      await mount()
      await act(async () => byText('Entrar com Alvanista')!.click())
      await settle()
      expect(section('profile-alvanista-invite')!.textContent).toContain('Você não autorizou o acesso')
    })

    it('tier chip takes the tier accent, not the cover palette', async () => {
      connect()
      await mount()
      const chip = host.querySelector('[data-chip="tier"]') as HTMLElement
      const probe = document.createElement('span')
      probe.style.backgroundColor = '#22d3ee'
      probe.style.color = readableInk('#22d3ee')
      expect(chip.style.backgroundColor).toBe(probe.style.backgroundColor)
      expect(chip.style.color).toBe(probe.style.color)
      const level = host.querySelector('[data-chip="level"]') as HTMLElement
      expect(level.style.color).toBe('')
    })

    it('without tier colors the tier chip keeps the default style, whatever the palette', async () => {
      connect({}, { ...FULL, identity: { ...FULL.identity, tier: { slug: 'collector', rank: 2, name: { pt: 'Colecionador' } } } })
      await mount()
      const chip = host.querySelector('[data-chip="tier"]') as HTMLElement
      expect(chip.textContent).toBe('Colecionador')
      expect(chip.style.color).toBe('')
      expect(chip.style.backgroundColor).toBe('')
      expect(section('profile-hero')!.querySelector('[data-testid="tier-ring"]')).toBeNull()
    })

    it('the hero avatar wears the tier ring from the card, pulsing', async () => {
      connect()
      await mount()
      const ring = section('profile-hero')!.querySelector('[data-testid="tier-ring"]') as HTMLElement
      expect(ring).toBeTruthy()
      expect(ring.hasAttribute('data-pulse')).toBe(true)
      const layer = ring.querySelector('[data-tier-ring-layer]') as HTMLElement
      expect(layer.getAttribute('style')).toContain('#34d399')
    })

    it('renders the sidebar only for the recent games or Alvanista content', async () => {
      connect({}, null)
      await mount()
      expect(section('profile-sidebar')!.contains(section('profile-alvanista-checkins'))).toBe(true)
      await unmount()
      host.remove()
      connect()
      await mount()
      expect(section('profile-sidebar')!.contains(section('profile-alvanista-collection'))).toBe(true)
      await unmount()
      host.remove()
      api.listRoms.mockResolvedValue([rom('a', { lastPlayedAt: 1, playTimeSecs: 60 })])
      connect({}, null)
      await mount()
      expect(section('profile-sidebar')!.contains(section('profile-recent'))).toBe(true)
      expect(section('profile-sidebar')!.contains(section('profile-most-played'))).toBe(false)
    })
  })
})
