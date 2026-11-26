// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { ALVANISTA_CACHED_KEY } from '../hooks/useEffectiveProfile'
import type { AlvanistaStatus, PartnerFriend } from '../lib/tauri'
import { Network } from './Network'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const tauriMocks = vi.hoisted(() => ({
  alvanistaCachedStatus: vi.fn<() => Promise<AlvanistaStatus>>(),
  alvanistaConnect: vi.fn<(...args: unknown[]) => Promise<AlvanistaStatus>>(),
  alvanistaCancel: vi.fn<() => Promise<void>>(),
  alvanistaFriends: vi.fn<() => Promise<PartnerFriend[]>>(),
  alvanistaOpenUser: vi.fn<(u: string) => Promise<void>>(),
}))

vi.mock('../lib/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../lib/tauri')>()
  return {
    ...actual,
    ...tauriMocks,
  }
})

function makeStatus(scopes: string[]): AlvanistaStatus {
  return {
    connected: true,
    username: 'toy',
    avatar_url: null,
    hero_url: null,
    bio: null,
    profile_url: 'https://www.alvanista.com/users/toy',
    offline: false,
    ended: false,
    card: null,
    fetched_at: 1700000000,
    connected_at: 1700000000,
    device_label: 'ReEmu (Windows)',
    save_sync: true,
    scopes,
  }
}

async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 20))
  })
}

describe('Network screen', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    window.localStorage.removeItem('reemu.network.showOnline')
  })

  it('keeps online presence hidden by default until the user turns the toggle on', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    qc.setQueryData(ALVANISTA_CACHED_KEY, makeStatus(['profile:read', 'friends:read']))
    tauriMocks.alvanistaFriends.mockResolvedValue([
      {
        id: 'alva_kluis',
        username: 'kluis',
        avatarUrl: null,
        profileUrl: 'https://www.alvanista.com/users/kluis',
        online: true,
        playingNow: {
          gameId: null,
          title: 'Chrono Trigger',
          platform: 'SNES',
          customMessage: null,
        },
      },
    ])

    const host = document.createElement('div')
    document.body.appendChild(host)
    const root = createRoot(host)
    await act(async () => {
      root.render(
        <QueryClientProvider client={qc}>
          <FluentProvider theme={webDarkTheme}>
            <MemoryRouter>
              <Network />
            </MemoryRouter>
          </FluentProvider>
        </QueryClientProvider>,
      )
    })
    await flush()

    expect(host.textContent).toContain('kluis')
    expect(host.textContent).not.toContain('Chrono Trigger')

    const toggle = host.querySelector('input[type="checkbox"]') as HTMLInputElement
    expect(toggle).toBeTruthy()
    await act(async () => {
      toggle.click()
    })
    await flush()

    expect(host.textContent).toContain('Chrono Trigger')

    await act(async () => root.unmount())
    host.remove()
  })
})
