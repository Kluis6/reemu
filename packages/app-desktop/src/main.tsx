import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './index.css'
import { applyUiScale, getUiScale } from './lib/uiScale'

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
// CSP (`app.security.csp` no tauri.conf.json) bloqueia em silêncio — sem isto
// uma capa/imagem de origem nova só "some" da tela.
document.addEventListener('securitypolicyviolation', (e) =>
  reportToRust(`CSP bloqueou ${e.blockedURI || '(inline)'} (${e.effectiveDirective})`),
)

// Tamanho da interface escolhido em Aparência — antes do 1º render, pra não
// piscar no tamanho padrão.
if (getUiScale() !== 1) applyUiScale(getUiScale()).catch(() => {})

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
