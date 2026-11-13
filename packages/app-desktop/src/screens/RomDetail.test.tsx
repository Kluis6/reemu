// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { InstalledCore, RomEntry, SyncConflict } from '../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  listRoms: vi.fn(),
  listInstalledCores: vi.fn(),
  syncStatus: vi.fn(),
  syncSaveStatus: vi.fn(),
  syncConflicts: vi.fn(),
  syncResolve: vi.fn(),
}))
vi.mock('../lib/tauri', async (orig) => ({ ...(await orig<typeof import('../lib/tauri')>()), ...api }))
// O Dialog do Fluent não monta no happy-dom desta árvore ("presenceFn is not a
// function"): o diálogo de tela cheia vira uma camada simples.
vi.mock('../components/FullscreenDialog', () => ({
  FullscreenDialog: (p: {
    open: boolean
    onClose: () => void
    title: string
    children: React.ReactNode
    actions?: React.ReactNode
  }) =>
    p.open ? (
      <div className="fui-DialogSurface" role="dialog">
        <h2>{p.title}</h2>
        <button onClick={p.onClose}>fechar</button>
        {p.children}
        {p.actions}
      </div>
    ) : null,
}))

import '../i18n'
import { STATUS } from '../components/sync/syncTestSupport'
import { RomDetail } from './RomDetail'

const ROM: RomEntry = {
  id: 'r1',
  title: 'Zelda',
  systemId: 'gba',
  filePath: 'C:/roms/Zelda.gba',
  boxart: null,
  lastPlayedAt: null,
  addedAt: 1,
  isFavorite: false,
  playTimeSecs: 0,
}
const CORE: InstalledCore = {
  coreId: 'mgba_libretro',
  name: 'mGBA',
  version: '1',
  extensions: ['gba'],
  renderBackend: null,
  systems: ['gba'],
}
const KEY = 'file:gba/Zelda.srm'
const CONFLICT: SyncConflict = {
  key: KEY,
  romId: 'r1',
  title: 'Zelda',
  local: { machine: 'PC-ED', savedAt: 1_700_000_000, size: 2048 },
  remote: { machine: 'Notebook-Ed', savedAt: 1_700_003_600, size: 4096 },
}

function Where() {
  return <div data-testid="where">{useLocation().pathname}</div>
}

let host: HTMLDivElement
let unmount: () => Promise<void>

async function settle() {
  for (let i = 0; i < 8; i++) {
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
        <MemoryRouter initialEntries={['/game/r1']}>
          <Where />
          <Routes>
            <Route path="/game/:romId" element={<RomDetail />} />
            <Route path="/play/:romId" element={<div>jogando</div>} />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

const where = () => host.querySelector('[data-testid="where"]')!.textContent
const byText = (text: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === text)
const dialog = () => document.querySelector('.fui-DialogSurface')

async function play() {
  await act(async () => byText('Jogar')!.click())
  await settle()
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.listRoms.mockResolvedValue([ROM])
  api.listInstalledCores.mockResolvedValue([CORE])
  api.syncStatus.mockResolvedValue(STATUS)
  api.syncResolve.mockResolvedValue(undefined)
  api.syncConflicts.mockResolvedValue([CONFLICT])
  api.syncSaveStatus.mockResolvedValue({ state: 'conflict', syncedAt: null, versions: 2, key: KEY })
})
afterEach(async () => {
  await unmount()
  host.remove()
})

describe('RomDetail play with a game save conflict', () => {
  it('play asks before launching a conflicted game, and Decidir depois launches anyway', async () => {
    await mount()
    await play()
    expect(api.syncSaveStatus).toHaveBeenCalledWith('r1')
    expect(where()).toBe('/game/r1')
    expect(dialog()?.textContent).toContain('Zelda mudou nas duas máquinas')
    await act(async () => byText('Decidir depois')!.click())
    await settle()
    expect(api.syncResolve).not.toHaveBeenCalled()
    expect(where()).toBe('/play/r1')
  })

  it('choosing a side resolves it and then launches', async () => {
    await mount()
    await play()
    await act(async () => byText('Manter esta')!.click())
    await settle()
    expect(api.syncResolve).toHaveBeenCalledWith(KEY, 'local')
    expect(where()).toBe('/play/r1')
  })

  it('closing the dialog gives up on launching', async () => {
    await mount()
    await play()
    await act(async () => byText('fechar')!.click())
    await settle()
    expect(where()).toBe('/game/r1')
    expect(dialog()).toBeNull()
  })

  it('launches straight when there is no conflict', async () => {
    api.syncSaveStatus.mockResolvedValue({ state: 'synced', syncedAt: 1, versions: 1, key: KEY })
    await mount()
    await play()
    expect(dialog()).toBeNull()
    expect(where()).toBe('/play/r1')
  })

  it('never blocks play: a failing check (no account, offline) launches normally', async () => {
    api.syncSaveStatus.mockRejectedValue('no_account')
    await mount()
    await play()
    expect(where()).toBe('/play/r1')
  })

  it('skips the check when sync or the game saves switch is off', async () => {
    api.syncStatus.mockResolvedValue({ ...STATUS, savesEnabled: false })
    await mount()
    await play()
    expect(api.syncSaveStatus).not.toHaveBeenCalled()
    expect(where()).toBe('/play/r1')
  })
})
