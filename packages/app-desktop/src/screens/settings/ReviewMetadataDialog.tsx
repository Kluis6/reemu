import { Body1, Button, Caption1, Field, Input, MessageBar, MessageBarBody, Spinner, Tab, TabList, Title3, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FullscreenDialog } from '../../components/FullscreenDialog'
import {
  GameArtworkFields, GameMetadataFields, gameMetadataDraft, gameMetadataLabels, isLocalArtwork, readGameArtwork, validMetadataDraft, type GameArtworkKey,
} from '../../components/GameMetadataFields'
import { providerLabel } from '../../lib/metadataFormat'
import { getRomMetadata, listRoms, pickImage, resolvePendingMatch, reviewPendingMatch, type GameMetadataEdits, type PendingMatch } from '../../lib/tauri'
import { errorToast, sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'
import { useModalButtonStyles } from '../../styles/buttons'
import { useTabStyles } from '../../styles/xbox'

const useStyles = makeStyles({
  content: { minHeight: 0, overflow: 'hidden' },
  comparison: {
    display: 'grid',
    height: '100%',
    gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
    gridTemplateRows: 'minmax(0, 1fr)',
    gap: tokens.spacingHorizontalL,
    '@media (max-width: 760px)': { gap: tokens.spacingHorizontalS },
  },
  panel: {
    display: 'flex',
    flexDirection: 'column',
    minWidth: 0,
    minHeight: 0,
    overflow: 'hidden',
    backgroundColor: 'var(--reemuSurface1)',
    borderRadius: tokens.borderRadiusMedium,
  },
  panelHeader: {
    display: 'grid',
    flexShrink: 0,
    gap: tokens.spacingVerticalS,
    padding: tokens.spacingHorizontalM,
  },
  heading: { margin: 0 },
  column: {
    display: 'flex',
    flexDirection: 'column',
    flexGrow: 1,
    minHeight: 0,
    gap: tokens.spacingVerticalM,
    overflowY: 'auto',
    overscrollBehavior: 'contain',
    scrollbarGutter: 'stable',
    padding: tokens.spacingHorizontalM,
    scrollPaddingBlock: tokens.spacingVerticalS,
  },
  values: { display: 'grid', gap: tokens.spacingVerticalS, margin: 0 },
  entry: { display: 'grid', gap: tokens.spacingVerticalXXS, overflowWrap: 'anywhere' },
  label: { color: tokens.colorNeutralForeground3 },
  value: { margin: 0, whiteSpace: 'pre-wrap' },
  tabs: { marginLeft: 0 },
  hidden: { display: 'none' },
  currentArtwork: { display: 'block', maxWidth: '100%', maxHeight: '112px', objectFit: 'contain' },
})

export function ReviewMetadataDialog({ match, onClose }: { match: PendingMatch; onClose: () => void }) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const btn = useModalButtonStyles()
  const tabs = useTabStyles()
  const qc = useQueryClient()
  const push = useToastStore((state) => state.push)
  const id = match.romId
  const language = i18n.resolvedLanguage ?? i18n.language
  const proposalKey = ['pending-match-review', id] as const
  const current = useQuery({
    queryKey: ['rom-metadata', id, language],
    queryFn: () => getRomMetadata(id, language),
    retry: false,
  })
  const library = useQuery({
    queryKey: ['roms'],
    queryFn: listRoms,
    enabled: current.isSuccess && !current.data,
    retry: false,
  })
  const proposal = useQuery({
    queryKey: proposalKey,
    queryFn: () => reviewPendingMatch(id),
    retry: false,
  })
  const [editedTitle, setTitle] = useState<string>()
  const [tab, setTab] = useState<'data' | 'artwork'>('data')
  const [edits, setEdits] = useState<GameMetadataEdits>({})
  const [notice, setNotice] = useState<'refreshEmpty' | 'refreshSuccess' | null>(null)
  const title = editedTitle ?? proposal.data?.title ?? match.title
  const suggested = gameMetadataDraft(proposal.data)
  const draft = { ...suggested, ...edits }
  const currentTitle = current.data?.title ?? library.data?.find((rom) => rom.id === id)?.title ?? match.fileStem

  const refresh = useMutation({
    mutationFn: () => reviewPendingMatch(id, true),
    onMutate: async () => {
      setNotice(null)
      await qc.cancelQueries({ queryKey: proposalKey })
    },
    onSuccess: (next) => {
      if (next) {
        qc.setQueryData(proposalKey, next)
        void qc.invalidateQueries({ queryKey: ['pending-matches'] })
      }
      setNotice(next ? 'refreshSuccess' : 'refreshEmpty')
    },
    onError: (error) => push(errorToast(error, 'fetchMetadata')),
  })
  const apply = useMutation({
    mutationFn: () => resolvePendingMatch(id, true, title.trim(), Object.keys(edits).length ? edits : undefined),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['pending-matches'] })
      void qc.invalidateQueries({ queryKey: ['roms'] })
      void qc.invalidateQueries({ queryKey: ['rom-metadata', id] })
      push(sysToast(t('game.updated'), 'Success'))
      onClose()
    },
    onError: (error) => push(errorToast(error, 'saveChanges')),
  })
  const change = (key: keyof GameMetadataEdits, value: string) => setEdits((previous) => ({ ...previous, [key]: value }))
  const upload = useMutation({
    mutationFn: async ({ file }: { key: GameArtworkKey; file?: File }) => {
      const selected = file ?? await pickImage()
      return selected ? readGameArtwork(selected) : null
    },
    onSuccess: (url, { key }) => { if (url) change(key, url) },
  })
  const busy = refresh.isPending || apply.isPending || upload.isPending
  const loading = current.isLoading || proposal.isLoading || library.isLoading

  return (
    <FullscreenDialog
      open
      onClose={() => { if (!busy) onClose() }}
      title={t('metadata.compareTitle')}
      contentClassName={s.content}
      actions={(
        <>
          <Button
            appearance="secondary"
            className={btn.secondaryFlat}
            aria-busy={refresh.isPending}
            disabledFocusable={busy || !proposal.data}
            onClick={() => refresh.mutate()}
          >
            {t(refresh.isPending ? 'game.rescraping' : 'game.rescrape')}
          </Button>
          <Button
            appearance="primary"
            disabledFocusable={busy || loading || current.isError || !proposal.data || !title.trim() || !validMetadataDraft(edits)}
            onClick={() => apply.mutate()}
          >
            {t('metadata.applyEdits')}
          </Button>
        </>
      )}
    >
      {loading ? <Spinner label={t('common.loading')} /> : (
        <div className={s.comparison}>
          <section className={s.panel}>
            <div className={s.panelHeader}>
              <Title3 className={s.heading} as="h2" id="metadata-current-heading">{t('metadata.currentData')}</Title3>
            </div>
            <div className={s.column} role="region" aria-labelledby="metadata-current-heading" tabIndex={0}>
              {current.isError ? <Body1>{t('metadata.compareUnavailable')}</Body1> : (
                <>
                  <Caption1>{t('metadata.file', { name: match.fileStem })}</Caption1>
                  <dl className={s.values}>
                    <div className={s.entry}>
                      <dt className={s.label}>{t('common.name')}</dt>
                      <dd className={s.value}>{currentTitle}</dd>
                    </div>
                    {gameMetadataLabels.map(([key, label]) => {
                      const value = current.data?.[key]
                      return value == null || value === '' ? null : (
                        <div key={key} className={s.entry}>
                          <dt className={s.label}>{t(label)}</dt>
                          <dd className={s.value}>
                            {typeof value === 'string' && isLocalArtwork(value)
                              ? <img className={s.currentArtwork} src={value} alt={t(label)} />
                              : value}
                          </dd>
                        </div>
                      )
                    })}
                  </dl>
                </>
              )}
            </div>
          </section>
          <section className={s.panel}>
            <div className={s.panelHeader}>
              <Title3 className={s.heading} as="h2" id="metadata-proposal-heading">{t('metadata.apiSuggestion')}</Title3>
              {notice && (
                <MessageBar intent={notice === 'refreshSuccess' ? 'success' : 'info'}>
                  <MessageBarBody>{t(`metadata.${notice}`)}</MessageBarBody>
                </MessageBar>
              )}
              {proposal.data && (
                <TabList
                  className={mergeClasses(tabs.tabs, s.tabs)}
                  aria-labelledby="metadata-proposal-heading"
                  selectedValue={tab}
                  onTabSelect={(_, { value }) => setTab(value === 'artwork' ? 'artwork' : 'data')}
                >
                  <Tab value="data" id="metadata-review-data-tab" aria-controls="metadata-review-data">{t('game.catalog.data')}</Tab>
                  <Tab value="artwork" id="metadata-review-artwork-tab" aria-controls="metadata-review-artwork">{t('game.catalog.artwork')}</Tab>
                </TabList>
              )}
            </div>
            {!proposal.data ? <Body1 className={s.column}>{t('metadata.compareUnavailable')}</Body1> : (
              <>
                <div
                  className={mergeClasses(s.column, tab !== 'data' && s.hidden)}
                  id="metadata-review-data"
                  role="tabpanel"
                  aria-labelledby="metadata-review-data-tab"
                  tabIndex={0}
                  hidden={tab !== 'data'}
                >
                  <Caption1>{providerLabel(proposal.data.providerSource) ?? providerLabel(match.provider)}</Caption1>
                  <Field label={t('common.name')} required>
                    <Input value={title} onChange={(_, data) => setTitle(data.value)} />
                  </Field>
                  <GameMetadataFields draft={draft} catalog={proposal.data.gamekave} onChange={change} />
                </div>
                <div
                  className={mergeClasses(s.column, tab !== 'artwork' && s.hidden)}
                  id="metadata-review-artwork"
                  role="tabpanel"
                  aria-labelledby="metadata-review-artwork-tab"
                  tabIndex={0}
                  hidden={tab !== 'artwork'}
                >
                  {upload.isError && (
                    <MessageBar intent="error">
                      <MessageBarBody>{t('game.catalog.invalidArtFile')}</MessageBarBody>
                    </MessageBar>
                  )}
                  <GameArtworkFields
                    draft={draft}
                    catalog={proposal.data.gamekave}
                    onChange={change}
                    disabled={busy}
                    onUpload={(key, file) => upload.mutate({ key, file })}
                  />
                </div>
              </>
            )}
          </section>
        </div>
      )}
    </FullscreenDialog>
  )
}
