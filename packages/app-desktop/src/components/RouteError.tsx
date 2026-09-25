import { Button, makeStyles, tokens } from '@fluentui/react-components'
import { useEffect } from 'react'
import { isRouteErrorResponse, useNavigate, useRouteError } from 'react-router-dom'
import { jsLog } from '../lib/tauri'
import { EmptyState } from './EmptyState'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: {
    minHeight: '60vh',
    display: 'grid',
    placeItems: 'center',
    padding: tokens.spacingHorizontalXXL,
  },
  actions: { display: 'flex', gap: tokens.spacingHorizontalM, justifyContent: 'center' },
  detail: {
    marginTop: tokens.spacingVerticalS,
    color: tokens.colorNeutralForeground3,
    fontSize: tokens.fontSizeBase200,
    overflowWrap: 'anywhere',
  },
})

/**
 * Erro inesperado numa tela: em vez da página técnica do React Router (em
 * inglês, com stack trace), uma mensagem em português com saída — tentar de
 * novo ou voltar ao início. O erro vai pro log do Rust pra diagnóstico.
 * Montado como `errorElement` em `router.tsx` (dentro da casca, então rail e
 * topbar continuam funcionando).
 */
export function RouteError() {
  const { t } = useTranslation()
  const s = useStyles()
  const error = useRouteError()
  const navigate = useNavigate()
  const detail = isRouteErrorResponse(error)
    ? `${error.status} ${error.statusText}`
    : error instanceof Error
      ? error.message
      : String(error)

  useEffect(() => {
    const stack = error instanceof Error && error.stack ? `\n${error.stack}` : ''
    jsLog('error', `tela quebrou: ${detail}${stack}`)
  }, [error, detail])

  return (
    <div className={s.root} role="alert">
      <EmptyState
        title={t('shell2.routeError')}
        action={
          <div className={s.actions}>
            <Button appearance="primary" onClick={() => navigate(0)}>
              {t('common.retry')}
            </Button>
            <Button onClick={() => navigate('/', { replace: true })}>{t('shell2.goHome')}</Button>
          </div>
        }
      >
        {t('shell2.routeErrorHint')}
        <div className={s.detail}>{detail}</div>
      </EmptyState>
    </div>
  )
}
