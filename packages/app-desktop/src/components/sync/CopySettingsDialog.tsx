import {
  Badge,
  Body1,
  Button,
  Caption1,
  Checkbox,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { syncErrorCode } from '../../hooks/useSync'
import { applyAppearance, exportAppearance } from '../../lib/appearanceGroup'
import { formatDateTime } from '../../lib/metadataFormat'
import {
  SETTINGS_GROUPS,
  clearAppearanceUndo,
  saveAppearanceUndo,
  type SettingsDbGroup,
  type SettingsGroupName,
} from '../../lib/settingsSync'
import {
  syncSettingsCopy,
  syncSettingsPreview,
  type CopyReport,
  type SettingsPreview,
  type SyncMachine,
} from '../../lib/tauri'
import { hardwareLine } from '../../screens/settings/syncFormat'
import { FullscreenDialog } from '../FullscreenDialog'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalL, maxWidth: '760px' },
  rows: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalS },
  row: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: '2px',
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalL}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  dim: { opacity: 0.6 },
  title: { display: 'flex', alignItems: 'center', columnGap: tokens.spacingHorizontalS, flexWrap: 'wrap' },
  hint: { color: tokens.colorNeutralForeground3, paddingLeft: tokens.spacingHorizontalXXXL },
  result: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalXS, margin: 0, paddingLeft: tokens.spacingHorizontalXL },
  muted: { color: tokens.colorNeutralForeground3 },
})

const hintKey = (g: SettingsGroupName) =>
  g === 'core-options' || g === 'shaders-decorations' || g === 'appearance' ? (`sync.copyDlg.hint.${g}` as const) : null

/** O que a cópia deixou de fora ou preservou, em linhas para a pessoa. */
function ResultLines({ report, machine }: { report: CopyReport; machine: string }) {
  const { t } = useTranslation()
  const s = useStyles()
  const lines: string[] = []
  for (const { report: r } of report.perGroup) {
    for (const f of r.missingFiles) {
      lines.push(
        t('sync.copyDlg.missingLine', {
          item: t(`sync.copyDlg.item.${f.item}`),
          name: f.name,
          reason: t(`sync.copyDlg.missing.${f.kind}`, { machine }),
        }),
      )
    }
  }
  const sum = (pick: (r: CopyReport['perGroup'][number]['report']) => number) =>
    report.perGroup.reduce((n, g) => n + pick(g.report), 0)
  const kept = sum((r) => r.keptLocal)
  const unknown = sum((r) => r.skippedUnknownGames)
  const cores = sum((r) => r.coresNotInstalled.length)
  if (kept > 0) lines.push(t('sync.copyDlg.keptLocal', { count: kept }))
  if (unknown > 0) lines.push(t('sync.copyDlg.unknownGames', { count: unknown }))
  if (cores > 0) lines.push(t('sync.copyDlg.coresNotInstalled', { count: cores }))
  if (lines.length === 0) return null
  return (
    <ul className={s.result} data-slot="copy-result">
      {lines.map((l, i) => (
        <li key={i}>
          <Body1>{l}</Body1>
        </li>
      ))}
    </ul>
  )
}

/**
 * "Copiar de {{machine}} para esta máquina": um grupo por linha, o vídeo
 * desmarcado quando o hardware difere, e depois da cópia o que não veio. As
 * configurações de agora ficam num backup (o "Desfazer" fica na tela de
 * Sincronização). O foco do controle começa no primeiro grupo.
 */
export function CopySettingsDialog({ machine, onClose }: { machine: SyncMachine; onClose: () => void }) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const [preview, setPreview] = useState<SettingsPreview | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [ticked, setTicked] = useState<Record<string, boolean>>({})
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [report, setReport] = useState<CopyReport | null>(null)
  const [appearanceFailed, setAppearanceFailed] = useState(false)
  const firstRef = useRef<HTMLInputElement>(null)
  const closeRef = useRef<HTMLButtonElement>(null)
  const [attempt, setAttempt] = useState(0)

  useEffect(() => {
    let alive = true
    syncSettingsPreview(machine.id).then(
      (p) => {
        if (!alive) return
        setPreview(p)
        const init: Record<string, boolean> = {}
        for (const g of p.groups) {
          const usable = g.present && g.error === null
          init[g.group] = usable && !(g.group === 'video' && p.videoDefaultOff)
        }
        setTicked(init)
      },
      (err: unknown) => alive && setLoadError(syncErrorCode(err)),
    )
    return () => {
      alive = false
    }
  }, [machine.id, attempt])

  // O foco do controle: no primeiro grupo que se pode marcar, e não no X.
  const firstUsable = preview?.groups.find((g) => g.present && g.error === null)?.group
  useEffect(() => {
    if (!preview || report) return
    const id = window.setTimeout(() => firstRef.current?.focus(), 0)
    return () => window.clearTimeout(id)
  }, [preview, report])

  // Depois da cópia o foco vai para "Fechar": o grupo em que estava sumiu.
  useEffect(() => {
    if (!report) return
    const id = window.setTimeout(() => closeRef.current?.focus(), 0)
    return () => window.clearTimeout(id)
  }, [report])

  const count = SETTINGS_GROUPS.filter((g) => ticked[g]).length

  const copy = async () => {
    if (busy || count === 0) return
    setBusy(true)
    setError(null)
    const appearanceWanted = ticked.appearance === true
    const groups = SETTINGS_GROUPS.filter((g): g is SettingsDbGroup => g !== 'appearance' && ticked[g] === true)
    try {
      // A aparência de agora, antes de a cópia trazer a outra.
      const before = appearanceWanted ? await exportAppearance().catch(() => null) : null
      const done = await syncSettingsCopy(machine.id, groups, appearanceWanted)
      if (appearanceWanted && done.appearanceDoc != null) {
        // Sem a aparência de antes, uma guardada de outra cópia seria velha.
        // Guardada com o instante desta cópia: só o "Desfazer" dela a traz.
        if (before) saveAppearanceUndo(before, done.copiedAt)
        else clearAppearanceUndo()
        try {
          await applyAppearance(done.appearanceDoc)
        } catch {
          setAppearanceFailed(true)
        }
      } else {
        // Sem aparência nesta cópia, o "Desfazer" dela não traz uma mais velha.
        clearAppearanceUndo()
      }
      setReport(done)
      // Tudo de novo, não só o Desfazer e o status: as telas de Áudio, Sistema,
      // HDR e companhia gravam a configuração inteira a partir do que leram, e
      // um valor de antes da cópia gravaria por cima dela.
      void qc.invalidateQueries()
    } catch (err) {
      setError(syncErrorCode(err))
    } finally {
      setBusy(false)
    }
  }

  const lang = i18n.language
  const theirs = preview?.machine.hardware ?? machine.hardware
  const unknown = t('sync.copyDlg.unknownHw')
  const nz = (v: string | undefined) => (v && v.trim() !== '' ? v : unknown)

  const rowFor = (g: SettingsPreview['groups'][number]): ReactNode => {
    const usable = g.present && g.error === null
    const isVideo = g.group === 'video'
    const hk = hintKey(g.group)
    let hint: string | null = hk ? t(hk) : null
    if (!g.present) hint = t('sync.copyDlg.notSent')
    else if (g.error !== null) hint = t(`sync.err.${syncErrorCode(g.error)}`)
    else if (isVideo && preview) {
      hint = preview.videoDefaultOff
        ? t('sync.copyDlg.videoDiffers', {
            theirGpu: nz(theirs?.gpu),
            theirBackend: nz(theirs?.gpu_backend),
            myGpu: nz(preview.hereGpu),
            myBackend: nz(preview.hereBackend),
          })
        : t('sync.copyDlg.videoSame', { gpu: nz(preview.hereGpu), backend: nz(preview.hereBackend) })
    }
    return (
      <div key={g.group} className={s.row}>
        <Checkbox
          ref={g.group === firstUsable ? firstRef : undefined}
          checked={ticked[g.group] === true}
          disabled={!usable || busy}
          onChange={(_, d) => setTicked((cur) => ({ ...cur, [g.group]: d.checked === true }))}
          label={
            <span className={s.title}>
              <Body1>{t(`sync.settingsGroup.${g.group}`)}</Body1>
              {isVideo && usable && preview?.videoDefaultOff && (
                <Badge appearance="tint" color="warning">
                  {t('sync.copyDlg.videoPill')}
                </Badge>
              )}
            </span>
          }
        />
        {hint && <Caption1 className={s.hint}>{hint}</Caption1>}
      </div>
    )
  }

  const sentLine = preview && preview.sentAt > 0 ? t('sync.copyDlg.sentAt', { when: formatDateTime(preview.sentAt) }) : ''
  const hw = hardwareLine(theirs, lang)
  const sentence = [sentLine, hw ? `${hw}.` : '', t('sync.copyDlg.undoNote')].filter(Boolean).join(' ')

  return (
    <FullscreenDialog
      open
      onClose={onClose}
      title={t('sync.copyDlg.title', { machine: machine.label })}
      actions={
        report ? (
          <Button ref={closeRef} appearance="primary" onClick={onClose}>
            {t('sync.copyDlg.close')}
          </Button>
        ) : (
          <>
            <Button appearance="secondary" onClick={onClose}>
              {t('sync.copyDlg.cancel')}
            </Button>
            <Button
              appearance="primary"
              disabledFocusable={busy || count === 0 || !preview}
              icon={busy ? <Spinner size="tiny" /> : undefined}
              onClick={() => void copy()}
            >
              {busy ? t('sync.copyDlg.copying') : t('sync.copyDlg.copyN', { count })}
            </Button>
          </>
        )
      }
    >
      <div className={s.root}>
        {loadError && (
          <MessageBar intent="warning">
            <MessageBarBody>{t(`sync.err.${loadError as 'failed'}`)}</MessageBarBody>
            <MessageBarActions>
              <Button onClick={() => {
                  setLoadError(null)
                  setAttempt((n) => n + 1)
                }}>{t('sync.copyDlg.retry')}</Button>
            </MessageBarActions>
          </MessageBar>
        )}
        {!preview && !loadError && <Spinner size="tiny" label={t('sync.copyDlg.loading')} labelPosition="after" />}
        {preview && !report && (
          <>
            <Body1>{sentence}</Body1>
            {error && (
              <MessageBar intent="warning">
                <MessageBarBody>{t(`sync.err.${error as 'failed'}`)}</MessageBarBody>
              </MessageBar>
            )}
            <div className={s.rows}>
              {preview.groups.map(rowFor)}
              <div className={`${s.row} ${s.dim}`}>
                <Checkbox checked={false} disabled label={<Body1>{t('sync.copyDlg.neverTitle')}</Body1>} />
                <Caption1 className={s.hint}>{t('sync.copyDlg.neverHint')}</Caption1>
              </div>
            </div>
          </>
        )}
        {report && (
          <>
            <MessageBar intent="success">
              <MessageBarBody>{t('sync.copyDlg.done')}</MessageBarBody>
            </MessageBar>
            {appearanceFailed && (
              <MessageBar intent="warning">
                <MessageBarBody>{t('sync.copyDlg.appearanceFailed')}</MessageBarBody>
              </MessageBar>
            )}
            <ResultLines report={report} machine={machine.label} />
          </>
        )}
      </div>
    </FullscreenDialog>
  )
}
