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
 * menu de contexto do item focado. No `/play/:id` só age com o jogo pausado.
 */
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

      switch (nav) {
        case 'confirm':
          ;(document.activeElement as HTMLElement | null)?.click()
          break
        case 'back':
          if (locRef.current.startsWith('/play/')) void toggleFocus().catch(() => {})
          else navigate(-1)
          break
        case 'search':
          openSearch()
          break
        case 'context':
          contextMenu()
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
