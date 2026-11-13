// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { SelectBlock } from './SettingBlock'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true
// As animações do popover (Web Animations API) são canceladas quando o
// componente sai, e o happy-dom rejeita isso como erro não tratado. Sem
// `animate`, o Fluent pula a animação.
;(Element.prototype as { animate?: unknown }).animate = undefined

const OPTIONS = [
  { value: '', label: 'Automático' },
  { value: '1', label: 'Porta 1' },
  { value: '2', label: 'Porta 2' },
]

let host: HTMLDivElement
let unmount: () => Promise<void>

async function mount(props: Partial<Parameters<typeof SelectBlock>[0]> = {}) {
  host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  const onChange = vi.fn()
  await act(async () =>
    root.render(
      <FluentProvider theme={webDarkTheme}>
        <SelectBlock label="Porta" value="" options={OPTIONS} onChange={onChange} {...props} />
      </FluentProvider>,
    ),
  )
  unmount = () => act(async () => root.unmount())
  return onChange
}

async function settle() {
  for (let i = 0; i < 4; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0))
    })
  }
}

afterEach(async () => {
  await unmount()
  host.remove()
  document.body.innerHTML = ''
})

describe('SelectBlock', () => {
  it('shows the label and the current value, and says both to screen readers', async () => {
    await mount({ value: '2' })
    const btn = host.querySelector('button')!
    expect(btn.textContent).toContain('Porta')
    expect(btn.textContent).toContain('Porta 2')
    expect(btn.getAttribute('aria-label')).toBe('Porta: Porta 2')
  })

  it('opens the list and reports the chosen value', async () => {
    const onChange = await mount()
    await act(async () => host.querySelector('button')!.click())
    await settle()
    const items = [...document.querySelectorAll<HTMLElement>('[role="menuitemradio"]')]
    expect(items.map((i) => i.textContent)).toEqual(['Automático', 'Porta 1', 'Porta 2'])
    expect(items[0].getAttribute('aria-checked')).toBe('true')
    await act(async () => items[1].click())
    expect(onChange).toHaveBeenCalledWith('1')
  })

  it('stays closed when disabled', async () => {
    const onChange = await mount({ disabled: true })
    const btn = host.querySelector('button')!
    expect(btn.disabled || btn.getAttribute('aria-disabled') === 'true').toBe(true)
    await act(async () => btn.click())
    await settle()
    expect(document.querySelector('[role="menuitemradio"]')).toBeNull()
    expect(onChange).not.toHaveBeenCalled()
  })
})
