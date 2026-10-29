// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { AlvanistaStatus } from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

const api = vi.hoisted(() => ({
  alvanistaStatus: vi.fn(),
  alvanistaCachedStatus: vi.fn(),
  alvanistaConnect: vi.fn(),
  alvanistaCancel: vi.fn(),
  alvanistaDisconnect: vi.fn(),
  alvanistaOpenProfile: vi.fn(),
  getProfile: vi.fn(),
}))

vi.mock('../../lib/tauri', async (orig) => ({
  ...(await orig<typeof import('../../lib/tauri')>()),
  ...api,
}))

import { SettingsProfile } from './SettingsProfile'

const OFF: AlvanistaStatus = {
  connected: false,
  username: null,
  avatar_url: null,
  hero_url: null,
  bio: null,
  profile_url: null,
  offline: false,
  insecure_storage: false,
  ended: false,
}
const ON: AlvanistaStatus = {
  ...OFF,
  connected: true,
  username: 'toy',
  avatar_url: 'https://cdn.alvanista.com/a/toy.png',
  profile_url: 'https://www.alvanista.com/users/toy',
}

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
        <SettingsProfile />
      </QueryClientProvider>,
    ),
  )
  await settle()
  unmount = () => act(async () => root.unmount())
}

const byText = (text: string) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent?.includes(text))

beforeEach(() => {
  Object.values(api).forEach((f) => f.mockReset())
  api.getProfile.mockResolvedValue({ name: 'Local', bio: null, avatar: 'preset:2', onboarded: true })
  api.alvanistaStatus.mockResolvedValue(OFF)
  api.alvanistaCachedStatus.mockResolvedValue(OFF)
  api.alvanistaCancel.mockResolvedValue(undefined)
  api.alvanistaOpenProfile.mockResolvedValue(undefined)
  api.alvanistaDisconnect.mockResolvedValue(undefined)
  useToastStore.setState({ queue: [] })
})
afterEach(async () => {
  await unmount()
  host.remove()
})

describe('SettingsProfile with Alvanista', () => {
  it('disconnected: shows the button and keeps the local form editable', async () => {
    await mount()
    expect(byText('Entrar com Alvanista')).toBeTruthy()
    const name = host.querySelector<HTMLInputElement>('input')!
    expect(name.value).toBe('Local')
    expect(name.disabled).toBe(false)
    expect(name.readOnly).toBe(false)
  })

  it('connected: shows the Alvanista username and avatar read-only, edit and disconnect', async () => {
    api.alvanistaStatus.mockResolvedValue(ON)
    await mount()
    expect(host.textContent).toContain('toy')
    expect(host.querySelector('img[src="https://cdn.alvanista.com/a/toy.png"]')).toBeTruthy()
    expect(host.querySelector('input')).toBeNull()
    expect(byText('Entrar com Alvanista')).toBeUndefined()
    await act(async () => byText('Editar no Alvanista')!.click())
    expect(api.alvanistaOpenProfile).toHaveBeenCalledTimes(1)
    expect(byText('Desconectar')).toBeTruthy()
  })

  it('connected without avatar: falls back to the ReEmu preset avatar', async () => {
    api.alvanistaStatus.mockResolvedValue({ ...ON, avatar_url: null })
    await mount()
    expect(host.querySelector('img[src^="data:image/svg+xml"]')).toBeTruthy()
  })

  it('shows the offline badge and the insecure storage notice', async () => {
    api.alvanistaStatus.mockResolvedValue({ ...ON, offline: true, insecure_storage: true })
    await mount()
    expect(host.textContent).toContain('Sem conexão: mostrando o último perfil salvo')
    expect(host.textContent).toContain('O chaveiro do sistema não está disponível')
  })

  it('ended: toasts and goes back to the disconnected view', async () => {
    api.alvanistaStatus.mockResolvedValue({ ...OFF, ended: true })
    await mount()
    const messages = useToastStore.getState().queue.map((x) => x.message)
    expect(messages).toContain('Conexão com o Alvanista encerrada')
    expect(byText('Entrar com Alvanista')).toBeTruthy()
    expect(host.querySelector<HTMLInputElement>('input')!.value).toBe('Local')
  })

  it('connecting: shows the waiting state and Cancel calls alvanista_cancel', async () => {
    let reject!: (e: unknown) => void
    api.alvanistaConnect.mockReturnValue(new Promise((_, rej) => (reject = rej)))
    await mount()
    await act(async () => byText('Entrar com Alvanista')!.click())
    expect(host.textContent).toContain('Continue no navegador')
    expect(byText('Entrar com Alvanista')).toBeUndefined()
    await act(async () => byText('Cancelar')!.click())
    expect(api.alvanistaCancel).toHaveBeenCalledTimes(1)
    await act(async () => reject('cancelled'))
    await settle()
    expect(byText('Entrar com Alvanista')).toBeTruthy()
    expect(host.textContent).not.toContain('Você não autorizou')
  })

  it('denied: shows the outcome message and the button again', async () => {
    api.alvanistaConnect.mockRejectedValue('denied')
    await mount()
    await act(async () => byText('Entrar com Alvanista')!.click())
    await settle()
    expect(host.textContent).toContain('Você não autorizou o acesso')
    expect(byText('Entrar com Alvanista')).toBeTruthy()
  })

  it('offline: the message is impersonal, like the rest of the app', async () => {
    api.alvanistaConnect.mockRejectedValue('offline')
    await mount()
    await act(async () => byText('Entrar com Alvanista')!.click())
    await settle()
    expect(host.textContent).toContain('Não foi possível falar com o Alvanista')
  })

  it('connect succeeds: switches to the connected view', async () => {
    api.alvanistaConnect.mockResolvedValue(ON)
    await mount()
    await act(async () => byText('Entrar com Alvanista')!.click())
    await settle()
    expect(host.textContent).toContain('toy')
    expect(byText('Desconectar')).toBeTruthy()
  })

  it('disconnect: goes back to the local profile', async () => {
    api.alvanistaStatus.mockResolvedValue(ON)
    await mount()
    await act(async () => byText('Desconectar')!.click())
    await settle()
    expect(api.alvanistaDisconnect).toHaveBeenCalledTimes(1)
    expect(byText('Entrar com Alvanista')).toBeTruthy()
    expect(host.querySelector<HTMLInputElement>('input')!.value).toBe('Local')
  })

  it('renders from the cached status while the full status is still pending', async () => {
    api.alvanistaStatus.mockReturnValue(new Promise(() => {}))
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    await mount()
    expect(host.textContent).toContain('toy')
    expect(byText('Desconectar')).toBeTruthy()
    expect(host.querySelector('[role="progressbar"], .fui-Spinner')).toBeNull()
  })

  it('ignores a second click on the button before it re-renders', async () => {
    api.alvanistaConnect.mockReturnValue(new Promise(() => {}))
    await mount()
    const btn = byText('Entrar com Alvanista')!
    await act(async () => {
      btn.click()
      btn.click()
    })
    expect(api.alvanistaConnect).toHaveBeenCalledTimes(1)
  })

  it('no database: hides the Alvanista block and keeps the local form', async () => {
    api.alvanistaCachedStatus.mockRejectedValue('failed')
    api.alvanistaStatus.mockRejectedValue('failed')
    await mount()
    expect(byText('Entrar com Alvanista')).toBeUndefined()
    expect(host.textContent).not.toContain('Conecte sua conta do Alvanista')
    expect(host.querySelector<HTMLInputElement>('input')!.value).toBe('Local')
  })

  it('connect and disconnect refresh the identity shown in the rest of the app', async () => {
    // Como no backend: conectar e desconectar mudam o que fica guardado.
    api.alvanistaConnect.mockImplementation(async () => {
      api.alvanistaCachedStatus.mockResolvedValue(ON)
      return ON
    })
    api.alvanistaDisconnect.mockImplementation(async () => {
      api.alvanistaCachedStatus.mockResolvedValue(OFF)
    })
    await mount()
    const reads = () => api.alvanistaCachedStatus.mock.calls.length
    const before = reads()
    await act(async () => byText('Entrar com Alvanista')!.click())
    await settle()
    expect(reads()).toBeGreaterThan(before)
    const afterConnect = reads()
    await act(async () => byText('Desconectar')!.click())
    await settle()
    expect(reads()).toBeGreaterThan(afterConnect)
  })

  it('a full status that differs from the cached one refreshes the identity', async () => {
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    api.alvanistaStatus.mockResolvedValue({ ...ON, username: 'toy2' })
    await mount()
    expect(api.alvanistaCachedStatus.mock.calls.length).toBeGreaterThan(1)
  })

  it('a full status equal to the cached one does not read it again', async () => {
    api.alvanistaCachedStatus.mockResolvedValue(ON)
    api.alvanistaStatus.mockResolvedValue({ ...ON, offline: true })
    await mount()
    expect(api.alvanistaCachedStatus).toHaveBeenCalledTimes(1)
  })

  it('disconnect is busy while it runs: one call and one toast', async () => {
    let finish!: () => void
    api.alvanistaStatus.mockResolvedValue(ON)
    api.alvanistaDisconnect.mockReturnValue(new Promise<void>((res) => (finish = res)))
    await mount()
    const btn = byText('Desconectar')!
    await act(async () => {
      btn.click()
      btn.click()
    })
    expect(api.alvanistaDisconnect).toHaveBeenCalledTimes(1)
    // Desabilitado mas focável: o foco do controle não some do botão.
    expect(byText('Desconectar')!.getAttribute('aria-disabled')).toBe('true')
    await act(async () => byText('Desconectar')!.click())
    expect(api.alvanistaDisconnect).toHaveBeenCalledTimes(1)
    await act(async () => finish())
    await settle()
    const messages = useToastStore.getState().queue.map((x) => x.message)
    expect(messages.filter((m) => m.startsWith('Desconectado do Alvanista'))).toHaveLength(1)
  })

  it('cancels a connection in progress when the screen unmounts', async () => {
    api.alvanistaConnect.mockReturnValue(new Promise(() => {}))
    await mount()
    await act(async () => byText('Entrar com Alvanista')!.click())
    expect(api.alvanistaCancel).not.toHaveBeenCalled()
    await unmount()
    expect(api.alvanistaCancel).toHaveBeenCalledTimes(1)
    unmount = async () => {}
  })
})
