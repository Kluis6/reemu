import { useEffect } from 'react'
import { inputKey } from '../lib/tauri'

// Códigos que a UI usa (foco no menu, campos de texto) e não devem ir pro core.
const UI_KEYS = new Set(['Tab', 'F5', 'F12'])

/**
 * Encaminha teclado da webview → backend (`input_key`). O Rust decide o que
 * é hotkey e o que é input de jogo (só em `GameFocused`).
 */
export function useKeyboardInput() {
  useEffect(() => {
    const onKey = (pressed: boolean) => (e: KeyboardEvent) => {
      if (e.repeat || UI_KEYS.has(e.code)) return
      // Não rouba teclado de inputs/textareas do menu.
      const t = e.target as HTMLElement | null
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return
      void inputKey(e.code, pressed).catch(() => {})
    }
    const down = onKey(true)
    const up = onKey(false)
    window.addEventListener('keydown', down)
    window.addEventListener('keyup', up)
    return () => {
      window.removeEventListener('keydown', down)
      window.removeEventListener('keyup', up)
    }
  }, [])
}
