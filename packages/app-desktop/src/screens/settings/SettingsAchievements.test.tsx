// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../../i18n'
import type { RaStatus } from '../../lib/tauri'
import { SettingsAchievements } from './SettingsAchievements'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const mocks = vi.hoisted(() => ({
  raStatus: vi.fn<() => Promise<RaStatus>>(),
  raLogin: vi.fn<(u: string, p: string) => Promise<RaStatus>>(),
  raLogout: vi.fn<() => Promise<RaStatus>>(),
  raSetPreferences: vi.fn<(p: Record<string, unknown>) => Promise<RaStatus>>(),
}))

vi.mock('../../lib/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/tauri')>()
  return {
    ...actual,
    raStatus: mocks.raStatus,
    raLogin: mocks.raLogin,
    raLogout: mocks.raLogout,
    raSetPreferences: mocks.raSetPreferences,
  }
})

async function renderScreen() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () => {
    root.render(
      <FluentProvider theme={webDarkTheme}>
        <QueryClientProvider client={qc}>
          <SettingsAchievements />
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

describe('SettingsAchievements', () => {
  beforeEach(() => {
    mocks.raStatus.mockReset()
    mocks.raLogin.mockReset()
    mocks.raLogout.mockReset()
    mocks.raSetPreferences.mockReset()
  })

  it('pre-fills suggested Alvanista RA username when disconnected', async () => {
    mocks.raStatus.mockResolvedValue({
      connected: false,
      enabled: true,
      hardcore: false,
      unofficial: false,
      encore: false,
      username: null,
      displayName: null,
      avatarUrl: null,
      score: 0,
      scoreSoftcore: 0,
      linkedAlvanistaUsername: 'edward_ra',
    })
    const { host, unmount } = await renderScreen()
    const inputs = host.querySelectorAll('input')
    expect(inputs.length).toBe(2)
    expect((inputs[0] as HTMLInputElement).value).toBe('edward_ra')
    await unmount()
  })

  it('shows scores and toggles Hardcore mode when connected', async () => {
    const connectedState: RaStatus = {
      connected: true,
      enabled: true,
      hardcore: false,
      unofficial: false,
      encore: false,
      username: 'edward_ra',
      displayName: 'Edward',
      avatarUrl: null,
      score: 1420,
      scoreSoftcore: 300,
      linkedAlvanistaUsername: 'edward_ra',
    }
    mocks.raStatus.mockResolvedValue(connectedState)
    mocks.raSetPreferences.mockResolvedValue({ ...connectedState, hardcore: true })

    const { host, unmount } = await renderScreen()
    expect(host.textContent).toContain('Edward')
    expect(host.textContent).toContain('1420')

    const switches = host.querySelectorAll('input[type="checkbox"]')
    expect(switches.length).toBe(4)
    await act(async () => {
      ;(switches[1] as HTMLInputElement).click()
    })
    expect(mocks.raSetPreferences).toHaveBeenCalledWith({ hardcore: true })
    await unmount()
  })
})
