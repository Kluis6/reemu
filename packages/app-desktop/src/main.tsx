import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './index.css'
// idiomas (pt-BR/en/es) — inicializa antes do 1º render
import './i18n'
import { startUiScale } from './lib/uiScale'

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

// Botão direito do mouse bloqueado: nem o menu nativo do webview (Recarregar,
// Inspecionar…) nem os menus de contexto do app abrem por ele. Captura na
// janela, antes de qualquer componente. Só o clique real do botão direito
// (`isTrusted` + `button === 2`): o botão Menu do controle (evento sintético,
// ver hooks/useMenuNav.ts) e a tecla Menu do teclado (`button === 0`)
// continuam abrindo os menus do app. Em desenvolvimento fica liberado, pro
// "Inspecionar elemento".
if (!import.meta.env.DEV) {
  window.addEventListener(
    'contextmenu',
    (e) => {
      if (e.isTrusted && e.button === 2) {
        e.preventDefault()
        e.stopPropagation()
      }
    },
    { capture: true },
  )
}

// Escala modo Xbox (tela lógica de 1366×768 epx, ver lib/uiScale.ts) — aplicada
// antes do 1º render, pra não piscar no tamanho errado.
startUiScale()
  .catch((e) => reportToRust(`escala da interface: ${String(e)}`))
  .finally(() =>
    createRoot(document.getElementById('root')!).render(
      <StrictMode>
        <App />
      </StrictMode>,
    ),
  )
