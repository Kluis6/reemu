import {
  Body1,
  Button,
  Caption1,
  Field,
  Image,
  Input,
  ProgressBar,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkRegular, DismissRegular, SearchRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { LoadingState } from '../../components/EmptyState'
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

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL, maxWidth: '520px' },
  form: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS, maxWidth: '360px' },
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
    queryKey: ['metadata-progress'],
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
    mutationFn: () => startMetadataScan(),
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

  return (
    <div className={s.root}>
      <Caption1>
        <Trans i18nKey="metadata.intro" components={{ b: <Text as="strong" weight="semibold" /> }} />
      </Caption1>

      <div className={s.form}>
        <Field label={t('metadata.ssUser')}>
          <Input
            value={form.screenscraperUser ?? ''}
            onChange={(_, d) => setForm({ ...form, screenscraperUser: d.value || null })}
          />
        </Field>
        <Field label={t('metadata.ssPassword')}>
          <Input
            type="password"
            value={form.screenscraperPassword ?? ''}
            onChange={(_, d) => setForm({ ...form, screenscraperPassword: d.value || null })}
          />
        </Field>
        <Field
          label={t('metadata.tgdbKey')}
          hint={t('metadata.tgdbKeyHint')}
        >
          <Input
            type="password"
            value={form.thegamesdbApiKey ?? ''}
            onChange={(_, d) => setForm({ ...form, thegamesdbApiKey: d.value || null })}
          />
        </Field>
        <Button
          appearance="primary"
          disabled={save.isPending}
          onClick={() => save.mutate(form)}
        >
          {t('common.save')}
        </Button>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <div style={{ display: 'flex', gap: 8 }}>
          <Button
            icon={<SearchRegular />}
            disabled={running || scan.isPending}
            onClick={() => scan.mutate()}
          >
            {running ? t('metadata.scanning') : t('metadata.scan')}
          </Button>
          {running && (
            <Button appearance="subtle" onClick={() => void cancelMetadataScan()}>
              {t('common.cancel')}
            </Button>
          )}
        </div>
        {p && (p.running || p.done > 0) && (
          <>
            <ProgressBar value={p.total ? p.done / p.total : undefined} />
            <Caption1 className={s.dim}>
              {t('metadata.progress', { done: p.done, total: p.total, auto: p.auto, pending: p.pending, failed: p.failed })}
            </Caption1>
          </>
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
                size="small"
                icon={<CheckmarkRegular />}
                disabled={resolve.isPending}
                onClick={() => resolve.mutate({ romId: m.romId, accept: true })}
              >
                {t('metadata.accept')}
              </Button>
              <Button
                size="small"
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
