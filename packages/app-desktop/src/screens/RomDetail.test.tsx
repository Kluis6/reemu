// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { InstalledCore, RemoteState, RomEntry, SaveState, SyncConflict } from '../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  listRoms: vi.fn(),
  listInstalledCores: vi.fn(),
  syncStatus: vi.fn(),
  syncSaveStatus: vi.fn(),
  syncConflicts: vi.fn(),
  syncResolve: vi.fn(),
  listSaveStates: vi.fn(),
  syncStatesFor: vi.fn(),
  syncDownloadState: vi.fn(),
  syncStateNotes: vi.fn(),
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
import { useToastStore } from '../stores/useToastStore'
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
  local: { machine: 'PC-ED', deviceId: 'dev1', savedAt: 1_700_000_000, size: 2048, deleted: false },
  remote: { machine: 'Notebook-Ed', deviceId: 'dev2', savedAt: 1_700_003_600, size: 4096, deleted: false },
}

function Where() {
  const loc = useLocation()
  return (
    <div data-testid="where" data-search={loc.search}>
      {loc.pathname}
    </div>
  )
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
  api.listSaveStates.mockResolvedValue([])
  api.syncStatesFor.mockResolvedValue({ states: [], offline: false, installedVersion: null })
  api.syncStateNotes.mockResolvedValue([])
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

  it('state none (sync off, revoked or saves off) launches straight, and the page never calls syncStatus', async () => {
    api.syncSaveStatus.mockResolvedValue({ state: 'none', syncedAt: null, versions: 0, key: null })
    await mount()
    await play()
    expect(api.syncSaveStatus).toHaveBeenCalledWith('r1')
    expect(api.syncStatus).not.toHaveBeenCalled()
    expect(where()).toBe('/play/r1')
  })

  it('a second Jogar while the check runs is ignored', async () => {
    let finish!: (v: unknown) => void
    api.syncSaveStatus.mockReturnValue(new Promise((r) => (finish = r)))
    await mount()
    // a linha do jogo já fez a sua consulta ao montar
    const before = api.syncSaveStatus.mock.calls.length
    await act(async () => byText('Jogar')!.click())
    await act(async () => byText('Jogar')!.click())
    expect(api.syncSaveStatus.mock.calls.length - before).toBe(1)
    await act(async () => finish({ state: 'conflict', syncedAt: null, versions: 2, key: KEY }))
    await settle()
    expect(dialog()).not.toBeNull()
  })
})

const remote = (over: Partial<RemoteState>): RemoteState => ({
  id: 's1',
  machine: 'Notebook',
  slot: 1,
  createdAt: 1_700_000_000,
  core: 'mgba_libretro',
  coreVersion: '0.10.1',
  available: true,
  reason: null,
  hasThumbnail: false,
  size: 10,
  ...over,
})
const SAVED: SaveState = {
  id: 'local-1',
  slot: 1,
  createdAt: 1,
  filePath: 'x',
  hasThumbnail: false,
  playTimeAtSave: null,
  coreVersion: '0.10.1',
  versionChanged: false,
}
const OTHER_BUILD = remote({
  id: 's2',
  machine: 'Deck',
  slot: 2,
  coreVersion: '0.10.0',
  available: false,
  reason: 'other_version',
})

async function openStates() {
  await act(async () => byText('Save states')!.click())
  await settle()
}
const rows = (kind: 'available' | 'unavailable') =>
  [...document.querySelectorAll<HTMLElement>(`[data-remote-state="${kind}"]`)]

describe('RomDetail states from other machines', () => {
  beforeEach(() => {
    api.syncStatesFor.mockResolvedValue({
      states: [remote({}), OTHER_BUILD],
      offline: false,
      installedVersion: '0.10.1',
    })
    api.syncDownloadState.mockResolvedValue(SAVED)
    api.syncSaveStatus.mockResolvedValue({ state: 'synced', syncedAt: 1, versions: 1, key: KEY })
  })

  it('shows both kinds of row, and the dimmed one has no button', async () => {
    await mount()
    await openStates()
    expect(api.syncStatesFor).toHaveBeenCalledWith('r1', 'mgba_libretro')
    const [ok] = rows('available')
    expect(ok.textContent).toContain('Notebook')
    expect(ok.textContent).toContain('mesmo core e versão')
    expect([...ok.querySelectorAll('button')].map((b) => b.textContent)).toEqual(['Baixar e carregar'])
    const [dim] = rows('unavailable')
    expect(dim.textContent).toContain('mgba_libretro 0.10.0 em Deck, 0.10.1 aqui')
    expect(dim.querySelectorAll('button, a, input')).toHaveLength(0)
    expect(document.querySelectorAll('[data-remote-state]')).toHaveLength(2)
  })

  it('download and load: downloads into the slot and launches with that state', async () => {
    await mount()
    await openStates()
    await act(async () => byText('Baixar e carregar')!.click())
    await settle()
    expect(api.syncDownloadState).toHaveBeenCalledWith('r1', 'mgba_libretro', 's1', false)
    expect(where()).toBe('/play/r1')
    expect(host.querySelector('[data-testid="where"]')!.getAttribute('data-search')).toContain('loadState=local-1')
  })

  it('an occupied slot asks first: Cancelar keeps everything, Substituir retries with overwrite', async () => {
    api.syncDownloadState.mockRejectedValueOnce('slot_occupied')
    await mount()
    await openStates()
    await act(async () => byText('Baixar e carregar')!.click())
    await settle()
    expect(document.body.textContent).toContain('O state de agora deste slot vai para a pasta de resguardo.')
    expect(api.syncDownloadState).toHaveBeenCalledTimes(1)
    // o foco cai no Cancelar, o lado seguro
    expect(document.activeElement?.textContent).toBe('Cancelar')

    await act(async () => byText('Cancelar')!.click())
    await settle()
    expect(document.body.textContent).not.toContain('vai para a pasta de resguardo')
    expect(where()).toBe('/game/r1')
    expect(api.syncDownloadState).toHaveBeenCalledTimes(1)
    // o foco volta ao botão da linha
    expect(document.activeElement?.textContent).toBe('Baixar e carregar')

    api.syncDownloadState.mockRejectedValueOnce('slot_occupied')
    await act(async () => byText('Baixar e carregar')!.click())
    await settle()
    await act(async () => byText('Substituir')!.click())
    await settle()
    expect(api.syncDownloadState).toHaveBeenLastCalledWith('r1', 'mgba_libretro', 's1', true)
    expect(where()).toBe('/play/r1')
  })

  it('a download that fails stays on the page and says why', async () => {
    api.syncDownloadState.mockRejectedValueOnce('hash_mismatch')
    await mount()
    await openStates()
    await act(async () => byText('Baixar e carregar')!.click())
    await settle()
    expect(where()).toBe('/game/r1')
    expect(byText('Baixar e carregar')).toBeDefined()
    const texts = useToastStore.getState().queue.map((q) => q.message)
    expect(texts).toContain('O state chegou incompleto. Nada foi trocado. Tente de novo.')
  })

  it('a known remote version with an unknown local one says so, and a core that is not chosen asks nothing', async () => {
    api.syncStatesFor.mockResolvedValue({
      states: [{ ...OTHER_BUILD, reason: 'unversioned' }],
      offline: false,
      installedVersion: null,
    })
    await mount()
    await openStates()
    const [dim] = rows('unavailable')
    expect(dim.textContent).toContain('mgba_libretro 0.10.0 em Deck, versão daqui desconhecida')
    expect(dim.textContent).not.toContain('sem a versão do core registrada')
    await unmount()
    host.remove()

    api.syncStatesFor.mockClear()
    api.listInstalledCores.mockResolvedValue([])
    await mount()
    await openStates()
    expect(api.syncStatesFor).not.toHaveBeenCalled()
  })

  it('asks nothing with the states switch off, offline or with no rows', async () => {
    api.syncStatus.mockResolvedValue({ ...STATUS, statesEnabled: false })
    await mount()
    await openStates()
    expect(api.syncStatesFor).not.toHaveBeenCalled()
    expect(document.querySelectorAll('[data-remote-state]')).toHaveLength(0)
    await unmount()
    host.remove()

    api.syncStatus.mockResolvedValue(STATUS)
    api.syncStatesFor.mockResolvedValue({ states: [], offline: true, installedVersion: null })
    await mount()
    await openStates()
    expect(api.syncStatesFor).toHaveBeenCalled()
    expect(document.querySelectorAll('[data-remote-state]')).toHaveLength(0)
    expect(document.body.textContent).not.toContain('Em outras máquinas')
  })

  it('paused on the site, the other machines are not asked (D128)', async () => {
    api.syncStatus.mockResolvedValue({ ...STATUS, pausedSite: true })
    await mount()
    await openStates()
    expect(api.syncStatesFor).not.toHaveBeenCalled()
    expect(document.querySelectorAll('[data-remote-state]')).toHaveLength(0)
  })

  it('a download that ends after the person started the game does not launch it again', async () => {
    let finish!: (s: SaveState) => void
    api.syncDownloadState.mockReturnValue(new Promise<SaveState>((r) => (finish = r)))
    await mount()
    await openStates()
    await act(async () => byText('Baixar e carregar')!.click())
    // "Jogar" segue na página: o jogo abre com o save de agora.
    await play()
    expect(where()).toBe('/play/r1')
    const search = () => host.querySelector('[data-testid="where"]')!.getAttribute('data-search')
    expect(search()).not.toContain('loadState')
    await act(async () => finish(SAVED))
    await settle()
    expect(where()).toBe('/play/r1')
    expect(search()).not.toContain('loadState')
    expect(api.syncSaveStatus).toHaveBeenCalledTimes(2)
  })

  it('closing the panel during a download leaves the page where it is', async () => {
    let finish!: (s: SaveState) => void
    api.syncDownloadState.mockReturnValue(new Promise<SaveState>((r) => (finish = r)))
    await mount()
    await openStates()
    await act(async () => byText('Baixar e carregar')!.click())
    await act(async () => byText('fechar')!.click())
    await settle()
    await act(async () => finish(SAVED))
    await settle()
    expect(where()).toBe('/game/r1')
  })
})

const local = (over: Partial<SaveState>): SaveState => ({ ...SAVED, ...over })

describe('RomDetail local states that stay on this machine', () => {
  beforeEach(() => {
    api.syncSaveStatus.mockResolvedValue({ state: 'synced', syncedAt: 1, versions: 1, key: KEY })
    api.listSaveStates.mockResolvedValue([
      local({ id: 'old', slot: 1, coreVersion: null }),
      local({ id: 'big', slot: 2 }),
      local({ id: 'bad', slot: 3 }),
      local({ id: 'ok', slot: 4 }),
    ])
    api.syncStateNotes.mockResolvedValue([
      { stateId: 'big', reason: 'too_large' },
      { stateId: 'bad', reason: 'rejected' },
      { stateId: 'ok', reason: 'wiped' },
    ])
  })
  const notes = () => [...document.querySelectorAll<HTMLElement>('[data-state-note]')].map((n) => n.textContent)

  it('each slot says why it stays here: no core version, too large, refused', async () => {
    await mount()
    await openStates()
    expect(api.syncStateNotes).toHaveBeenCalledWith('r1')
    // Na ordem das vagas; o apagado no site (o slot 4) não ganha nota aqui.
    expect(notes()).toEqual([
      'Feito antes de o ReEmu registrar a versão do core: fica só nesta máquina.',
      'Grande demais para a nuvem: fica só nesta máquina.',
      'A nuvem recusou este state: fica só nesta máquina.',
    ])
  })

  it('says nothing with the states switch off or sync off, and asks nothing', async () => {
    api.syncStatus.mockResolvedValue({ ...STATUS, statesEnabled: false })
    await mount()
    await openStates()
    expect(notes()).toEqual([])
    expect(api.syncStateNotes).not.toHaveBeenCalled()
    await unmount()
    host.remove()

    api.syncStatus.mockResolvedValue({ ...STATUS, enabled: false })
    await mount()
    await openStates()
    expect(notes()).toEqual([])
    expect(api.syncStateNotes).not.toHaveBeenCalled()
  })
})
