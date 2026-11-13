// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { HardwarePayload, SyncMachine, SyncStatus } from '../../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  syncStatus: vi.fn(),
  syncMachines: vi.fn(),
  syncSetEnabled: vi.fn(),
  syncSetLabel: vi.fn(),
  syncSetPaused: vi.fn(),
  syncNow: vi.fn(),
  alvanistaConnect: vi.fn(),
  alvanistaCancel: vi.fn(),
  alvanistaOpenSite: vi.fn(),
}))

vi.mock('../../lib/tauri', async (orig) => ({
  ...(await orig<typeof import('../../lib/tauri')>()),
  ...api,
}))

import { SettingsSync } from './SettingsSync'
import { hardwareLine, machineColor } from './syncFormat'

const HW: HardwarePayload = {
  os: 'Windows 11',
  arch: 'x86_64',
  cpu: 'Ryzen 7 5800X',
  cpu_cores: 8,
  cpu_threads: 16,
  memory_mb: 32768,
  gpu: 'RTX 3070',
  gpu_backend: 'Vulkan',
  gpu_driver: '555.1',
  app_version: '0.9.0',
  captured_at: '2026-11-13T10:00:00Z',
}
const HW_LINE = 'Windows 11 · Ryzen 7 5800X · 32 GB · RTX 3070 · Vulkan'

const OFF: SyncStatus = {
  availability: 'ready',
  enabled: false,
  pausedLocal: false,
  pausedSite: false,
  revoked: false,
  deviceId: null,
  label: 'PC-SALA',
  hardware: HW,
  quota: null,
  lastPassAt: null,
  conflicts: 0,
  lastError: null,
}
const ON: SyncStatus = {
  ...OFF,
  enabled: true,
  deviceId: 'dev1',
  quota: { usedBytes: 120 * 1024 ** 2, limitBytes: 2 * 1024 ** 3 },
}
const THIS: SyncMachine = {
  id: 'dev1',
  label: 'PC-SALA',
  clientKind: 'reemu',
  isThis: true,
  hardware: HW,
  lastUsedAt: new Date().toISOString(),
  revoked: false,
  paused: false,
}
const OTHER: SyncMachine = {
  id: 'dev2',
  label: 'Notebook',
  clientKind: 'reemu',
  isThis: false,
  hardware: { ...HW, cpu: 'Core i5', gpu: 'Iris Xe', gpu_backend: 'OpenGL' },
  lastUsedAt: null,
  revoked: false,
  paused: false,
}
const RETRO: SyncMachine = { ...OTHER, id: 'dev3', label: 'Deck', clientKind: 'retroarch', hardware: null }

let host: HTMLDivElement
let unmount: () => Promise<void>

async function settle() {
  for (let i = 0; i < 5; i++) {
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
        <SettingsSync />
      </QueryClientProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

const byText = (text: string) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent?.includes(text))

function typeInto(input: HTMLInputElement, value: string) {
  const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!
  set.call(input, value)
  input.dispatchEvent(new Event('input', { bubbles: true }))
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.syncStatus.mockResolvedValue(OFF)
  api.syncMachines.mockResolvedValue([THIS])
  api.alvanistaCancel.mockResolvedValue(undefined)
  api.alvanistaOpenSite.mockResolvedValue(undefined)
})
afterEach(async () => {
  await unmount()
  host.remove()
})

describe('SettingsSync', () => {
  it('shows the connect button without an account', async () => {
    api.syncStatus.mockResolvedValue({ ...OFF, availability: 'no_account' })
    await mount()
    expect(byText('Entrar com Alvanista')).toBeTruthy()
    expect(host.textContent).toContain('Conecte sua conta do Alvanista')
    expect(host.querySelector('[role="switch"]')).toBeNull()
  })

  it('shows the plan sentence when the tier lacks sync', async () => {
    api.syncStatus.mockResolvedValue({ ...OFF, availability: 'no_tier' })
    await mount()
    expect(host.textContent).toContain('Sincronização de saves é para o plano Collector ou acima.')
    await act(async () => byText('Ver planos')!.click())
    expect(api.alvanistaOpenSite).toHaveBeenCalledWith('plans')
  })

  it('turns sync on and shows the machine with its hardware', async () => {
    api.syncSetEnabled.mockResolvedValue(ON)
    await mount()
    expect(host.textContent).not.toContain('Máquinas desta conta')
    await act(async () => host.querySelector<HTMLInputElement>('[role="switch"]')!.click())
    await settle()
    expect(api.syncSetEnabled).toHaveBeenCalledWith(true)
    expect(host.textContent).toContain('Máquinas desta conta')
    expect(host.textContent).toContain('esta')
    expect(host.textContent).toContain(HW_LINE)
    expect(host.textContent).toContain('120 MB de 2 GB')
    expect(host.textContent).toContain('1 máquina nesta conta')
    // o resultado da mutação vira o estado: sem nova leitura do status
    expect(api.syncStatus).toHaveBeenCalledTimes(1)
  })

  it('lists other machines and disables copy with em breve', async () => {
    api.syncStatus.mockResolvedValue(ON)
    api.syncMachines.mockResolvedValue([THIS, OTHER, RETRO])
    await mount()
    expect(host.textContent).toContain('Notebook')
    expect(host.textContent).toContain('Core i5 · 32 GB · Iris Xe · OpenGL')
    expect(host.textContent).toContain('ainda não visto')
    expect(host.textContent).toContain('Só saves. Sem hardware nem configurações do ReEmu.')
    const copies = [...host.querySelectorAll('button')].filter((b) => b.textContent?.includes('Copiar configurações'))
    // só a máquina ReEmu que não é esta
    expect(copies).toHaveLength(1)
    expect(copies[0].disabled).toBe(true)
    expect(copies[0].textContent).toContain('em breve')
  })

  it('shows the removed banner and relinks', async () => {
    api.syncStatus.mockResolvedValue({ ...OFF, revoked: true })
    api.syncSetEnabled.mockResolvedValue(ON)
    await mount()
    expect(host.textContent).toContain('Esta máquina foi removida no site.')
    expect(api.syncMachines).not.toHaveBeenCalled()
    await act(async () => byText('Ligar de novo')!.click())
    await settle()
    expect(api.syncSetEnabled).toHaveBeenCalledWith(true)
    expect(host.textContent).not.toContain('Esta máquina foi removida no site.')
  })

  it('renames through the field', async () => {
    api.syncStatus.mockResolvedValue(ON)
    api.syncSetLabel.mockResolvedValue({ ...ON, label: 'PC-ED' })
    await mount()
    const input = host.querySelector<HTMLInputElement>('input:not([role="switch"])')!
    expect(input.value).toBe('PC-SALA')
    await act(async () => typeInto(input, 'PC-ED'))
    await act(async () => {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    })
    await settle()
    expect(api.syncSetLabel).toHaveBeenCalledWith('PC-ED')
    expect(host.querySelector<HTMLInputElement>('input:not([role="switch"])')!.value).toBe('PC-ED')
  })

  it('does not rename while paused on the site', async () => {
    api.syncStatus.mockResolvedValue({ ...ON, pausedSite: true })
    await mount()
    expect(host.textContent).toContain('Pausada no site.')
    expect(host.querySelector<HTMLInputElement>('input:not([role="switch"])')!.disabled).toBe(true)
  })

  it('shows the waiting state while the browser is open and cancels through Alvanista', async () => {
    let reject!: (e: unknown) => void
    api.syncSetEnabled.mockReturnValue(new Promise((_, rej) => (reject = rej)))
    await mount()
    await act(async () => host.querySelector<HTMLInputElement>('[role="switch"]')!.click())
    expect(host.textContent).toContain('Continue no navegador')
    await act(async () => byText('Cancelar')!.click())
    expect(api.alvanistaCancel).toHaveBeenCalledTimes(1)
    await act(async () => reject('cancelled'))
    await settle()
    expect(host.textContent).not.toContain('Continue no navegador')
    expect(host.textContent).not.toContain('A autorização foi cancelada')
  })

  it('translates a command error', async () => {
    api.syncSetEnabled.mockRejectedValue('quota')
    await mount()
    await act(async () => host.querySelector<HTMLInputElement>('[role="switch"]')!.click())
    await settle()
    expect(host.textContent).toContain('O espaço da sua conta acabou')
  })

  it('opens the site page for pause, revoke and erase', async () => {
    await mount()
    await act(async () => byText('alvanista.com/sync')!.click())
    expect(api.alvanistaOpenSite).toHaveBeenCalledWith('sync')
  })
})

describe('helpers', () => {
  it('builds the hardware line and skips empty parts', () => {
    expect(hardwareLine(HW, 'pt-BR')).toBe(HW_LINE)
    expect(hardwareLine({ ...HW, gpu: '', gpu_backend: '', memory_mb: 0 }, 'pt-BR')).toBe('Windows 11 · Ryzen 7 5800X')
    expect(hardwareLine(null, 'pt-BR')).toBe('')
  })

  it('gives each id a stable colour', () => {
    expect(machineColor('dev1')).toBe(machineColor('dev1'))
    expect(machineColor('dev1')).not.toBe(machineColor('dev2'))
  })
})
