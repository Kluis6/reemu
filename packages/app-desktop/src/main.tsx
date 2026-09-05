import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './index.css'

// A webview é opaca (o vídeo do jogo é desenhado num canvas dentro dela, não
// atrás) — ver apps/desktop/src-tauri/src/main.rs pro histórico.

// Manda erros não tratados pro stdout do Rust (facilita diagnóstico da webview).
function reportToRust(message: string) {
  try {
    ;(window as { __TAURI_INTERNALS__?: { invoke?: (c: string, a: unknown) => unknown } })
      .__TAURI_INTERNALS__?.invoke?.('js_log', { level: 'error', message })
  } catch {
    /* fora do Tauri */
  }
}
window.addEventListener('error', (e) => reportToRust(`${e.message} @ ${e.filename}:${e.lineno}`))
window.addEventListener('unhandledrejection', (e) =>
  reportToRust(`unhandledrejection: ${String(e.reason?.stack ?? e.reason)}`),
)

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
