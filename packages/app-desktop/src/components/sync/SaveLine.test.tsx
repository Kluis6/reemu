// @vitest-environment happy-dom
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SaveLineStatus } from '../../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  syncStatus: vi.fn(),
  syncSaveStatus: vi.fn(),
  syncConflicts: vi.fn(),
  syncVersions: vi.fn(),
  syncRestore: vi.fn(),
}))
vi.mock('../../lib/tauri', async (orig) => ({ ...(await orig<typeof import('../../lib/tauri')>()), ...api }))
// O Dialog do Fluent não monta no happy-dom desta árvore ("presenceFn is not a
// function"): o diálogo de tela cheia vira uma camada simples.
vi.mock('../FullscreenDialog', () => ({
  FullscreenDialog: (p: { open: boolean; title: string; children: React.ReactNode }) =>
    p.open ? (
      <div className="fui-DialogSurface" role="dialog">
        <h2>{p.title}</h2>
        {p.children}
      </div>
    ) : null,
}))

import '../../i18n'
import { SaveLine } from './SaveLine'
import { STATUS, button, mountWithQuery, settle } from './syncTestSupport'

const line = (over: Partial<SaveLineStatus>): SaveLineStatus => ({
  state: 'synced',
  syncedAt: Math.floor(Date.now() / 1000) - 5 * 60,
  versions: 0,
  key: 'file:gba/Zelda.srm',
  ...over,
})

let ui: Awaited<ReturnType<typeof mountWithQuery>>
const mount = async () => {
  ui = await mountWithQuery(<SaveLine romId="r1" />)
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.syncStatus.mockResolvedValue(STATUS)
  api.syncVersions.mockResolvedValue([])
})
afterEach(async () => {
  await ui.unmount()
})

describe('SaveLine', () => {
  it('synced: says when', async () => {
    api.syncSaveStatus.mockResolvedValue(line({ state: 'synced' }))
    await mount()
    expect(ui.host.textContent).toContain('Game save sincronizado')
    expect(ui.host.textContent).toContain('5 minutos')
  })

  it('pending: not uploaded yet', async () => {
    api.syncSaveStatus.mockResolvedValue(line({ state: 'pending', syncedAt: null }))
    await mount()
    expect(ui.host.textContent).toContain('Game save ainda não enviado')
  })

  it('conflict: names both machines from syncConflicts', async () => {
    api.syncSaveStatus.mockResolvedValue(line({ state: 'conflict', syncedAt: null }))
    const side = (machine: string) => ({ machine, savedAt: 1, size: 1 })
    api.syncConflicts.mockResolvedValue([
      { key: 'outra', romId: null, title: 'X', local: side('A'), remote: side('B') },
      { key: 'file:gba/Zelda.srm', romId: 'r1', title: 'Zelda', local: side('PC-ED'), remote: side('Notebook-Ed') },
    ])
    await mount()
    expect(ui.host.textContent).toContain('Conflito: PC-ED e Notebook-Ed mudaram')
  })

  it('local_only: only on this machine', async () => {
    api.syncSaveStatus.mockResolvedValue(line({ state: 'local_only', syncedAt: null }))
    await mount()
    expect(ui.host.textContent).toContain('Só nesta máquina')
  })

  it('none renders nothing', async () => {
    api.syncSaveStatus.mockResolvedValue(line({ state: 'none', syncedAt: null, key: null }))
    await mount()
    expect(ui.host.textContent).toBe('')
  })

  it('renders nothing and asks nothing with the game saves switch off, or sync off', async () => {
    api.syncStatus.mockResolvedValue({ ...STATUS, savesEnabled: false })
    api.syncSaveStatus.mockResolvedValue(line({ state: 'synced' }))
    await mount()
    expect(ui.host.textContent).toBe('')
    expect(api.syncSaveStatus).not.toHaveBeenCalled()
    await ui.unmount()

    api.syncStatus.mockResolvedValue({ ...STATUS, enabled: false })
    await mount()
    expect(ui.host.textContent).toBe('')
    expect(api.syncSaveStatus).not.toHaveBeenCalled()
  })

  it('shows the history link only with a key and versions, and opens the history', async () => {
    api.syncSaveStatus.mockResolvedValue(line({ versions: 0 }))
    await mount()
    expect(button('histórico')).toBeUndefined()
    await ui.unmount()

    api.syncSaveStatus.mockResolvedValue(line({ versions: 3, key: null }))
    await mount()
    expect(button('histórico')).toBeUndefined()
    await ui.unmount()

    api.syncSaveStatus.mockResolvedValue(line({ versions: 3 }))
    api.syncVersions.mockResolvedValue([
      { versionId: 'v2', machine: 'PC-ED', createdAt: 1_700_000_000, size: 2048, isCurrent: true },
    ])
    await mount()
    expect(button('histórico (3 versões)')).toBeTruthy()
    await act(async () => button('histórico')!.click())
    await settle()
    expect(api.syncVersions).toHaveBeenCalledWith('file:gba/Zelda.srm')
    expect(document.querySelector('.fui-DialogSurface')?.textContent).toContain('Histórico do game save')
  })
})
