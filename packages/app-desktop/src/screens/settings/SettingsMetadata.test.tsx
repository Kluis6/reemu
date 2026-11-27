// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CatalogStatus, MetadataConfig, PendingMatch, ScrapeProgress } from '../../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  catalogStatus: vi.fn(),
  catalogSetEnabled: vi.fn(),
  catalogRunNow: vi.fn(),
  getMetadataConfig: vi.fn(),
  metadataScanProgress: vi.fn(),
  listPendingMatches: vi.fn(),
  resolvePendingMatch: vi.fn(),
  resolvePendingMatchesBulk: vi.fn(),
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

function makeMatch(idx: number, provider = 'gamekave'): PendingMatch {
  return {
    romId: `rom-${idx}`,
    fileStem: `File_${idx}`,
    provider,
    title: `Game Title ${idx}`,
    description: null,
    coverUrl: `https://example.com/cover-${idx}.png`,
    releaseDate: '1995-08-15',
    genre: 'adventure',
  }
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

const buttonByText = (text: string) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent?.includes(text))

function typeInto(input: HTMLInputElement, value: string) {
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  set.call(input, value)
  input.dispatchEvent(new Event('input', { bubbles: true }))
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.catalogStatus.mockResolvedValue(CONNECTED_STATUS)
  api.catalogSetEnabled.mockImplementation(async (enabled: boolean) => ({
    ...CONNECTED_STATUS,
    enabled,
  }))
  api.catalogRunNow.mockResolvedValue({ ...CONNECTED_STATUS, found: 15, pending: 0 })
  api.getMetadataConfig.mockResolvedValue(CFG)
  api.metadataScanProgress.mockResolvedValue(PROGRESS)
  api.listPendingMatches.mockResolvedValue([])
  api.resolvePendingMatch.mockResolvedValue(undefined)
  api.resolvePendingMatchesBulk.mockImplementation(async (ids: string[]) => ids.length)
})

afterEach(async () => {
  if (unmount) await unmount()
  host?.remove()
  sessionStorage.clear()
})

describe('SettingsMetadata catalog section', () => {
  it('status_line_shows_counts_and_syncs_catalog_on_demand', async () => {
    await mount()
    expect(host.textContent).toContain('Usar o catálogo do Alvanista para identificar meus jogos')
    expect(host.textContent).toContain(
      'Os nomes e os hashes dos arquivos vão para o Alvanista para identificar os jogos. Nada sobre você fica junto do pedido.',
    )
    const line = host.querySelector('[data-testid="catalog-status-line"]')
    expect(line?.textContent).toBe('12 identificados, 3 na fila, 1 sem resposta')

    const syncBtn = buttonByText('Sincronizar catálogo')
    expect(syncBtn).toBeTruthy()
    await act(async () => {
      syncBtn!.click()
    })
    await settle()
    expect(api.catalogRunNow).toHaveBeenCalledTimes(1)
    expect(host.querySelector('[data-testid="catalog-status-line"]')?.textContent).toBe(
      '15 identificados, 0 na fila, 1 sem resposta',
    )
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

  it('keeps role=tabpanel without tabindex=0 so focus rules never break scrolling', async () => {
    await mount()
    const panel = host.querySelector('[role="tabpanel"]')
    expect(panel).toBeTruthy()
    expect(panel?.getAttribute('tabindex')).toBeNull()
  })
})

describe('SettingsMetadata bulk actions and large queue pagination', () => {
  it('paginates DOM rows while approving all pending matches in one bulk call', async () => {
    const items = Array.from({ length: 75 }, (_, i) => makeMatch(i + 1))
    api.listPendingMatches.mockResolvedValue(items)
    await mount()

    expect(host.querySelectorAll('[data-testid="pending-match-row"]')).toHaveLength(50)
    expect(buttonByText('Mostrar mais (25 restantes)')).toBeTruthy()

    const approveAll = buttonByText('Aprovar todas (75)')
    expect(approveAll).toBeTruthy()
    api.listPendingMatches.mockResolvedValue([])
    await act(async () => {
      approveAll!.click()
    })
    await settle()

    expect(api.resolvePendingMatchesBulk).toHaveBeenCalledWith(
      items.map((m) => m.romId),
      true,
    )
    expect(host.querySelectorAll('[data-testid="pending-match-row"]')).toHaveLength(0)
  })

  it('supports row selection, search/provider filter, and two-step bulk rejection', async () => {
    const items = [
      { ...makeMatch(1, 'gamekave'), title: 'Chrono Trigger', fileStem: 'Chrono_Trigger_U' },
      { ...makeMatch(2, 'gamekave'), title: 'Super Metroid', fileStem: 'Super_Metroid_JU' },
      { ...makeMatch(3, 'thegamesdb'), title: 'Chrono Cross', fileStem: 'Chrono_Cross_Disc1' },
    ]
    api.listPendingMatches.mockResolvedValue(items)
    await mount()

    // Seleciona apenas o primeiro item e aprova selecionadas (1).
    const firstBox = host.querySelector<HTMLInputElement>(
      '[data-testid="pending-match-row"] input[type="checkbox"]',
    )!
    await act(async () => {
      firstBox.click()
    })
    expect(buttonByText('Aprovar selecionadas (1)')).toBeTruthy()

    await act(async () => {
      buttonByText('Limpar seleção')!.click()
    })
    expect(buttonByText('Aprovar todas (3)')).toBeTruthy()

    // Filtra por texto "Chrono" -> 2 itens filtrados; rejeita com confirmação em duas etapas.
    const searchInput = host.querySelector<HTMLInputElement>('.reemu-list-search input')!
    await act(async () => {
      typeInto(searchInput, 'Chrono')
    })
    await settle()

    expect(host.querySelectorAll('[data-testid="pending-match-row"]')).toHaveLength(2)
    const rejectFiltered = buttonByText('Rejeitar filtradas (2)')
    expect(rejectFiltered).toBeTruthy()
    await act(async () => {
      rejectFiltered!.click()
    })
    expect(api.resolvePendingMatchesBulk).not.toHaveBeenCalled()

    const confirmBtn = buttonByText('Confirmar rejeição (2)')
    expect(confirmBtn).toBeTruthy()
    await act(async () => {
      confirmBtn!.click()
    })
    await settle()

    expect(api.resolvePendingMatchesBulk).toHaveBeenCalledWith(['rom-1', 'rom-3'], false)
  })
})

describe('SettingsMetadata ScreenScraper warning behavior', () => {
  it('does not render ss_dev_login warning when ScreenScraper credentials are not configured', async () => {
    api.metadataScanProgress.mockResolvedValue({
      ...PROGRESS,
      running: true,
      status: 'running',
      done: 12,
      total: 100,
      error: 'ss_dev_login',
    })
    await mount()
    expect(host.querySelector('[data-testid="ss-dev-warning"]')).toBeNull()
  })

  it('allows dismissing ss_dev_login warning when ScreenScraper credentials are configured', async () => {
    api.getMetadataConfig.mockResolvedValue({
      ...CFG,
      screenscraperUser: 'edward',
      screenscraperPassword: 'secret',
    })
    api.metadataScanProgress.mockResolvedValue({
      ...PROGRESS,
      running: true,
      status: 'running',
      done: 12,
      total: 100,
      error: 'ss_dev_login',
    })
    await mount()

    const banner = host.querySelector('[data-testid="ss-dev-warning"]')
    expect(banner).toBeTruthy()
    const closeBtn = banner!.querySelector<HTMLButtonElement>('button[aria-label="Fechar"]')
    expect(closeBtn).toBeTruthy()

    await act(async () => {
      closeBtn!.click()
    })
    await settle()
    expect(host.querySelector('[data-testid="ss-dev-warning"]')).toBeNull()
    expect(sessionStorage.getItem('reemu.dismissSsDevWarning')).toBe('1')
  })
})
