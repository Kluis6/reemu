import { useCallback, useEffect } from 'react'
import { isFullscreen, setFullscreen } from '../lib/tauri'
import { useFullscreenStore } from '../stores/useFullscreenStore'

/**
 * Sincroniza o estado de tela cheia e liga o atalho **F11**. Montado uma vez
 * no `RootLayout` (vale em todas as telas, inclusive no jogo).
 * `useFullscreen()` também devolve `{ on, toggle }` pra botões.
 */
export function useFullscreen() {
  const on = useFullscreenStore((s) => s.on)
  const setOn = useFullscreenStore((s) => s.setOn)

  const toggle = useCallback(async () => {
    const next = !useFullscreenStore.getState().on
    try {
      await setFullscreen(next)
      setOn(next)
    } catch {
      /* fora do Tauri */
    }
  }, [setOn])

  return { on, toggle }
}

/** Efeitos globais — chamar só no RootLayout. */
export function useFullscreenSync() {
  const setOn = useFullscreenStore((s) => s.setOn)
  const { toggle } = useFullscreen()

  useEffect(() => {
    void isFullscreen()
      .then(setOn)
      .catch(() => {})
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'F11') {
        e.preventDefault()
        void toggle()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [setOn, toggle])
}
