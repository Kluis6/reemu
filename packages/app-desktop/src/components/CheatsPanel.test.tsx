// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Cheat, RaStatus } from '../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  listCheats: vi.fn(),
  addCheat: vi.fn(),
  updateCheat: vi.fn(),
  deleteCheat: vi.fn(),
  importCheatsCht: vi.fn(),
  downloadLibretroCheats: vi.fn(),
  pickChtFile: vi.fn(),
  raStatus: vi.fn(),
  raSetPreferences: vi.fn(),
}))

vi.mock('../lib/tauri', async (orig) => ({
  ...(await orig<typeof import('../lib/tauri')>()),
  ...api,
}))

import '../i18n'
import { CheatsPanel } from './CheatsPanel'

const SAMPLE_CHEATS: Cheat[] = [
  { id: 1, romId: 'rom-1', description: 'Infinite Lives', code: '7E0DBE09', enabled: true },
  { id: 2, romId: 'rom-1', description: 'Moon Jump', code: '7E007D80', enabled: false },
]

const RA_OFF: RaStatus = {
  connected: false,
  username: null,
  displayName: null,
  avatarUrl: null,
  score: 0,
  scoreSoftcore: 0,
  linkedAlvanistaUsername: null,
  enabled: true,
  hardcore: false,
  unofficial: false,
  encore: false,
}

const RA_HARDCORE: RaStatus = {
  ...RA_OFF,
  connected: true,
  username: 'edward',
  enabled: true,
  hardcore: true,
}

let host: HTMLDivElement
let unmount: () => Promise<void>

async function settle() {
  for (let i = 0; i < 6; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0))
    })
  }
}

async function mountPanel(romId = 'rom-1') {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  await act(async () =>
    root.render(
      <FluentProvider theme={webDarkTheme}>
        <QueryClientProvider client={qc}>
          <CheatsPanel romId={romId} />
        </QueryClientProvider>
      </FluentProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

function setInputValue(el: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
  setter?.call(el, value)
  el.dispatchEvent(new Event('input', { bubbles: true }))
  el.dispatchEvent(new Event('change', { bubbles: true }))
}

beforeEach(() => {
  Object.values(api).forEach((fn) => fn.mockReset())
  api.listCheats.mockResolvedValue(SAMPLE_CHEATS)
  api.raStatus.mockResolvedValue(RA_OFF)
  api.updateCheat.mockImplementation(
    async (id: number, description: string, code: string, enabled: boolean) => ({
      id,
      romId: 'rom-1',
      description,
      code,
      enabled,
    }),
  )
  api.downloadLibretroCheats.mockResolvedValue(3)
  api.pickChtFile.mockResolvedValue('C:/cheats/Super Mario World (USA).cht')
  api.importCheatsCht.mockResolvedValue(2)
})

afterEach(async () => {
  await unmount()
  host.remove()
})

describe('CheatsPanel', () => {
  it('renders cheats, CC-BY-SA 4.0 attribution, and supports inline editing', async () => {
    await mountPanel()

    expect(host.textContent).toContain('Infinite Lives')
    expect(host.textContent).toContain('CC-BY-SA 4.0')

    const editBtn = host.querySelector<HTMLButtonElement>(
      'button[aria-label*="Infinite Lives"]:not([aria-label*="Remover"]):not([aria-label*="Remove"]):not([aria-label*="Quitar"])',
    )
    expect(editBtn).toBeTruthy()
    await act(async () => editBtn!.click())
    await settle()

    const inputs = [...host.querySelectorAll<HTMLInputElement>('input[type="text"], input:not([type])')]
    // First two text inputs inside the inline edit row hold editDesc and editCode
    const descInput = inputs.find((i) => i.value === 'Infinite Lives')
    const codeInput = inputs.find((i) => i.value === '7E0DBE09')
    expect(descInput).toBeTruthy()
    expect(codeInput).toBeTruthy()

    await act(async () => {
      setInputValue(descInput!, '99 Lives')
      setInputValue(codeInput!, '7E0DBE63')
    })
    await settle()

    const saveBtn = [...host.querySelectorAll('button')].find(
      (b) => b.textContent?.trim() === 'Salvar' || b.textContent?.trim() === 'Save' || b.textContent?.trim() === 'Guardar',
    )
    expect(saveBtn).toBeTruthy()
    await act(async () => saveBtn!.click())
    await settle()

    expect(api.updateCheat).toHaveBeenCalledWith(1, '99 Lives', '7E0DBE63', true)
  })

  it('downloads from libretro-database and imports .cht files on demand', async () => {
    await mountPanel()

    const downloadBtn = [...host.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('libretro'),
    )
    expect(downloadBtn).toBeTruthy()
    await act(async () => downloadBtn!.click())
    await settle()
    expect(api.downloadLibretroCheats).toHaveBeenCalledWith('rom-1')

    const importBtn = [...host.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('.cht'),
    )
    expect(importBtn).toBeTruthy()
    await act(async () => importBtn!.click())
    await settle()
    expect(api.pickChtFile).toHaveBeenCalled()
    expect(api.importCheatsCht).toHaveBeenCalledWith(
      'rom-1',
      'C:/cheats/Super Mario World (USA).cht',
    )
  })

  it('shows Hardcore warning, disables cheat switches, and allows exiting Hardcore', async () => {
    api.raStatus.mockResolvedValue(RA_HARDCORE)
    api.raSetPreferences.mockResolvedValue({ ...RA_HARDCORE, hardcore: false })
    await mountPanel()

    expect(host.textContent).toContain('Hardcore')
    const switches = [...host.querySelectorAll<HTMLInputElement>('input[role="switch"]')]
    expect(switches.length).toBe(2)
    for (const sw of switches) {
      expect(sw.disabled).toBe(true)
      expect(sw.checked).toBe(false)
    }

    const disableHardcoreBtn = [...host.querySelectorAll('button')].find((b) =>
      b.textContent?.includes('Hardcore'),
    )
    expect(disableHardcoreBtn).toBeTruthy()
    await act(async () => disableHardcoreBtn!.click())
    await settle()
    expect(api.raSetPreferences).toHaveBeenCalledWith({ hardcore: false })
  })
})
