// @vitest-environment happy-dom
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { describe, expect, it, vi } from 'vitest'
import { AlvanistaButton } from './AlvanistaButton'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

async function render(node: React.ReactElement) {
  const host = document.createElement('div')
  document.body.appendChild(host)
  const root = createRoot(host)
  await act(async () => root.render(node))
  return { host, unmount: () => act(async () => root.unmount()) }
}

describe('AlvanistaButton', () => {
  it('shows the translated label with the symbol on the left', async () => {
    const { host, unmount } = await render(<AlvanistaButton onClick={() => {}} />)
    const btn = host.querySelector('button')!
    expect(btn.textContent).toContain('Entrar com Alvanista')
    const first = btn.firstElementChild!
    expect(first.tagName).toBe('IMG')
    expect(first.getAttribute('alt')).toBe('')
    await unmount()
  })

  it('calls onClick and respects disabled', async () => {
    const onClick = vi.fn()
    const a = await render(<AlvanistaButton onClick={onClick} />)
    await act(async () => a.host.querySelector('button')!.click())
    expect(onClick).toHaveBeenCalledTimes(1)
    await a.unmount()

    const onClick2 = vi.fn()
    const b = await render(<AlvanistaButton onClick={onClick2} disabled />)
    await act(async () => b.host.querySelector('button')!.click())
    expect(onClick2).not.toHaveBeenCalled()
    await b.unmount()
  })

  it('draws the ReEmu focus ring on keyboard focus', async () => {
    const { host, unmount } = await render(<AlvanistaButton onClick={() => {}} />)
    const classes = [...host.querySelector('button')!.classList]
    const rules = [...document.styleSheets].flatMap((sheet) =>
      [...sheet.cssRules].map((r) => r.cssText),
    )
    // CSS atômico (Griffel): uma regra por propriedade.
    const onFocus = rules
      .filter((r) => r.includes(':focus-visible') && classes.some((c) => r.includes(`.${c}`)))
      .join('\n')
    expect(onFocus).toMatch(/outline-width:\s*3px/)
    expect(onFocus).toMatch(/outline-style:\s*solid/)
    expect(onFocus).toMatch(/outline-color:\s*var\(--colorBrandStroke1\)/)
    expect(onFocus).toMatch(/outline-offset:\s*3px/)
    await unmount()
  })

  it('offers an outline variant', async () => {
    const solid = await render(<AlvanistaButton onClick={() => {}} />)
    const outline = await render(<AlvanistaButton variant="outline" onClick={() => {}} />)
    expect(solid.host.querySelector('button')!.getAttribute('data-variant')).toBe('solid')
    expect(outline.host.querySelector('button')!.getAttribute('data-variant')).toBe('outline')
    await solid.unmount()
    await outline.unmount()
  })
})
