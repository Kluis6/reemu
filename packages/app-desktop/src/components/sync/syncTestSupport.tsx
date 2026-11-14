import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, type ReactNode } from 'react'
import { createRoot } from 'react-dom/client'
import type { SyncStatus } from '../../lib/tauri'

// Apoio dos testes de `components/sync`: monta num container, espera as
// consultas e entrega o estado do sync como o backend o devolveria.

export const STATUS: SyncStatus = {
  availability: 'ready',
  enabled: true,
  pausedLocal: false,
  pausedSite: false,
  savesEnabled: true,
  statesEnabled: true,
  revoked: false,
  deviceId: 'dev1',
  label: 'PC-ED',
  hardware: {
    os: '',
    arch: '',
    cpu: '',
    cpu_cores: 0,
    cpu_threads: 0,
    memory_mb: 0,
    gpu: '',
    gpu_backend: '',
    gpu_driver: '',
    app_version: '',
    captured_at: '',
  },
  quota: null,
  lastPassAt: null,
  conflicts: 0,
  statesLocalOnly: 0,
  statesWiped: 0,
  lastError: null,
  lastErrorFile: null,
}

export async function settle() {
  for (let i = 0; i < 6; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0))
    })
  }
}

/** Monta `ui` com um `QueryClient` novo. `unmount` limpa o container. */
export async function mountWithQuery(ui: ReactNode) {
  const host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  await act(async () => root.render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>))
  await settle()
  return {
    host,
    qc,
    unmount: async () => {
      await act(async () => root.unmount())
      host.remove()
    },
  }
}

export const buttons = () => [...document.querySelectorAll('button')]
export const button = (text: string) => buttons().find((b) => b.textContent?.includes(text))
