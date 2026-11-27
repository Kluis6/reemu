import {
  Badge,
  Body1,
  Button,
  Caption1,
  Field,
  Input,
  makeStyles,
  mergeClasses,
  Select,
  Spinner,
  Switch,
  Textarea,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowUploadRegular,
  BookRegular,
  CameraRegular,
  DeleteRegular,
  FolderOpenRegular,
  MicRegular,
  NoteAddRegular,
  OpenRegular,
  RecordStopRegular,
  SendRegular,
  VideoRegular,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useAlvanistaConnect } from '../hooks/useAlvanistaConnect'
import { ALVANISTA_CACHED_KEY } from '../hooks/useEffectiveProfile'
import {
  alvanistaCachedStatus,
  describeRawInput,
  gamelogAttachToSession,
  gamelogCancelAudioNote,
  gamelogCaptureScreenshot,
  gamelogCreateNote,
  gamelogDeleteItem,
  gamelogListItems,
  gamelogOpenCapturesDir,
  gamelogOpenSessionPost,
  gamelogPatchItem,
  gamelogSetPrefs,
  gamelogStartAudioNote,
  gamelogStartVideoRecording,
  gamelogState,
  gamelogStopAudioNote,
  gamelogStopVideoRecording,
  gamelogUploadFile,
  gamelogUsage,
  getRomMetadata,
  listSystemHotkeys,
  onGamelogStateChanged,
  onGamelogUpdated,
  pickGamelogMediaFile,
  type GamelogItem,
  type GamelogVisibility,
  type SystemActionKey,
} from '../lib/tauri'
import { errorToast, sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

const VISIBILITY_OPTIONS: GamelogVisibility[] = ['friends', 'public', 'private']

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 MB'
  const mb = bytes / (1024 * 1024)
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  return `${mb.toFixed(mb < 10 ? 1 : 0)} MB`
}

function formatDurationMs(ms: number | null | undefined): string {
  if (!ms || ms <= 0) return '0:00'
  const totalSecs = Math.max(0, Math.round(ms / 1000))
  const mins = Math.floor(totalSecs / 60)
  const secs = totalSecs % 60
  return `${mins}:${String(secs).padStart(2, '0')}`
}

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalM,
  },
  card: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  recordingCard: {
    border: `1px solid ${tokens.colorPaletteRedBorder2}`,
    backgroundColor: tokens.colorPaletteRedBackground1,
  },
  row: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  spaceBetween: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  actionsGrid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))',
    gap: tokens.spacingHorizontalS,
  },
  compactStack: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXS,
  },
  mediaPreview: {
    width: '100%',
    maxHeight: '280px',
    objectFit: 'contain',
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground1,
  },
  audioPlayer: {
    width: '100%',
  },
  muted: {
    color: tokens.colorNeutralForeground3,
  },
  quotaTrack: {
    width: '100%',
    height: '6px',
    borderRadius: '999px',
    backgroundColor: tokens.colorNeutralBackground4,
    overflow: 'hidden',
  },
  quotaFill: {
    height: '100%',
    backgroundColor: tokens.colorBrandBackground,
  },
  itemCard: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXS,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
  },
})

export function GameLogPanel({
  romId,
  gameUuid: propGameUuid,
  inGame = false,
  onPickCatalogGame,
}: {
  romId: string
  gameUuid?: string | null
  inGame?: boolean
  onPickCatalogGame?: () => void
}) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const [composerOpen, setComposerOpen] = useState(false)
  const [noteTitle, setNoteTitle] = useState('')
  const [noteBody, setNoteBody] = useState('')
  const [captureCaption, setCaptureCaption] = useState('')
  const [tickNow, setTickNow] = useState(0)

  const metaQuery = useQuery({
    queryKey: ['rom-metadata', romId, i18n.resolvedLanguage ?? i18n.language],
    queryFn: () => getRomMetadata(romId, i18n.resolvedLanguage ?? i18n.language),
    enabled: Boolean(romId) && propGameUuid === undefined,
    retry: false,
  })

  const stateQuery = useQuery({
    queryKey: ['gamelog-state'],
    queryFn: gamelogState,
    retry: false,
    staleTime: 1000,
  })

  const hotkeysQuery = useQuery({
    queryKey: ['system-hotkeys'],
    queryFn: listSystemHotkeys,
    retry: false,
  })

  const cachedAlva = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
  })

  const connected = Boolean(cachedAlva.data?.connected && !cachedAlva.data?.ended)
  const hasScope = Boolean(cachedAlva.data?.scopes?.includes('gamelog'))
  const { connect, cancel, connecting } = useAlvanistaConnect({ scopes: ['gamelog'] })

  const runtime = stateQuery.data
  const effectiveGameUuid =
    propGameUuid ??
    metaQuery.data?.gameUuid ??
    (runtime?.activeRomId === romId ? runtime?.activeGameId : null) ??
    null

  const canQueryRemote = connected && hasScope && Boolean(effectiveGameUuid)

  const usageQuery = useQuery({
    queryKey: ['gamelog-usage'],
    queryFn: gamelogUsage,
    enabled: connected && hasScope,
    retry: false,
  })

  const itemsQuery = useQuery({
    queryKey: ['gamelog-items', effectiveGameUuid],
    queryFn: () => gamelogListItems(effectiveGameUuid!),
    enabled: canQueryRemote,
    retry: false,
  })

  // Escuta mudancas de gravacao e novos envios vindos de atalhos em tempo real
  useEffect(() => {
    let offState: (() => void) | undefined
    let offUpd: (() => void) | undefined
    let alive = true
    void onGamelogStateChanged((next) => {
      qc.setQueryData(['gamelog-state'], next)
    }).then((fn) => {
      if (alive) offState = fn
      else fn()
    })
    void onGamelogUpdated(() => {
      void qc.invalidateQueries({ queryKey: ['gamelog-items'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-usage'] })
    }).then((fn) => {
      if (alive) offUpd = fn
      else fn()
    })
    return () => {
      alive = false
      offState?.()
      offUpd?.()
    }
  }, [qc])

  // Atualiza cronometro ao vivo durante gravacao de nota de audio ou video
  const isRecordingAny = Boolean(runtime?.audioRecording || runtime?.videoRecording)
  useEffect(() => {
    if (!isRecordingAny) return
    const id = window.setInterval(() => {
      setTickNow((n) => n + 1)
      void qc.invalidateQueries({ queryKey: ['gamelog-state'] })
    }, 1000)
    return () => window.clearInterval(id)
  }, [isRecordingAny, qc])
  void tickNow

  const shortcutLabel = (key: SystemActionKey): string | null => {
    const found = hotkeysQuery.data?.find((b) => b.action === key)
    if (!found || found.trigger.length === 0) return null
    return found.trigger.map(describeRawInput).join(' + ')
  }

  const shotShortcut = shortcutLabel('capture_screenshot')
  const audioShortcut = shortcutLabel('toggle_audio_note')
  const videoShortcut = shortcutLabel('toggle_video_recording')

  const notifyCaptureOutcome = (kindKey: 'screenshot' | 'audio' | 'video', attachedPostId: string | null, uploaded: boolean, warning: string | null) => {
    if (attachedPostId) {
      push(sysToast(t(`gamelog.toast.${kindKey}Attached`), 'Success'))
    } else if (uploaded) {
      push(sysToast(t(`gamelog.toast.${kindKey}Uploaded`), 'Success'))
    } else if (warning === 'needs_consent') {
      push(sysToast(t('gamelog.toast.savedLocalNeedsConsent'), 'Warning'))
    } else if (warning === 'game_not_mapped') {
      push(sysToast(t('gamelog.toast.savedLocalUnmapped'), 'Info'))
    } else {
      push(sysToast(t('gamelog.toast.savedLocalOnly'), 'Success'))
    }
  }

  const prefsMut = useMutation({
    mutationFn: gamelogSetPrefs,
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['gamelog-state'] }),
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const shotMut = useMutation({
    mutationFn: () =>
      gamelogCaptureScreenshot({
        caption: captureCaption.trim() || null,
      }),
    onSuccess: (res) => {
      setCaptureCaption('')
      void qc.invalidateQueries({ queryKey: ['gamelog-items'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-usage'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-state'] })
      notifyCaptureOutcome('screenshot', res.attachedPostId, Boolean(res.uploadedItem), res.warning)
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const startAudioMut = useMutation({
    mutationFn: () =>
      gamelogStartAudioNote({
        romId: romId || null,
        gameId: effectiveGameUuid,
        caption: captureCaption.trim() || null,
      }),
    onSuccess: (st) => {
      qc.setQueryData(['gamelog-state'], st)
      push(sysToast(t('gamelog.toast.audioStarted'), 'Info'))
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const stopAudioMut = useMutation({
    mutationFn: () =>
      gamelogStopAudioNote({
        caption: captureCaption.trim() || null,
      }),
    onSuccess: (res) => {
      setCaptureCaption('')
      void qc.invalidateQueries({ queryKey: ['gamelog-state'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-items'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-usage'] })
      notifyCaptureOutcome('audio', res.attachedPostId, Boolean(res.uploadedItem), res.warning)
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const cancelAudioMut = useMutation({
    mutationFn: gamelogCancelAudioNote,
    onSuccess: (st) => {
      qc.setQueryData(['gamelog-state'], st)
      push(sysToast(t('gamelog.toast.audioCancelled'), 'Info'))
    },
  })

  const startVideoMut = useMutation({
    mutationFn: () => gamelogStartVideoRecording(captureCaption.trim() || null),
    onSuccess: (st) => {
      qc.setQueryData(['gamelog-state'], st)
      push(sysToast(t('gamelog.toast.videoStarted'), 'Info'))
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const stopVideoMut = useMutation({
    mutationFn: () =>
      gamelogStopVideoRecording({
        caption: captureCaption.trim() || null,
      }),
    onSuccess: (res) => {
      setCaptureCaption('')
      void qc.invalidateQueries({ queryKey: ['gamelog-state'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-items'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-usage'] })
      notifyCaptureOutcome('video', res.attachedPostId, Boolean(res.uploadedItem), res.warning)
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const createNoteMut = useMutation({
    mutationFn: () =>
      gamelogCreateNote({
        gameId: effectiveGameUuid!,
        romId: romId || null,
        title: noteTitle.trim() || null,
        body: noteBody.trim(),
      }),
    onSuccess: () => {
      setNoteTitle('')
      setNoteBody('')
      setComposerOpen(false)
      void qc.invalidateQueries({ queryKey: ['gamelog-items'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-usage'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-state'] })
      push(sysToast(t('gamelog.toast.noteSaved'), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const uploadFileMut = useMutation({
    mutationFn: async () => {
      const picked = await pickGamelogMediaFile()
      if (!picked) return null
      return gamelogUploadFile({
        gameId: effectiveGameUuid!,
        romId: romId || null,
        filePath: picked,
        caption: captureCaption.trim() || null,
      })
    },
    onSuccess: (item) => {
      if (!item) return
      setCaptureCaption('')
      void qc.invalidateQueries({ queryKey: ['gamelog-items'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-usage'] })
      push(sysToast(t('gamelog.toast.fileUploaded'), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const attachMut = useMutation({
    mutationFn: (item: GamelogItem) =>
      gamelogAttachToSession({
        itemId: item.id,
        gameId: item.gameId,
        caption: item.body || null,
        visibility: VISIBILITY_OPTIONS.find((visibility) => visibility === item.visibility),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['gamelog-state'] })
      push(sysToast(t('gamelog.toast.itemAttached'), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const visibilityMut = useMutation({
    mutationFn: ({ itemId, visibility }: { itemId: string; visibility: GamelogVisibility }) =>
      gamelogPatchItem({ itemId, visibility }),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ['gamelog-items'] }),
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const deleteMut = useMutation({
    mutationFn: (itemId: string) => gamelogDeleteItem(itemId),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['gamelog-items'] })
      void qc.invalidateQueries({ queryKey: ['gamelog-usage'] })
      push(sysToast(t('gamelog.toast.itemDeleted'), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'gamelogAction')),
  })

  const prefs = runtime?.prefs
  const currentVis: GamelogVisibility = VISIBILITY_OPTIONS.includes(
    (prefs?.visibility ?? 'friends') as GamelogVisibility,
  )
    ? ((prefs?.visibility ?? 'friends') as GamelogVisibility)
    : 'friends'

  const usage = usageQuery.data
  const quotaPct =
    usage && usage.quotaBytes > 0
      ? Math.min(100, Math.round((usage.usedBytes / usage.quotaBytes) * 100))
      : 0

  return (
    <div className={s.root} data-testid="gamelog-panel">
      {/* Banner ao vivo de gravacao ativa */}
      {runtime?.audioRecording && (
        <div className={mergeClasses(s.card, s.recordingCard)}>
          <div className={s.spaceBetween}>
            <Body1>
              {t('gamelog.recordingAudioLive', {
                time: formatDurationMs(runtime.audioElapsedMs),
              })}
            </Body1>
            <div className={s.row}>
              <Button
                appearance="primary"
                size="small"
                icon={<RecordStopRegular />}
                disabled={stopAudioMut.isPending}
                onClick={() => stopAudioMut.mutate()}
              >
                {t('gamelog.stopAndSend')}
              </Button>
              <Button
                appearance="subtle"
                size="small"
                disabled={cancelAudioMut.isPending}
                onClick={() => cancelAudioMut.mutate()}
              >
                {t('common.cancel')}
              </Button>
            </div>
          </div>
        </div>
      )}

      {runtime?.videoRecording && (
        <div className={mergeClasses(s.card, s.recordingCard)}>
          <div className={s.spaceBetween}>
            <Body1>
              {t('gamelog.recordingVideoLive', {
                time: formatDurationMs(runtime.videoElapsedMs),
              })}
            </Body1>
            <Button
              appearance="primary"
              size="small"
              icon={<RecordStopRegular />}
              disabled={stopVideoMut.isPending}
              onClick={() => stopVideoMut.mutate()}
            >
              {stopVideoMut.isPending ? t('gamelog.finalizingVideo') : t('gamelog.stopAndSend')}
            </Button>
          </div>
        </div>
      )}

      {/* Acoes rapidas de captura / upload */}
      <div className={s.card}>
        <Field label={t('gamelog.captionLabel')}>
          <Input
            value={captureCaption}
            onChange={(_, d) => setCaptureCaption(d.value)}
            placeholder={t('gamelog.captionPlaceholder')}
          />
        </Field>

        <div className={inGame ? s.compactStack : s.actionsGrid}>
          {inGame && (
            <Button
              appearance="primary"
              icon={<CameraRegular />}
              disabled={shotMut.isPending}
              onClick={() => shotMut.mutate()}
            >
              {shotShortcut
                ? `${t('gamelog.captureScreenshot')} (${shotShortcut})`
                : t('gamelog.captureScreenshot')}
            </Button>
          )}

          <Button
            appearance={runtime?.audioRecording ? 'primary' : 'secondary'}
            icon={runtime?.audioRecording ? <RecordStopRegular /> : <MicRegular />}
            disabled={startAudioMut.isPending || stopAudioMut.isPending}
            onClick={() =>
              runtime?.audioRecording ? stopAudioMut.mutate() : startAudioMut.mutate()
            }
          >
            {runtime?.audioRecording
              ? t('gamelog.stopAudioNote')
              : audioShortcut && inGame
                ? `${t('gamelog.recordAudioNote')} (${audioShortcut})`
                : t('gamelog.recordAudioNote')}
          </Button>

          {inGame && (
            <Button
              appearance={runtime?.videoRecording ? 'primary' : 'secondary'}
              icon={runtime?.videoRecording ? <RecordStopRegular /> : <VideoRegular />}
              disabled={
                startVideoMut.isPending ||
                stopVideoMut.isPending ||
                (!runtime?.videoRecording && runtime?.ffmpegAvailable === false)
              }
              onClick={() =>
                runtime?.videoRecording ? stopVideoMut.mutate() : startVideoMut.mutate()
              }
            >
              {runtime?.videoRecording
                ? t('gamelog.stopVideo')
                : videoShortcut
                  ? `${t('gamelog.recordVideo')} (${videoShortcut})`
                  : t('gamelog.recordVideo')}
            </Button>
          )}

          {canQueryRemote && (
            <Button
              appearance="secondary"
              icon={<ArrowUploadRegular />}
              disabled={uploadFileMut.isPending}
              onClick={() => uploadFileMut.mutate()}
            >
              {uploadFileMut.isPending ? t('gamelog.uploadingFile') : t('gamelog.uploadFile')}
            </Button>
          )}

          {canQueryRemote && (
            <Button
              appearance="secondary"
              icon={<NoteAddRegular />}
              onClick={() => setComposerOpen((v) => !v)}
            >
              {t('gamelog.newTextNote')}
            </Button>
          )}

          <Button
            appearance="subtle"
            icon={<FolderOpenRegular />}
            onClick={() => void gamelogOpenCapturesDir(romId || null)}
          >
            {t('gamelog.openLocalFolder')}
          </Button>
        </div>

        {inGame && runtime?.ffmpegAvailable === false && (
          <Caption1 className={s.muted}>{t('gamelog.ffmpegMissingHint')}</Caption1>
        )}
      </div>

      {/* Compositor de nota de texto */}
      {composerOpen && canQueryRemote && (
        <div className={s.card}>
          <Field label={t('gamelog.noteTitleLabel')}>
            <Input
              value={noteTitle}
              onChange={(_, d) => setNoteTitle(d.value)}
              placeholder={t('gamelog.noteTitlePlaceholder')}
            />
          </Field>
          <Field label={t('gamelog.noteBodyLabel')}>
            <Textarea
              rows={3}
              value={noteBody}
              onChange={(_, d) => setNoteBody(d.value)}
              placeholder={t('gamelog.noteBodyPlaceholder')}
            />
          </Field>
          <div className={s.row}>
            <Button
              appearance="primary"
              icon={<SendRegular />}
              disabled={!noteBody.trim() || createNoteMut.isPending}
              onClick={() => createNoteMut.mutate()}
            >
              {t('gamelog.saveNote')}
            </Button>
            <Button appearance="subtle" onClick={() => setComposerOpen(false)}>
              {t('common.cancel')}
            </Button>
          </div>
        </div>
      )}

      {/* Configuracao da thread automatica de check-in + conquistas + capturas */}
      <div className={s.card}>
        <div className={s.spaceBetween}>
          <Body1>{t('gamelog.sessionThreadTitle')}</Body1>
          {runtime?.activeSessionPostId && (
            <Button
              size="small"
              appearance="subtle"
              icon={<OpenRegular />}
              onClick={() => void gamelogOpenSessionPost(runtime.activeSessionPostId!)}
            >
              {t('gamelog.openSessionThread')}
            </Button>
          )}
        </div>
        <Caption1 className={s.muted}>{t('gamelog.sessionThreadHint')}</Caption1>

        <Switch
          checked={prefs?.autoSessionCheckin ?? true}
          label={t('gamelog.autoSessionCheckin')}
          onChange={(_, d) => prefsMut.mutate({ autoSessionCheckin: d.checked })}
        />
        <Switch
          checked={prefs?.autoAttachCaptures ?? true}
          label={t('gamelog.autoAttachCaptures')}
          onChange={(_, d) => prefsMut.mutate({ autoAttachCaptures: d.checked })}
        />

        <div className={s.row}>
          <Caption1>{t('gamelog.visibilityLabel')}</Caption1>
          <Select
            size="small"
            aria-label={t('gamelog.visibilityLabel')}
            value={currentVis}
            onChange={(_, d) =>
              prefsMut.mutate({ visibility: d.value as GamelogVisibility })
            }
          >
            {VISIBILITY_OPTIONS.map((v) => (
              <option key={v} value={v}>
                {t(`gamelog.visibility.${v}`)}
              </option>
            ))}
          </Select>
        </div>
      </div>

      {/* Estado de conexao / autorizacao / mapeamento */}
      {!connected && (
        <div className={s.card} data-testid="gamelog-connect-prompt">
          <div className={s.row}>
            <BookRegular aria-hidden />
            <Body1>{t('gamelog.notConnectedTitle')}</Body1>
          </div>
          <Caption1 className={s.muted}>{t('gamelog.notConnectedHint')}</Caption1>
          <div className={s.row}>
            {connecting ? (
              <>
                <Spinner size="tiny" />
                <Caption1>{t('collection.connecting')}</Caption1>
                <Button size="small" appearance="subtle" onClick={cancel}>
                  {t('common.cancel')}
                </Button>
              </>
            ) : (
              <Button size="small" appearance="primary" onClick={connect}>
                {t('gamelog.connectButton')}
              </Button>
            )}
          </div>
        </div>
      )}

      {connected && !hasScope && (
        <div className={s.card} data-testid="gamelog-consent-prompt">
          <div className={s.row}>
            <BookRegular aria-hidden />
            <Body1>{t('gamelog.consentTitle')}</Body1>
          </div>
          <Caption1 className={s.muted}>{t('gamelog.consentHint')}</Caption1>
          <div className={s.row}>
            {connecting ? (
              <>
                <Spinner size="tiny" />
                <Caption1>{t('collection.connecting')}</Caption1>
                <Button size="small" appearance="subtle" onClick={cancel}>
                  {t('common.cancel')}
                </Button>
              </>
            ) : (
              <Button size="small" appearance="primary" onClick={connect}>
                {t('gamelog.authorizeButton')}
              </Button>
            )}
          </div>
        </div>
      )}

      {connected && hasScope && !effectiveGameUuid && (
        <div className={s.card} data-testid="gamelog-unmapped-prompt">
          <Body1>{t('gamelog.unmappedTitle')}</Body1>
          <Caption1 className={s.muted}>{t('gamelog.unmappedHint')}</Caption1>
          {onPickCatalogGame && (
            <div className={s.row}>
              <Button size="small" appearance="secondary" onClick={onPickCatalogGame}>
                {t('catalog.pick.button')}
              </Button>
            </div>
          )}
        </div>
      )}

      {/* Cota de armazenamento do Meu Log */}
      {connected && hasScope && usage && usage.quotaBytes > 0 && (
        <div className={s.card}>
          <div className={s.spaceBetween}>
            <Caption1>
              {t('gamelog.quotaLabel', {
                used: formatBytes(usage.usedBytes),
                quota: formatBytes(usage.quotaBytes),
              })}
            </Caption1>
            <Caption1 className={s.muted}>
              {t('gamelog.itemCount', { count: usage.itemCount })}
            </Caption1>
          </div>
          <div className={s.quotaTrack} aria-hidden>
            <div className={s.quotaFill} style={{ width: `${quotaPct}%` }} />
          </div>
        </div>
      )}

      {/* Lista de registros do Meu Log para este jogo */}
      {canQueryRemote && (
        <>
          {itemsQuery.isLoading && (
            <div className={s.row}>
              <Spinner size="tiny" />
              <Caption1>{t('common.loading')}</Caption1>
            </div>
          )}

          {itemsQuery.isSuccess && itemsQuery.data.items.length === 0 && (
            <Caption1 className={s.muted}>{t('gamelog.emptyItems')}</Caption1>
          )}

          {itemsQuery.data?.items.map((item) => {
            const media = item.media[0]
            return (
              <article key={item.id} className={s.itemCard}>
                <div className={s.spaceBetween}>
                  <div className={s.row}>
                    <Badge appearance="tint" color="brand">
                      {t(`gamelog.kinds.${item.kind}`, { defaultValue: item.kind })}
                    </Badge>
                    <Caption1 className={s.muted}>
                      {new Date(item.takenAt ?? item.createdAt).toLocaleString(i18n.language)}
                    </Caption1>
                  </div>

                  <div className={s.row}>
                    <Select
                      size="small"
                      aria-label={t('gamelog.visibilityLabel')}
                      value={item.visibility}
                      disabled={visibilityMut.isPending}
                      onChange={(_, d) =>
                        visibilityMut.mutate({
                          itemId: item.id,
                          visibility: d.value as GamelogVisibility,
                        })
                      }
                    >
                      {VISIBILITY_OPTIONS.map((v) => (
                        <option key={v} value={v}>
                          {t(`gamelog.visibility.${v}`)}
                        </option>
                      ))}
                    </Select>

                    {inGame && (item.visibility === 'friends' || item.visibility === 'public') && (
                      <Button
                        size="small"
                        appearance="subtle"
                        icon={<SendRegular />}
                        disabled={attachMut.isPending}
                        onClick={() => attachMut.mutate(item)}
                      >
                        {t('gamelog.attachToSession')}
                      </Button>
                    )}

                    <Button
                      size="small"
                      appearance="subtle"
                      icon={<DeleteRegular />}
                      aria-label={t('common.delete')}
                      disabled={deleteMut.isPending}
                      onClick={() => deleteMut.mutate(item.id)}
                    />
                  </div>
                </div>

                {item.title && <Body1>{item.title}</Body1>}

                {item.kind === 'screenshot' && media?.url && (
                  <img
                    className={s.mediaPreview}
                    src={media.url}
                    alt={item.body || item.title || ''}
                    loading="lazy"
                  />
                )}

                {item.kind === 'video' && media?.url && (
                  <video
                    className={s.mediaPreview}
                    src={media.url}
                    poster={media.posterUrl ?? undefined}
                    controls
                    preload="metadata"
                  />
                )}

                {item.kind === 'audio' && media?.url && (
                  <div className={s.compactStack}>
                    <audio className={s.audioPlayer} src={media.url} controls preload="metadata" />
                    {media.durationMs != null && (
                      <Caption1 className={s.muted}>
                        {formatDurationMs(media.durationMs)}
                      </Caption1>
                    )}
                  </div>
                )}

                {item.body && <Body1>{item.body}</Body1>}
              </article>
            )
          })}
        </>
      )}
    </div>
  )
}
