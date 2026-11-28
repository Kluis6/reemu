// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import '../../i18n'
import type {
  AspectSettings,
  RomEntry,
  ShaderAssignments,
  ShaderInfo,
  ShaderParam,
} from '../../lib/tauri'
import { SettingsVideo } from './SettingsVideo'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true
;(Element.prototype as { animate?: unknown }).animate = undefined

const mocks = vi.hoisted(() => ({
  getAspectSettings: vi.fn<() => Promise<AspectSettings>>(),
  setAspect: vi.fn<(...args: unknown[]) => Promise<void>>(),
  getShaderInfo: vi.fn<() => Promise<ShaderInfo>>(),
  getShaderAssignments: vi.fn<() => Promise<ShaderAssignments>>(),
  setShader: vi.fn<(...args: unknown[]) => Promise<null>>(),
  getShaderParams: vi.fn<(...args: unknown[]) => Promise<ShaderParam[]>>(),
  setShaderParam: vi.fn<(...args: unknown[]) => Promise<void>>(),
  resetShaderParams: vi.fn<(...args: unknown[]) => Promise<void>>(),
  shaderPreviewSource: vi.fn<() => Promise<ArrayBuffer>>(),
  renderShaderPreview: vi.fn<() => Promise<ArrayBuffer>>(),
  listRoms: vi.fn<() => Promise<RomEntry[]>>(),
  listPendingMatches: vi.fn<() => Promise<never[]>>(),
}))

vi.mock('../../lib/tauri', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../lib/tauri')>()
  return {
    ...actual,
    ...mocks,
  }
})

let host: HTMLDivElement
let unmount: () => Promise<void>

async function settle() {
  for (let i = 0; i < 6; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10))
    })
  }
}

async function mount(section?: 'proporcao' | 'shaders' | 'molduras') {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () => {
    root.render(
      <FluentProvider theme={webDarkTheme}>
        <QueryClientProvider client={qc}>
          <MemoryRouter initialEntries={['/settings/video']}>
            <Routes>
              <Route path="/settings/video" element={<SettingsVideo section={section} />} />
            </Routes>
          </MemoryRouter>
        </QueryClientProvider>
      </FluentProvider>,
    )
  })
  await settle()
  unmount = async () => {
    await act(async () => root.unmount())
    host.remove()
    document.body.innerHTML = ''
  }
}

beforeEach(() => {
  Object.values(mocks).forEach((fn) => fn.mockReset())
  mocks.listPendingMatches.mockResolvedValue([])
  mocks.listRoms.mockResolvedValue([])
  mocks.setAspect.mockResolvedValue(undefined)
  mocks.setShader.mockResolvedValue(null)
  mocks.setShaderParam.mockResolvedValue(undefined)
  mocks.resetShaderParams.mockResolvedValue(undefined)
  mocks.shaderPreviewSource.mockResolvedValue(new ArrayBuffer(0))
  mocks.renderShaderPreview.mockResolvedValue(new ArrayBuffer(0))
  mocks.getShaderParams.mockResolvedValue([])
  mocks.getShaderInfo.mockResolvedValue({
    active: 'plain',
    available: ['plain', 'crt', 'lcd'],
    curated: [],
    gpu: false,
  })
})

afterEach(async () => {
  if (unmount) await unmount()
})

describe('SettingsVideo', () => {
  it('renders hub cards for Proporcao, Shaders, and Molduras', async () => {
    await mount()
    const links = [...host.querySelectorAll('a')].map((a) => a.getAttribute('href'))
    expect(links).toEqual([
      '/settings/video/proporcao',
      '/settings/video/shaders',
      '/settings/video/molduras',
    ])
    expect(host.textContent).toContain('Proporção da imagem')
    expect(host.textContent).toContain('Shaders')
    expect(host.textContent).toContain('Molduras')
  })

  it('edits global aspect ratio and resets per-platform overrides with one click', async () => {
    mocks.getAspectSettings.mockResolvedValue({
      atDefault: '4:3',
      systems: { snes: '16:9', gba: 'core' },
    })
    await mount('proporcao')

    expect(host.textContent).toContain('Super Nintendo')
    expect(host.textContent).toContain('Game Boy Advance')
    expect(host.textContent).toContain('16:9')

    // Reset SNES override with 1-click button
    const resetSnes = host.querySelector<HTMLButtonElement>(
      'button[aria-label="Voltar Super Nintendo para o padrão geral"]',
    )
    expect(resetSnes).not.toBeNull()
    await act(async () => {
      resetSnes!.click()
    })
    expect(mocks.setAspect).toHaveBeenCalledWith('system', '', null, 'snes')

    // Change global default aspect via SelectBlock menu
    const globalTrigger = [...host.querySelectorAll<HTMLButtonElement>('button')].find((b) =>
      b.getAttribute('aria-label')?.startsWith('Todos os jogos:'),
    )
    expect(globalTrigger).not.toBeNull()
    await act(async () => {
      globalTrigger!.click()
    })
    await settle()
    const option169 = [...document.body.querySelectorAll<HTMLElement>('[role="menuitemradio"]')].find(
      (el) => el.textContent === '16:9',
    )
    expect(option169).not.toBeNull()
    await act(async () => {
      option169!.click()
    })
    expect(mocks.setAspect).toHaveBeenCalledWith('default', '', '16:9')
  })

  it('switches shader scope to a specific platform and sets or inherits shader', async () => {
    mocks.getShaderInfo.mockResolvedValue({
      active: 'crt',
      available: ['plain', 'crt', 'lcd'],
      curated: [],
      gpu: true,
    })
    mocks.getShaderAssignments.mockResolvedValue({
      atDefault: 'crt',
      systems: { snes: 'lcd' },
    })
    await mount('shaders')

    // Initially in 'default' scope, 'CRT' is selected
    expect(host.textContent).toContain('Todos os jogos (padrão)')
    expect(mocks.getShaderParams).toHaveBeenCalledWith('default')

    // Switch scope to 'Plataforma específica'
    const scopeBtn = [...host.querySelectorAll<HTMLButtonElement>('button')].find((b) =>
      b.getAttribute('aria-label')?.startsWith('Aplicar em:'),
    )
    expect(scopeBtn).not.toBeNull()
    await act(async () => {
      scopeBtn!.click()
    })
    await settle()
    const platformScopeItem = [
      ...document.body.querySelectorAll<HTMLElement>('[role="menuitemradio"]'),
    ].find((el) => el.textContent === 'Plataforma específica')
    expect(platformScopeItem).not.toBeNull()
    await act(async () => {
      platformScopeItem!.click()
    })
    await settle()

    // 'snes' is auto-selected because it has an override ('lcd')
    expect(host.textContent).toContain('Super Nintendo')
    expect(host.textContent).toContain('Herdar')
    expect(mocks.getShaderParams).toHaveBeenCalledWith('system', undefined, 'snes')

    // Click 'Herdar padrão' button to clear platform override
    const resetBtn = [...host.querySelectorAll<HTMLButtonElement>('button')].find(
      (b) => b.textContent === 'Herdar padrão',
    )
    expect(resetBtn).not.toBeNull()
    await act(async () => {
      resetBtn!.click()
    })
    expect(mocks.setShader).toHaveBeenCalledWith('', 'system', undefined, 'snes')
  })

  it('opens and resets shader parameters for a platform scope', async () => {
    mocks.getShaderInfo.mockResolvedValue({
      active: 'curated:crt_guest',
      available: ['plain', 'crt', 'lcd'],
      curated: [{ id: 'curated:crt_guest', label: 'CRT Guest', desc: 'CRT', available: true }],
      gpu: true,
    })
    mocks.getShaderAssignments.mockResolvedValue({
      atDefault: 'plain',
      systems: { snes: 'curated:crt_guest' },
    })
    mocks.getShaderParams.mockResolvedValue([
      {
        name: 'scanline',
        label: 'Scanline',
        value: 0.6,
        default: 0.5,
        min: 0,
        max: 1,
        step: 0.1,
      },
    ])
    await mount('shaders')

    // Switch scope to platform
    const scopeBtn = [...host.querySelectorAll<HTMLButtonElement>('button')].find((b) =>
      b.getAttribute('aria-label')?.startsWith('Aplicar em:'),
    )!
    await act(async () => scopeBtn.click())
    await settle()
    const platformScopeItem = [
      ...document.body.querySelectorAll<HTMLElement>('[role="menuitemradio"]'),
    ].find((el) => el.textContent === 'Plataforma específica')!
    await act(async () => platformScopeItem.click())
    await settle()

    // Open shader settings drawer from the active platform shader card
    const gearBtn = host.querySelector<HTMLButtonElement>(
      'button[aria-label="Configurações do shader"]',
    )
    expect(gearBtn).not.toBeNull()
    await act(async () => gearBtn!.click())
    await settle()

    expect(host.textContent).toContain('Scanline')
    const resetParamsBtn = [...host.querySelectorAll<HTMLButtonElement>('button')].find(
      (b) => b.textContent === 'Restaurar padrões',
    )
    expect(resetParamsBtn).toBeDefined()
    await act(async () => resetParamsBtn!.click())
    await settle()
    expect(mocks.resetShaderParams).toHaveBeenCalledWith('system', undefined, 'snes')
  })
})
