import {
  Badge,
  Body1,
  Button,
  Caption1,
  Input,
  Tab,
  TabList,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowDownloadRegular,
  CheckmarkCircleFilled,
  DeleteRegular,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { LoadingState } from '../../components/EmptyState'
import { errorToast, sysToast } from '../../lib/toast'
import {
  downloadCore,
  listCoreCatalog,
  listInstalledCores,
  removeCore,
  type CatalogCore,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { Trans, useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  // 2 colunas; 3 em tela larga (com o rail e a navegação de Configurações
  // ao lado, abaixo disso a 3ª coluna aperta nome + sistema + botão).
  list: {
    display: 'grid',
    gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
    gap: tokens.spacingVerticalM,
    '@media (min-width: 1600px)': { gridTemplateColumns: 'repeat(3, minmax(0, 1fr))' },
  },
  // mensagem de lista vazia ocupa a linha inteira do grid
  fullRow: { gridColumn: '1 / -1' },
  row: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalM,
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
  },
  // `minWidth: 0` deixa o texto quebrar dentro da coluna em vez de empurrar
  // o botão pra fora do card.
  meta: { display: 'flex', flexDirection: 'column', gap: '2px', minWidth: 0, overflowWrap: 'anywhere' },
})

export function SettingsCores() {
  const { t } = useTranslation()
  const styles = useStyles()
  const [tab, setTab] = useState<'installed' | 'catalog'>('installed')

  return (
    <div className={styles.root}>
      <TabList selectedValue={tab} onTabSelect={(_, d) => setTab(d.value as typeof tab)}>
        <Tab value="installed">{t('cores.installed')}</Tab>
        <Tab value="catalog">{t('cores.catalog')}</Tab>
      </TabList>
      {tab === 'installed' ? <Installed /> : <Catalog />}
    </div>
  )
}

function Installed() {
  const { t } = useTranslation()
  const styles = useStyles()
  const cores = useQuery({ queryKey: ['installed-cores'], queryFn: listInstalledCores, retry: false })

  if (cores.isLoading) return <LoadingState label={t('cores.reading')} />
  if (cores.isError) return <Body1>{t('cores.loadFailed')}</Body1>
  if ((cores.data?.length ?? 0) === 0)
    return <Caption1>{t('cores.noneYet')}</Caption1>

  return (
    <div className={styles.list}>
      {cores.data?.map((c) => (
        <div key={c.coreId} className={styles.row}>
          <span className={styles.meta}>
            <Body1>
              <Text as="strong" weight="semibold">{c.name}</Text>
            </Body1>
            <Caption1>
              {c.version || t('cores.noVersion')}
              {c.renderBackend ? ` · ${c.renderBackend}` : ''}
              {c.extensions.length > 0 ? ` · .${c.extensions.slice(0, 5).join(' .')}` : ''}
            </Caption1>
          </span>
        </div>
      ))}
    </div>
  )
}

function Catalog() {
  const { t } = useTranslation()
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const catalog = useQuery({ queryKey: ['core-catalog'], queryFn: listCoreCatalog, retry: false })
  const [filter, setFilter] = useState('')

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
    (uninstall.isPending && uninstall.variables === id)

  const f = filter.trim().toLowerCase()
  const sorted = [...(catalog.data ?? [])]
    .filter((c) => !f || `${c.name} ${c.systems}`.toLowerCase().includes(f))
    .sort((a, b) => a.name.localeCompare(b.name))

  return (
    <>
      <Caption1>
        <Trans i18nKey="cores.intro" components={{ b: <Text as="strong" weight="semibold" /> }} />
      </Caption1>
      <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <Input
          size="small"
          placeholder={t('cores.filter')}
          value={filter}
          onChange={(_, d) => setFilter(d.value)}
          style={{ flex: 1 }}
        />
        <Caption1>
          {t('cores.count', { shown: sorted.length, total: catalog.data?.length ?? 0 })}
        </Caption1>
      </div>
      <div className={styles.list}>
        {sorted.length === 0 && <Caption1 className={styles.fullRow}>{t('cores.noneFound')}</Caption1>}
        {sorted.map((c: CatalogCore) => (
          <div key={c.coreId} className={styles.row}>
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
              <Caption1>
                {c.systems} · {c.license}
              </Caption1>
            </span>
            {c.installed ? (
              <span style={{ display: 'flex', gap: 8, alignItems: 'center', flexShrink: 0, whiteSpace: 'nowrap' }}>
                <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                  {t('cores.installedBadge')}
                </Badge>
                <Button
                  size="small"
                  appearance="subtle"
                  icon={<DeleteRegular />}
                  disabled={busy(c.coreId)}
                  onClick={() => uninstall.mutate(c.coreId)}
                >
                  {t('common.remove')}
                </Button>
              </span>
            ) : (
              <Button
                size="small"
                appearance="primary"
                icon={<ArrowDownloadRegular />}
                disabled={busy(c.coreId)}
                onClick={() => install.mutate(c.coreId)}
              >
                {busy(c.coreId) ? t('cores.downloading') : t('cores.install')}
              </Button>
            )}
          </div>
        ))}
      </div>
    </>
  )
}
