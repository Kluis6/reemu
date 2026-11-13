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
    await act(async () => emit({ type: 'pass', uploaded: 1, downloaded: 0 }))
    await act(async () => emit({ type: 'pass', uploaded: 3, downloaded: 0 }))
    expect(messages()).toEqual([
      '1 game save enviado para a sua conta.',
      '3 game saves enviados para a sua conta.',
    ])
    expect(useToastStore.getState().queue.every((t) => t.variant === 'Success')).toBe(true)
  })

  it('a pass that only downloaded, or did nothing, stays silent', async () => {
    await act(async () => emit({ type: 'pass', uploaded: 0, downloaded: 2 }))
    await act(async () => emit({ type: 'pass', uploaded: 0, downloaded: 0 }))
    expect(messages()).toEqual([])
  })
})
