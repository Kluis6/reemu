import {
  Body1,
  Button,
  Caption1,
  Field,
  Input,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  MessageBarTitle,
  ProgressBar,
  Spinner,
  Tab,
  TabList,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { DismissRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { Trans, useTranslation } from 'react-i18next'
import { LoadingState } from '../../components/EmptyState'
import { BlockGroup, FieldBlock, ToggleBlock } from '../../components/SettingBlock'
import { useCatalogStatus, useRunCatalogNow, useSetCatalogEnabled } from '../../hooks/useCatalog'
import {
  cancelMetadataScan,
  getMetadataConfig,
  listPendingMatches,
  metadataScanProgress,
  setMetadataConfig,
  startMetadataScan,
  type MetadataConfig,
  type PendingMatch,
} from '../../lib/tauri'
import { errorToast, sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'
import { useTabStyles } from '../../styles/xbox'
import { PendingMatchesSection } from './PendingMatchesSection'
import { ReviewMetadataDialog } from './ReviewMetadataDialog'

const progressKey = ['metadata-progress'] as const
const DISMISS_SS_DEV_WARNING_KEY = 'reemu.dismissSsDevWarning'

function readSsWarningDismissed(): boolean {
  try {
    return sessionStorage.getItem(DISMISS_SS_DEV_WARNING_KEY) === '1'
  } catch {
    return false
  }
}

function writeSsWarningDismissed(dismissed: boolean) {
  try {
    if (dismissed) sessionStorage.setItem(DISMISS_SS_DEV_WARNING_KEY, '1')
    else sessionStorage.removeItem(DISMISS_SS_DEV_WARNING_KEY)
  } catch {
    // sessionStorage indisponível
  }
}

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
  },
  tabs: { marginLeft: '-3px' },
  panel: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
  },
  save: { alignSelf: 'flex-start' },
  scan: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
  },
  catalogStatusRow: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalM,
  },
  scanActions: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  scanStatus: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    padding: tokens.spacingHorizontalM,
    backgroundColor: tokens.colorNeutralBackground3,
    borderRadius: tokens.borderRadiusMedium,
  },
  dim: { color: tokens.colorNeutralForeground3 },
})

export function SettingsMetadata() {
  const { t } = useTranslation()
  const s = useStyles()
  const tb = useTabStyles()
  const [review, setReview] = useState<PendingMatch | null>(null)
  const [tab, setTab] = useState<'results' | 'credentials'>('results')
  const [ssWarningDismissed, setSsWarningDismissed] = useState<boolean>(readSsWarningDismissed)
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const catalog = useCatalogStatus()
  const setCatalog = useSetCatalogEnabled()
  const runCatalog = useRunCatalogNow()

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

  // Recarrega a fila de pendências quando o scan termina.
  useEffect(() => {
    if (!running) {
      void pending.refetch()
      void qc.invalidateQueries({ queryKey: ['roms'] })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running])

  const save = useMutation({
    mutationFn: (c: MetadataConfig) => setMetadataConfig(c),
    onSuccess: () => {
      writeSsWarningDismissed(false)
      setSsWarningDismissed(false)
      void qc.invalidateQueries({ queryKey: ['metadata-config'] })
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

  const matches = pending.data ?? []
  const p = progress.data
  const status = scan.isPending
    ? 'preparing'
    : cancel.isPending
      ? 'cancelling'
      : scan.isError && !running
        ? 'failed'
        : progress.isError
          ? 'unavailable'
          : p?.status === 'completed' && p.total === 0
            ? 'empty'
            : (p?.status ?? 'idle')
  const active = scan.isPending || running
  const showCounts = !scan.isPending && (!scan.isError || running) && !!p?.total
  const ratio = showCounts && p ? p.done / p.total : undefined
  const statusText = t(`metadata.status.${status}`)
  const catalogBusy = runCatalog.isPending || Boolean(catalog.data?.running)
  const hasSsCredentials = Boolean(
    cfg.data?.screenscraperUser?.trim() && cfg.data?.screenscraperPassword?.trim(),
  )
  const showSsDevWarning =
    !scan.isPending && hasSsCredentials && p?.error === 'ss_dev_login' && !ssWarningDismissed
  const hasProgressWarning =
    Boolean(p?.error && p.error !== 'ss_dev_login') ||
    showSsDevWarning ||
    status === 'failed' ||
    status === 'cancelled'

  const dismissSsWarning = () => {
    writeSsWarningDismissed(true)
    setSsWarningDismissed(true)
  }

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

      {tab === 'results' ? (
        <div id="metadata-results" role="tabpanel" aria-labelledby="metadata-results-tab" className={s.panel}>
          {catalog.data?.connected && (
            <div className={s.scan} data-testid="catalog-section">
              <BlockGroup>
                <ToggleBlock
                  label={t('catalog.toggle')}
                  description={t('catalog.privacy')}
                  checked={catalog.data.enabled}
                  busy={setCatalog.isPending}
                  onChange={(on) =>
                    setCatalog.mutate(on, {
                      onError: (e) => push(errorToast(e, 'saveMetadataConfig')),
                    })
                  }
                />
              </BlockGroup>
              {catalog.data.enabled && (
                <div className={s.catalogStatusRow}>
                  <Caption1 className={s.dim} role="status" data-testid="catalog-status-line">
                    {catalog.data.disabledByServer
                      ? t('catalog.disabledByServer')
                      : t('catalog.counts', {
                        found: catalog.data.found,
                        pending: catalog.data.pending,
                        unknown: catalog.data.unknown,
                      })}
                  </Caption1>
                  {!catalog.data.disabledByServer && (
                    <Button
                      size="small"
                      appearance="secondary"
                      disabledFocusable={catalogBusy}
                      aria-busy={catalogBusy || undefined}
                      icon={catalogBusy ? <Spinner size="tiny" /> : undefined}
                      onClick={() =>
                        runCatalog.mutate(undefined, {
                          onSuccess: (st) =>
                            push(
                              sysToast(
                                t('catalog.syncedToast', { found: st.found, pending: st.pending }),
                                'Success',
                              ),
                            ),
                          onError: (e) => push(errorToast(e, 'fetchMetadata')),
                        })
                      }
                    >
                      {catalogBusy ? t('catalog.syncing') : t('catalog.syncNow')}
                    </Button>
                  )}
                </div>
              )}
            </div>
          )}

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
                <Text role="status" weight="semibold">
                  {statusText}
                </Text>
                {(active || ratio !== undefined) && (
                  <ProgressBar
                    aria-label={t('metadata.scan')}
                    thickness="large"
                    value={ratio}
                    color={hasProgressWarning ? 'warning' : 'brand'}
                  />
                )}
                {showCounts && p && (
                  <Caption1 className={s.dim}>
                    {ratio !== undefined ? Math.floor(ratio * 100) + '% · ' : ''}
                    {t('metadata.progress', {
                      done: p.done,
                      total: p.total,
                      auto: p.auto,
                      pending: p.pending,
                      failed: p.failed,
                    })}
                  </Caption1>
                )}
              </div>
            )}
            {showSsDevWarning && (
              <MessageBar intent="warning" data-testid="ss-dev-warning">
                <MessageBarBody>
                  <MessageBarTitle>{t('metadata.errors.ssDevLoginTitle')}.</MessageBarTitle>{' '}
                  {t('metadata.errors.ssDevLogin')}
                </MessageBarBody>
                <MessageBarActions
                  containerAction={
                    <Button
                      appearance="transparent"
                      icon={<DismissRegular />}
                      aria-label={t('shell.close')}
                      onClick={dismissSsWarning}
                    />
                  }
                />
              </MessageBar>
            )}
          </div>

          {pending.isLoading && <LoadingState />}
          {pending.isError && <Body1>{t('metadata.pendingUnavailable')}</Body1>}
          {pending.isSuccess && matches.length === 0 && <Caption1>{t('metadata.noPending')}</Caption1>}
          {matches.length > 0 && (
            <PendingMatchesSection matches={matches} onReview={setReview} />
          )}
        </div>
      ) : (
        <div id="metadata-credentials" role="tabpanel" aria-labelledby="metadata-credentials-tab" className={s.panel}>
          {cfg.isLoading ? (
            <LoadingState />
          ) : cfg.isError || !form ? (
            <Body1>{t('metadata.unavailable')}</Body1>
          ) : (
            <>
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
            </>
          )}
        </div>
      )}
      {review && <ReviewMetadataDialog key={review.romId} match={review} onClose={() => setReview(null)} />}
    </div>
  )
}
