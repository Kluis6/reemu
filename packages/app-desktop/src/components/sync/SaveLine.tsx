import { Button, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import {
  CloudArrowUpRegular,
  CloudCheckmarkRegular,
  CloudOffRegular,
  WarningRegular,
} from '@fluentui/react-icons'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { relativeAge } from '../../screens/profile/relativeAge'
import { savesActive, useConflicts, useSaveStatus, useSyncStatus } from '../../hooks/useSync'
import { HistoryDialog } from './HistoryDialog'

const useStyles = makeStyles({
  line: { display: 'flex', alignItems: 'center', flexWrap: 'wrap', columnGap: '8px', color: tokens.colorNeutralForeground2 },
  icon: { fontSize: '20px', flexShrink: 0 },
  link: { minWidth: 0, paddingInline: tokens.spacingHorizontalXS },
})

/**
 * A linha do game save na página do jogo: nuvem + o estado (sincronizado,
 * ainda não enviado, em conflito, só nesta máquina) e o link do histórico.
 * Não mostra nada sem conta, sem sync ligado ou com o interruptor dos game
 * saves desligado, nem quando o jogo não tem save.
 */
export function SaveLine({
  romId,
  className,
  iconClassName,
}: {
  romId: string
  className?: string
  iconClassName?: string
}) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const status = useSyncStatus()
  const active = savesActive(status.data)
  const line = useSaveStatus(romId, active)
  const state = line.data?.state
  const conflicts = useConflicts(active && state === 'conflict')
  const [historyOpen, setHistoryOpen] = useState(false)
  const [nowMs] = useState(() => Date.now())

  if (!active || !line.data || state === undefined || state === 'none') return null
  const { key, versions, syncedAt } = line.data

  let text: string
  let Icon = CloudCheckmarkRegular
  if (state === 'synced') {
    text = t('sync.save.synced', { when: relativeAge(syncedAt ?? nowMs / 1000, nowMs, i18n.language) })
  } else if (state === 'pending') {
    text = t('sync.save.pending')
    Icon = CloudArrowUpRegular
  } else if (state === 'conflict') {
    const c = conflicts.data?.find((x) => x.key === key)
    text = c
      ? t('sync.save.conflict', { local: c.local.machine, remote: c.remote.machine })
      : t('sync.save.conflictShort')
    Icon = WarningRegular
  } else {
    text = t('sync.save.localOnly')
    Icon = CloudOffRegular
  }

  return (
    <div className={mergeClasses(s.line, className)}>
      <Icon className={mergeClasses(s.icon, iconClassName)} aria-hidden />
      <span>{text}</span>
      {key !== null && versions > 0 && (
        <Button appearance="transparent" size="small" className={s.link} onClick={() => setHistoryOpen(true)}>
          {t('sync.save.history', { count: versions })}
        </Button>
      )}
      {historyOpen && key !== null && <HistoryDialog romKey={key} onClose={() => setHistoryOpen(false)} />}
    </div>
  )
}
