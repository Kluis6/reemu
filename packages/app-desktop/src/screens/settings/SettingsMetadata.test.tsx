// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CatalogStatus, MetadataConfig, ScrapeProgress } from '../../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  catalogStatus: vi.fn(),
  catalogSetEnabled: vi.fn(),
  getMetadataConfig: vi.fn(),
  metadataScanProgress: vi.fn(),
  listPendingMatches: vi.fn(),
}))

vi.mock('../../lib/tauri', async (orig) => ({
  ...(await orig<typeof import('../../lib/tauri')>()),
  ...api,
}))

import '../../i18n'
import { SettingsMetadata } from './SettingsMetadata'

const CFG: MetadataConfig = {
  provider: 'screenscraper',
  screenscraperUser: null,
  screenscraperPassword: null,
  thegamesdbApiKey: null,
}

const PROGRESS: ScrapeProgress = {
  running: false,
  status: 'idle',
  done: 0,
  total: 0,
  auto: 0,
  pending: 0,
  failed: 0,
  error: null,
}

const CONNECTED_STATUS: CatalogStatus = {
  connected: true,
  enabled: true,
  disabledByServer: false,
  running: false,
  found: 12,
  pending: 3,
  unknown: 1,
}

let host: HTMLDivElement
let unmount: () => Promise<void>

async function settle() {
  for (let i = 0; i < 6; i++) {
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
        <SettingsMetadata />
      </QueryClientProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.catalogStatus.mockResolvedValue(CONNECTED_STATUS)
  api.catalogSetEnabled.mockImplementation(async (enabled: boolean) => ({
    ...CONNECTED_STATUS,
    enabled,
  }))
  api.getMetadataConfig.mockResolvedValue(CFG)
  api.metadataScanProgress.mockResolvedValue(PROGRESS)
  api.listPendingMatches.mockResolvedValue([])
})

afterEach(async () => {
  if (unmount) await unmount()
  host?.remove()
})

describe('SettingsMetadata catalog section', () => {
  it('status_line_shows_counts', async () => {
    await mount()
    expect(host.textContent).toContain('Usar o catálogo do Alvanista para identificar meus jogos')
    expect(host.textContent).toContain(
      'Os nomes e os hashes dos arquivos vão para o Alvanista para identificar os jogos. Nada sobre você fica junto do pedido.',
    )
    const line = host.querySelector('[data-testid="catalog-status-line"]')
    expect(line?.textContent).toBe('12 identificados, 3 na fila, 1 sem resposta')
  })

  it('toggle_off_calls_catalog_set_enabled_false', async () => {
    await mount()
    const sw = host.querySelector<HTMLInputElement>('input[role="switch"]')
    expect(sw?.checked).toBe(true)
    await act(async () => {
      sw!.click()
    })
    await settle()
    expect(api.catalogSetEnabled).toHaveBeenCalledWith(false)
    expect(host.querySelector('[data-testid="catalog-status-line"]')).toBeNull()
  })

  it('hides the catalog toggle when not connected to Alvanista', async () => {
    api.catalogStatus.mockResolvedValue({ ...CONNECTED_STATUS, connected: false })
    await mount()
    expect(host.querySelector('[data-testid="catalog-section"]')).toBeNull()
  })
})
