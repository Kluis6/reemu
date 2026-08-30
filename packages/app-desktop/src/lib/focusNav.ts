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

/** Move o foco do DOM geometricamente na direção dada (bom pra grades). */
export function moveFocus(dir: NavDir) {
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
    els[0].focus()
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
  if (best) {
    best.focus()
    best.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' })
  }
}
