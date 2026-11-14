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
import { useRef, useState } from 'react'
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
import {
  alvanistaOpenSite,
  syncOpenBackups,
  syncSettingsSent,
  syncSettingsUndo,
  syncSettingsUndoForget,
  syncSettingsUndoInfo,
  syncSettingsUpload,
} from '../../lib/tauri'
import { machineColor } from './syncFormat'

const useStyles = makeStyles({
  title: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalS, flexWrap: 'wrap' },
  muted: { color: tokens.colorNeutralForeground3, overflowWrap: 'anywhere' },
  actions: { display: 'flex', flexWrap: 'wrap', columnGap: tokens.spacingHorizontalS, rowGap: tokens.spacingVerticalS },
  dot: { display: 'inline-block', width: '10px', height: '10px', borderRadius: tokens.borderRadiusCircular, marginRight: tokens.spacingHorizontalXS },
  list: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalXS },
  // onde o foco pousa depois do "Desfazer" com o sync desligado (sem botão na
  // linha): só para não cair no nada, sem anel
  anchor: { outlineStyle: 'none' },
})

/** Recusas de um grupo: quem conta é o grupo. O que parou o envio todo
 *  (plano, cota, rede) é uma frase só. */
const PER_GROUP = ['too_large', 'rejected']

/**
 * A linha "Configurações" de Sincronização: o envio manual das desta máquina
 * (com o que mudou desde o último) e o "Desfazer" da última cópia, que o
 * Rust lembra depois de reiniciar o app. Um "Desfazer" cujo backup sumiu
 * oferece a pasta de resguardo e "Esquecer" (sai só o registro da cópia).
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
  const [appearanceError, setAppearanceError] = useState(false)
  const [forgetting, setForgetting] = useState(false)
  const sendRef = useRef<HTMLButtonElement>(null)
  const titleRef = useRef<HTMLDivElement>(null)

  const sent = useQuery({
    queryKey: SETTINGS_SENT_KEY,
    queryFn: async () => syncSettingsSent(await exportAppearance()),
    enabled,
  })
  const undoInfo = useQuery({ queryKey: SETTINGS_UNDO_KEY, queryFn: syncSettingsUndoInfo })

  // O erro do "Desfazer" é daquela cópia: uma cópia nova (outro `copiedAt`)
  // começa sem ele, e o "Esquecer" não aparece para a cópia errada.
  const copiedAt = undoInfo.data?.copiedAt ?? null
  const [undoErrorAt, setUndoErrorAt] = useState<number | null>(null)

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

  // A barra do "Desfazer" sai da tela: o foco do controle vai para "Enviar as
  // desta máquina" (sem o sync ligado, para a própria linha), não para o nada.
  const focusAfterBar = () => (sendRef.current ?? titleRef.current)?.focus()

  const undo = async () => {
    if (undoing) return
    setUndoing(true)
    setUndoError(null)
    setAppearanceError(false)
    try {
      // A cópia que este "Desfazer" volta, lida do Rust agora: a aparência de
      // antes mora no localStorage e só volta se for desta cópia (o mesmo
      // `copiedAt`) e se ela trouxe aparência.
      const copy = await syncSettingsUndoInfo()
      const undone = await syncSettingsUndo()
      // Só volta se o Rust desfez; se não aplicar, o documento fica guardado.
      const before = undone && copy?.groups.includes('appearance') ? readAppearanceUndo(copy.copiedAt) : null
      if (before !== null) {
        try {
          await applyAppearance(before)
          clearAppearanceUndo()
        } catch {
          setAppearanceError(true)
        }
      } else if (undone) {
        // o que ficou guardado era de outra cópia
        clearAppearanceUndo()
      }
      // Tudo de novo: as telas que gravam a configuração inteira a partir do
      // que leram (Áudio, Sistema, HDR) gravariam por cima do que voltou.
      void qc.invalidateQueries()
      focusAfterBar()
    } catch (err) {
      setUndoError(syncErrorCode(err))
      setUndoErrorAt(copiedAt)
    } finally {
      setUndoing(false)
    }
  }

  // "Esquecer": o backup desta cópia não volta mais. Sai só o registro dela no
  // Rust; os arquivos ficam na pasta de resguardo.
  const forget = async () => {
    if (forgetting) return
    setForgetting(true)
    try {
      await syncSettingsUndoForget()
      // a aparência guardada era do "Desfazer" desta cópia
      clearAppearanceUndo()
      setUndoError(null)
      void qc.invalidateQueries({ queryKey: SETTINGS_UNDO_KEY })
      focusAfterBar()
    } catch (err) {
      setUndoError(syncErrorCode(err))
      setUndoErrorAt(copiedAt)
    } finally {
      setForgetting(false)
    }
  }

  const info = undoInfo.data ?? null
  const sentAt = sent.data?.sentAt ?? null
  const changed = (sent.data?.changed.length ?? 0) > 0
  // o backup da cópia sumiu ou não confere: o "Desfazer" não volta sozinho
  const shownUndoError = undoError !== null && undoErrorAt === copiedAt ? undoError : null
  const backupGone = shownUndoError === 'backup_failed'
  return (
    <FieldBlock wide>
      <div ref={titleRef} tabIndex={-1} className={s.anchor}>
        <Body1>{t('sync.settings')}</Body1>
      </div>
      <Caption1 className={s.muted}>{t('sync.settingsHint')}</Caption1>
      {enabled && sentAt !== null && <Caption1 className={s.muted}>{t('sync.settingsRow.sentAt', { when: formatDateTime(sentAt) })}</Caption1>}
      {enabled && sentAt !== null && changed && <Caption1 className={s.muted}>{t('sync.settingsRow.changed')}</Caption1>}
      {info && (
        <MessageBar intent="info">
          <MessageBarBody>
            <span className={s.dot} style={{ backgroundColor: machineColor(info.machineId ?? info.machine) }} aria-hidden />
            {t('sync.settingsRow.undo', { machine: info.machine, when: formatDateTime(info.copiedAt) })}
            {shownUndoError && (
              <>
                <br />
                {t(`sync.err.${shownUndoError as 'failed'}`)}
              </>
            )}
            {backupGone && (
              <>
                <br />
                {t('sync.settingsRow.forgetHint')}
              </>
            )}
          </MessageBarBody>
          <MessageBarActions>
            <Button disabledFocusable={undoing} icon={undoing ? <Spinner size="tiny" /> : undefined} onClick={() => void undo()}>
              {t('sync.settingsRow.undoButton')}
            </Button>
            {backupGone && (
              <Button onClick={() => void syncOpenBackups().catch(() => {})}>{t('sync.settingsRow.openFolder')}</Button>
            )}
            {backupGone && (
              <Button
                disabledFocusable={forgetting}
                icon={forgetting ? <Spinner size="tiny" /> : undefined}
                onClick={() => void forget()}
              >
                {t('sync.settingsRow.forget')}
              </Button>
            )}
          </MessageBarActions>
        </MessageBar>
      )}
      {appearanceError && (
        <MessageBar intent="warning">
          <MessageBarBody>{t('sync.settingsRow.appearanceUndoFailed')}</MessageBarBody>
        </MessageBar>
      )}
      {enabled && (
        <div className={s.actions}>
          <Button
            ref={sendRef}
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
