export type NavDir = 'up' | 'down' | 'left' | 'right'

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'

/** Elementos que a navegação por controle deve pular (ex.: o campo de busca —
 *  focá-lo dispara `onFocus` → abre a busca / troca de rota). */
function navSkipped(el: HTMLElement): boolean {
  if (el.closest('[data-nav-skip]')) return true
  // `<button tabindex="-1">` continua batendo em `button:not([disabled])`.
  const ti = el.getAttribute('tabindex')
  return ti === '-1'
}

/**
 * Marca no `<html>` que o foco está vindo da navegação (controle ou setas):
 * `data-nav-focus`. O `<Button>` do Fluent só desenha foco quando ele vem
 * do TECLADO (`data-fui-focus-visible`), e o `.focus()` daqui não conta —
 * o botão focado pelo controle ficava sem anel. Com a marca, o `.app`
 * (`useShellStyles`) mostra o anel padrão em `button:focus`. Sai no
 * primeiro clique/toque, pra o mouse não deixar anel nos botões clicados.
 */
export const NAV_FOCUS_ATTR = 'data-nav-focus'
let navFocusListening = false
function markNavFocus() {
  document.documentElement.setAttribute(NAV_FOCUS_ATTR, '')
  if (navFocusListening) return
  navFocusListening = true
  window.addEventListener(
    'pointerdown',
    () => document.documentElement.removeAttribute(NAV_FOCUS_ATTR),
    { capture: true },
  )
}

/** Move o foco do DOM geometricamente na direção dada (bom pra grades). */
export function moveFocus(dir: NavDir) {
  markNavFocus()
  // Nada de `offsetParent` aqui: no WebKitGTK ele volta `null` pra elementos
  // dentro de um container `position: fixed` (o overlay de pausa), o que
  // zerava a lista e a navegação do menu de pausa não funcionava.
  const els = Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => {
    const r = el.getBoundingClientRect()
    return r.width > 0 && r.height > 0 && !navSkipped(el)
  })
  if (els.length === 0) return
  const cur = document.activeElement as HTMLElement | null
  if (!cur || !els.includes(cur)) {
    focusAndReveal(els[0])
    return
  }
  const c = cur.getBoundingClientRect()
  const cx = c.left + c.width / 2
  const cy = c.top + c.height / 2
  let best: HTMLElement | null = null
  let bestScore = Infinity
  for (const el of els) {
    if (el === cur) continue
    const r = el.getBoundingClientRect()
    const dx = r.left + r.width / 2 - cx
    const dy = r.top + r.height / 2 - cy
    const forward = dir === 'up' ? -dy : dir === 'down' ? dy : dir === 'left' ? -dx : dx
    if (forward <= 2) continue
    const cross = dir === 'up' || dir === 'down' ? Math.abs(dx) : Math.abs(dy)
    const score = forward + cross * 2.5
    if (score < bestScore) {
      bestScore = score
      best = el
    }
  }
  if (best) focusAndReveal(best)
}

/** Foca `el` como se viesse do controle/teclado (com o anel de foco). */
export function focusFromNav(el: HTMLElement) {
  markNavFocus()
  focusAndReveal(el)
}

/**
 * Foca sem a rolagem automática do navegador e rola só a página: o
 * `.focus()`/`scrollIntoView` padrão rola TODOS os ancestrais, inclusive
 * caixas com `overflow: hidden` — no carrossel da Início (Fluent `Carousel`)
 * isso deslocava o slide pro lado e a página pra cima, e o destaque ficava
 * cortado pela metade embaixo da topbar (print do usuário, controle). Dentro
 * do carrossel quem aparece na tela é o carrossel inteiro; nas prateleiras
 * o card centraliza na horizontal (faixa rolável estilo Xbox).
 */
function focusAndReveal(el: HTMLElement) {
  el.focus({ preventScroll: true })
  const carousel = el.closest<HTMLElement>('.fui-Carousel')
  const target = carousel ?? el
  target.scrollIntoView({
    block: 'nearest',
    inline: carousel ? 'nearest' : 'center',
    behavior: 'smooth',
  })
}
