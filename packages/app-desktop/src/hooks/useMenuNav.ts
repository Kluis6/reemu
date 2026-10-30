import { useEffect, useRef } from 'react'
import { useLocation, useNavigate } from 'react-router-dom'
import { moveFocus, type NavDir } from '../lib/focusNav'
import { onMenuNav, toggleFocus } from '../lib/tauri'
import { useFocusStore } from '../stores/useFocusStore'
import { useSearchStore } from '../stores/useSearchStore'

/**
 * Navegação da UI no estilo console (modo Xbox) — um único listener estável
 * (montado no `RootLayout`). Setas do teclado sempre; gamepad via evento
 * `menu-nav` do backend.
 *
 * `confirm` (A) = clica o foco · `back` (B) = volta / retoma o jogo ·
 * `search` (Y / `/`) = abre a busca · `context` (☰ / tecla ContextMenu) =
 * menu de contexto do item focado · `action` (X) = ação secundária do item
 * focado, entregue como o evento `reemu-action` (ver `REEMU_ACTION`) que
 * sobe a partir do foco. No `/play/:id` só age com o jogo pausado.
 */
/** Evento da ação secundária (X do controle), disparado no elemento focado
 *  e borbulhando — quem tem uma ação secundária escuta no próprio elemento. */
export const REEMU_ACTION = 'reemu-action'

/** B do controle no onboarding: volta um passo do fluxo (a tela escuta este
 *  evento na `window`) em vez de voltar no histórico. */
export const REEMU_ONBOARDING_BACK = 'reemu-onboarding-back'

/** LB/RB do controle: aba anterior (`detail: -1`) ou próxima (`detail: 1`),
 *  disparado na `window` — quem tem abas escuta (o menu de pausa). Sem
 *  atalho de teclado (pedido do usuário). */
export const REEMU_TAB = 'reemu-tab'
const switchTab = (step: -1 | 1) =>
  window.dispatchEvent(new CustomEvent(REEMU_TAB, { detail: step }))

/** Tipos de `<input>` em que se digita texto (o A abre o teclado na tela). */
const TEXT_INPUT = new Set(['text', 'search', 'email', 'url', 'tel', 'password', ''])

/** O foco é um campo de texto editável? */
function textField(el: Element | null): HTMLInputElement | HTMLTextAreaElement | null {
  if (el instanceof HTMLTextAreaElement) return el.readOnly || el.disabled ? null : el
  if (el instanceof HTMLInputElement && TEXT_INPUT.has(el.type))
    return el.readOnly || el.disabled ? null : el
  return null
}

export function useMenuNav() {
  const navigate = useNavigate()
  const location = useLocation()
  const focus = useFocusStore((s) => s.focus)
  const setSearchOpen = useSearchStore((s) => s.setOpen)

  const locRef = useRef(location.pathname)
  const focusRef = useRef(focus)
  useEffect(() => {
    locRef.current = location.pathname
    focusRef.current = focus
  })

  useEffect(() => {
    const onPlayIdle = () =>
      locRef.current.startsWith('/play/') && focusRef.current !== 'MenuFocused'
    const openSearch = () => {
      navigate('/library')
      setSearchOpen(true)
    }
    // Pelo controle (Y): além do campo, abre o teclado na tela.
    const openSearchPad = () => {
      navigate('/library')
      useSearchStore.getState().setKeyboard(true)
    }
    const secondaryAction = () => {
      document.activeElement?.dispatchEvent(new CustomEvent(REEMU_ACTION, { bubbles: true }))
    }
    const contextMenu = () => {
      const el = document.activeElement as HTMLElement | null
      el?.dispatchEvent(
        new MouseEvent('contextmenu', { bubbles: true, cancelable: true, view: window }),
      )
    }

    // --- teclado (sempre) ---
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null
      const typing =
        t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)
      if (typing) return
      if ((e.key === '/' || e.key === 'F3') && !onPlayIdle()) {
        e.preventDefault()
        openSearch()
        return
      }
      if (e.key === 'ContextMenu' && !onPlayIdle()) {
        e.preventDefault()
        contextMenu()
        return
      }
      const map: Record<string, NavDir> = {
        ArrowUp: 'up',
        ArrowDown: 'down',
        ArrowLeft: 'left',
        ArrowRight: 'right',
      }
      if (map[e.key] && !onPlayIdle()) {
        e.preventDefault()
        moveFocus(map[e.key])
      }
    }
    window.addEventListener('keydown', onKey)

    // --- gamepad ---
    let last = 0
    let lastKind = ''
    const handle = (nav: string) => {
      if (onPlayIdle()) return
      const now = performance.now()
      if (nav === lastKind && now - last < 90) return
      last = now
      lastKind = nav
      // Teclado na tela aberto: ele recebe os pulsos primeiro.
      const kbd = useSearchStore.getState().keyboardNav
      if (kbd && kbd(nav)) return

      switch (nav) {
        case 'confirm': {
          // Campo de texto: o A abre o teclado na tela pra ele (com o
          // controle não há outro jeito de digitar); o resto, clica.
          const field = textField(document.activeElement)
          if (field) useSearchStore.getState().openKeyboardFor(field)
          else (document.activeElement as HTMLElement | null)?.click()
          break
        }
        case 'back': {
          // Drawer ou modal aberto (perfil, Informações, Encerrar...): o B
          // fecha ele, como o Esc do teclado — o Fluent fecha no `keydown`
          // de Escape dentro da superfície. Antes o B voltava a página por
          // trás do painel aberto.
          const modal = document.querySelector<HTMLElement>('.fui-OverlayDrawer, .fui-DialogSurface')
          if (locRef.current.startsWith('/play/')) void toggleFocus().catch(() => {})
          // o onboarding é um modal também, mas o B nele volta um passo
          else if (locRef.current === '/onboarding')
            window.dispatchEvent(new CustomEvent(REEMU_ONBOARDING_BACK))
          else if (modal) {
            const target =
              document.activeElement instanceof HTMLElement && modal.contains(document.activeElement)
                ? document.activeElement
                : modal
            target.dispatchEvent(
              new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape', bubbles: true, cancelable: true }),
            )
          } else navigate(-1)
          break
        }
        case 'search':
          openSearchPad()
          break
        case 'context':
          contextMenu()
          break
        case 'action':
          secondaryAction()
          break
        case 'tabPrev':
        case 'tabNext':
          switchTab(nav === 'tabPrev' ? -1 : 1)
          break
        default:
          moveFocus(nav as NavDir)
      }
    }

    let disposed = false
    let dispose: (() => void) | undefined
    void onMenuNav(handle).then((d) => {
      if (disposed) d()
      else dispose = d
    })

    return () => {
      window.removeEventListener('keydown', onKey)
      disposed = true
      dispose?.()
    }
  }, [navigate, setSearchOpen])
}
