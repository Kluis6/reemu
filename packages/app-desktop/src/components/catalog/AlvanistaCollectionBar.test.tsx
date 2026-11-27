// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../../i18n'
import { ALVANISTA_CACHED_KEY } from '../../hooks/useEffectiveProfile'
import type { AlvanistaStatus, CollectionItem } from '../../lib/tauri'
import { AlvanistaCollectionBar } from './AlvanistaCollectionBar'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const tauriMocks = vi.hoisted(() => ({
  alvanistaCachedStatus: vi.fn<() => Promise<AlvanistaStatus>>(),
  alvanistaConnect: vi.fn<(...args: unknown[]) => Promise<AlvanistaStatus>>(),
  alvanistaCancel: vi.fn<() => Promise<void>>(),
  alvanistaDisconnect: vi.fn<() => Promise<void>>(),
  alvanistaCollectionStatus: vi.fn<(ids: string[]) => Promise<CollectionItem[]>>(),
  alvanistaCollectionUpdate: vi.fn<(id: string, patch: unknown) => Promise<CollectionItem>>(),
}))

vi.mock('../../lib/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/tauri')>()
  return {
    ...actual,
    ...tauriMocks,
  }
})

const GAME_UUID = '11111111-2222-3333-4444-555555555555'

function makeStatus(scopes: string[], connected = true): AlvanistaStatus {
  return {
    connected,
    username: connected ? 'player1' : null,
    avatar_url: null,
    hero_url: null,
    bio: null,
    profile_url: connected ? 'https://www.alvanista.com/users/player1' : null,
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

async function mountBar(qc: QueryClient, playTimeSeconds = 7200) {
  const host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () => {
    root.render(
      <QueryClientProvider client={qc}>
        <FluentProvider theme={webDarkTheme}>
          <AlvanistaCollectionBar
            gameUuid={GAME_UUID}
            systemId="snes"
            playTimeSeconds={playTimeSeconds}
          />
        </FluentProvider>
      </QueryClientProvider>,
    )
  })
  await flush()
  return {
    host,
    unmount: async () => {
      await act(async () => root.unmount())
      host.remove()
    },
  }
}

describe('AlvanistaCollectionBar', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('asks for incremental consent when connected without collection scope', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    qc.setQueryData(ALVANISTA_CACHED_KEY, makeStatus(['profile:read', 'saves']))
    tauriMocks.alvanistaConnect.mockResolvedValue(
      makeStatus(['profile:read', 'saves', 'collection']),
    )
    tauriMocks.alvanistaCollectionStatus.mockResolvedValue([
      {
        gameId: GAME_UUID,
        inCollection: false,
        possessed: false,
        ownership: 'none',
        inWishlist: false,
        interest: 'none',
        status: 'no_status',
        playtimeMinutes: 0,
        lastPlayedAt: null,
      },
    ])

    const view = await mountBar(qc)
    const btn = view.host.querySelector('button')!
    expect(btn).toBeTruthy()
    await act(async () => {
      btn.click()
    })
    await flush()

    expect(tauriMocks.alvanistaConnect).toHaveBeenCalledWith(
      expect.any(String),
      false,
      ['collection'],
    )
    await view.unmount()
  })

  it('renders collection, wishlist, journey and playtime sync controls when granted', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    qc.setQueryData(ALVANISTA_CACHED_KEY, makeStatus(['profile:read', 'collection']))
    tauriMocks.alvanistaCollectionStatus.mockResolvedValue([
      {
        gameId: GAME_UUID,
        inCollection: true,
        possessed: true,
        ownership: 'owned',
        inWishlist: false,
        interest: 'none',
        status: 'playing',
        playtimeMinutes: 30,
        lastPlayedAt: null,
      },
    ])
    tauriMocks.alvanistaCollectionUpdate.mockImplementation(async (_id, patch) => ({
      gameId: GAME_UUID,
      inCollection: true,
      possessed: true,
      ownership: 'owned',
      inWishlist: false,
      interest: 'none',
      status: (patch as { status?: string }).status ?? 'playing',
      playtimeMinutes: (patch as { playtimeMinutes?: number }).playtimeMinutes ?? 30,
      lastPlayedAt: null,
    }))

    const view = await mountBar(qc, 7200) // 120 min > 30 min remote
    const bar = view.host.querySelector('[data-testid="alva-collection-bar"]')
    expect(bar).toBeTruthy()

    const buttons = [...view.host.querySelectorAll('button')]
    const syncBtn = buttons.find((b) => b.textContent?.includes('120'))
    expect(syncBtn).toBeTruthy()
    await act(async () => {
      syncBtn!.click()
    })
    await flush()

    expect(tauriMocks.alvanistaCollectionUpdate).toHaveBeenCalledWith(GAME_UUID, {
      playtimeMinutes: 120,
      system: 'snes',
    })

    const wishlistBtn = [...view.host.querySelectorAll('button')].find(
      (b) => b.getAttribute('aria-pressed') === 'false',
    )
    expect(wishlistBtn).toBeTruthy()
    await act(async () => {
      wishlistBtn!.click()
    })
    await flush()

    expect(tauriMocks.alvanistaCollectionUpdate).toHaveBeenCalledWith(GAME_UUID, {
      inWishlist: true,
      system: 'snes',
    })
    await view.unmount()
  })
})
