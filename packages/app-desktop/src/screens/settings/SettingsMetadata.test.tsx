// @vitest-environment happy-dom

import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterAll, afterEach, beforeAll, beforeEach, expect, it, vi } from 'vitest'
import type { ScrapeProgress } from '../../lib/tauri'

const api = vi.hoisted(() => ({
  getMetadataConfig: vi.fn(),
  metadataScanProgress: vi.fn(),
  listPendingMatches: vi.fn(),
  startMetadataScan: vi.fn(),
  cancelMetadataScan: vi.fn(),
}))
vi.mock('../../lib/tauri', async (original) => ({ ...(await original<object>()), ...api }))

import i18n from '../../i18n'
import { SettingsMetadata } from './SettingsMetadata'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true
const empty = { running: false, status: 'idle', done: 0, total: 0, auto: 0, pending: 0, failed: 0, error: null } satisfies ScrapeProgress
let host: HTMLDivElement
let root: ReturnType<typeof createRoot>
let qc: QueryClient

// O happy-dom rejeita a animação do ProgressBar ao sair do modo indeterminado.
const animate = Element.prototype.animate
beforeAll(() => {
  ;(Element.prototype as { animate?: unknown }).animate = undefined
})
afterAll(() => {
  Element.prototype.animate = animate
})

beforeEach(async () => {
  vi.resetAllMocks()
  await i18n.changeLanguage('pt-BR')
  api.getMetadataConfig.mockResolvedValue({
    provider: 'screenscraper', screenscraperUser: null, screenscraperPassword: null, thegamesdbApiKey: null,
  })
  api.listPendingMatches.mockResolvedValue([])
  api.metadataScanProgress.mockResolvedValue(empty)
  host = document.createElement('div')
  document.body.append(host)
  root = createRoot(host)
  qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  await act(async () => root.render(<QueryClientProvider client={qc}><SettingsMetadata /></QueryClientProvider>))
  await waitForStatus(() => expect(host.textContent).toContain('Escanear metadata da biblioteca'))
})

afterEach(async () => {
  await act(async () => root.unmount())
  qc.clear()
  host.remove()
})

async function waitForStatus(assert: () => void) {
  await vi.waitFor(async () => {
    await act(async () => new Promise((resolve) => setTimeout(resolve, 0)))
    assert()
  })
}

it('shows preparation and explains an empty queue even if the scan already finished', async () => {
  let complete!: () => void
  api.startMetadataScan.mockImplementation(() => new Promise<void>((resolve) => { complete = resolve }))
  const button = [...host.querySelectorAll('button')].find((item) => item.textContent?.includes('Escanear metadata'))!
  await act(async () => button.click())
  await waitForStatus(() => expect(host.textContent).toContain('Preparando a busca'))
  expect(host.querySelector('[role="progressbar"]')).not.toBeNull()
  api.metadataScanProgress.mockResolvedValue({ ...empty, status: 'completed' })
  await act(async () => complete())
  await waitForStatus(() => expect(host.textContent).toContain('Nenhum jogo na fila'))
  expect(host.querySelector('[role="progressbar"]')).toBeNull()
})

it('shows counts while running and keeps the cancelled result visible', async () => {
  const active = { ...empty, status: 'running', running: true, done: 2, total: 4, pending: 1 } satisfies ScrapeProgress
  await act(async () => { qc.setQueryData(['metadata-progress'], active) })
  await waitForStatus(() => expect(host.textContent).toContain('50%'))
  api.cancelMetadataScan.mockResolvedValue(undefined)
  api.metadataScanProgress.mockResolvedValue({ ...active, running: false, status: 'cancelled' })
  const button = [...host.querySelectorAll('button')].find((item) => item.textContent?.trim() === 'Cancelar')!
  await act(async () => button.click())
  await waitForStatus(() => expect(host.textContent).toContain('Busca cancelada.'))
  expect(host.textContent).toContain('2/4')
})


it('keeps provider results and scan progress in a separate Fluent tab from credentials', async () => {
  await act(async () => {
    qc.setQueryData(['pending-matches'], [{
      romId: 'one', fileStem: 'Sample ROM', title: 'Sample Game', provider: 'gamekave',
      description: null, coverUrl: null, releaseDate: '1992-06-01', genre: null,
    }])
  })
  await waitForStatus(() => expect(host.querySelector('#metadata-results')?.textContent).toContain('GameKave · 1992-06-01'))
  const results = host.querySelector<HTMLButtonElement>('#metadata-results-tab')!
  const credentials = host.querySelector<HTMLButtonElement>('#metadata-credentials-tab')!
  expect(results.getAttribute('aria-selected')).toBe('true')
  expect(host.querySelector('input[type="password"]')).toBeNull()

  await act(async () => credentials.click())
  expect(credentials.getAttribute('aria-selected')).toBe('true')
  expect(credentials.getAttribute('aria-controls')).toBe('metadata-credentials')
  expect(host.querySelector('#metadata-credentials')?.getAttribute('aria-labelledby')).toBe(credentials.id)
  expect(host.querySelectorAll('input[type="password"]')).toHaveLength(2)
  expect(host.querySelector('#metadata-results')?.hasAttribute('hidden')).toBe(true)
  expect(host.textContent).not.toContain('Sample Game')

  await act(async () => {
    qc.setQueryData(['metadata-progress'], { ...empty, status: 'running', running: true, done: 1, total: 2 })
  })
  await act(async () => results.click())
  await waitForStatus(() => expect(host.textContent).toContain('50%'))
  expect(host.textContent).toContain('Sample Game')
  expect(host.querySelector('input[type="password"]')).toBeNull()
})
