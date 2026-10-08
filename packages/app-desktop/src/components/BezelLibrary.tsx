import {
  Badge,
  Body1,
  Button,
  Text,
  Caption1,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowSyncRegular,
  CheckmarkCircleFilled,
  FrameRegular,
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
import { DownloadButton } from './DownloadButton'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  // crédito das molduras baixadas (The Bezel Project), em texto secundário
  credits: { color: tokens.colorNeutralForeground3 },
  // Cards em 2 colunas, como a lista de cores (uma só em janela estreita).
  // Sem rolagem própria: a área das Configurações já rola.
  list: {
    display: 'grid',
    gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
    gap: tokens.spacingVerticalM,
    '@media (max-width: 640px)': { gridTemplateColumns: 'minmax(0, 1fr)' },
  },
  // ícone | sistema e status (cresce) | botão — mesmo card da lista de cores
  row: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
  },
  icon: { fontSize: '24px', flexShrink: 0, color: tokens.colorNeutralForeground2 },
  // Nome e selo "baixado" na mesma linha: o nome fica na mesma altura em
  // todos os cards, baixados ou não.
  meta: {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    gap: tokens.spacingHorizontalS,
    minWidth: 0,
    flexGrow: 1,
    overflowWrap: 'anywhere',
  },
  action: { flexShrink: 0 },
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
            <FrameRegular className={s.icon} />
            <span className={s.meta}>
              <Body1>
                <Text as="strong" weight="semibold">
                  {platformLabel(c.systemId)}
                </Text>
              </Body1>
              {c.installed && (
                <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                  {t('bezels.downloaded')}
                </Badge>
              )}
            </span>
            {c.installed ? (
              <Button
                className={s.action}
                appearance="subtle"
                icon={
                  dl.isPending && dl.variables === c.systemId ? (
                    <Spinner size="tiny" />
                  ) : (
                    <ArrowSyncRegular />
                  )
                }
                disabled={dl.isPending}
                onClick={() => dl.mutate(c.systemId)}
              >
                {t('bezels.reinstall')}
              </Button>
            ) : (
              <DownloadButton
                className={s.action}
                label={t('bezels.download')}
                busy={dl.isPending && dl.variables === c.systemId}
                disabled={dl.isPending}
                onClick={() => dl.mutate(c.systemId)}
              />
            )}
          </div>
        ))}
      </div>
      <Caption1>{t('bezels.footer')}</Caption1>
      <Caption1 className={s.credits}>{t('bezels.credits')}</Caption1>
    </div>
  )
}
