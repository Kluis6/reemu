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
  syncSetSavesEnabled: vi.fn(),
  syncConflicts: vi.fn(),
  syncResolve: vi.fn(),
  syncNow: vi.fn(),
  alvanistaConnect: vi.fn(),
  alvanistaCancel: vi.fn(),
  alvanistaOpenSite: vi.fn(),
}))

vi.mock('../../lib/tauri', async (orig) => ({
  ...(await orig<typeof import('../../lib/tauri')>()),
  ...api,
}))

// O Dialog do Fluent não monta no happy-dom desta árvore ("presenceFn is not a
// function"): o diálogo de tela cheia vira uma camada simples.
vi.mock('../../components/FullscreenDialog', () => ({
  FullscreenDialog: (p: { open: boolean; onClose: () => void; title: string; children: React.ReactNode; actions?: React.ReactNode }) =>
    p.open ? (
      <div className="fui-DialogSurface" role="dialog">
        <h2>{p.title}</h2>
        <button onClick={p.onClose}>fechar</button>
        {p.children}
        {p.actions}
      </div>
    ) : null,
}))

import { SettingsSync } from './SettingsSync'
import { hardwareLine, machineColor } from './syncFormat'

const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled])'

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
  savesEnabled: true,
  statesEnabled: true,
  conflicts: 0,
  statesLocalOnly: 0,
  lastError: null,
  lastErrorFile: null,
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

  it('coming back from the plans page re-reads the status and offers the toggle', async () => {
    api.syncStatus.mockResolvedValueOnce({ ...OFF, availability: 'no_tier' }).mockResolvedValue(OFF)
    await mount()
    expect(host.querySelector('[role="switch"]')).toBeNull()
    await act(async () => byText('Ver planos')!.click())
    // O navegador abre e a pessoa assina; a janela do ReEmu volta a ter foco.
    await act(async () => window.dispatchEvent(new Event('focus')))
    await settle()
    expect(api.syncStatus).toHaveBeenCalledTimes(2)
    expect(host.querySelector('[role="switch"]')).toBeTruthy()
    expect(host.textContent).not.toContain('é para o plano Collector')
    // Pronto: o foco não relê mais.
    await act(async () => window.dispatchEvent(new Event('focus')))
    await settle()
    expect(api.syncStatus).toHaveBeenCalledTimes(2)
  })

  it('a window focus before opening the plans page re-reads nothing', async () => {
    api.syncStatus.mockResolvedValue({ ...OFF, availability: 'no_tier' })
    await mount()
    await act(async () => window.dispatchEvent(new Event('focus')))
    await settle()
    expect(api.syncStatus).toHaveBeenCalledTimes(1)
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

  it('only states and settings stay em breve; game saves have a real switch', async () => {
    api.syncStatus.mockResolvedValue(ON)
    await mount()
    for (const row of ['Save states', 'Configurações']) {
      const title = [...host.querySelectorAll('span')].find((e) => e.firstElementChild?.textContent === row)
      expect(title?.textContent).toContain('em breve')
    }
    const saves = [...host.querySelectorAll<HTMLInputElement>('[role="switch"]')][1]
    expect(saves.closest('label')?.textContent).toContain('Game saves')
    expect(saves.closest('label')?.textContent).not.toContain('em breve')
    expect(saves.checked).toBe(true)
  })

  it('turns the game saves switch off through syncSetSavesEnabled', async () => {
    api.syncStatus.mockResolvedValue(ON)
    api.syncSetSavesEnabled.mockResolvedValue({ ...ON, savesEnabled: false })
    await mount()
    const saves = () => [...host.querySelectorAll<HTMLInputElement>('[role="switch"]')][1]
    await act(async () => saves().click())
    await settle()
    expect(api.syncSetSavesEnabled).toHaveBeenCalledWith(false)
    expect(saves().checked).toBe(false)
  })

  it('the game saves switch is disabled while sync is off', async () => {
    await mount()
    expect([...host.querySelectorAll<HTMLInputElement>('[role="switch"]')][1].disabled).toBe(true)
  })

  it('hides Conflitos with zero and resolves the conflicts one after another', async () => {
    api.syncStatus.mockResolvedValue(ON)
    await mount()
    expect(host.textContent).not.toContain('Conflitos')
    await unmount()
    host.remove()

    const side = (machine: string, deleted = false) => ({ machine, deviceId: null, savedAt: 1_700_000_000, size: 2048, deleted })
    const c = (key: string, title: string) => ({
      key,
      romId: null,
      title,
      local: side('PC-SALA'),
      remote: side('Notebook'),
    })
    let open = [c('k1', 'Zelda'), c('k2', 'Chrono')]
    api.syncStatus.mockResolvedValue({ ...ON, conflicts: 2 })
    api.syncConflicts.mockImplementation(async () => open)
    api.syncResolve.mockImplementation(async (key: string) => {
      open = open.filter((x) => x.key !== key)
    })
    await mount()
    expect(host.textContent).toContain('Conflitos')
    expect(host.textContent).toContain('2 game saves mudaram nas duas máquinas.')
    await act(async () => byText('Resolver')!.click())
    await settle()
    const dialog = () => document.querySelector('.fui-DialogSurface')
    expect(dialog()?.textContent).toContain('Zelda mudou nas duas máquinas')
    const keepThis = () => [...document.querySelectorAll('button')].find((b) => b.textContent === 'Manter esta')!
    await act(async () => keepThis().click())
    await settle()
    expect(api.syncResolve).toHaveBeenCalledWith('k1', 'local')
    expect(dialog()?.textContent).toContain('Chrono mudou nas duas máquinas')
    await act(async () => keepThis().click())
    await settle()
    expect(api.syncResolve).toHaveBeenCalledWith('k2', 'local')
    expect(dialog()).toBeNull()
  })

  it('Conflitos counts a site deletion apart from the two-machine conflicts (I-4)', async () => {
    const side = (machine: string, deleted = false) => ({ machine, deviceId: null, savedAt: 1_700_000_000, size: 2048, deleted })
    api.syncStatus.mockResolvedValue({ ...ON, conflicts: 3 })
    api.syncConflicts.mockResolvedValue([
      { key: 'k1', romId: null, title: 'Zelda', local: side('PC-SALA'), remote: side('Notebook') },
      { key: 'k2', romId: null, title: 'Chrono', local: side('PC-SALA'), remote: side('', true) },
      { key: 'k3', romId: null, title: 'Mario', local: side('PC-SALA'), remote: side('Deck') },
    ])
    await mount()
    expect(host.textContent).toContain('2 game saves mudaram nas duas máquinas.')
    expect(host.textContent).toContain('1 game save foi apagado no site e mudou aqui.')
    expect(host.textContent).not.toContain('3 game saves')
  })

  it('the Conflitos hint tells the truth for deletions (N4)', async () => {
    const side = (machine: string, deleted = false) => ({ machine, deviceId: null, savedAt: 1_700_000_000, size: 2048, deleted })
    const two = { key: 'k1', romId: null, title: 'Zelda', local: side('PC-SALA'), remote: side('Notebook') }
    const del = { key: 'k2', romId: null, title: 'Chrono', local: side('PC-SALA'), remote: side('', true) }
    const hint = 'Nada é apagado: o save que você não escolher fica no histórico.'
    const mixed = 'Nada é apagado: o que você não escolher fica no histórico ou na pasta de resguardo.'
    const footer = 'Nada é apagado nesta máquina.'

    api.syncStatus.mockResolvedValue({ ...ON, conflicts: 2 })
    api.syncConflicts.mockResolvedValue([two, { ...two, key: 'k3' }])
    await mount()
    expect(host.textContent).toContain(hint)
    expect(host.textContent).not.toContain(footer)
    await unmount()
    host.remove()

    api.syncConflicts.mockResolvedValue([del, { ...del, key: 'k4' }])
    await mount()
    expect(host.textContent).toContain(footer)
    expect(host.textContent).not.toContain(hint)
    expect(host.textContent).not.toContain(mixed)
    await unmount()
    host.remove()

    api.syncConflicts.mockResolvedValue([two, del])
    await mount()
    expect(host.textContent).toContain(mixed)
    expect(host.textContent).not.toContain(hint)
    expect(host.textContent).not.toContain(footer)
  })

  it('offers Copiar configurações only on other ReEmu machines (D119)', async () => {
    const kinds = ['companion', 'launcher', 'operator', 'delta']
    api.syncStatus.mockResolvedValue(ON)
    api.syncMachines.mockResolvedValue([
      THIS,
      ...kinds.map((k, i) => ({ ...OTHER, id: `k${i}`, label: k, clientKind: k })),
      RETRO,
    ])
    await mount()
    for (const k of kinds) expect(host.textContent).toContain(k)
    const copies = [...host.querySelectorAll('button')].filter((b) => b.textContent?.includes('Copiar configurações'))
    expect(copies).toHaveLength(0)
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

  it('keeps the name field focusable with aria-busy while its own rename runs', async () => {
    let finish!: (s: SyncStatus) => void
    api.syncStatus.mockResolvedValue(ON)
    api.syncSetLabel.mockReturnValue(new Promise<SyncStatus>((r) => (finish = r)))
    await mount()
    const input = host.querySelector<HTMLInputElement>('input:not([role="switch"])')!
    input.focus()
    await act(async () => typeInto(input, 'PC-ED'))
    await act(async () => {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    })
    expect(api.syncSetLabel).toHaveBeenCalledWith('PC-ED')
    const during = host.querySelector<HTMLInputElement>('input:not([role="switch"])')!
    expect(during).toBe(input)
    expect(during.disabled).toBe(false)
    expect(during.getAttribute('aria-busy')).toBe('true')
    expect(document.activeElement).toBe(during)
    // O nome enviado fica na tela durante o envio.
    expect(during.value).toBe('PC-ED')
    await act(async () => finish({ ...ON, label: 'PC-ED' }))
    await settle()
    expect(input.getAttribute('aria-busy')).toBeNull()
    expect(input.value).toBe('PC-ED')
    expect(document.activeElement).toBe(input)
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

  it('first focusable element is the toggle', async () => {
    await mount()
    expect(host.querySelector<HTMLElement>(FOCUSABLE)!.getAttribute('role')).toBe('switch')
  })

  it('first focusable element is "Ligar de novo" when revoked', async () => {
    api.syncStatus.mockResolvedValue({ ...OFF, revoked: true })
    await mount()
    expect(host.querySelector<HTMLElement>(FOCUSABLE)!.textContent).toContain('Ligar de novo')
  })

  it('a failed mutation with offline shows the badge', async () => {
    api.syncSetEnabled.mockRejectedValue('offline')
    await mount()
    expect(host.textContent).not.toContain('Sem conexão')
    await act(async () => host.querySelector<HTMLInputElement>('[role="switch"]')!.click())
    await settle()
    expect(host.textContent).toContain('Sem conexão')
  })

  it('a save the cloud refused is named under its message', async () => {
    api.syncStatus.mockResolvedValue({ ...ON, lastError: 'too_large', lastErrorFile: 'Mario Kart 64.srm' })
    await mount()
    expect(host.textContent).toContain('Este save passa do tamanho que a nuvem aceita para o sistema dele.')
    expect(host.textContent).toContain('Arquivo: Mario Kart 64.srm')
    await unmount()
    host.remove()

    api.syncStatus.mockResolvedValue({ ...ON, lastError: 'rejected', lastErrorFile: 'Zelda.srm' })
    await mount()
    expect(host.textContent).toContain('A nuvem recusou este save.')
    expect(host.textContent).toContain('Arquivo: Zelda.srm')
  })

  it('does not repeat the removed message when the banner is shown', async () => {
    api.syncStatus.mockResolvedValue({ ...OFF, revoked: true, lastError: 'revoked' })
    await mount()
    expect(host.textContent!.split('Esta máquina foi removida no site.').length - 1).toBe(1)
  })

  it('no account: connects through Alvanista and explains with sync.explain', async () => {
    api.syncStatus.mockResolvedValue({ ...OFF, availability: 'no_account' })
    api.alvanistaConnect.mockReturnValue(new Promise(() => {}))
    await mount()
    expect(host.textContent).toContain('guardar os saves desta máquina')
    await act(async () => byText('Entrar com Alvanista')!.click())
    expect(api.alvanistaConnect).toHaveBeenCalledTimes(1)
    // Um consentimento só, já com o escopo do cloud save.
    expect(api.alvanistaConnect.mock.calls[0][1]).toBe(true)
  })

  it('Pausar and Retomar call syncSetPaused with the opposite of pausedLocal', async () => {
    api.syncStatus.mockResolvedValue(ON)
    api.syncSetPaused.mockResolvedValueOnce({ ...ON, pausedLocal: true }).mockResolvedValueOnce(ON)
    await mount()
    await act(async () => byText('Pausar')!.click())
    await settle()
    expect(api.syncSetPaused).toHaveBeenLastCalledWith(true)
    await act(async () => byText('Retomar')!.click())
    await settle()
    expect(api.syncSetPaused).toHaveBeenLastCalledWith(false)
  })

  it('Sincronizar agora calls syncNow', async () => {
    api.syncStatus.mockResolvedValue(ON)
    api.syncNow.mockResolvedValue(ON)
    await mount()
    await act(async () => byText('Sincronizar agora')!.click())
    await settle()
    expect(api.syncNow).toHaveBeenCalledTimes(1)
  })

  it('keeps the toggle focusable with aria-busy while an action runs and ignores changes', async () => {
    api.syncStatus.mockResolvedValue(ON)
    api.syncNow.mockReturnValue(new Promise(() => {}))
    await mount()
    await act(async () => byText('Sincronizar agora')!.click())
    const sw = host.querySelector<HTMLInputElement>('[role="switch"]')!
    expect(sw.disabled).toBe(false)
    expect(sw.getAttribute('aria-busy')).toBe('true')
    await act(async () => sw.click())
    expect(api.syncSetEnabled).not.toHaveBeenCalled()
  })

  it('keeps a typed name while another action runs and sends it afterwards', async () => {
    let finish!: (s: SyncStatus) => void
    api.syncStatus.mockResolvedValue(ON)
    api.syncNow.mockReturnValue(new Promise<SyncStatus>((r) => (finish = r)))
    api.syncSetLabel.mockResolvedValue({ ...ON, label: 'PC-ED' })
    await mount()
    await act(async () => byText('Sincronizar agora')!.click())
    const input = host.querySelector<HTMLInputElement>('input:not([role="switch"])')!
    await act(async () => typeInto(input, 'PC-ED'))
    await act(async () => {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    })
    expect(api.syncSetLabel).not.toHaveBeenCalled()
    expect(host.querySelector<HTMLInputElement>('input:not([role="switch"])')!.value).toBe('PC-ED')
    await act(async () => finish(ON))
    await settle()
    await act(async () => {
      host.querySelector<HTMLInputElement>('input:not([role="switch"])')!.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
      )
    })
    await settle()
    expect(api.syncSetLabel).toHaveBeenCalledWith('PC-ED')
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
    expect(new Set(['d1', 'd2', 'd3'].map(machineColor)).size).toBeGreaterThanOrEqual(2)
  })
})
