// @vitest-environment happy-dom
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import '../i18n'
import { useNotificationStore } from '../stores/useNotificationStore'
import { NotificationBell } from './NotificationBell'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

let host: HTMLDivElement
let unmount: () => Promise<void>

async function mount() {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () =>
    root.render(
      <MemoryRouter>
        <NotificationBell />
      </MemoryRouter>,
    ),
  )
  unmount = () => act(async () => root.unmount())
}

const notice = (id: string, read = false) => ({
  id,
  kind: 'notice' as const,
  title: 'Aviso',
  summary: 'texto',
  at: Date.now(),
  read,
})

beforeEach(() => {
  useNotificationStore.setState({ items: [] })
})

afterEach(async () => {
  await unmount()
  host.remove()
})

describe('NotificationBell', () => {
  it('is an icon-only button without a badge when nothing is unread', async () => {
    await mount()
    const btn = host.querySelector('button')!
    expect(btn.getAttribute('aria-label')).toBe('Notificações')
    expect(btn.querySelector('.fui-CounterBadge')).toBeNull()
    // sem filhos de texto: o Fluent trata como botão só de ícone (centralizado)
    expect(btn.classList.contains('fui-Button--iconOnly') || !btn.textContent?.trim()).toBe(true)
  })

  it('puts the unread badge inside the icon slot, over the bell', async () => {
    useNotificationStore.setState({ items: [notice('a'), notice('b'), notice('c', true)] })
    await mount()
    const btn = host.querySelector('button')!
    expect(btn.getAttribute('aria-label')).toBe('Notificações — 2 novas')
    const badge = btn.querySelector('.fui-CounterBadge')!
    expect(badge.textContent).toBe('2')
    // o badge e o sino ficam juntos no slot do ícone (antes ia como texto do
    // botão e empurrava o sino pro lado)
    const icon = btn.querySelector('.fui-Button__icon')!
    expect(icon.contains(badge)).toBe(true)
    expect(icon.querySelector('svg')).not.toBeNull()
  })

  it('caps the count at 9+', async () => {
    useNotificationStore.setState({ items: Array.from({ length: 12 }, (_, i) => notice(String(i))) })
    await mount()
    expect(host.querySelector('.fui-CounterBadge')!.textContent).toBe('9+')
  })
})
