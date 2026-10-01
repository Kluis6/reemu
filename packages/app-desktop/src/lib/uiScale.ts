// Escala da interface no modelo do Xbox (Microsoft, "Designing for Xbox and
// TV"): o Xbox renderiza 1920×1080 a 200%, então toda a interface é desenhada
// em pixels efetivos (epx) numa tela lógica de 960×540, e a TELA INTEIRA
// escala por igual — as proporções ficam idênticas em 720p, 1080p ou 4K.
//
// Aqui o mesmo efeito vem do zoom NATIVO do webview (`setZoom`): com
// `zoom = min(largura / 960, altura / 540)` (tamanho lógico da janela), 1 px
// do CSS vale 1 epx e a viewport tem sempre pelo menos 960×540 — 1080p dá
// zoom 2 (os 200% do Xbox), 4K dá 4. Os estilos usam valores fixos em px (=
// epx, ver styles/metrics.ts), nunca `vw`. Telas mais largas que 16:9
// (ultrawide) ou mais altas (16:10, 4:3) ganham espaço a mais na outra
// dimensão, que o layout preenche (mais cards por fileira, mais linhas).
//
// A preferência em Configurações › Aparência só multiplica esse zoom.

/** Tela lógica de referência do Xbox, em epx. */
export const BASE_W = 960
export const BASE_H = 540

// `label` = chave de tradução (i18n).
export const UI_SCALES = [
  { value: 0.8, label: 'appearance.uiScale.compact' },
  { value: 1, label: 'appearance.uiScale.default' },
  { value: 1.15, label: 'appearance.uiScale.large' },
  { value: 1.3, label: 'appearance.uiScale.larger' },
] as const

const KEY = 'reemu.uiScale'

export function getUiScale(): number {
  try {
    const v = Number(localStorage.getItem(KEY))
    return UI_SCALES.some((s) => s.value === v) ? v : 1
  } catch {
    return 1
  }
}

/** Zoom que encaixa a tela lógica de 960×540 epx numa janela de `w`×`h` px
 *  lógicos (já sem a escala de DPI do sistema), vezes a preferência. */
export function fitZoom(w: number, h: number, user: number): number {
  const fit = Math.min(w / BASE_W, h / BASE_H)
  // Piso só pra janela minúscula não virar zoom ~0 (texto ilegível).
  return Math.max(0.5, fit * user)
}

// Reaplica o zoom pro tamanho atual da janela. `null` fora do Tauri.
let reapply: (() => Promise<void>) | null = null

/** Liga a escala: aplica agora e acompanha redimensionar / trocar de
 *  monitor (mudança de DPI). Chamar uma vez, antes do 1º render. */
export async function startUiScale(): Promise<void> {
  if (!('__TAURI_INTERNALS__' in window)) return
  const [{ getCurrentWebview }, { getCurrentWindow }] = await Promise.all([
    import('@tauri-apps/api/webview'),
    import('@tauri-apps/api/window'),
  ])
  const win = getCurrentWindow()
  const webview = getCurrentWebview()
  let current = 0
  reapply = async () => {
    const [size, sf] = await Promise.all([win.innerSize(), win.scaleFactor()])
    const z = fitZoom(size.width / sf, size.height / sf, getUiScale())
    // Arredonda pra não refazer o layout por diferenças de arredondamento.
    if (Math.abs(z - current) < 0.005) return
    current = z
    await webview.setZoom(z)
  }
  await reapply()
  await win.onResized(() => void reapply?.())
  await win.onScaleChanged(() => void reapply?.())
}

export async function setUiScale(scale: number): Promise<void> {
  try {
    localStorage.setItem(KEY, String(scale))
  } catch {
    // sem storage: vale só até fechar o app
  }
  await reapply?.()
}
