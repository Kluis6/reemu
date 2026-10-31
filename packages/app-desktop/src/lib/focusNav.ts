export type NavDir = 'up' | 'down' | 'left' | 'right'

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'

/** Camadas que tomam o controle: enquanto uma está aberta, o foco não sai dela. */
const OVERLAY = '[role="menu"], .fui-PopoverSurface, .fui-DialogSurface, .fui-OverlayDrawer'

/** Área de conteúdo do `AppShell`, onde o foco entra quando ainda não há foco. */
export const NAV_CONTENT = '[data-nav-content]'

/** Avisa quem mostra as dicas de botão que o estado da navegação mudou. */
export const NAV_STATE_EVENT = 'reemu-nav-state'

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
 * (`useShellStyles`) mostra o anel padrão em `button:focus`, e o `index.css`
 * esconde cursor, hover e o botão Voltar. Sai ao mexer ou clicar com o mouse.
 */
export const NAV_FOCUS_ATTR = 'data-nav-focus'
let navFocusListening = false
function markNavFocus() {
  document.documentElement.setAttribute(NAV_FOCUS_ATTR, '')
  if (navFocusListening) return
  navFocusListening = true
  const leave = () => {
    if (!document.documentElement.hasAttribute(NAV_FOCUS_ATTR)) return
    document.documentElement.removeAttribute(NAV_FOCUS_ATTR)
    notifyNavState()
  }
  window.addEventListener('pointerdown', leave, { capture: true })
  window.addEventListener('wheel', leave, { capture: true, passive: true })
  // Compara a posição: o sistema manda `pointermove` sem o ponteiro ter saído do
  // lugar, por exemplo ao mostrar a janela.
  let at: { x: number; y: number } | null = null
  window.addEventListener(
    'pointermove',
    (e) => {
      if (at && (at.x !== e.clientX || at.y !== e.clientY)) leave()
      at = { x: e.clientX, y: e.clientY }
    },
    { capture: true },
  )
}

/** O foco está vindo do controle ou das setas? */
export function inNavMode(): boolean {
  return document.documentElement.hasAttribute(NAV_FOCUS_ATTR)
}

function notifyNavState() {
  window.dispatchEvent(new Event(NAV_STATE_EVENT))
}

/** A camada aberta por cima de tudo, ou o documento. Os portais do Fluent entram
 *  na ordem em que abrem, então a última é a de cima. */
export function navScope(): ParentNode {
  const open = Array.from(document.querySelectorAll<HTMLElement>(OVERLAY)).filter((el) => {
    const r = el.getBoundingClientRect()
    return r.width > 0 && r.height > 0
  })
  return open[open.length - 1] ?? document
}

/** O que o controle alcança dentro de `scope`. */
export function navCandidates(scope: ParentNode = navScope()): HTMLElement[] {
  // Nada de `offsetParent` aqui: no WebKitGTK ele volta `null` pra elementos
  // dentro de um container `position: fixed` (o overlay de pausa), o que
  // zerava a lista e a navegação do menu de pausa não funcionava.
  return Array.from(scope.querySelectorAll<HTMLElement>(FOCUSABLE)).filter((el) => {
    const r = el.getBoundingClientRect()
    return (
      r.width > 0 &&
      r.height > 0 &&
      !navSkipped(el) &&
      getComputedStyle(el).visibility !== 'hidden'
    )
  })
}

/** Move o foco do DOM geometricamente na direção dada (bom pra grades). */
export function moveFocus(dir: NavDir) {
  markNavFocus()
  const els = navCandidates()
  if (els.length === 0) return
  const cur = document.activeElement as HTMLElement | null
  if (!cur || !els.includes(cur)) {
    focusAndReveal(els.find((el) => el.closest(NAV_CONTENT)) ?? els[0])
    return
  }
  const c = cur.getBoundingClientRect()
  const cx = c.left + c.width / 2
  const cy = c.top + c.height / 2
  const vertical = dir === 'up' || dir === 'down'
  const inRail = !!cur.closest('[data-nav-rail]')
  let best: HTMLElement | null = null
  let bestScore = Infinity
  for (const el of els) {
    if (el === cur) continue
    // Só se entra e sai da barra lateral na horizontal: no fim do conteúdo, descer
    // pararia no sino, lá embaixo na barra.
    if (vertical && !!el.closest('[data-nav-rail]') !== inRail) continue
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

/** LT/RT: pula cerca de uma tela, para o item mais próximo daquela altura. */
export function pageFocus(step: -1 | 1) {
  markNavFocus()
  const els = navCandidates()
  const cur = document.activeElement as HTMLElement | null
  if (!cur || !els.includes(cur)) {
    moveFocus(step < 0 ? 'up' : 'down')
    return
  }
  const c = cur.getBoundingClientRect()
  const cx = c.left + c.width / 2
  const targetY = c.top + c.height / 2 + step * window.innerHeight * 0.8
  let best: HTMLElement | null = null
  let bestScore = Infinity
  for (const el of els) {
    // o salto vale só para o conteúdo: a barra lateral fica fora da rolagem
    if (el === cur || !el.closest(NAV_CONTENT)) continue
    const r = el.getBoundingClientRect()
    const dy = (r.top + r.height / 2 - (c.top + c.height / 2)) * step
    if (dy <= 2) continue
    const score =
      Math.abs(r.top + r.height / 2 - targetY) + Math.abs(r.left + r.width / 2 - cx) * 0.25
    if (score < bestScore) {
      bestScore = score
      best = el
    }
  }
  if (best) focusAndReveal(best)
}

let beforeRail: HTMLElement | null = null

/** View: leva o foco à barra lateral; de lá, volta para onde estava. */
export function toggleRailFocus() {
  markNavFocus()
  const cur = document.activeElement as HTMLElement | null
  const rail = document.querySelector<HTMLElement>('[data-nav-rail]')
  if (!rail || navScope() !== document) return
  if (cur && rail.contains(cur)) {
    if (beforeRail?.isConnected) focusAndReveal(beforeRail)
    else moveFocus('right')
    return
  }
  beforeRail = cur
  const target =
    rail.querySelector<HTMLElement>('[aria-current="page"]') ?? navCandidates(rail)[0]
  if (target) focusFromNav(target)
}

/** Listas de abas de verdade: os pontinhos do carrossel também são `tablist`. */
export function tabLists(scope: ParentNode = navScope()): HTMLElement[] {
  return Array.from(scope.querySelectorAll<HTMLElement>('[role="tablist"]')).filter(
    (l) => !l.closest('.fui-Carousel'),
  )
}

/** LB/RB: troca a aba selecionada da lista de abas da camada aberta. */
export function cycleTab(step: -1 | 1): boolean {
  const lists = tabLists()
  // a do item em foco ganha; senão, a primeira da tela (a principal)
  const active = document.activeElement
  const list = lists.find((l) => active && l.contains(active)) ?? lists[0]
  if (!list) return false
  const tabs = Array.from(list.querySelectorAll<HTMLElement>('[role="tab"]')).filter(
    (t) => t.getAttribute('aria-disabled') !== 'true' && !t.hasAttribute('disabled'),
  )
  if (tabs.length < 2) return false
  const cur = tabs.findIndex((t) => t.getAttribute('aria-selected') === 'true')
  const next = tabs[(cur + step + tabs.length) % tabs.length]
  markNavFocus()
  next.click()
  focusAndReveal(next)
  return true
}

/** Seletor e slider: com o controle, A entra no ajuste e B sai. Sem isso, as setas
 *  não sairiam mais do controle (Microsoft, "focus engagement"). */
export function isAdjustable(el: Element | null): el is HTMLSelectElement | HTMLInputElement {
  return (
    el instanceof HTMLSelectElement || (el instanceof HTMLInputElement && el.type === 'range')
  )
}

const ENGAGED_ATTR = 'data-nav-engaged'
const SLIDER_PRESSES = 50
let engaged: HTMLSelectElement | HTMLInputElement | null = null

export function engagedControl() {
  return engaged
}

export function disengage() {
  if (!engaged) return
  engaged.removeAttribute(ENGAGED_ATTR)
  engaged.removeEventListener('blur', disengage)
  engaged = null
  notifyNavState()
}

export function engage(el: HTMLSelectElement | HTMLInputElement) {
  disengage()
  engaged = el
  el.setAttribute(ENGAGED_ATTR, '')
  el.addEventListener('blur', disengage)
  notifyNavState()
}

/** Muda o valor do controle em ajuste, como o usuário faria. */
export function adjustEngaged(step: -1 | 1) {
  const el = engaged
  if (!el) return
  if (el instanceof HTMLSelectElement) {
    const next = el.selectedIndex + step
    if (next < 0 || next >= el.options.length) return
    el.selectedIndex = next
    el.dispatchEvent(new Event('change', { bubbles: true }))
    return
  }
  const before = el.value
  // Cerca de 50 toques de ponta a ponta, mesmo em slider de passo fino.
  const unit = Number(el.step) || 1
  const count = Math.max(1, Math.ceil((Number(el.max) - Number(el.min)) / unit / SLIDER_PRESSES))
  if (step > 0) el.stepUp(count)
  else el.stepDown(count)
  if (el.value !== before) el.dispatchEvent(new Event('input', { bubbles: true }))
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
