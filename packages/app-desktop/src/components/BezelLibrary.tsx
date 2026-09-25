import {
  Body1,
  Button,
  Caption1,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowDownloadRegular,
  ArrowSyncRegular,
  CheckmarkCircleRegular,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useRef } from 'react'
import { platformLabel } from '../lib/platform'
import {
  bezelCatalog,
  downloadBezelPack,
  listRoms,
  type BezelProgress,
} from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'
import { errorPatch } from '../lib/toast'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
  list: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXXS,
    maxHeight: '300px',
    overflowY: 'auto',
    paddingRight: tokens.spacingHorizontalXS,
  },
  row: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalS,
    justifyContent: 'space-between',
  },
  name: { display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalXS },
  done: { color: tokens.colorPaletteGreenForeground1 },
})

const mb = (n: number) => (n / 1048576).toFixed(0)

/**
 * Baixa bezels do **The Bezel Project** (um repo `bezelproject-<sistema>`) e
 * re-importa a árvore como pack de decoração. Lista só os sistemas que estão
 * na biblioteca do usuário E têm bezel publicado. Os packs são pesados
 * (100–600 MB) — download é sob demanda, por sistema.
 */
export function BezelLibrary() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const updateToast = useToastStore((st) => st.update)
  const dlId = useRef<string | null>(null)

  const cat = useQuery({ queryKey: ['bezel-catalog'], queryFn: bezelCatalog, retry: false })
  const roms = useQuery({ queryKey: ['roms'], queryFn: listRoms, retry: false })

  const dl = useMutation({
    mutationFn: (systemId: string) =>
      downloadBezelPack(systemId, (p: BezelProgress) => {
        if (!dlId.current) return
        const label = platformLabel(systemId)
        const pct = p.total ? p.received / p.total : null
        updateToast(dlId.current, {
          message:
            p.phase === 'download'
              ? t('bezels.downloadingMb', { label, received: mb(p.received), total: p.total ? `/${mb(p.total)}` : '' })
              : p.phase === 'extract'
                ? t('bezels.extracting', { label })
                : t('bezels.matching'),
          progress: p.phase === 'download' ? pct : null,
        })
      }),
    onMutate: (systemId) => {
      const id = crypto.randomUUID()
      dlId.current = id
      push({
        id,
        message: t('bezels.downloading', { label: platformLabel(systemId) }),
        variant: 'Info',
        durationMs: 0,
        source: 'System',
        progress: null,
      })
    },
    onSuccess: (n, systemId) => {
      if (dlId.current)
        updateToast(dlId.current, {
          message: t('bezels.ready', { label: platformLabel(systemId), count: n }),
          variant: 'Success',
          durationMs: 4000,
          progress: undefined,
        })
      qc.invalidateQueries({ queryKey: ['bezel-catalog'] })
    },
    onError: (e) => {
      if (dlId.current)
        updateToast(dlId.current, errorPatch(e, 'downloadBezels'))
    },
    onSettled: () => {
      dlId.current = null
    },
  })

  if (cat.isLoading || roms.isLoading) return <Spinner size="tiny" label={t('common.loading')} />
  if (cat.isError || !cat.data) return <Body1>{t('bezels.unavailable')}</Body1>

  const libSystems = new Set((roms.data ?? []).map((r) => r.systemId))
  const rows = cat.data.filter((c) => libSystems.has(c.systemId))

  if (rows.length === 0)
    return (
      <Caption1>
        {t('bezels.none')}
      </Caption1>
    )

  return (
    <div className={s.root}>
      <div className={s.list}>
        {rows.map((c) => (
          <div key={c.systemId} className={s.row}>
            <span className={s.name}>
              {c.installed && (
                <CheckmarkCircleRegular className={s.done} aria-label={t('bezels.downloaded')} />
              )}
              <Body1>{platformLabel(c.systemId)}</Body1>
            </span>
            <Button
              size="small"
              appearance={c.installed ? 'subtle' : 'secondary'}
              icon={
                dl.isPending && dl.variables === c.systemId ? (
                  <Spinner size="tiny" />
                ) : c.installed ? (
                  <ArrowSyncRegular />
                ) : (
                  <ArrowDownloadRegular />
                )
              }
              disabled={dl.isPending}
              onClick={() => dl.mutate(c.systemId)}
            >
              {c.installed ? t('bezels.reinstall') : t('bezels.download')}
            </Button>
          </div>
        ))}
      </div>
      <Caption1>{t('bezels.footer')}</Caption1>
    </div>
  )
}
