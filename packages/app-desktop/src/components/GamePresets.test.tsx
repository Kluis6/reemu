// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { CoreOption } from '../lib/bindings/CoreOption'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  getCoreOptions: vi.fn(),
  setCoreOption: vi.fn(),
  resetCoreOptions: vi.fn(),
}))
vi.mock('../lib/tauri', async (original) => ({ ...(await original<object>()), ...api }))

import '../i18n'
import { GamePresets } from './GamePresets'

const option = (key: string, choices: string[], romValue: string | null = null): CoreOption => ({
  key,
  displayName: key,
  choices,
  defaultValue: choices[0],
  value: romValue ?? choices[0],
  coreValue: null,
  romValue,
})

const PSP = (res: string | null = null) => [
  option('ppsspp_internal_resolution', ['480x272', '1920x1088'], res),
  option('ppsspp_auto_frameskip', ['disabled', 'enabled']),
]

let host: HTMLDivElement
let unmount: () => Promise<void>

async function mount(coreId: string) {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  await act(async () =>
    root.render(
      <QueryClientProvider client={qc}>
        <GamePresets coreId={coreId} romId="r1" />
      </QueryClientProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

async function settle() {
  for (let i = 0; i < 5; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0))
    })
  }
}

const radios = () => [...host.querySelectorAll<HTMLButtonElement>('[role="radio"]')]
const radio = (text: string) => radios().find((b) => b.textContent === text)!

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.setCoreOption.mockResolvedValue(undefined)
})

afterEach(async () => {
  await unmount()
  host.remove()
})

describe('GamePresets', () => {
  it('a core without a preset shows only Personalizado', async () => {
    api.getCoreOptions.mockResolvedValue([option('x', ['a', 'b'])])
    await mount('snes9x_libretro')
    expect(radios().map((b) => b.textContent)).toEqual(['Personalizado'])
  })

  it('Qualidade writes the preset values for this game', async () => {
    api.getCoreOptions.mockResolvedValue(PSP())
    await mount('ppsspp_libretro')
    expect(radio('Normal').getAttribute('aria-checked')).toBe('true')
    await act(async () => radio('Qualidade').click())
    await settle()
    expect(api.setCoreOption).toHaveBeenCalledWith(
      'ppsspp_libretro',
      'ppsspp_internal_resolution',
      '1920x1088',
      'r1',
    )
  })

  it('without the option list, the presets stay disabled until the game runs', async () => {
    api.getCoreOptions.mockResolvedValue([])
    await mount('ppsspp_libretro')
    expect(radio('Desempenho').disabled).toBe(true)
    expect(radio('Qualidade').disabled).toBe(true)
    expect(host.textContent).toContain('Jogue uma vez')
  })

  it('a value outside the preset marks Personalizado', async () => {
    api.getCoreOptions.mockResolvedValue(PSP('960x544'))
    await mount('ppsspp_libretro')
    expect(radio('Personalizado').getAttribute('aria-checked')).toBe('true')
  })
})
