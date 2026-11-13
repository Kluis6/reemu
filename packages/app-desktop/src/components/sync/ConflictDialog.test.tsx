// @vitest-environment happy-dom
import { act } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SyncConflict } from '../../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({ syncResolve: vi.fn(), syncStatus: vi.fn() }))
vi.mock('../../lib/tauri', async (orig) => ({ ...(await orig<typeof import('../../lib/tauri')>()), ...api }))
// O Dialog do Fluent não monta no happy-dom desta árvore ("presenceFn is not a
// function"): o diálogo de tela cheia vira uma camada simples.
vi.mock('../FullscreenDialog', () => ({
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

import '../../i18n'
import { ConflictDialog } from './ConflictDialog'
import { button, mountWithQuery, settle } from './syncTestSupport'

const CONFLICT: SyncConflict = {
  key: 'file:gba/Zelda.srm',
  romId: 'r1',
  title: 'Zelda',
  local: { machine: 'PC-ED', savedAt: 1_700_000_000, size: 2048 },
  remote: { machine: 'Notebook-Ed', savedAt: 1_700_003_600, size: 4096 },
}

let ui: Awaited<ReturnType<typeof mountWithQuery>>
const onDone = vi.fn()
const mount = async () => {
  ui = await mountWithQuery(<ConflictDialog conflict={CONFLICT} onDone={onDone} />)
}

beforeEach(() => {
  api.syncResolve.mockReset().mockResolvedValue(undefined)
  api.syncStatus.mockReset().mockRejectedValue('no_account')
  onDone.mockReset()
})
afterEach(async () => {
  await ui.unmount()
})

describe('ConflictDialog', () => {
  it('shows the sentence, both cards and the reassurance footer', async () => {
    await mount()
    const text = document.body.textContent ?? ''
    expect(text).toContain('Zelda mudou nas duas máquinas')
    expect(text).toContain(
      'As duas jogaram desde a última sincronização. Escolha qual save continua. A outra fica no histórico e numa pasta de resguardo nesta máquina.',
    )
    expect(text).toContain('PC-ED')
    expect(text).toContain('Notebook-Ed')
    expect(text).toContain('Salvo')
    expect(text).toContain('Tamanho')
    expect(text).toContain('2 KB')
    expect(text).toContain('4 KB')
    expect(text).toContain('Nada é apagado. Dá para voltar pelo histórico.')
    expect(button('Manter a de Notebook-Ed')).toBeTruthy()
    expect(button('Decidir depois')).toBeTruthy()
  })

  it('starts on the local card button, with the controller alone', async () => {
    await mount()
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10))
    })
    expect(document.activeElement).toBe(button('Manter esta'))
  })

  it('Manter esta resolves with local, then onDone(local)', async () => {
    await mount()
    await act(async () => button('Manter esta')!.click())
    await settle()
    expect(api.syncResolve).toHaveBeenCalledWith('file:gba/Zelda.srm', 'local')
    expect(onDone).toHaveBeenCalledWith('local')
  })

  it('Manter a de X resolves with remote', async () => {
    await mount()
    await act(async () => button('Manter a de Notebook-Ed')!.click())
    await settle()
    expect(api.syncResolve).toHaveBeenCalledWith('file:gba/Zelda.srm', 'remote')
    expect(onDone).toHaveBeenCalledWith('remote')
  })

  it('Decidir depois resolves nothing and reports later; the close button only dismisses', async () => {
    await mount()
    await act(async () => button('Decidir depois')!.click())
    expect(onDone).toHaveBeenCalledWith('later')
    await act(async () => button('fechar')!.click())
    expect(onDone).toHaveBeenLastCalledWith('dismiss')
    expect(api.syncResolve).not.toHaveBeenCalled()
  })

  it('a failed resolve keeps the dialog and shows the reason', async () => {
    api.syncResolve.mockRejectedValue('offline')
    await mount()
    await act(async () => button('Manter esta')!.click())
    await settle()
    expect(onDone).not.toHaveBeenCalled()
    expect(document.body.textContent).toContain('Não foi possível falar com o Alvanista')
  })
})
