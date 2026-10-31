import { useEffect, type RefObject } from 'react'
import { useLocation, useNavigationType } from 'react-router-dom'
import { inNavMode, navCandidates, navScope } from '../lib/focusNav'

interface Remembered {
  label: string | null
  index: number
  scrollTop: number
}

/** Fora do componente: o `AppShell` desmonta ao abrir um jogo, e a volta precisa
 *  achar o card de onde saiu. */
const memory = new Map<string, Remembered>()

/** Quanto esperar o conteúdo da tela aparecer (dados vêm do backend). */
const WAIT_MS = 1500
const INITIAL_SKIP = '.fui-CarouselButton, .fui-CarouselNav, .fui-CarouselNavButton'

function findRemembered(root: HTMLElement, saved: Remembered): HTMLElement | null {
  const els = navCandidates(root)
  return (
    (saved.label ? els.find((el) => el.getAttribute('aria-label') === saved.label) : null) ??
    els[saved.index] ??
    null
  )
}

/**
 * Foco ao trocar de tela, com o controle ou as setas em uso: ao voltar, o item de
 * onde o usuário saiu; ao entrar, o primeiro item visível do conteúdo. Com o mouse
 * nada muda. Tela que já escolhe o próprio foco (ex.: "Jogar") é respeitada.
 */
export function useFocusMemory(contentRef: RefObject<HTMLElement | null>) {
  const { pathname } = useLocation()
  const navigationType = useNavigationType()

  useEffect(() => {
    const root = contentRef.current
    if (!root) return
    const remember = (e: FocusEvent) => {
      const el = e.target as HTMLElement
      if (!root.contains(el)) return
      memory.set(pathname, {
        label: el.getAttribute('aria-label'),
        index: navCandidates(root).indexOf(el),
        scrollTop: root.scrollTop,
      })
    }
    document.addEventListener('focusin', remember)
    return () => document.removeEventListener('focusin', remember)
  }, [contentRef, pathname])

  useEffect(() => {
    const root = contentRef.current
    if (!root || !inNavMode()) return
    const saved = navigationType === 'POP' ? memory.get(pathname) : undefined
    const started = performance.now()
    let frame = 0
    const attempt = () => {
      // Tela com foco próprio, ou o usuário já se mexeu: não toma o foco de volta.
      if (!saved && root.contains(document.activeElement)) return
      if (navScope() !== document) return
      const target = saved
        ? findRemembered(root, saved)
        : (navCandidates(root).find((el) => {
            const r = el.getBoundingClientRect()
            return !el.matches(INITIAL_SKIP) && r.top >= 0 && r.bottom <= window.innerHeight
          }) ?? null)
      if (target) {
        if (saved) root.scrollTop = saved.scrollTop
        target.focus({ preventScroll: true })
        return
      }
      if (performance.now() - started < WAIT_MS) frame = requestAnimationFrame(attempt)
    }
    // depois do "volta ao topo" e dos efeitos das telas
    frame = requestAnimationFrame(attempt)
    return () => cancelAnimationFrame(frame)
  }, [contentRef, pathname, navigationType])
}
