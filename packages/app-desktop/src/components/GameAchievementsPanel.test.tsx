// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import type { RaGameView } from '../lib/tauri'
import { GameAchievementsPanel } from './GameAchievementsPanel'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const mocks = vi.hoisted(() => ({
  raGameAchievements: vi.fn<(romId: string) => Promise<RaGameView>>(),
}))

vi.mock('../lib/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../lib/tauri')>()
  return {
    ...actual,
    raGameAchievements: mocks.raGameAchievements,
  }
})

async function renderPanel(romId = 'rom-1') {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () => {
    root.render(
      <FluentProvider theme={webDarkTheme}>
        <QueryClientProvider client={qc}>
          <GameAchievementsPanel romId={romId} />
        </QueryClientProvider>
      </FluentProvider>,
    )
  })
  await act(async () => {
    await new Promise((r) => setTimeout(r, 20))
  })
  return {
    host,
    unmount: async () => {
      await act(async () => root.unmount())
      host.remove()
    },
  }
}

describe('GameAchievementsPanel', () => {
  beforeEach(() => {
    mocks.raGameAchievements.mockReset()
  })

  it('renders live rich presence, active indicators, measured progress, and leaderboards toggle', async () => {
    mocks.raGameAchievements.mockResolvedValue({
      status: 'loaded',
      gameId: 42,
      title: 'Super Metroid',
      badgeUrl: 'http://rabadge.localhost/game_001234.png',
      hardcore: true,
      hardcoreDowngraded: false,
      compatibleRomId: null,
      totalAchievements: 2,
      unlockedAchievements: 1,
      totalPoints: 35,
      unlockedPoints: 10,
      playingLive: true,
      richPresence: 'Norfair | 4 Energy Tanks',
      activeChallenges: [
        {
          id: 11,
          title: 'Damageless Ridley',
          description: 'Defeat Ridley without taking damage',
          points: 25,
          badgeUrl: 'http://rabadge.localhost/66666.png',
        },
      ],
      activeTrackers: [{ id: 1, display: '00:42.50' }],
      achievements: [
        {
          id: 10,
          title: 'First Missile',
          description: 'Pick up a missile pack',
          points: 10,
          unlocked: true,
          unlockedHardcore: true,
          unlockTime: 1700000000,
          badgeUrl: 'http://rabadge.localhost/55555.png',
          badgeLockedUrl: 'http://rabadge.localhost/55555_lock.png',
          measuredProgress: null,
          measuredPercent: 100,
          rarity: 80,
          rarityHardcore: 50,
          kind: 'progression',
          unofficial: false,
          unsupported: false,
        },
        {
          id: 11,
          title: 'Missile Master',
          description: 'Collect 10 missile packs',
          points: 25,
          unlocked: false,
          unlockedHardcore: false,
          unlockTime: null,
          badgeUrl: 'http://rabadge.localhost/66666.png',
          badgeLockedUrl: 'http://rabadge.localhost/66666_lock.png',
          measuredProgress: '4/10',
          measuredPercent: 40,
          rarity: 20,
          rarityHardcore: 10,
          kind: 'missable',
          unofficial: false,
          unsupported: false,
        },
      ],
      leaderboards: [
        {
          id: 900,
          title: 'Ceres Escape',
          description: 'Escape Ceres Station fastest',
          trackerValue: '00:42.50',
          state: 'tracking',
          format: 'time',
          lowerIsBetter: true,
        },
      ],
    })

    const { host, unmount } = await renderPanel()
    expect(host.querySelector('[data-testid="ra-rich-presence"]')?.textContent).toContain(
      'Norfair | 4 Energy Tanks',
    )
    expect(host.querySelector('[data-testid="ra-live-indicators"]')?.textContent).toContain(
      'Damageless Ridley',
    )
    expect(host.querySelector('[data-testid="ra-live-indicators"]')?.textContent).toContain(
      '00:42.50',
    )
    expect(host.textContent).toContain('4/10')

    // Alterna para a secao de Placares.
    const buttons = Array.from(host.querySelectorAll('button'))
    const lbTab = buttons.find((b) => b.textContent?.includes('(1)') && b !== buttons[0])
    expect(lbTab).toBeTruthy()
    await act(async () => {
      lbTab!.click()
    })

    const lbList = host.querySelector('[data-testid="ra-leaderboards-list"]')
    expect(lbList?.textContent).toContain('Ceres Escape')
    expect(lbList?.textContent).toContain('Escape Ceres Station fastest')
    expect(lbList?.textContent).toContain('00:42.50')

    await unmount()
  })
})
