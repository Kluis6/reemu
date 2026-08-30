import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouterProvider } from 'react-router-dom'
import './index.css'
import { router } from './router'

// A webview é opaca (o vídeo do jogo é desenhado num canvas dentro dela, não
// atrás) — ver apps/desktop/src-tauri/src/main.rs pro histórico.
// `refetchOnWindowFocus` desligado: o WebKitGTK dispara foco/visibilidade
// espúrios e a biblioteca re-renderizava sozinha (cartões "piscando"). Os
// dados só mudam em ações explícitas, que já invalidam a query.
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { refetchOnWindowFocus: false, staleTime: 5 * 60_000 },
  },
})

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
    <FluentProvider theme={webDarkTheme} style={{ minHeight: '100%' }}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </FluentProvider>
  </StrictMode>,
)
