import { MessageBar, MessageBarBody, makeStyles, tokens } from '@fluentui/react-components'
import { useEffect } from 'react'
import { useToastStore, type ToastVariant } from '../stores/useToastStore'

const useStyles = makeStyles({
  // Camada independente da state machine de foco: sempre por cima, nunca
  // captura input (`pointerEvents: none`), nunca pausa o core.
  layer: {
    position: 'fixed',
    top: tokens.spacingVerticalM,
    right: tokens.spacingHorizontalM,
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    zIndex: 9999,
    pointerEvents: 'none',
    width: '360px',
    maxWidth: 'calc(100vw - 32px)',
  },
  bar: { pointerEvents: 'auto', overflowWrap: 'anywhere' },
})

const INTENT: Record<ToastVariant, 'info' | 'success' | 'warning' | 'error'> = {
  Info: 'info',
  Success: 'success',
  Warning: 'warning',
  Error: 'error',
}

export function ToastLayer() {
  const styles = useStyles()
  const queue = useToastStore((s) => s.queue)
  const dismiss = useToastStore((s) => s.dismiss)

  // Auto-dismiss por `durationMs` (0 = fica até o usuário fechar).
  useEffect(() => {
    const timers = queue
      .filter((t) => t.durationMs > 0)
      .map((t) => window.setTimeout(() => dismiss(t.id), t.durationMs))
    return () => timers.forEach(window.clearTimeout)
  }, [queue, dismiss])

  return (
    <div className={styles.layer}>
      {queue.map((t) => (
        <MessageBar key={t.id} className={styles.bar} intent={INTENT[t.variant]}>
          <MessageBarBody>{t.message}</MessageBarBody>
        </MessageBar>
      ))}
    </div>
  )
}
