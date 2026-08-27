import { useEffect } from 'react'
import { onFocusChanged } from '../lib/tauri'
import { useFocusStore } from '../stores/useFocusStore'

/** Espelha o `InputFocus` decidido no Rust (evento `focus-changed`). */
export function useFocusBridge() {
  const setFocus = useFocusStore((s) => s.setFocus)
  useEffect(() => {
    let unlisten: (() => void) | undefined
    onFocusChanged(setFocus).then((fn) => {
      unlisten = fn
    })
    return () => unlisten?.()
  }, [setFocus])
}
