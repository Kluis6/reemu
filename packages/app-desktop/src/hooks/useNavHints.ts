import { useEffect, useReducer } from 'react'
import { useTranslation } from 'react-i18next'
import { useLocation } from 'react-router-dom'
import type { Hint } from '../components/ButtonHints'
import {
  NAV_STATE_EVENT,
  engagedControl,
  isAdjustable,
  navScope,
  tabLists,
} from '../lib/focusNav'

/** Dicas do que cada botão faz agora: dependem do item em foco e da camada aberta,
 *  não só da tela. Um botão sem efeito não aparece. */
export function useNavHints(scrollEl: () => HTMLElement | null): Hint[] {
  const { t } = useTranslation()
  const { pathname } = useLocation()
  const [, refresh] = useReducer((n: number) => n + 1, 0)

  useEffect(() => {
    window.addEventListener('focusin', refresh)
    window.addEventListener('focusout', refresh)
    window.addEventListener(NAV_STATE_EVENT, refresh)
    return () => {
      window.removeEventListener('focusin', refresh)
      window.removeEventListener('focusout', refresh)
      window.removeEventListener(NAV_STATE_EVENT, refresh)
    }
  }, [])

  if (engagedControl()) {
    return [
      { glyph: 'DPAD', label: t('hints.adjust') },
      { glyph: 'B', label: t('hints.done') },
    ]
  }

  const focused = document.activeElement
  const scope = navScope()
  const inOverlay = scope !== document
  const hints: Hint[] = []

  const typing =
    focused instanceof HTMLTextAreaElement ||
    (focused instanceof HTMLInputElement && focused.type !== 'range' && focused.type !== 'checkbox' && focused.type !== 'radio')
  hints.push({
    glyph: 'A',
    label: t(typing ? 'hints.type' : isAdjustable(focused) ? 'hints.adjust' : 'hints.select'),
  })
  if (inOverlay) hints.push({ glyph: 'B', label: t('hints.close') })
  else if (pathname !== '/') hints.push({ glyph: 'B', label: t('hints.back') })
  if (focused?.closest('[data-nav-action]')) {
    hints.push({ glyph: 'X', label: t('hints.lightDark') })
  }
  if (!inOverlay) hints.push({ glyph: 'Y', label: t('hints.search') })
  if (focused?.closest('[data-nav-menu]')) hints.push({ glyph: 'MENU', label: t('hints.options') })
  if (tabLists(scope).length > 0) hints.push({ glyph: 'LB/RB', label: t('hints.tabs') })
  const scroll = scrollEl()
  if (!inOverlay && scroll && scroll.scrollHeight > scroll.clientHeight) {
    hints.push({ glyph: 'LT/RT', label: t('hints.page') })
  }
  return hints
}
