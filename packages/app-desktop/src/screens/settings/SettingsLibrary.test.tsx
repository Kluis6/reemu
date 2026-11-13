// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  listRoms: vi.fn(),
  listInstalledCores: vi.fn(),
  listSystemCores: vi.fn(),
  listRomSources: vi.fn(),
  clearLibrary: vi.fn(),
  removeRomSystem: vi.fn(),
  removeRomSource: vi.fn(),
  setSystemCore: vi.fn(),
  rescanLibrary: vi.fn(),
}))
vi.mock('../../lib/tauri', async (original) => ({ ...(await original<object>()), ...api }))

import '../../i18n'
import { SettingsLibrary } from './SettingsLibrary'

const rom = (id: string, systemId: string) => ({
  id,
  title: id,
  systemId,
  filePath: `D:/roms/${id}`,
  boxart: null,
  lastPlayedAt: null,
  addedAt: 0,
  isFavorite: false,
})

let host: HTMLDivElement
let unmount: () => Promise<void>

async function settle() {
  for (let i = 0; i < 6; i++) {
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
        <SettingsLibrary />
      </QueryClientProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

const buttons = () => [...host.querySelectorAll<HTMLButtonElement>('button')]
const button = (text: string) => buttons().find((b) => b.textContent?.trim() === text)
const tab = (text: string) =>
  [...host.querySelectorAll<HTMLButtonElement>('[role="tab"]')].find((b) => b.textContent?.includes(text))!

async function click(el: HTMLElement) {
  await act(async () => el.click())
  await settle()
}

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.listRoms.mockResolvedValue([rom('a', 'snes'), rom('b', 'snes'), rom('c', 'gba')])
  api.listInstalledCores.mockResolvedValue([
    { coreId: 'snes9x', name: 'Snes9x', version: '1', extensions: [], renderBackend: null, systems: ['snes'] },
  ])
  api.listSystemCores.mockResolvedValue({})
  api.listRomSources.mockResolvedValue([{ path: 'D:/roms', count: 3 }])
  api.clearLibrary.mockResolvedValue(3)
  api.removeRomSystem.mockResolvedValue(2)
})

afterEach(async () => {
  await unmount()
  host.remove()
})

describe('SettingsLibrary (Gerenciar biblioteca)', () => {
  it('lists each platform with its count and the default-core selector', async () => {
    await mount()
    expect(tab('Plataformas').getAttribute('aria-selected')).toBe('true')
    const text = host.textContent ?? ''
    expect(text).toContain('Super Nintendo')
    expect(text).toContain('2 jogos')
    // seletor no padrão dos blocos: rótulo e valor atual no próprio botão
    const select = buttons().find((b) => b.getAttribute('aria-label')?.startsWith('Core padrão'))
    expect(select?.getAttribute('aria-label')).toBe('Core padrão: Automático (por extensão)')
    // os textos de explicação saíram (pedido do usuário, 2026-11-10)
    expect(text).not.toContain('core padrão e remoção')
  })

  it('shows refresh and clear-all next to the tabs on both tabs', async () => {
    await mount()
    expect(button('Atualizar biblioteca')).toBeDefined()
    expect(button('Limpar tudo')).toBeDefined()
    await click(tab('Pastas'))
    expect(tab('Pastas').getAttribute('aria-selected')).toBe('true')
    expect(button('Atualizar biblioteca')).toBeDefined()
    expect(button('Limpar tudo')).toBeDefined()
    expect(host.textContent).toContain('D:/roms')
  })

  it('clear-all only deletes on the second click', async () => {
    await mount()
    await click(button('Limpar tudo')!)
    expect(api.clearLibrary).not.toHaveBeenCalled()
    const confirm = button('Confirmar: apagar tudo')
    expect(confirm).toBeDefined()
    await click(confirm!)
    expect(api.clearLibrary).toHaveBeenCalledTimes(1)
  })

  it('removing a platform also asks for confirmation', async () => {
    await mount()
    const remove = buttons().filter((b) => b.textContent?.trim() === 'Remover')
    expect(remove).toHaveLength(2)
    await click(remove[1])
    expect(api.removeRomSystem).not.toHaveBeenCalled()
    await click(button('Confirmar')!)
    expect(api.removeRomSystem).toHaveBeenCalledWith('snes')
  })
})
