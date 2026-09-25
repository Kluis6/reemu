// Tamanho da interface (Configurações › Aparência). Usa o zoom NATIVO do
// webview (`setZoom`), não o `zoom` do CSS: o nativo muda o tamanho do pixel
// CSS e o layout inteiro é recalculado (inclusive medidas em `vw`), como o
// Ctrl+ de um navegador. Pensado pra TV: a Microsoft recomenda texto de no
// mínimo 15 epx a ~3 m da tela e o próprio Xbox renderiza a 200% em 1080p.

// `label` = chave de tradução (i18n).
export const UI_SCALES = [
  { value: 1, label: 'appearance.uiScale.default' },
  { value: 1.25, label: 'appearance.uiScale.large' },
  { value: 1.5, label: 'appearance.uiScale.larger' },
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

export async function applyUiScale(scale: number): Promise<void> {
  if (!('__TAURI_INTERNALS__' in window)) return
  const { getCurrentWebview } = await import('@tauri-apps/api/webview')
  await getCurrentWebview().setZoom(scale)
}

export async function setUiScale(scale: number): Promise<void> {
  try {
    localStorage.setItem(KEY, String(scale))
  } catch {
    // sem storage: vale só até fechar o app
  }
  await applyUiScale(scale)
}
