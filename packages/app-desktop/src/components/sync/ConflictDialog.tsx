import { Badge, Body1, Body1Strong, Button, Caption1, MessageBar, MessageBarBody, makeStyles, tokens } from '@fluentui/react-components'
import { useEffect, useRef, useState, type Ref } from 'react'
import { useTranslation } from 'react-i18next'
import { syncErrorCode, useResolve } from '../../hooks/useSync'
import { formatDateTime } from '../../lib/metadataFormat'
import type { Side, SyncConflict } from '../../lib/tauri'
import { formatBytes, machineColor } from '../../screens/settings/syncFormat'
import { FullscreenDialog } from '../FullscreenDialog'

/** Como o diálogo terminou: o lado que ficou, "Decidir depois" ou fechado
 *  pelo X, Esc ou Voltar do controle (`dismiss`, que não decide nem segue). */
export type ConflictOutcome = 'local' | 'remote' | 'later' | 'dismiss'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalXL, maxWidth: '980px' },
  cards: {
    display: 'grid',
    gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
    columnGap: tokens.spacingHorizontalXL,
    '@media (max-width: 720px)': { gridTemplateColumns: 'minmax(0, 1fr)', rowGap: tokens.spacingVerticalL },
  },
  card: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalM,
    padding: tokens.spacingHorizontalXL,
    borderRadius: tokens.borderRadiusLarge,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  head: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalM, flexWrap: 'wrap' },
  avatar: {
    flexShrink: 0,
    width: '40px',
    height: '40px',
    borderRadius: tokens.borderRadiusCircular,
    display: 'grid',
    placeItems: 'center',
    color: '#fff',
    fontWeight: tokens.fontWeightSemibold,
  },
  facts: { display: 'grid', gridTemplateColumns: 'auto 1fr', columnGap: tokens.spacingHorizontalL, rowGap: '2px', margin: 0 },
  label: { color: tokens.colorNeutralForeground3, fontSize: tokens.fontSizeBase200 },
  value: { margin: 0, fontSize: tokens.fontSizeBase300 },
  muted: { color: tokens.colorNeutralForeground3 },
})

function Card({
  side,
  mine,
  lang,
  action,
  busy,
  onPick,
  buttonRef,
}: {
  side: Side
  mine: boolean
  lang: string
  action: string
  busy: boolean
  onPick: () => void
  buttonRef?: Ref<HTMLButtonElement>
}) {
  const { t } = useTranslation()
  const s = useStyles()
  return (
    <div className={s.card}>
      <div className={s.head}>
        <span className={s.avatar} style={{ backgroundColor: machineColor(side.deviceId ?? side.machine) }} aria-hidden>
          {(side.machine.trim()[0] ?? '?').toUpperCase()}
        </span>
        <Body1Strong>{side.machine}</Body1Strong>
        {mine && (
          <Badge appearance="tint" color="brand">
            {t('sync.conflict.thisMachine')}
          </Badge>
        )}
      </div>
      <dl className={s.facts}>
        <dt className={s.label}>{t('sync.conflict.saved')}</dt>
        <dd className={s.value}>{formatDateTime(side.savedAt)}</dd>
        <dt className={s.label}>{t('sync.conflict.size')}</dt>
        <dd className={s.value}>{formatBytes(side.size, lang)}</dd>
      </dl>
      <Button
        ref={buttonRef}
        autoFocus={mine}
        appearance={mine ? 'primary' : 'secondary'}
        disabledFocusable={busy}
        onClick={onPick}
      >
        {action}
      </Button>
    </div>
  )
}

/**
 * O conflito de um save (D118): as duas máquinas jogaram desde a última
 * sincronização e a pessoa escolhe qual continua. Nada é apagado.
 * O foco começa no botão do save desta máquina; esquerda e direita levam de um
 * cartão ao outro (a navegação geométrica do app).
 */
export function ConflictDialog({
  conflict,
  onDone,
}: {
  conflict: SyncConflict
  onDone: (outcome: ConflictOutcome) => void
}) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const resolve = useResolve()
  const [failed, setFailed] = useState<string | null>(null)
  const localRef = useRef<HTMLButtonElement>(null)

  // `autoFocus` no botão do save desta máquina é o caminho certo; o foco
  // por tempo só cobre o caso do Fluent ter levado o foco ao fechar antes.
  useEffect(() => {
    const id = window.setTimeout(() => localRef.current?.focus(), 0)
    return () => window.clearTimeout(id)
  }, [conflict.key])

  const pick = async (keep: 'local' | 'remote') => {
    if (resolve.isPending) return
    setFailed(null)
    try {
      await resolve.mutateAsync({ key: conflict.key, keep })
      onDone(keep)
    } catch (err) {
      setFailed(syncErrorCode(err))
    }
  }

  return (
    <FullscreenDialog
      open
      onClose={() => onDone('dismiss')}
      title={t('sync.conflict.title', { title: conflict.title })}
      actions={
        <Button appearance="secondary" disabledFocusable={resolve.isPending} onClick={() => onDone('later')}>
          {t('sync.conflict.later')}
        </Button>
      }
    >
      <div className={s.root}>
        <Body1>{t('sync.conflict.body')}</Body1>
        {failed && (
          <MessageBar intent="warning">
            <MessageBarBody>{t(`sync.err.${failed as 'failed'}`)}</MessageBarBody>
          </MessageBar>
        )}
        <div className={s.cards}>
          <Card
            side={conflict.local}
            mine
            lang={i18n.language}
            action={t('sync.conflict.keepThis')}
            busy={resolve.isPending}
            onPick={() => void pick('local')}
            buttonRef={localRef}
          />
          <Card
            side={conflict.remote}
            mine={false}
            lang={i18n.language}
            action={t('sync.conflict.keepOther', { machine: conflict.remote.machine })}
            busy={resolve.isPending}
            onPick={() => void pick('remote')}
          />
        </div>
        <Caption1 className={s.muted}>{t('sync.conflict.footer')}</Caption1>
      </div>
    </FullscreenDialog>
  )
}
