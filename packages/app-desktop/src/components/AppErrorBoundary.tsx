import { Component, type ErrorInfo, type ReactNode } from 'react'
import i18n from '../i18n'
import { jsLog } from '../lib/tauri'

/**
 * Último recurso: um erro de renderização fora das rotas (tema, provedores,
 * a própria casca) apagava o app inteiro, sem mensagem e sem log. As telas
 * já têm o `RouteError` (errorElement do router); este cobre o que fica por
 * fora dele. Sem Fluent de propósito: o tema pode ser a causa.
 */
export class AppErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null }

  static getDerivedStateFromError(error: Error) {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    jsLog('error', `app quebrou: ${error.message}\n${error.stack ?? ''}${info.componentStack ?? ''}`)
  }

  render() {
    const { error } = this.state
    if (!error) return this.props.children
    return (
      <div
        role="alert"
        data-app-error
        style={{
          minHeight: '100vh',
          display: 'grid',
          placeItems: 'center',
          padding: 32,
          background: '#1f1f1f',
          color: '#f3f3f3',
          fontFamily: 'Segoe UI, system-ui, sans-serif',
          textAlign: 'center',
        }}
      >
        <div style={{ maxWidth: 560 }}>
          <h1 style={{ fontSize: 24, margin: '0 0 12px' }}>{i18n.t('shell2.appCrashed')}</h1>
          <p style={{ margin: '0 0 20px', opacity: 0.85 }}>{i18n.t('shell2.appCrashedHint')}</p>
          <button
            type="button"
            onClick={() => location.reload()}
            style={{
              fontSize: 16,
              padding: '10px 24px',
              borderRadius: 4,
              border: 'none',
              background: '#107c10',
              color: '#fff',
              cursor: 'pointer',
            }}
          >
            {i18n.t('common.retry')}
          </button>
          <p style={{ marginTop: 20, fontSize: 12, opacity: 0.6, overflowWrap: 'anywhere' }}>{error.message}</p>
        </div>
      </div>
    )
  }
}
