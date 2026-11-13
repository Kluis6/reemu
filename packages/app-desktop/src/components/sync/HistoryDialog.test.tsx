// @vitest-environment happy-dom
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({ syncVersions: vi.fn(), syncRestore: vi.fn(), syncStatus: vi.fn() }))
vi.mock('../../lib/tauri', async (orig) => ({ ...(await orig<typeof import('../../lib/tauri')>()), ...api }))
// O Dialog do Fluent não monta no happy-dom desta árvore ("presenceFn is not a
// function"): o diálogo de tela cheia vira uma camada simples.
vi.mock('../FullscreenDialog', () => ({
  FullscreenDialog: (p: { open: boolean; onClose: () => void; title: string; children: React.ReactNode }) =>
    p.open ? (
      <div className="fui-DialogSurface" role="dialog">
        <h2>{p.title}</h2>
        <button onClick={p.onClose}>fechar</button>
        {p.children}
      </div>
    ) : null,
}))

import '../../i18n'
import { HistoryDialog } from './HistoryDialog'
import { button, buttons, mountWithQuery, settle } from './syncTestSupport'

const KEY = 'file:gba/Zelda.srm'
const VERSIONS = [
  { versionId: 'v3', machine: 'PC-ED', createdAt: 1_700_007_200, size: 2048, isCurrent: true },
  { versionId: 'v2', machine: 'Notebook-Ed', createdAt: 1_700_003_600, size: 1024, isCurrent: false },
  { versionId: 'v1', machine: 'PC-ED', createdAt: 1_700_000_000, size: 512, isCurrent: false },
]

let ui: Awaited<ReturnType<typeof mountWithQuery>>
const onClose = vi.fn()
const mount = async () => {
  ui = await mountWithQuery(<HistoryDialog romKey={KEY} onClose={onClose} />)
}

beforeEach(() => {
  api.syncVersions.mockReset().mockResolvedValue(VERSIONS)
  api.syncRestore.mockReset().mockResolvedValue(undefined)
  api.syncStatus.mockReset().mockRejectedValue('no_account')
  onClose.mockReset()
})
afterEach(async () => {
  await ui.unmount()
})

describe('HistoryDialog', () => {
  it('lists machine, date and size, with atual on the current version only', async () => {
    await mount()
    expect(api.syncVersions).toHaveBeenCalledWith(KEY)
    const text = document.body.textContent ?? ''
    expect(text).toContain('PC-ED ·')
    expect(text).toContain('Notebook-Ed ·')
    expect(text).toContain('2 KB')
    expect(text).toContain('1 KB')
    expect(text.match(/atual/g)).toHaveLength(1)
    expect(buttons().filter((b) => b.textContent === 'Restaurar esta')).toHaveLength(2)
  })

  it('asks before restoring, then restores the chosen version', async () => {
    await mount()
    await act(async () => button('Restaurar esta')!.click())
    expect(api.syncRestore).not.toHaveBeenCalled()
    expect(document.body.textContent).toContain('O save de agora vai para a pasta de resguardo.')
    await act(async () => buttons().find((b) => b.textContent === 'Restaurar')!.click())
    await settle()
    expect(api.syncRestore).toHaveBeenCalledWith(KEY, 'v2')
    expect(onClose).toHaveBeenCalled()
  })

  it('cancelling the confirmation restores nothing', async () => {
    await mount()
    await act(async () => button('Restaurar esta')!.click())
    await act(async () => button('Cancelar')!.click())
    expect(api.syncRestore).not.toHaveBeenCalled()
    expect(document.body.textContent).not.toContain('O save de agora vai para a pasta de resguardo.')
  })
})
