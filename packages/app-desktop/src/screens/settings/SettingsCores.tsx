import {
  Badge,
  Body1,
  Button,
  Caption1,
  Input,
  Spinner,
  Text,
  Tooltip,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowCircleUpRegular,
  CheckmarkCircleFilled,
  DeleteRegular,
  DeveloperBoardRegular,
  HistoryRegular,
  SettingsRegular,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState, type ReactElement } from 'react'
import { LoadingState } from '../../components/EmptyState'
import { CORE_UPDATES_KEY, CORE_UPDATES_STALE_MS } from '../../hooks/useCoreUpdates'
import { errorToast, sysToast } from '../../lib/toast'
import { TOOLTIP_POSITION } from '../../lib/tooltip'
import {
  checkCoreUpdates,
  downloadCore,
  listCoreCatalog,
  removeCore,
  restorePreviousCore,
  type CatalogCore,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { DownloadButton } from '../../components/DownloadButton'
import { Trans, useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'

// Botões da direita quadrados, só com ícone. Três vagas, para não mudarem de lugar nas
// linhas com Atualizar ou Versão anterior.
const ACTION_W = '40px'
// Cabe "atualização disponível" numa linha, nos três idiomas.
const STATUS_W = '190px'
const ACTIONS_W = `calc(3 * ${ACTION_W} + 2 * ${tokens.spacingHorizontalS})`

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  // Busca: o campo com largura de leitura e a contagem ao lado.
  filterRow: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
    '& .fui-Input': { width: 'min(420px, 100%)' },
  },
  updateAll: { marginLeft: 'auto', flexShrink: 0 },
  // Lista em linhas, como a Biblioteca da Microsoft Store (print do usuário):
  // uma coluna, cada core numa faixa da largura toda.
  list: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  // ícone + nome | sistemas | situação | botão
  row: {
    display: 'grid',
    gridTemplateColumns: `minmax(0, 2fr) minmax(0, 1.4fr) ${STATUS_W} ${ACTIONS_W}`,
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalL,
    // o mesmo padding nos 4 lados (pedido do usuário); a altura sai do
    // quadrado do ícone (48) + 12 + 12
    padding: tokens.spacingHorizontalM,
    boxSizing: 'border-box',
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
    // janela estreita / interface grande: sistemas e situação vão pra
    // baixo do nome
    '@media (max-width: 900px)': { gridTemplateColumns: `minmax(0, 1fr) ${ACTIONS_W}` },
  },
  // ícone dentro de um quadrado (o "ícone do app" da Store)
  lead: { display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalM, minWidth: 0 },
  tile: {
    width: '48px',
    height: '48px',
    flexShrink: 0,
    display: 'grid',
    placeItems: 'center',
    borderRadius: tokens.borderRadiusMedium,
    // Cor sólida do tema com o ícone na cor de contraste dela (o par do
    // tema, `reemuBrandSolid`/`reemuOnBrand`), como os ícones coloridos da
    // Store: destaca do cinza da linha em qualquer tema. Com o
    // `Background3` neutro (e depois o `BrandBackground2`), nos temas
    // claros o quadrado sumia (print do usuário).
    backgroundColor: 'var(--reemuBrandSolid)',
    color: 'var(--reemuOnBrand)',
    fontSize: '28px',
  },
  // `minWidth: 0` deixa o texto quebrar dentro da coluna
  meta: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
    minWidth: 0,
    overflowWrap: 'anywhere',
  },
  sub: { color: tokens.colorNeutralForeground3 },
  // colunas do meio: some na janela estreita (o nome mostra os sistemas)
  col: {
    minWidth: 0,
    overflowWrap: 'anywhere',
    '@media (max-width: 900px)': { display: 'none' },
  },
  narrowOnly: { display: 'none', '@media (max-width: 900px)': { display: 'inline' } },
  action: {
    width: ACTION_W,
    height: ACTION_W,
    padding: '0 !important',
    minWidth: '0 !important',
    maxWidth: 'none !important',
  },
  badge: { whiteSpace: 'nowrap' },
  actions: {
    display: 'flex',
    justifyContent: 'flex-end',
    columnGap: tokens.spacingHorizontalS,
  },
})

/** Só com ícone: o nome vai na dica e no rótulo acessível (Fluent 2, Button). */
function IconAction({
  label,
  icon,
  className,
  primary = false,
  disabled,
  onClick,
}: {
  label: string
  icon: ReactElement
  className: string
  primary?: boolean
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <Tooltip positioning={TOOLTIP_POSITION} content={label} relationship="label">
      <Button
        className={className}
        appearance={primary ? 'primary' : 'secondary'}
        icon={icon}
        disabled={disabled}
        onClick={onClick}
      />
    </Tooltip>
  )
}

/**
 * Configurações › Cores: só o catálogo (pedido do usuário — a aba
 * "Instalados" saiu). Os instalados aparecem no próprio catálogo, com o
 * selo "Instalado" e o botão de remover.
 */
export function SettingsCores() {
  const styles = useStyles()
  return (
    <div className={styles.root}>
      <Catalog />
    </div>
  )
}

function Catalog() {
  const { t } = useTranslation()
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const navigate = useNavigate()
  const catalog = useQuery({ queryKey: ['core-catalog'], queryFn: listCoreCatalog, retry: false })
  const updates = useQuery({
    queryKey: CORE_UPDATES_KEY,
    queryFn: checkCoreUpdates,
    staleTime: CORE_UPDATES_STALE_MS,
    retry: false,
  })
  const outdated = updates.data ?? []
  const [filter, setFilter] = useState('')

  // Atualizou: sai da lista sem consultar tudo de novo. A DLL nova é tão nova
  // quanto a última compilação, e o backend diria o mesmo.
  const updated = (coreId: string) => {
    qc.setQueryData<string[]>(CORE_UPDATES_KEY, (ids) => ids?.filter((id) => id !== coreId))
    qc.invalidateQueries({ queryKey: ['core-catalog'] })
    qc.invalidateQueries({ queryKey: ['installed-cores'] })
  }
  const update = useMutation({
    mutationFn: (coreId: string) => downloadCore(coreId),
    onSuccess: (_d, coreId) => {
      updated(coreId)
      push(sysToast(t('cores.updatedToast', { id: coreId }), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'downloadCore')),
  })
  const updateAll = useMutation({
    mutationFn: async (ids: string[]) => {
      // um por vez: cada um é um download de dezenas de MB
      for (const id of ids) {
        await downloadCore(id)
        updated(id)
      }
    },
    onSuccess: (_d, ids) =>
      push(sysToast(t('cores.updatedAllToast', { count: ids.length }), 'Success')),
    onError: (e) => push(errorToast(e, 'downloadCore')),
  })
  const restore = useMutation({
    mutationFn: (coreId: string) => restorePreviousCore(coreId),
    onSuccess: (_d, coreId) => {
      qc.invalidateQueries({ queryKey: ['core-catalog'] })
      qc.invalidateQueries({ queryKey: ['installed-cores'] })
      // a versão de volta é antiga: o aviso de atualização pode voltar
      qc.invalidateQueries({ queryKey: CORE_UPDATES_KEY })
      push(sysToast(t('cores.restoredToast', { id: coreId }), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'restoreCore')),
  })

  const install = useMutation({
    mutationFn: (coreId: string) => downloadCore(coreId),
    onSuccess: (_d, coreId) => {
      qc.invalidateQueries({ queryKey: ['core-catalog'] })
      qc.invalidateQueries({ queryKey: ['installed-cores'] })
      push(sysToast(t('cores.installedToast', { id: coreId }), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'downloadCore')),
  })
  const uninstall = useMutation({
    mutationFn: (coreId: string) => removeCore(coreId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['core-catalog'] })
      qc.invalidateQueries({ queryKey: ['installed-cores'] })
    },
    onError: (e) => push(errorToast(e, 'removeCore')),
  })

  if (catalog.isLoading) return <LoadingState label={t('cores.loadingCatalog')} />
  if (catalog.isError) return <Body1>{t('cores.catalogUnavailable')}</Body1>

  const busy = (id: string) =>
    (install.isPending && install.variables === id) ||
    (uninstall.isPending && uninstall.variables === id) ||
    (update.isPending && update.variables === id) ||
    (restore.isPending && restore.variables === id) ||
    updateAll.isPending

  const f = filter.trim().toLowerCase()
  const sorted = [...(catalog.data ?? [])]
    .filter((c) => !f || `${c.name} ${c.systems}`.toLowerCase().includes(f))
    .sort((a, b) => a.name.localeCompare(b.name))

  return (
    <>
      <Caption1>
        <Trans i18nKey="cores.intro" components={{ b: <Text as="strong" weight="semibold" /> }} />
      </Caption1>
      <div className={styles.filterRow}>
        <Input
          // sem o anel de foco (pedido do usuário; ver `reemu-no-ring` no
          // index.css) e com fundo/borda visíveis em qualquer tema
          className="reemu-no-ring reemu-list-search"
          appearance="filled-darker"
          placeholder={t('cores.filter')}
          value={filter}
          onChange={(_, d) => setFilter(d.value)}
        />
        <Caption1>
          {t('cores.count', { shown: sorted.length, total: catalog.data?.length ?? 0 })}
        </Caption1>
        {outdated.length > 0 && (
          <Button
            className={styles.updateAll}
            appearance="primary"
            icon={<ArrowCircleUpRegular />}
            disabled={updateAll.isPending || update.isPending}
            onClick={() => updateAll.mutate(outdated)}
          >
            {updateAll.isPending ? t('cores.updating') : t('cores.updateAll', { count: outdated.length })}
          </Button>
        )}
      </div>
      <div className={styles.list}>
        {sorted.length === 0 && <Caption1>{t('cores.noneFound')}</Caption1>}
        {sorted.map((c: CatalogCore) => (
          <div key={c.coreId} className={styles.row}>
            <span className={styles.lead}>
              <span className={styles.tile} aria-hidden>
                <DeveloperBoardRegular />
              </span>
              <span className={styles.meta}>
                <Body1>
                  <Text as="strong" weight="semibold">{c.name}</Text>
                  {c.hw === 'opengl' && (
                    <Badge appearance="outline" color="informative" style={{ marginLeft: 8 }}>
                      OpenGL
                    </Badge>
                  )}
                  {c.hw === 'vulkan' && (
                    <Badge appearance="outline" color="severe" style={{ marginLeft: 8 }}>
                      Vulkan
                    </Badge>
                  )}
                </Body1>
                <Caption1 className={styles.sub}>
                  <span className={styles.narrowOnly}>{c.systems} · </span>
                  {c.license}
                </Caption1>
              </span>
            </span>
            <Body1 className={styles.col}>{c.systems}</Body1>
            <span className={styles.col}>
              {c.installed &&
                (outdated.includes(c.coreId) ? (
                  <Badge className={styles.badge} appearance="tint" color="brand" icon={<ArrowCircleUpRegular />}>
                    {t('cores.updateBadge')}
                  </Badge>
                ) : (
                  <Badge className={styles.badge} appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                    {t('cores.installedBadge')}
                  </Badge>
                ))}
            </span>
            <span className={styles.actions}>
              {c.installed && outdated.includes(c.coreId) ? (
                <IconAction
                  className={styles.action}
                  primary
                  label={busy(c.coreId) ? t('cores.updating') : t('cores.update')}
                  icon={busy(c.coreId) ? <Spinner size="tiny" /> : <ArrowCircleUpRegular />}
                  disabled={busy(c.coreId)}
                  onClick={() => update.mutate(c.coreId)}
                />
              ) : (
                c.installed &&
                c.hasPrevious && (
                  <IconAction
                    className={styles.action}
                    label={t('cores.restorePrevious')}
                    icon={<HistoryRegular />}
                    disabled={busy(c.coreId)}
                    onClick={() => restore.mutate(c.coreId)}
                  />
                )
              )}
              {c.installed && (
                <IconAction
                  className={styles.action}
                  label={t('settings.title')}
                  icon={<SettingsRegular />}
                  onClick={() => navigate(`/settings/cores/${c.coreId}`)}
                />
              )}
              {c.installed ? (
                <IconAction
                  className={styles.action}
                  label={t('common.remove')}
                  icon={<DeleteRegular />}
                  disabled={busy(c.coreId)}
                  onClick={() => uninstall.mutate(c.coreId)}
                />
              ) : (
                <DownloadButton
                  className={styles.action}
                  appearance="primary"
                  label={busy(c.coreId) ? t('cores.downloading') : t('cores.install')}
                  busy={busy(c.coreId)}
                  onClick={() => install.mutate(c.coreId)}
                />
              )}
            </span>
          </div>
        ))}
      </div>
    </>
  )
}
