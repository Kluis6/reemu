// Escala da interface nas proporções do app Xbox do Windows: a interface é
// desenhada em pixels efetivos (epx) numa tela lógica de 1366×768 — onde o
// app Xbox desenha 1 px = 1 px, e de onde vêm as medidas de
// styles/metrics.ts — e a TELA INTEIRA escala por igual, então as proporções
// ficam idênticas em 720p, 1080p ou 4K.
//
// Antes a referência era 960×540 (Xbox console, 1080p a 200%): com ela, em
// 1366×768 tudo saía 1,42× maior que no app Xbox (rail de 91 px contra 68).
//
// O efeito vem do zoom NATIVO do webview (`setZoom`): com
// `zoom = min(largura / 1366, altura / 768)` (tamanho lógico da janela), 1 px
// do CSS vale 1 epx e a viewport tem sempre pelo menos 1366×768 — 1080p dá
// zoom ~1,41, 4K dá ~2,81. Os estilos usam valores fixos em px (=
// epx, ver styles/metrics.ts), nunca `vw`. Telas mais largas que 16:9
// (ultrawide) ou mais altas (16:10, 4:3) ganham espaço a mais na outra
// dimensão, que o layout preenche (mais cards por fileira, mais linhas).
//
// A preferência em Configurações › Sistema só multiplica esse zoom.
//
// Janela menor que a tela lógica não encolhe: o zoom para em 1 e a tela se
// reorganiza. Janela maior escala igual em tela cheia e em janela, como o app Xbox
// (capturas de 2026-10-30, ver docs/ai-context/18-experiencia-console-tv.md). O vídeo
// do jogo não depende do zoom: a janela nativa usa o tamanho físico da janela do
// app, e o `<canvas>` mede em pixels do dispositivo.

/** Tela lógica de referência do Xbox, em epx. */
// 1280×720, e não 1366×768: nas capturas de 2026-10-30 (4K), barra lateral, busca,
// texto e cards do app Xbox batem com esta base, com diferença de até 3%.
export const BASE_W = 1280
export const BASE_H = 720

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

const OVERSCAN_KEY = 'reemu.overscan'

/** Margem contra o corte das bordas de uma TV, em % da tela. */
export const OVERSCANS = [
  { value: 0, label: 'appearance.overscan.none' },
  { value: 2.5, label: 'appearance.overscan.small' },
  { value: 5, label: 'appearance.overscan.large' },
] as const

export function getOverscan(): number {
  try {
    const v = Number(localStorage.getItem(OVERSCAN_KEY))
    return OVERSCANS.some((o) => o.value === v) ? v : 0
  } catch {
    return 0
  }
}

let isFullscreen = false

/** Em janela, a margem não faz sentido: só a tela cheia pode ser cortada pela TV. */
function applyOverscan() {
  document.documentElement.style.setProperty(
    '--reemuOverscan',
    String(isFullscreen ? getOverscan() : 0),
  )
}

/** Zoom de uma janela de `w`×`h` px lógicos (já sem a escala de DPI do sistema):
 *  encaixa a tela lógica de 1366×768 epx quando ela cabe, sem encolher abaixo
 *  dela, vezes a preferência. */
export function fitZoom(w: number, h: number, user: number): number {
  const fit = Math.max(1, Math.min(w / BASE_W, h / BASE_H))
  return fit * user
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
    const [size, sf, fullscreen] = await Promise.all([
      win.innerSize(),
      win.scaleFactor(),
      win.isFullscreen(),
    ])
    isFullscreen = fullscreen
    applyOverscan()
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

export function setOverscan(percent: number) {
  try {
    localStorage.setItem(OVERSCAN_KEY, String(percent))
  } catch {
    // sem storage: vale só até fechar o app
  }
  applyOverscan()
}
