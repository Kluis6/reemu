// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SyncConflict } from '../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  loadGame: vi.fn(),
  unloadGame: vi.fn(),
  nativeVideoActive: vi.fn(),
  nativeVideoOverlay: vi.fn(),
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
import { PlayScreen } from './PlayScreen'

const KEY = 'file:gba/Zelda.srm'
const CONFLICT: SyncConflict = {
  key: KEY,
  romId: 'r1',
  title: 'Zelda',
  local: { machine: 'PC-ED', deviceId: 'dev1', savedAt: 1_700_000_000, size: 2048, deleted: false },
  remote: { machine: 'Notebook-Ed', deviceId: 'dev2', savedAt: 1_700_003_600, size: 4096, deleted: false },
}
const LAUNCH = { coreId: 'mgba_libretro', romPath: 'C:/roms/Zelda.gba', title: 'Zelda', system: 'gba' }
/** O que o `load_game` devolve quando a checagem da nuvem acha o conflito. */
const SYNC_CONFLICT = { code: 'syncConflict', message: 'o save deste jogo mudou aqui e em outra máquina' }

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

/** A tela de jogo aberta pela página do jogo (`/rom/r1` antes no histórico). */
async function mount(search = '') {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  await act(async () =>
    root.render(
      <QueryClientProvider client={qc}>
        <MemoryRouter initialEntries={['/rom/r1', { pathname: '/play/r1', search, state: LAUNCH }]} initialIndex={1}>
          <Where />
          <Routes>
            <Route path="/rom/:romId" element={<div>página do jogo</div>} />
            <Route path="/play/:romId" element={<PlayScreen />} />
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
/** O `decideLater` de cada chamada do `load_game`. */
const decideLater = () => api.loadGame.mock.calls.map((c) => c[3])

async function click(text: string) {
  await act(async () => byText(text)!.click())
  await settle()
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.unloadGame.mockResolvedValue(undefined)
  api.nativeVideoActive.mockResolvedValue(false)
  api.nativeVideoOverlay.mockResolvedValue(false)
  api.syncSaveStatus.mockResolvedValue({ state: 'conflict', syncedAt: null, versions: 2, key: KEY })
  api.syncConflicts.mockResolvedValue([CONFLICT])
  api.syncResolve.mockResolvedValue(undefined)
  // A primeira tentativa para no conflito; a seguinte fica abrindo.
  api.loadGame.mockRejectedValueOnce(SYNC_CONFLICT).mockReturnValue(new Promise(() => {}))
})
afterEach(async () => {
  await unmount()
  host.remove()
})

describe('PlayScreen with a game save conflict found before launch', () => {
  it('shows the conflict dialog, and Decidir depois opens the game with the check skipped', async () => {
    await mount()
    expect(api.loadGame).toHaveBeenCalledWith('mgba_libretro', 'C:/roms/Zelda.gba', 'r1', false)
    expect(api.syncSaveStatus).toHaveBeenCalledWith('r1')
    expect(dialog()?.textContent).toContain('Zelda mudou nas duas máquinas')
    expect(document.querySelector('[data-launch-ask]')).not.toBeNull()

    await click('Decidir depois')
    expect(api.syncResolve).not.toHaveBeenCalled()
    expect(decideLater()).toEqual([false, true])
    expect(dialog()).toBeNull()
    expect(where()).toBe('/play/r1')
  })

  it('choosing a side resolves it and opens again with the check', async () => {
    await mount()
    await click('Manter a de Notebook-Ed')
    expect(api.syncResolve).toHaveBeenCalledWith(KEY, 'remote')
    expect(decideLater()).toEqual([false, false])
    expect(dialog()).toBeNull()
  })

  it('closing the dialog gives up on opening and goes back', async () => {
    await mount()
    await click('fechar')
    expect(where()).toBe('/rom/r1')
    expect(api.loadGame).toHaveBeenCalledTimes(1)
    expect(api.syncResolve).not.toHaveBeenCalled()
  })

  it('Decidir depois on the game page is not asked again here', async () => {
    api.loadGame.mockReset().mockReturnValue(new Promise(() => {}))
    await mount('?core=mgba_libretro&sync=later')
    expect(decideLater()).toEqual([true])
    expect(dialog()).toBeNull()
  })

  it('without the conflict to show, the game opens with this machine save', async () => {
    api.syncSaveStatus.mockResolvedValue({ state: 'pending', syncedAt: null, versions: 2, key: KEY })
    await mount()
    expect(dialog()).toBeNull()
    expect(decideLater()).toEqual([false, true])
  })
})
