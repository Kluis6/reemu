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

const api = vi.hoisted(() => ({
  listRoms: vi.fn(),
  listRecentSaveStates: vi.fn(),
  saveThumbnailUrl: vi.fn(),
  getProfile: vi.fn(),
  alvanistaStatus: vi.fn(),
  alvanistaCachedStatus: vi.fn(),
  alvanistaOpenProfile: vi.fn(),
  alvanistaOpenSite: vi.fn(),
  alvanistaDisconnect: vi.fn(),
  alvanistaConnect: vi.fn(),
  alvanistaCancel: vi.fn(),
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
  api.alvanistaDisconnect.mockResolvedValue(undefined)
  api.alvanistaCancel.mockResolvedValue(undefined)
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
    expect(first('profile-recent').getAttribute('aria-label')).toBe('Novo')
    // a linha de "Mais jogados" se nomeia pelo conteúdo: título e tempo, só spans
    const row = first('profile-most-played')
    expect(row.getAttribute('aria-label')).toBeNull()
    expect(row.textContent).toContain('Antigo')
    expect(row.textContent).toContain('2 h 30 min')
    expect(section('profile-most-played')!.querySelector('button div')).toBeNull()
    await act(async () => (section('profile-recent')!.querySelector('button, [role="button"]') as HTMLElement).click())
    await settle()
    expect(document.body.textContent).toContain('tela do jogo')
  })

  it('moments hide when there are no saves with thumbnails', async () => {
    api.listRoms.mockResolvedValue([rom('a', { playTimeSecs: 60 })])
    await mount()
    expect(section('profile-moments')).toBeNull()
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
    const refresh = () => host.querySelector('[aria-label="Atualizar agora"]') as HTMLElement
    expect(refresh()).toBeTruthy()
    const before = api.alvanistaStatus.mock.calls.length
    await act(async () => refresh().click())
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
    expect(section('profile-cols')).toBeNull()
  })

  it('recent shows play time and relative date under the cover', async () => {
    const now = Math.floor(Date.now() / 1000)
    api.listRoms.mockResolvedValue([
      rom('a', { title: 'Kuon', lastPlayedAt: now - 3 * 86400, playTimeSecs: 3600 }),
      rom('b', { title: 'Zero', lastPlayedAt: now - 5 * 86400, playTimeSecs: 0 }),
    ])
    await mount()
    const lines = [...section('profile-recent')!.querySelectorAll('[data-caption]')].map((e) => e.textContent)
    expect(lines[0]).toBe('1 h · há 3 dias')
    expect(lines[1]).toBe('há 5 dias')
  })

  describe('Alvanista card', () => {
    const FULL: AlvanistaCard = {
      member_since: 2010,
      identity: {
        location: 'Newcastle Upon Tyne',
        level: 19,
        tier: { slug: 'collector', rank: 3, name: { pt: 'Colecionador', en: 'Collector' } },
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

    it('No Alvanista shows collection, trophies with networks, hours, finished, check-ins, gamers, up to four badges with the total, playing now', async () => {
      connect()
      await mount()
      const b = section('profile-alvanista')!
      const tile = (k: string) => b.querySelector(`[data-tile="${k}"]`)!.textContent!
      expect(tile('collection')).toContain('2.047')
      expect(tile('collection')).toContain('44 plataformas')
      expect(tile('trophies')).toContain('3.943')
      expect(tile('trophies')).toContain('PlayStation')
      expect(tile('trophies')).toContain('Xbox')
      expect(tile('trophies')).not.toContain('psn')
      // rede que o ReEmu não conhece: como veio
      expect(tile('trophies')).toContain('nova-rede')
      expect(tile('hours')).toContain('614 h')
      expect(tile('finished')).toContain('23')
      expect(tile('checkins')).toContain('244')
      expect(tile('gamers')).toContain('173')
      expect(section('profile-alvanista-badges')!.querySelectorAll('img').length).toBe(4)
      expect(section('profile-alvanista-badges')!.textContent).toContain('27 badges')
      expect(section('profile-alvanista-now')!.textContent).toContain('Silver Pines')
    })

    it('a card without badges or playing now hides just those rows', async () => {
      connect({}, { ...FULL, badges: undefined, playing_now: undefined })
      await mount()
      expect(section('profile-alvanista')).toBeTruthy()
      expect(section('profile-alvanista-badges')).toBeNull()
      expect(section('profile-alvanista-now')).toBeNull()
      expect(section('profile-alvanista')!.querySelector('[data-tile="hours"]')).toBeTruthy()
    })

    it('a card computed three days before the status read shows its own age and no playing now', async () => {
      const now = Math.floor(Date.now() / 1000)
      connect({ fetched_at: now }, { ...FULL, computed_at: new Date((now - 3 * 86400) * 1000).toISOString() })
      await mount()
      const b = section('profile-alvanista')!
      expect(b.textContent).toContain('atualizado há 3 dias')
      expect(section('profile-alvanista-now')).toBeNull()
      expect(b.textContent).not.toContain('Jogando agora')
    })

    it('a fresh card shows no age and keeps playing now; ten minutes old shows the age and still plays', async () => {
      const now = Math.floor(Date.now() / 1000)
      connect({ fetched_at: now }, { ...FULL, computed_at: new Date((now - 60) * 1000).toISOString() })
      await mount()
      expect(section('profile-alvanista')!.textContent).not.toContain('atualizado há')
      expect(section('profile-alvanista-now')!.textContent).toContain('Silver Pines')
      await unmount()
      host.remove()
      connect({ fetched_at: now }, { ...FULL, computed_at: new Date((now - 600) * 1000).toISOString() })
      await mount()
      expect(section('profile-alvanista')!.textContent).toContain('atualizado há 10 minutos')
      expect(section('profile-alvanista-now')!.textContent).toContain('Silver Pines')
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
      expect(section('profile-connection')).toBeTruthy()
    })

    it('connection card: device label, connected since, keychain, Gerenciar apps opens apps, Desconectar returns to the local profile without reload', async () => {
      connect()
      await mount()
      const c = section('profile-connection')!
      expect(c.textContent).toContain('toy')
      expect(c.textContent).toContain('alvanista.com/u/toy')
      expect(c.textContent).toContain('ReEmu (Windows)')
      expect(c.textContent).toContain('conectado desde')
      expect(c.textContent).toContain('guardada no chaveiro do sistema')
      const back = section('profile-connection-back')!
      expect(back.textContent).toContain('Local')
      await act(async () => byText('Gerenciar apps conectados no Alvanista')!.click())
      expect(api.alvanistaOpenSite).toHaveBeenCalledWith('apps')
      // depois de desconectar, o estado guardado já é o desconectado
      api.alvanistaCachedStatus.mockResolvedValue(OFF)
      api.alvanistaStatus.mockResolvedValue(OFF)
      await act(async () => byText('Desconectar')!.click())
      await settle()
      expect(api.alvanistaDisconnect).toHaveBeenCalledTimes(1)
      expect(section('profile-connection')).toBeNull()
      expect(section('profile-alvanista')).toBeNull()
      expect(section('profile-chips')).toBeNull()
      expect(section('profile-alvanista-invite')).toBeTruthy()
      expect(host.textContent).toContain('bio local')
      expect(useToastStore.getState().queue.map((x) => x.message)).toContain(
        'Desconectado do Alvanista. Seu perfil local voltou.',
      )
    })

    it('disconnect drops the stored card at once, even before the cached state is re-read', async () => {
      connect()
      await mount()
      // a releitura do estado guardado nunca chega: o topo não pode esperar por ela
      api.alvanistaCachedStatus.mockReturnValue(new Promise(() => {}))
      await act(async () => byText('Desconectar')!.click())
      await settle()
      expect(section('profile-alvanista')).toBeNull()
      expect(section('profile-connection')).toBeNull()
      expect(section('profile-alvanista-invite')).toBeTruthy()
      const hero = section('profile-hero')!
      expect(hero.textContent).toContain('Local')
      expect(hero.textContent).not.toContain('toy')
      expect(byText('Ver no Alvanista')).toBeUndefined()
      expect(byText('Editar no Alvanista')).toBeUndefined()
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
      expect(section('profile-alvanista-badges')!.querySelectorAll('img').length).toBe(3)
    })


    it('connected_at null hides connected since', async () => {
      connect({ connected_at: null })
      await mount()
      const c = section('profile-connection')!
      expect(c.textContent).not.toContain('conectado desde')
      expect(c.textContent).toContain('ReEmu (Windows)')
    })

    it('local state shows the invitation with the Entrar com Alvanista button and no connection card', async () => {
      await mount()
      const inv = section('profile-alvanista-invite')!
      expect(inv).toBeTruthy()
      expect(inv.textContent).toContain('Entrar com Alvanista')
      expect(section('profile-connection')).toBeNull()
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
      expect(section('profile-alvanista')).toBeTruthy()
      expect(section('profile-connection')).toBeTruthy()
    })

    it('a failed connect shows the outcome in the invitation', async () => {
      api.alvanistaConnect.mockRejectedValue('denied')
      await mount()
      await act(async () => byText('Entrar com Alvanista')!.click())
      await settle()
      expect(section('profile-alvanista-invite')!.textContent).toContain('Você não autorizou o acesso')
    })

    it('tier chip ink comes from readableInk(palette.mid)', async () => {
      connect()
      await mount()
      const chip = host.querySelector('[data-chip="tier"]') as HTMLElement
      const probe = document.createElement('span')
      probe.style.color = readableInk('#e0b84a')
      expect(chip.style.color).toBe(probe.style.color)
      expect(chip.style.color).not.toBe('')
      const level = host.querySelector('[data-chip="level"]') as HTMLElement
      expect(level.style.color).toBe('')
    })

    it('a palette mid that is not #rrggbb leaves the tier chip in the default style', async () => {
      connect({}, { ...FULL, palette: { base: '#111111', mid: 'gold', accent: '#ffffff', source: 'hero' } })
      await mount()
      const chip = host.querySelector('[data-chip="tier"]') as HTMLElement
      expect(chip.textContent).toBe('Colecionador')
      expect(chip.style.color).toBe('')
      expect(chip.style.backgroundColor).toBe('')
    })

    it('one platform reads in the singular', async () => {
      connect({}, { proofs: { collection: { games: 3, platforms: 1 } } })
      await mount()
      const tile = section('profile-alvanista')!.querySelector('[data-tile="collection"]')!.textContent!
      expect(tile).toContain('1 plataforma')
      expect(tile).not.toContain('plataformas')
    })

    it('the two-column row is not rendered empty, and a lone side takes the whole row', async () => {
      // conectado sem cartão e sem jogo com tempo: nada dos dois lados
      connect({}, null)
      await mount()
      expect(section('profile-cols')).toBeNull()
      await unmount()
      host.remove()
      // só o bloco do Alvanista
      connect()
      await mount()
      expect(section('profile-cols')!.getAttribute('data-sides')).toBe('1')
      expect(section('profile-alvanista')).toBeTruthy()
      await unmount()
      host.remove()
      // os dois lados
      api.listRoms.mockResolvedValue([rom('a', { playTimeSecs: 60 })])
      connect()
      await mount()
      expect(section('profile-cols')!.getAttribute('data-sides')).toBe('2')
    })
  })
})
