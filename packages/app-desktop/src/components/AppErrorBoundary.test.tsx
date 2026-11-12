// @vitest-environment happy-dom
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { describe, expect, it, vi } from 'vitest'
import { AppErrorBoundary } from './AppErrorBoundary'

vi.mock('../lib/tauri', () => ({ jsLog: vi.fn() }))
import { jsLog } from '../lib/tauri'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

function Boom(): never {
  throw new Error('quebrou de propósito')
}

describe('AppErrorBoundary', () => {
  it('renders children when nothing throws', async () => {
    const host = document.createElement('div')
    const root = createRoot(host)
    await act(async () =>
      root.render(
        <AppErrorBoundary>
          <p>ok</p>
        </AppErrorBoundary>,
      ),
    )
    expect(host.textContent).toBe('ok')
    expect(host.querySelector('[data-app-error]')).toBeNull()
    await act(async () => root.unmount())
  })

  it('shows the fallback, logs the error and offers a reload', async () => {
    // o React avisa no console sobre o erro capturado; não é o que se testa
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => {})
    const host = document.createElement('div')
    const root = createRoot(host)
    await act(async () =>
      root.render(
        <AppErrorBoundary>
          <Boom />
        </AppErrorBoundary>,
      ),
    )
    const alert = host.querySelector('[data-app-error]')!
    expect(alert).not.toBeNull()
    expect(alert.getAttribute('role')).toBe('alert')
    expect(alert.textContent).toContain('O ReEmu encontrou um erro')
    expect(alert.textContent).toContain('quebrou de propósito')
    expect(alert.querySelector('button')!.textContent).toBe('Tentar de novo')
    expect(jsLog).toHaveBeenCalledWith('error', expect.stringContaining('quebrou de propósito'))
    await act(async () => root.unmount())
    quiet.mockRestore()
  })
})
