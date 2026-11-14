// @vitest-environment happy-dom
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { RemoteState, SaveState } from '../../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  syncStatus: vi.fn(),
  syncStatesFor: vi.fn(),
  syncDownloadState: vi.fn(),
}))
vi.mock('../../lib/tauri', async (orig) => ({ ...(await orig<typeof import('../../lib/tauri')>()), ...api }))

import '../../i18n'
import { RemoteStateList } from './RemoteStateList'
import { STATUS, button, mountWithQuery } from './syncTestSupport'

const remote: RemoteState = {
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
}
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

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.syncStatus.mockResolvedValue(STATUS)
  api.syncStatesFor.mockResolvedValue({ states: [remote], offline: false, installedVersion: '0.10.1' })
})
afterEach(() => {
  document.body.innerHTML = ''
})

describe('RemoteStateList', () => {
  it('a download that finishes after the list is gone still refreshes the state lists, and launches nothing', async () => {
    let finish!: (s: SaveState) => void
    api.syncDownloadState.mockReturnValue(new Promise<SaveState>((r) => (finish = r)))
    const onDownloaded = vi.fn()
    const ui = await mountWithQuery(<RemoteStateList romId="r1" coreId="mgba_libretro" onDownloaded={onDownloaded} />)
    const invalidate = vi.spyOn(ui.qc, 'invalidateQueries')
    await act(async () => button('Baixar e carregar')!.click())
    await ui.unmount()
    invalidate.mockClear()
    await act(async () => finish(SAVED))
    const keys = invalidate.mock.calls.map((c) => JSON.stringify(c[0]?.queryKey))
    expect(keys).toContain(JSON.stringify(['save-states', 'r1']))
    expect(keys).toContain(JSON.stringify(['sync-state-notes']))
    expect(onDownloaded).not.toHaveBeenCalled()
  })

  it('while mounted it refreshes the lists and hands the state over', async () => {
    api.syncDownloadState.mockResolvedValue(SAVED)
    const onDownloaded = vi.fn()
    const ui = await mountWithQuery(<RemoteStateList romId="r1" coreId="mgba_libretro" onDownloaded={onDownloaded} />)
    const invalidate = vi.spyOn(ui.qc, 'invalidateQueries')
    await act(async () => button('Baixar e carregar')!.click())
    expect(onDownloaded).toHaveBeenCalledWith(SAVED)
    expect(invalidate.mock.calls.map((c) => JSON.stringify(c[0]?.queryKey))).toContain(
      JSON.stringify(['save-states', 'r1']),
    )
    await ui.unmount()
  })
})
