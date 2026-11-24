import {
  Body1,
  Button,
  Caption1,
  Field,
  Image,
  Input,
  MessageBar,
  MessageBarBody,
  MessageBarTitle,
  ProgressBar,
  Tab,
  TabList,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkRegular, DismissRegular, EditRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { LoadingState } from '../../components/EmptyState'
import { ReviewMetadataDialog } from './ReviewMetadataDialog'
import { BlockGroup, FieldBlock } from '../../components/SettingBlock'
import { providerLabel } from '../../lib/metadataFormat'
import { ListIconAction } from '../../components/ListIconAction'
import { PAGE_PAD_B, SAFE_Y } from '../../styles/metrics'
import { useTabStyles } from '../../styles/xbox'
import { errorToast, sysToast } from '../../lib/toast'
import {
  cancelMetadataScan,
  getMetadataConfig,
  listPendingMatches,
  metadataScanProgress,
  resolvePendingMatch,
  setMetadataConfig,
  startMetadataScan,
  type MetadataConfig,
  type PendingMatch,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { Trans, useTranslation } from 'react-i18next'

const progressKey = ['metadata-progress'] as const

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0, gap: tokens.spacingVerticalS },
  tabs: { flexShrink: 0, paddingInlineStart: tokens.spacingHorizontalS },
  tabPanel: {
    flexGrow: 1,
    minHeight: 0,
    overflowY: 'auto',
    overscrollBehavior: 'contain',
    scrollbarGutter: 'stable',
    padding: `8px ${tokens.spacingHorizontalS} ${PAGE_PAD_B - SAFE_Y + 24}px`,
    scrollPaddingBlock: '8px',
  },
  panel: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL },
  save: { alignSelf: 'flex-start' },
  scan: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  scanActions: { display: 'flex', flexWrap: 'wrap', gap: tokens.spacingHorizontalS },
  scanStatus: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    padding: tokens.spacingHorizontalM,
    backgroundColor: tokens.colorNeutralBackground3,
    borderRadius: tokens.borderRadiusMedium,
  },
  pending: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  row: {
    display: 'flex',
    gap: tokens.spacingHorizontalM,
    alignItems: 'center',
    padding: tokens.spacingHorizontalM,
    boxSizing: 'border-box',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  cover: {
    width: '36px',
    height: '48px',
    objectFit: 'cover',
    flexShrink: 0,
    borderRadius: tokens.borderRadiusMedium,
  },
  matchDetails: {
    display: 'flex',
    flexDirection: 'column',
    flexGrow: 1,
    minWidth: 0,
    overflowWrap: 'anywhere',
  },
  matchActions: { display: 'flex', flexShrink: 0, gap: tokens.spacingHorizontalS },
  dim: { color: tokens.colorNeutralForeground3 },
})

export function SettingsMetadata() {
  const { t } = useTranslation()
  const s = useStyles()
  const tb = useTabStyles()
  const [review, setReview] = useState<PendingMatch | null>(null)
  const [tab, setTab] = useState<'results' | 'credentials'>('results')
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const cfg = useQuery({ queryKey: ['metadata-config'], queryFn: getMetadataConfig, retry: false })
  const [form, setForm] = useState<MetadataConfig | null>(null)
  useEffect(() => {
    if (cfg.data && !form) setForm(cfg.data)
  }, [cfg.data, form])

  const progress = useQuery({
    queryKey: progressKey,
    queryFn: metadataScanProgress,
    refetchInterval: (q) => (q.state.data?.running ? 700 : false),
    retry: false,
  })
  const running = progress.data?.running ?? false

  const pending = useQuery({
    queryKey: ['pending-matches'],
    queryFn: listPendingMatches,
    retry: false,
  })
  // recarrega a fila de pendências quando o scan termina
  useEffect(() => {
    if (!running) {
      pending.refetch()
      qc.invalidateQueries({ queryKey: ['roms'] })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running])

  const save = useMutation({
    mutationFn: (c: MetadataConfig) => setMetadataConfig(c),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['metadata-config'] })
      push(sysToast(t('metadata.saved'), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'saveMetadataConfig')),
  })
  const scan = useMutation({
    mutationFn: startMetadataScan,
    onSuccess: async () => {
      await qc.cancelQueries({ queryKey: progressKey })
      const { data } = await progress.refetch()
      if (!data?.running) {
        void qc.invalidateQueries({ queryKey: ['pending-matches'] })
        void qc.invalidateQueries({ queryKey: ['roms'] })
      }
    },
    onError: async (e) => {
      push(errorToast(e, 'fetchMetadata'))
      await progress.refetch()
    },
  })
  const cancel = useMutation({
    mutationFn: cancelMetadataScan,
    onSuccess: () => progress.refetch(),
    onError: (e) => push(errorToast(e, 'fetchMetadata')),
  })
  const resolve = useMutation({
    mutationFn: ({ romId, accept }: { romId: string; accept: boolean }) =>
      resolvePendingMatch(romId, accept),
    onSuccess: () => {
      pending.refetch()
      qc.invalidateQueries({ queryKey: ['roms'] })
    },
    onError: (e) => push(errorToast(e, 'resolveMatch')),
  })

  const matches = pending.data ?? []
  const p = progress.data
  const status = scan.isPending ? 'preparing'
    : cancel.isPending ? 'cancelling'
    : scan.isError && !running ? 'failed'
    : progress.isError ? 'unavailable'
    : p?.status === 'completed' && p.total === 0 ? 'empty'
    : p?.status ?? 'idle'
  const active = scan.isPending || running
  const showCounts = !scan.isPending && (!scan.isError || running) && !!p?.total
  const ratio = showCounts && p ? p.done / p.total : undefined
  const statusText = t(`metadata.status.${status}`)

  return (
    <div className={s.root}>
      <TabList
        className={mergeClasses(tb.tabs, s.tabs)}
        aria-label={t('settings.tabs.metadata')}
        selectedValue={tab}
        onTabSelect={(_, { value }) => setTab(value === 'credentials' ? 'credentials' : 'results')}
      >
        <Tab id="metadata-results-tab" value="results" aria-controls="metadata-results">
          {t('metadata.tabs.results')}
        </Tab>
        <Tab id="metadata-credentials-tab" value="credentials" aria-controls="metadata-credentials">
          {t('metadata.tabs.credentials')}
        </Tab>
      </TabList>

      <div id="metadata-results" className={s.tabPanel} role="tabpanel" aria-labelledby="metadata-results-tab" tabIndex={0} hidden={tab !== 'results'}>
        {tab === 'results' && (
          <div className={s.panel}>
            <Caption1>
              <Trans i18nKey="metadata.intro" components={{ b: <Text as="strong" weight="semibold" /> }} />
            </Caption1>

            <div className={s.scan}>
              <div className={s.scanActions}>
                <Button
                  disabledFocusable={active}
                  aria-busy={active}
                  onClick={() => scan.mutate()}
                >
                  {active ? t('metadata.scanning') : t('metadata.scan')}
                </Button>
                {running && (
                  <Button
                    appearance="subtle"
                    disabledFocusable={status === 'cancelling'}
                    onClick={() => cancel.mutate()}
                  >
                    {t('common.cancel')}
                  </Button>
                )}
              </div>
              {status !== 'idle' && (
                <div className={s.scanStatus}>
                  <Text role="status" weight="semibold">{statusText}</Text>
                  {(active || ratio !== undefined) && (
                    <ProgressBar
                      aria-label={t('metadata.scan')}
                      thickness="large"
                      value={ratio}
                      color={p?.error || status === 'failed' || status === 'cancelled' ? 'warning' : 'brand'}
                    />
                  )}
                  {showCounts && p && (
                    <Caption1 className={s.dim}>
                      {ratio !== undefined ? Math.floor(ratio * 100) + '% · ' : ''}
                      {t('metadata.progress', { done: p.done, total: p.total, auto: p.auto, pending: p.pending, failed: p.failed })}
                    </Caption1>
                  )}
                </div>
              )}
              {!scan.isPending && p?.error === 'ss_dev_login' && (
                <MessageBar intent="warning">
                  <MessageBarBody>
                    <MessageBarTitle>{t('metadata.errors.ssDevLoginTitle')}</MessageBarTitle>
                    {t('metadata.errors.ssDevLogin')}
                  </MessageBarBody>
                </MessageBar>
              )}
            </div>

            {pending.isLoading && <LoadingState />}
            {pending.isError && <Body1>{t('metadata.pendingUnavailable')}</Body1>}
            {pending.isSuccess && matches.length === 0 && <Caption1>{t('metadata.noPending')}</Caption1>}
            {matches.length > 0 && (
              <div className={s.pending}>
                <Caption1>{t('metadata.review', { count: matches.length })}</Caption1>
                {matches.map((m) => {
                  const details = [providerLabel(m.provider), m.releaseDate, m.genre].filter(Boolean).join(' · ')
                  return (
                    <div key={m.romId} className={s.row}>
                      {m.coverUrl && <Image className={s.cover} src={m.coverUrl} alt="" loading="lazy" />}
                      <div className={s.matchDetails}>
                        <Text weight="semibold">{m.title}</Text>
                        <Caption1 className={s.dim}>{t('metadata.file', { name: m.fileStem })}</Caption1>
                        {details && <Caption1 className={s.dim}>{details}</Caption1>}
                      </div>
                      <div className={s.matchActions}>
                        <ListIconAction
                          label={t('metadata.editMatch')}
                          icon={<EditRegular />}
                          disabled={resolve.isPending}
                          onClick={() => setReview(m)}
                        />
                        <ListIconAction
                          label={t('metadata.accept')}
                          icon={<CheckmarkRegular />}
                          disabled={resolve.isPending}
                          onClick={() => resolve.mutate({ romId: m.romId, accept: true })}
                        />
                        <ListIconAction
                          label={t('metadata.reject')}
                          icon={<DismissRegular />}
                          disabled={resolve.isPending}
                          onClick={() => resolve.mutate({ romId: m.romId, accept: false })}
                        />
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        )}
      </div>

      <div id="metadata-credentials" className={s.tabPanel} role="tabpanel" aria-labelledby="metadata-credentials-tab" tabIndex={0} hidden={tab !== 'credentials'}>
        {tab === 'credentials' && (
          cfg.isLoading ? <LoadingState /> : cfg.isError || !form ? <Body1>{t('metadata.unavailable')}</Body1> : (
            <div className={s.panel}>
              <Caption1>{t('metadata.credentialsIntro')}</Caption1>

              <BlockGroup>
                <FieldBlock>
                  <Field label={t('metadata.ssUser')}>
                    <Input
                      appearance="filled-darker"
                      value={form.screenscraperUser ?? ''}
                      onChange={(_, d) => setForm({ ...form, screenscraperUser: d.value || null })}
                    />
                  </Field>
                </FieldBlock>
                <FieldBlock>
                  <Field label={t('metadata.ssPassword')}>
                    <Input
                      appearance="filled-darker"
                      type="password"
                      value={form.screenscraperPassword ?? ''}
                      onChange={(_, d) => setForm({ ...form, screenscraperPassword: d.value || null })}
                    />
                  </Field>
                </FieldBlock>
                <FieldBlock>
                  <Field label={t('metadata.tgdbKey')} hint={t('metadata.tgdbKeyHint')}>
                    <Input
                      appearance="filled-darker"
                      type="password"
                      value={form.thegamesdbApiKey ?? ''}
                      onChange={(_, d) => setForm({ ...form, thegamesdbApiKey: d.value || null })}
                    />
                  </Field>
                </FieldBlock>
              </BlockGroup>
              <Button
                className={s.save}
                appearance="primary"
                disabled={save.isPending}
                onClick={() => save.mutate(form)}
              >
                {t('common.save')}
              </Button>
            </div>
          )
        )}
      </div>
      {review && <ReviewMetadataDialog key={review.romId} match={review} onClose={() => setReview(null)} />}
    </div>
  )
}
