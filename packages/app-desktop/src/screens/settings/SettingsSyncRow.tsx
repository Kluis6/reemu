import {
  Body1,
  Button,
  Caption1,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FieldBlock } from '../../components/SettingBlock'
import { SYNC_STATUS_KEY, syncErrorCode } from '../../hooks/useSync'
import { applyAppearance, exportAppearance } from '../../lib/appearanceGroup'
import { formatDateTime } from '../../lib/metadataFormat'
import {
  SETTINGS_SENT_KEY,
  SETTINGS_UNDO_KEY,
  clearAppearanceUndo,
  readAppearanceUndo,
  settingsGroupErrorText,
  type SettingsGroupName,
} from '../../lib/settingsSync'
import { alvanistaOpenSite, syncSettingsSent, syncSettingsUndo, syncSettingsUndoInfo, syncSettingsUpload } from '../../lib/tauri'
import { machineColor } from './syncFormat'

const useStyles = makeStyles({
  title: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalS, flexWrap: 'wrap' },
  muted: { color: tokens.colorNeutralForeground3, overflowWrap: 'anywhere' },
  actions: { display: 'flex', flexWrap: 'wrap', columnGap: tokens.spacingHorizontalS, rowGap: tokens.spacingVerticalS },
  dot: { display: 'inline-block', width: '10px', height: '10px', borderRadius: tokens.borderRadiusCircular, marginRight: tokens.spacingHorizontalXS },
  list: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalXS },
})

/** Recusas de um grupo: quem conta é o grupo. O que parou o envio todo
 *  (plano, cota, rede) é uma frase só. */
const PER_GROUP = ['too_large', 'rejected']

/**
 * A linha "Configurações" de Sincronização: o envio manual das desta máquina
 * (com o que mudou desde o último) e o "Desfazer" da última cópia, que o
 * Rust lembra depois de reiniciar o app.
 */
export function SettingsSyncRow({ enabled, blocked }: { enabled: boolean; blocked: boolean }) {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const [sending, setSending] = useState(false)
  const [undoing, setUndoing] = useState(false)
  const [failedGroups, setFailedGroups] = useState<{ group: SettingsGroupName; code: string }[]>([])
  const [stop, setStop] = useState<string | null>(null)
  const [undoError, setUndoError] = useState<string | null>(null)

  const sent = useQuery({
    queryKey: SETTINGS_SENT_KEY,
    queryFn: async () => syncSettingsSent(await exportAppearance()),
    enabled,
  })
  const undoInfo = useQuery({ queryKey: SETTINGS_UNDO_KEY, queryFn: syncSettingsUndoInfo })

  const send = async () => {
    if (sending) return
    setSending(true)
    setFailedGroups([])
    setStop(null)
    try {
      const res = await syncSettingsUpload(await exportAppearance())
      setFailedGroups(
        res.groups.flatMap((g) => (!g.sent && g.error !== null && PER_GROUP.includes(g.error) ? [{ group: g.group, code: g.error }] : [])),
      )
      if (res.error !== null && !PER_GROUP.includes(res.error)) setStop(syncErrorCode(res.error))
    } catch (err) {
      setStop(syncErrorCode(err))
    } finally {
      setSending(false)
      // Um envio que subiu só alguns grupos também moveu a data.
      void qc.invalidateQueries({ queryKey: SETTINGS_SENT_KEY })
      void qc.invalidateQueries({ queryKey: SYNC_STATUS_KEY })
    }
  }

  const undo = async () => {
    if (undoing) return
    setUndoing(true)
    setUndoError(null)
    try {
      await syncSettingsUndo()
      // A aparência de antes fica no localStorage; o Rust guarda só o banco.
      const before = readAppearanceUndo()
      if (before !== null) {
        await applyAppearance(before).catch(() => {})
        clearAppearanceUndo()
      }
      void qc.invalidateQueries({ queryKey: SETTINGS_UNDO_KEY })
      void qc.invalidateQueries({ queryKey: SETTINGS_SENT_KEY })
    } catch (err) {
      setUndoError(syncErrorCode(err))
    } finally {
      setUndoing(false)
    }
  }

  const info = undoInfo.data ?? null
  const sentAt = sent.data?.sentAt ?? null
  const changed = (sent.data?.changed.length ?? 0) > 0
  return (
    <FieldBlock wide>
      <Body1>{t('sync.settings')}</Body1>
      <Caption1 className={s.muted}>{t('sync.settingsHint')}</Caption1>
      {enabled && sentAt !== null && <Caption1 className={s.muted}>{t('sync.settingsRow.sentAt', { when: formatDateTime(sentAt) })}</Caption1>}
      {enabled && sentAt !== null && changed && <Caption1 className={s.muted}>{t('sync.settingsRow.changed')}</Caption1>}
      {info && (
        <MessageBar intent="info">
          <MessageBarBody>
            <span className={s.dot} style={{ backgroundColor: machineColor(info.machineId ?? info.machine) }} aria-hidden />
            {t('sync.settingsRow.undo', { machine: info.machine, when: formatDateTime(info.copiedAt) })}
            {undoError && (
              <>
                <br />
                {t(`sync.err.${undoError as 'failed'}`)}
              </>
            )}
          </MessageBarBody>
          <MessageBarActions>
            <Button disabledFocusable={undoing} icon={undoing ? <Spinner size="tiny" /> : undefined} onClick={() => void undo()}>
              {t('sync.settingsRow.undoButton')}
            </Button>
          </MessageBarActions>
        </MessageBar>
      )}
      {enabled && (
        <div className={s.actions}>
          <Button
            appearance="secondary"
            disabledFocusable={sending || blocked}
            icon={sending ? <Spinner size="tiny" /> : undefined}
            onClick={() => void send()}
          >
            {t('sync.settingsRow.send')}
          </Button>
        </div>
      )}
      {failedGroups.length > 0 && (
        <MessageBar intent="warning">
          <MessageBarBody>
            <span className={s.list}>
              {failedGroups.map((f) => (
                <span key={f.group}>{settingsGroupErrorText(t, f.group, f.code)}</span>
              ))}
            </span>
          </MessageBarBody>
        </MessageBar>
      )}
      {stop === 'no_tier' && (
        <MessageBar intent="warning">
          <MessageBarBody>{t('sync.upgrade')}</MessageBarBody>
          <MessageBarActions>
            <Button onClick={() => void alvanistaOpenSite('plans').catch(() => {})}>{t('sync.seePlans')}</Button>
          </MessageBarActions>
        </MessageBar>
      )}
      {stop !== null && stop !== 'no_tier' && (
        <MessageBar intent="warning">
          <MessageBarBody>{t(`sync.err.${stop as 'failed'}`)}</MessageBarBody>
        </MessageBar>
      )}
    </FieldBlock>
  )
}
