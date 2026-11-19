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
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkRegular, DismissRegular, SearchRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { LoadingState } from '../../components/EmptyState'
import { BlockGroup, FieldBlock } from '../../components/SettingBlock'
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
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { Trans, useTranslation } from 'react-i18next'

const progressKey = ['metadata-progress'] as const

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL },
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
  pending: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
  row: {
    display: 'flex',
    gap: tokens.spacingHorizontalM,
    alignItems: 'center',
    padding: tokens.spacingVerticalS,
    borderRadius: tokens.borderRadiusMedium,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  cover: {
    width: '36px',
    height: '48px',
    objectFit: 'cover',
    flexShrink: 0,
    borderRadius: tokens.borderRadiusMedium,
  },
  grow: { flexGrow: 1, minWidth: 0 },
  dim: { color: tokens.colorNeutralForeground3 },
})

export function SettingsMetadata() {
  const { t } = useTranslation()
  const s = useStyles()
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

  if (cfg.isLoading) return <LoadingState />
  if (cfg.isError || !form) return <Body1>{t('metadata.unavailable')}</Body1>

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
      <Caption1>
        <Trans i18nKey="metadata.intro" components={{ b: <Text as="strong" weight="semibold" /> }} />
      </Caption1>

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

      <div className={s.scan}>
        <div className={s.scanActions}>
          <Button
            icon={<SearchRegular />}
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

      {(pending.data?.length ?? 0) > 0 && (
        <div className={s.pending}>
          <Caption1>{t('metadata.review', { count: pending.data!.length })}</Caption1>
          {pending.data!.map((m) => (
            <div key={m.romId} className={s.row}>
              {m.coverUrl && <Image className={s.cover} src={m.coverUrl} alt="" />}
              <div className={s.grow}>
                <Body1>{m.title}</Body1>
                <Caption1 className={s.dim}>
                  {m.fileStem}
                  {m.releaseDate ? ` · ${m.releaseDate}` : ''}
                  {m.genre ? ` · ${m.genre}` : ''}
                </Caption1>
              </div>
              <Button
                icon={<CheckmarkRegular />}
                disabled={resolve.isPending}
                onClick={() => resolve.mutate({ romId: m.romId, accept: true })}
              >
                {t('metadata.accept')}
              </Button>
              <Button
                appearance="subtle"
                icon={<DismissRegular />}
                disabled={resolve.isPending}
                onClick={() => resolve.mutate({ romId: m.romId, accept: false })}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
