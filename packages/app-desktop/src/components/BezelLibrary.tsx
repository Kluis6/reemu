import {
  Badge,
  Body1,
  Text,
  Caption1,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowCircleUpRegular,
  ArrowSyncRegular,
  CheckmarkCircleFilled,
  CloudArrowDownRegular,
  FrameRegular,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useRef } from 'react'
import { platformLabel } from '../lib/platform'
import {
  bezelCatalog,
  bezelUpdates,
  downloadBezelPack,
  listRoms,
  type BezelProgress,
} from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'
import { errorPatch } from '../lib/toast'
import { ListIconAction } from './ListIconAction'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  // Uma coluna, uma faixa por sistema (pedido do usuário, 2026-11-09).
  // Sem rolagem própria: a área das Configurações já rola.
  list: {
    display: 'grid',
    gridTemplateColumns: 'minmax(0, 1fr)',
    gap: tokens.spacingVerticalM,
  },
  // ícone num quadrado | sistema e status (cresce) | botão — o mesmo desenho
  // das listas de Cores e BIOS, com o mesmo padding nos 4 lados
  row: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalL,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
  },
  // ícone de moldura num quadrado na cor sólida do tema (visível em todos)
  tile: {
    width: '48px',
    height: '48px',
    flexShrink: 0,
    display: 'grid',
    placeItems: 'center',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuBrandSolid)',
    color: 'var(--reemuOnBrand)',
    fontSize: '28px',
  },
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
  // Versão nova no repositório do The Bezel Project. Falha (sem rede, limite
  // da API) não mostra nada: o "Reinstalar" continua lá.
  const updates = useQuery({
    queryKey: ['bezel-updates'],
    queryFn: () => bezelUpdates(),
    retry: false,
    staleTime: 60 * 60 * 1000,
  })
  const outdated = new Set(updates.data ?? [])

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
      qc.invalidateQueries({ queryKey: ['bezel-updates'] })
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
        {rows.map((c) => {
          const needsUpdate = c.installed && outdated.has(c.systemId)
          const downloading = dl.isPending && dl.variables === c.systemId
          const name = platformLabel(c.systemId)
          const ActionIcon = needsUpdate ? ArrowCircleUpRegular : c.installed ? ArrowSyncRegular : CloudArrowDownRegular
          const actionLabel = needsUpdate ? 'bezels.update' : c.installed ? 'bezels.reinstall' : 'bezels.download'
          return (
            <div key={c.systemId} className={s.row}>
              <span className={s.tile} aria-hidden>
                <FrameRegular />
              </span>
              <span className={s.meta}>
                <Body1>
                  <Text as="strong" weight="semibold">{name}</Text>
                </Body1>
                {needsUpdate ? (
                  <Badge appearance="tint" color="brand" icon={<ArrowCircleUpRegular />}>
                    {t('bezels.updateAvailable')}
                  </Badge>
                ) : c.installed ? (
                  <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                    {t('bezels.downloaded')}
                  </Badge>
                ) : null}
              </span>
              <ListIconAction
                primary={needsUpdate || !c.installed}
                label={downloading ? t('bezels.downloading', { label: name }) : t(actionLabel)}
                icon={downloading ? <Spinner size="tiny" /> : <ActionIcon />}
                disabled={dl.isPending}
                onClick={() => dl.mutate(c.systemId)}
              />
            </div>
          )
        })}
      </div>
    </div>
  )
}
