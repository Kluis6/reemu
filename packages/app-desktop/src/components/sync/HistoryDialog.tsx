import { Badge, Body1, Button, Caption1, MessageBar, MessageBarBody, Spinner, makeStyles, tokens } from '@fluentui/react-components'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { syncErrorCode, useRestore, useVersions } from '../../hooks/useSync'
import { formatBytes, machineColor } from '../../screens/settings/syncFormat'
import { FullscreenDialog } from '../FullscreenDialog'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalS, maxWidth: '980px' },
  row: {
    display: 'flex',
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalL,
    flexWrap: 'wrap',
    padding: `${tokens.spacingVerticalM} ${tokens.spacingHorizontalL}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  dot: { flexShrink: 0, width: '12px', height: '12px', borderRadius: tokens.borderRadiusCircular },
  text: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalS, flexWrap: 'wrap', flexGrow: 1, minWidth: 0 },
  confirm: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalS, flexWrap: 'wrap' },
  muted: { color: tokens.colorNeutralForeground3 },
})

/**
 * O histórico de um save (D124): `máquina · data · tamanho`, `atual` na versão
 * que vale e `Restaurar esta` nas outras, com uma confirmação antes (o save de
 * agora vai para a pasta de resguardo).
 */
export function HistoryDialog({ romKey, onClose }: { romKey: string; onClose: () => void }) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const versions = useVersions(romKey)
  const restore = useRestore()
  const [asking, setAsking] = useState<string | null>(null)
  const [failed, setFailed] = useState<string | null>(null)
  const firstRef = useRef<HTMLButtonElement>(null)
  const confirmRef = useRef<HTMLButtonElement>(null)

  const list = versions.data ?? []
  const firstOther = list.find((v) => !v.isCurrent)?.versionId

  // O foco inicial: o primeiro "Restaurar esta" (e o "Restaurar" da confirmação
  // quando ela abre), para o controle não começar no fechar.
  useEffect(() => {
    const id = window.setTimeout(() => (confirmRef.current ?? firstRef.current)?.focus(), 0)
    return () => window.clearTimeout(id)
  }, [firstOther, asking])

  const go = async (versionId: string) => {
    if (restore.isPending) return
    setFailed(null)
    try {
      await restore.mutateAsync({ key: romKey, versionId })
      onClose()
    } catch (err) {
      setFailed(syncErrorCode(err))
      setAsking(null)
    }
  }

  return (
    <FullscreenDialog open onClose={onClose} title={t('sync.history.title')}>
      <div className={s.root}>
        {failed && (
          <MessageBar intent="warning">
            <MessageBarBody>{t(`sync.err.${failed as 'failed'}`)}</MessageBarBody>
          </MessageBar>
        )}
        {versions.isLoading && <Spinner size="tiny" label={t('sync.history.loading')} labelPosition="after" />}
        {versions.isError && (
          <MessageBar intent="warning">
            <MessageBarBody>{t(`sync.err.${syncErrorCode(versions.error) as 'failed'}`)}</MessageBarBody>
          </MessageBar>
        )}
        {versions.isSuccess && list.length === 0 && <Caption1 className={s.muted}>{t('sync.history.empty')}</Caption1>}
        {list.map((v) => (
          <div key={v.versionId} className={s.row}>
            <span className={s.dot} style={{ backgroundColor: machineColor(v.machine) }} aria-hidden />
            <span className={s.text}>
              <Body1>
                {v.machine} · {new Date(v.createdAt * 1000).toLocaleString(i18n.language)} ·{' '}
                {formatBytes(v.size, i18n.language)}
              </Body1>
              {v.isCurrent && (
                <Badge appearance="tint" color="brand">
                  {t('sync.history.current')}
                </Badge>
              )}
            </span>
            {!v.isCurrent &&
              (asking === v.versionId ? (
                <span className={s.confirm}>
                  <Caption1 className={s.muted}>{t('sync.history.confirm')}</Caption1>
                  <Button
                    ref={confirmRef}
                    appearance="primary"
                    disabledFocusable={restore.isPending}
                    onClick={() => void go(v.versionId)}
                  >
                    {t('sync.history.confirmYes')}
                  </Button>
                  <Button appearance="secondary" disabledFocusable={restore.isPending} onClick={() => setAsking(null)}>
                    {t('common.cancel')}
                  </Button>
                </span>
              ) : (
                <Button
                  ref={v.versionId === firstOther ? firstRef : undefined}
                  appearance="secondary"
                  disabledFocusable={restore.isPending}
                  onClick={() => setAsking(v.versionId)}
                >
                  {t('sync.history.restore')}
                </Button>
              ))}
          </div>
        ))}
      </div>
    </FullscreenDialog>
  )
}
