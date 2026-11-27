import {
  Badge,
  Body1,
  Button,
  Caption1,
  Input,
  Spinner,
  Tab,
  TabList,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowCircleUpRegular,
  CheckmarkCircleFilled,
  CloudArrowDownRegular,
  DeleteRegular,
  DeveloperBoardRegular,
  HistoryRegular,
  InfoRegular,
  SettingsRegular,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { LIST_ICON_ACTION_WIDTH, ListIconAction } from '../../components/ListIconAction'
import { LoadingState } from '../../components/EmptyState'
import { CORE_UPDATES_KEY, CORE_UPDATES_STALE_MS } from '../../hooks/useCoreUpdates'
import { errorToast, sysToast } from '../../lib/toast'
import {
  checkCoreUpdates,
  downloadCore,
  listCoreCatalog,
  removeCore,
  restorePreviousCore,
  type CatalogCore,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { PAGE_PAD_B, SAFE_Y, SCROLLBAR_W } from '../../styles/metrics'
import { Trans, useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { CoreInfoDialog } from './CoreInfo'

// Até quatro ações por linha, incluindo Atualizar ou Versão anterior.
// Cabe "atualização disponível" numa linha, nos três idiomas.
const STATUS_W = '190px'
const ACTIONS_W = `calc(4 * ${LIST_ICON_ACTION_WIDTH} + 3 * ${tokens.spacingHorizontalS})`
const STATUS_FILTERS = [
  ['all', 'cores.filterAll'],
  ['installed', 'cores.filterInstalled'],
  ['updates', 'cores.filterUpdates'],
] as const
type StatusFilter = (typeof STATUS_FILTERS)[number][0]

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0, gap: tokens.spacingVerticalM },
  header: {
    display: 'flex', flexDirection: 'column', flexShrink: 0, gap: tokens.spacingVerticalM,
    paddingInlineEnd: `${SCROLLBAR_W}px`,
  },
  filterRow: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalM,
  },
  search: { flex: '1 1 230px', minWidth: 0, maxWidth: '420px' },
  filters: {
    flexWrap: 'wrap',
    rowGap: tokens.spacingVerticalS,
    '& .fui-Tab': {
      boxSizing: 'border-box',
      minWidth: '96px',
      paddingBlock: '5px',
      paddingInline: tokens.spacingHorizontalM,
      borderRadius: tokens.borderRadiusMedium,
      fontSize: tokens.fontSizeBase300,
    },
    '& .fui-Tab__content': { fontWeight: tokens.fontWeightRegular },
    '& .fui-Tab[aria-selected="false"]': {
      backgroundColor: 'var(--reemuControlFill)',
      color: tokens.colorNeutralForeground1,
      ':hover': { backgroundColor: 'var(--reemuControlFillHover)' },
      ':active': { backgroundColor: 'var(--reemuControlFillPressed)' },
    },
  },
  updateAll: { marginLeft: 'auto', flexShrink: 0 },
  list: {
    flexGrow: 1,
    minHeight: 0,
    overflowY: 'auto',
    overscrollBehavior: 'contain',
    scrollbarGutter: 'stable',
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    paddingBottom: `${PAGE_PAD_B - SAFE_Y + 24}px`,
  },
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
    flexShrink: 0,
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
  badge: { whiteSpace: 'nowrap' },
  // Selo da API de vídeo do core, preenchido (pedido do usuário, 2026-11-06):
  // Vulkan em vermelho (`danger` do Fluent), OpenGL em azul. O azul é fixo da
  // paleta do Fluent, não a cor de destaque, que muda com o tema; com o texto
  // branco passa de 4.5:1.
  hwBadge: { marginLeft: '8px' },
  hwOpenGl: {
    marginLeft: '8px',
    backgroundColor: tokens.colorPaletteBlueBorderActive,
    color: tokens.colorNeutralForegroundOnBrand,
  },
  actions: {
    display: 'flex',
    justifyContent: 'flex-end',
    columnGap: tokens.spacingHorizontalS,
  },
})

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
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all')
  const [infoCoreId, setInfoCoreId] = useState<string | null>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const infoTrigger = useRef<HTMLButtonElement>(null)
  const resetListScroll = () => {
    if (listRef.current) listRef.current.scrollTop = 0
  }
  const closeInfo = () => {
    const trigger = infoTrigger.current
    setInfoCoreId(null)
    requestAnimationFrame(() => {
      const target = trigger?.isConnected ? trigger : listRef.current
      target?.focus({ preventScroll: true })
    })
  }

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
    onSuccess: (_result, coreId) => updated(coreId),
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
  const outdatedIds = new Set(outdated)
  const infoCore = catalog.data?.find((c) => c.coreId === infoCoreId)
  const sorted = [...(catalog.data ?? [])]
    .filter((c) => statusFilter === 'all'
      || (c.installed && (statusFilter === 'installed' || outdatedIds.has(c.coreId))))
    .filter((c) => !f || `${c.name} ${c.systems}`.toLowerCase().includes(f))
    .sort((a, b) => a.name.localeCompare(b.name))

  return (
    <>
      <div className={styles.header}>
        <Caption1>
          <Trans i18nKey="cores.intro" components={{ b: <Text as="strong" weight="semibold" /> }} />
        </Caption1>
        <div className={styles.filterRow}>
          <Input
            // sem o anel de foco (pedido do usuário; ver `reemu-no-ring` no
            // index.css) e com fundo/borda visíveis em qualquer tema
            className={mergeClasses(styles.search, 'reemu-no-ring reemu-list-search')}
            appearance="filled-darker"
            aria-label={t('cores.searchLabel')}
            placeholder={t('cores.filter')}
            value={filter}
            onChange={(_, d) => {
              resetListScroll()
              setFilter(d.value)
            }}
          />
          <TabList
            className={styles.filters}
            appearance="filled-circular"
            aria-label={t('cores.statusFilter')}
            selectedValue={statusFilter}
            onTabSelect={(_, { value }) => {
              resetListScroll()
              setStatusFilter(value as StatusFilter)
            }}
          >
            {STATUS_FILTERS.map(([value, label]) => (
              <Tab key={value} id={`core-filter-${value}`} value={value} aria-controls="core-catalog-results">
                {t(label)}
              </Tab>
            ))}
          </TabList>
          <Caption1 aria-live="polite">
            {t('cores.count', { shown: sorted.length, total: catalog.data?.length ?? 0 })}
          </Caption1>
          {outdated.length > 0 && (
            <Button
              className={styles.updateAll}
              appearance="primary"
              disabled={updateAll.isPending || update.isPending}
              onClick={() => updateAll.mutate(outdated)}
            >
              {updateAll.isPending ? t('cores.updating') : t('cores.updateAll', { count: outdated.length })}
            </Button>
          )}
        </div>
      </div>
      <div
        id="core-catalog-results"
        ref={listRef}
        role="tabpanel"
        aria-labelledby={`core-filter-${statusFilter}`}
        tabIndex={0}
        className={styles.list}
      >
        {sorted.length === 0 && (
          statusFilter === 'updates' && updates.isPending ? <LoadingState label={t('cores.checkingUpdates')} />
            : <Caption1>{t(statusFilter === 'updates' && updates.isError ? 'cores.updatesUnavailable' : 'cores.noneFound')}</Caption1>
        )}
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
                    <Badge appearance="filled" className={styles.hwOpenGl}>
                      OpenGL
                    </Badge>
                  )}
                  {c.hw === 'vulkan' && (
                    <Badge appearance="filled" color="danger" className={styles.hwBadge}>
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
                (outdatedIds.has(c.coreId) ? (
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
              {c.installed && outdatedIds.has(c.coreId) ? (
                <ListIconAction
                  primary
                  label={busy(c.coreId) ? t('cores.updating') : t('cores.update')}
                  icon={busy(c.coreId) ? <Spinner size="tiny" /> : <ArrowCircleUpRegular />}
                  disabled={busy(c.coreId)}
                  onClick={() => update.mutate(c.coreId)}
                />
              ) : (
                c.installed &&
                c.hasPrevious && (
                  <ListIconAction
                    label={t('cores.restorePrevious')}
                    icon={<HistoryRegular />}
                    disabled={busy(c.coreId)}
                    onClick={() => restore.mutate(c.coreId)}
                  />
                )
              )}
              <ListIconAction
                label={t('cores.detail.information', { name: c.name })}
                icon={<InfoRegular />}
                aria-haspopup="dialog"
                onClick={(event) => {
                  infoTrigger.current = event.currentTarget
                  setInfoCoreId(c.coreId)
                }}
              />
              {c.installed && (
                <ListIconAction
                  label={t('settings.title')}
                  icon={<SettingsRegular />}
                  onClick={() => navigate(`/settings/cores/${c.coreId}`)}
                />
              )}
              {c.installed ? (
                <ListIconAction
                  label={t('common.remove')}
                  icon={<DeleteRegular />}
                  disabled={busy(c.coreId)}
                  onClick={() => uninstall.mutate(c.coreId)}
                />
              ) : (
                <ListIconAction
                  primary
                  label={busy(c.coreId) ? t('cores.downloading') : t('cores.install')}
                  icon={busy(c.coreId) ? <Spinner size="tiny" /> : <CloudArrowDownRegular />}
                  disabled={busy(c.coreId)}
                  onClick={() => install.mutate(c.coreId)}
                />
              )}
            </span>
          </div>
        ))}
      </div>
      {infoCore && (
        <CoreInfoDialog
          entry={infoCore}
          updateAvailable={outdatedIds.has(infoCore.coreId)}
          onClose={closeInfo}
        />
      )}
    </>
  )
}
