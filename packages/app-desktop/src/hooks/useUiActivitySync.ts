import { useEffect } from 'react'
import { alvanistaSetUiActive } from '../lib/tauri'

const DEBOUNCE_MS = 150

let activeSubscribers = 0
let debounceTimer: ReturnType<typeof setTimeout> | null = null
let lastSent: { visible: boolean; focused: boolean } | null = null

function readUiState(): { visible: boolean; focused: boolean } {
  if (typeof document === 'undefined') {
    return { visible: true, focused: true }
  }
  const visible = document.visibilityState === 'visible'
  const focused = typeof document.hasFocus === 'function' ? document.hasFocus() : true
  return { visible, focused }
}

function flushUiState() {
  debounceTimer = null
  const next = readUiState()
  if (lastSent && lastSent.visible === next.visible && lastSent.focused === next.focused) {
    return
  }
  lastSent = next
  void alvanistaSetUiActive(next.visible, next.focused).catch(() => {})
}

function scheduleUiStateSync() {
  if (debounceTimer !== null) {
    clearTimeout(debounceTimer)
  }
  debounceTimer = setTimeout(flushUiState, DEBOUNCE_MS)
}

/**
 * Sincroniza visibilidade (`document.visibilityState`) e foco (`window` `focus` / `blur`)
 * com o coordenador de presenca no Rust, agrupando transicoes rapidas em `150 ms`.
 */
export function useUiActivitySync() {
  useEffect(() => {
    if (typeof window === 'undefined' || typeof document === 'undefined') {
      return
    }
    activeSubscribers += 1
    if (activeSubscribers === 1) {
      lastSent = null
      flushUiState()
      document.addEventListener('visibilitychange', scheduleUiStateSync)
      window.addEventListener('focus', scheduleUiStateSync)
      window.addEventListener('blur', scheduleUiStateSync)
    }

    return () => {
      activeSubscribers = Math.max(0, activeSubscribers - 1)
      if (activeSubscribers === 0) {
        if (debounceTimer !== null) {
          clearTimeout(debounceTimer)
          debounceTimer = null
        }
        document.removeEventListener('visibilitychange', scheduleUiStateSync)
        window.removeEventListener('focus', scheduleUiStateSync)
        window.removeEventListener('blur', scheduleUiStateSync)
      }
    }
  }, [])
}
