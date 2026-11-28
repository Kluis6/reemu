// @vitest-environment happy-dom
import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { useGamepadStore } from '../stores/useGamepadStore'
import { FullscreenDialog } from './FullscreenDialog'

;(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true

describe('FullscreenDialog e CloseButton', () => {
  let host: HTMLDivElement
  let unmount: (() => Promise<void>) | null = null

  beforeEach(() => {
    vi.spyOn(Element.prototype, 'animate').mockImplementation(
      () =>
        ({
          cancel: () => {},
          finish: () => {},
          play: () => {},
          pause: () => {},
          addEventListener: () => {},
          removeEventListener: () => {},
          finished: Promise.resolve(),
          onfinish: null,
          oncancel: null,
        }) as unknown as Animation,
    )
    host = document.createElement('div')
    document.body.appendChild(host)
    useGamepadStore.getState().setAll([])
  })

  afterEach(async () => {
    if (unmount) {
      await unmount()
      unmount = null
    }
    host.remove()
    useGamepadStore.getState().setAll([])
    vi.restoreAllMocks()
  })

  async function renderDialog(onClose: () => void, children: React.ReactNode = <p>Vazio</p>) {
    const root = createRoot(host)
    await act(async () => {
      root.render(
        <FluentProvider theme={webDarkTheme}>
          <FullscreenDialog open onClose={onClose} title="Meu Log">
            {children}
          </FullscreenDialog>
        </FluentProvider>,
      )
    })
    await act(async () => {
      await new Promise((r) => setTimeout(r, 30))
    })
    unmount = async () => {
      await act(async () => root.unmount())
    }
  }

  it('mantem os botoes Voltar e Fechar visiveis e funcionais mesmo com controle conectado', async () => {
    useGamepadStore.getState().setAll([{ guid: 'pad-xbox-1', name: 'Xbox Wireless Controller' }])
    const onClose = vi.fn()
    await renderDialog(onClose)

    const closeBtn = document.body.querySelector<HTMLButtonElement>('button[data-overlay-close]')
    expect(closeBtn).toBeTruthy()

    const backBtn = Array.from(document.body.querySelectorAll<HTMLButtonElement>('button')).find(
      (b) => /Voltar|Back/i.test(b.textContent ?? ''),
    )
    expect(backBtn).toBeTruthy()

    await act(async () => {
      backBtn!.click()
    })
    expect(onClose).toHaveBeenCalledTimes(1)

    await act(async () => {
      closeBtn!.click()
    })
    expect(onClose).toHaveBeenCalledTimes(2)
  })

  it('foca o botao Voltar automaticamente quando o conteudo abre sem controles interativos', async () => {
    const onClose = vi.fn()
    await renderDialog(onClose, <div>Nenhuma conquista disponivel.</div>)

    const backBtn = Array.from(document.body.querySelectorAll<HTMLButtonElement>('button')).find(
      (b) => /Voltar|Back/i.test(b.textContent ?? ''),
    )
    expect(backBtn).toBeTruthy()
    expect(document.activeElement).toBe(backBtn)
  })
})
