// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { SyncEvent } from '../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({ onSyncEvent: vi.fn() }))
vi.mock('../lib/tauri', async (orig) => ({ ...(await orig<typeof import('../lib/tauri')>()), ...api }))

import '../i18n'
import { useToastStore } from '../stores/useToastStore'
import { useSyncEvents } from './useSync'

function Probe() {
  useSyncEvents()
  return null
}

let emit: (e: SyncEvent) => void
let host: HTMLDivElement
let unmount: () => Promise<void>

beforeEach(async () => {
  useToastStore.setState({ queue: [] })
  api.onSyncEvent.mockReset().mockImplementation(async (cb: (e: SyncEvent) => void) => {
    emit = cb
    return () => {}
  })
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () =>
    root.render(
      <QueryClientProvider client={new QueryClient()}>
        <Probe />
      </QueryClientProvider>,
    ),
  )
  unmount = () => act(async () => root.unmount())
})
afterEach(async () => {
  await unmount()
  host.remove()
})

const messages = () => useToastStore.getState().queue.map((t) => t.message)

describe('useSyncEvents toast', () => {
  it('a pass that uploaded saves pushes a success toast, singular and plural', async () => {
    await act(async () => emit({ type: 'pass', uploaded: 1, downloaded: 0, backedUp: 0 }))
    await act(async () => emit({ type: 'pass', uploaded: 3, downloaded: 0, backedUp: 0 }))
    expect(messages()).toEqual([
      '1 game save enviado para a sua conta.',
      '3 game saves enviados para a sua conta.',
    ])
    expect(useToastStore.getState().queue.every((t) => t.variant === 'Success')).toBe(true)
  })

  it('a pass that moved site deletions to the backup folder says so (S13)', async () => {
    await act(async () => emit({ type: 'pass', uploaded: 0, downloaded: 0, backedUp: 1 }))
    await act(async () => emit({ type: 'pass', uploaded: 0, downloaded: 0, backedUp: 2 }))
    expect(messages()).toEqual([
      '1 game save apagado no site foi para a pasta de resguardo.',
      '2 game saves apagados no site foram para a pasta de resguardo.',
    ])
  })

  it('a launch that could not check the cloud says so; other launches stay silent (M4)', async () => {
    await act(async () => emit({ type: 'prelaunch', outcome: 'skipped', reason: 'timeout' }))
    await act(async () => emit({ type: 'prelaunch', outcome: 'skipped', reason: 'offline' }))
    await act(async () => emit({ type: 'prelaunch', outcome: 'skipped', reason: 'saves_off' }))
    await act(async () => emit({ type: 'prelaunch', outcome: 'skipped', reason: 'no_account' }))
    await act(async () => emit({ type: 'prelaunch', outcome: 'downloaded', reason: null }))
    await act(async () => emit({ type: 'prelaunch', outcome: 'up_to_date', reason: null }))
    const cloud = 'Não deu para conferir a nuvem a tempo. O jogo abriu com o save desta máquina.'
    expect(messages()).toEqual([cloud, cloud])
    expect(useToastStore.getState().queue.every((t) => t.variant === 'Warning')).toBe(true)
  })

  it('a pass that only downloaded, or did nothing, stays silent', async () => {
    await act(async () => emit({ type: 'pass', uploaded: 0, downloaded: 2, backedUp: 0 }))
    await act(async () => emit({ type: 'pass', uploaded: 0, downloaded: 0, backedUp: 0 }))
    expect(messages()).toEqual([])
  })
})
