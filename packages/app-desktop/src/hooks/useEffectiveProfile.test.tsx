// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import type { AlvanistaStatus } from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  alvanistaCachedStatus: vi.fn(),
  getProfile: vi.fn(),
  onAlvanistaStatus: vi.fn(),
}))

vi.mock('../lib/tauri', async (orig) => ({
  ...(await orig<typeof import('../lib/tauri')>()),
  ...api,
}))

import { MemoryRouter } from 'react-router-dom'
import { NoGamesState } from '../components/NoGamesState'
import { ProfileDrawer } from '../components/ProfileDrawer'
import {
  ALVANISTA_CACHED_KEY,
  useAlvanistaStatusEvents,
  useEffectiveProfile,
  type EffectiveProfile,
} from './useEffectiveProfile'

const OFF: AlvanistaStatus = {
  connected: false,
  username: null,
  avatar_url: null,
  hero_url: null,
  bio: null,
  profile_url: null,
  offline: false,
  insecure_storage: false,
  ended: false,
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
const LOCAL = { name: 'Local', bio: 'bio local', avatar: 'preset:2', onboarded: true }

let host: HTMLDivElement
let unmount: () => Promise<void>
let qc: QueryClient

async function settle() {
  for (let i = 0; i < 5; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0))
    })
  }
}

async function mount(node: React.ReactElement) {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  await act(async () => root.render(<QueryClientProvider client={qc}>{node}</QueryClientProvider>))
  await settle()
  unmount = () => act(async () => root.unmount())
}

function Probe() {
  return <output data-probe="">{JSON.stringify(useEffectiveProfile() ?? null)}</output>
}

/** O que o `Probe` mostrou por último (`undefined` antes das duas leituras). */
function seen(): EffectiveProfile | undefined {
  const raw = host.querySelector('[data-probe]')?.textContent ?? 'null'
  return (JSON.parse(raw) as EffectiveProfile | null) ?? undefined
}

function Events() {
  useAlvanistaStatusEvents()
  return null
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.getProfile.mockResolvedValue(LOCAL)
  api.alvanistaCachedStatus.mockResolvedValue(OFF)
  api.onAlvanistaStatus.mockResolvedValue(() => {})
  useToastStore.setState({ queue: [] })
})
afterEach(async () => {
  await unmount()
  host.remove()
})

describe('useEffectiveProfile', () => {
  it('disconnected: the local profile', async () => {
    await mount(<Probe />)
    expect(seen()).toEqual({
      name: 'Local',
      avatar: 'preset:2',
      avatarUrl: null,
      bio: 'bio local',
      bannerUrl: null,
      source: 'local',
    })
  })

  it('connected: the Alvanista username and avatar; the local profile is not written', async () => {
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    await mount(<Probe />)
    expect(seen()).toEqual({
      name: 'toy',
      avatar: 'preset:1',
      avatarUrl: 'https://cdn.alvanista.com/a/toy.png',
      bio: 'bio do Alvanista',
      bannerUrl: 'https://cdn.alvanista.com/h/toy.png',
      source: 'alvanista',
    })
  })

  it('connected without bio: no bio, never the local one', async () => {
    api.alvanistaCachedStatus.mockResolvedValue({ ...ON, bio: null })
    await mount(<Probe />)
    expect(seen()).toMatchObject({ name: 'toy', bio: null, source: 'alvanista' })
  })

  it('connected without avatar: the ReEmu preset', async () => {
    api.alvanistaCachedStatus.mockResolvedValue({ ...ON, avatar_url: null })
    await mount(<Probe />)
    expect(seen()).toMatchObject({ name: 'toy', avatar: 'preset:1', avatarUrl: null })
  })

  it('connected but no username saved yet: the local profile', async () => {
    api.alvanistaCachedStatus.mockResolvedValue({ ...ON, username: null, avatar_url: null })
    await mount(<Probe />)
    expect(seen()).toMatchObject({ name: 'Local', source: 'local' })
  })

  it('no database (cached status rejects): the local profile', async () => {
    api.alvanistaCachedStatus.mockRejectedValue('failed')
    await mount(<Probe />)
    expect(seen()).toMatchObject({ name: 'Local', source: 'local' })
  })

  it('waits for both reads before answering (no flash of the local profile)', async () => {
    api.alvanistaCachedStatus.mockReturnValue(new Promise(() => {}))
    await mount(<Probe />)
    expect(seen()).toBeUndefined()
  })

  it('follows the cached status when its query is invalidated', async () => {
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    await mount(<Probe />)
    expect(seen()?.source).toBe('alvanista')
    api.alvanistaCachedStatus.mockResolvedValue(OFF)
    await act(async () => qc.invalidateQueries({ queryKey: ALVANISTA_CACHED_KEY }))
    await settle()
    expect(seen()?.source).toBe('local')
  })
})

describe('useAlvanistaStatusEvents', () => {
  it('the startup refresh refetches the cached status and toasts when the connection ended', async () => {
    let deliver!: (st: AlvanistaStatus) => void
    api.onAlvanistaStatus.mockImplementation(async (cb: (st: AlvanistaStatus) => void) => {
      deliver = cb
      return () => {}
    })
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    await mount(
      <>
        <Events />
        <Probe />
      </>,
    )
    expect(seen()?.source).toBe('alvanista')
    api.alvanistaCachedStatus.mockResolvedValue(OFF)
    await act(async () => deliver({ ...OFF, ended: true }))
    await settle()
    expect(seen()?.source).toBe('local')
    const messages = useToastStore.getState().queue.map((x) => x.message)
    expect(messages).toContain('Conexão com o Alvanista encerrada')
  })
})

describe('NoGamesState', () => {
  it('welcomes the Alvanista username when connected', async () => {
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    await mount(<NoGamesState onAdd={() => {}} />)
    expect(host.textContent).toContain('Bem-vindo, toy!')
  })

  it('welcomes the local name when disconnected', async () => {
    await mount(<NoGamesState onAdd={() => {}} />)
    expect(host.textContent).toContain('Bem-vindo, Local!')
  })
})

/** O drawer como o `AppShell` o monta: perfil efetivo e capa como banner. */
function Drawer() {
  const p = useEffectiveProfile()
  return p ? (
    <MemoryRouter>
      <ProfileDrawer open onOpenChange={() => {}} profile={p} bannerUrl={p.bannerUrl} />
    </MemoryRouter>
  ) : null
}

describe('ProfileDrawer', () => {
  // Sem Web Animations o Fluent abre e fecha o drawer sem animar; no happy-dom
  // a animação cancelada ao desmontar vira rejeição solta.
  const animate = Element.prototype.animate
  beforeAll(() => {
    ;(Element.prototype as { animate?: unknown }).animate = undefined
  })
  afterAll(() => {
    Element.prototype.animate = animate
  })

  it('connected: the Alvanista name, bio and cover', async () => {
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    await mount(<Drawer />)
    const text = document.body.textContent ?? ''
    expect(text).toContain('toy')
    expect(text).toContain('bio do Alvanista')
    expect(text).not.toContain('bio local')
    expect(document.body.innerHTML).toContain('https://cdn.alvanista.com/h/toy.png')
  })

  it('disconnected: the local name and bio, theme colors in the banner', async () => {
    await mount(<Drawer />)
    const text = document.body.textContent ?? ''
    expect(text).toContain('Local')
    expect(text).toContain('bio local')
    expect(document.body.innerHTML).not.toContain('cdn.alvanista.com')
  })
})
