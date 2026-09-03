import {
  Badge,
  Body1,
  Button,
  Caption1,
  Spinner,
  Tab,
  TabList,
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
import { sysToast } from '../../lib/toast'
import {
  downloadCore,
  listCoreCatalog,
  listInstalledCores,
  removeCore,
  type CatalogCore,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  list: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
  row: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalM,
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
  },
  meta: { display: 'flex', flexDirection: 'column', gap: '2px' },
})

export function SettingsCores() {
  const styles = useStyles()
  const [tab, setTab] = useState<'installed' | 'catalog'>('installed')

  return (
    <div className={styles.root}>
      <TabList selectedValue={tab} onTabSelect={(_, d) => setTab(d.value as typeof tab)}>
        <Tab value="installed">Instalados</Tab>
        <Tab value="catalog">Catálogo</Tab>
      </TabList>
      {tab === 'installed' ? <Installed /> : <Catalog />}
    </div>
  )
}

function Installed() {
  const styles = useStyles()
  const cores = useQuery({ queryKey: ['installed-cores'], queryFn: listInstalledCores, retry: false })

  if (cores.isLoading) return <Spinner label="Lendo pasta de cores…" />
  if (cores.isError) return <Body1>Indisponível (backend sem banco).</Body1>
  if ((cores.data?.length ?? 0) === 0)
    return (
      <Caption1>
        Nenhum core em <code>&lt;dados&gt;/cores/</code>. Instale pelo catálogo.
      </Caption1>
    )

  return (
    <div className={styles.list}>
      {cores.data?.map((c) => (
        <div key={c.coreId} className={styles.row}>
          <span className={styles.meta}>
            <Body1>
              <strong>{c.name}</strong>
            </Body1>
            <Caption1>
              {c.version || 's/ versão'}
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
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const catalog = useQuery({ queryKey: ['core-catalog'], queryFn: listCoreCatalog, retry: false })

  const install = useMutation({
    mutationFn: (coreId: string) => downloadCore(coreId),
    onSuccess: (_d, coreId) => {
      qc.invalidateQueries({ queryKey: ['core-catalog'] })
      qc.invalidateQueries({ queryKey: ['installed-cores'] })
      push(sysToast(`Core instalado: ${coreId}`, 'Success'))
    },
    onError: (e) => push(sysToast(`Falha no download: ${e}`, 'Error')),
  })
  const uninstall = useMutation({
    mutationFn: (coreId: string) => removeCore(coreId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['core-catalog'] })
      qc.invalidateQueries({ queryKey: ['installed-cores'] })
    },
    onError: (e) => push(sysToast(`Falha ao remover: ${e}`, 'Error')),
  })

  if (catalog.isLoading) return <Spinner label="Carregando catálogo…" />
  if (catalog.isError) return <Body1>Catálogo indisponível.</Body1>

  const busy = (id: string) =>
    (install.isPending && install.variables === id) ||
    (uninstall.isPending && uninstall.variables === id)

  const sorted = [...(catalog.data ?? [])].sort((a, b) => a.name.localeCompare(b.name))

  return (
    <>
      <Caption1>
        Cores do buildbot oficial da libretro. Os marcados <strong>OpenGL</strong> renderizam em 3D
        (precisam de GPU) — N64, PSX-hw, PSP, Saturn, DS. Cores exclusivamente Vulkan ficam de fora
        até a etapa 12.
      </Caption1>
      <div className={styles.list}>
        {sorted.map((c: CatalogCore) => (
          <div key={c.coreId} className={styles.row}>
            <span className={styles.meta}>
              <Body1>
                <strong>{c.name}</strong>
                {c.hw === 'opengl' && (
                  <Badge appearance="outline" color="informative" style={{ marginLeft: 8 }}>
                    OpenGL
                  </Badge>
                )}
              </Body1>
              <Caption1>
                {c.systems} · {c.license}
              </Caption1>
            </span>
            {c.installed ? (
              <span style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                  instalado
                </Badge>
                <Button
                  size="small"
                  appearance="subtle"
                  icon={<DeleteRegular />}
                  disabled={busy(c.coreId)}
                  onClick={() => uninstall.mutate(c.coreId)}
                >
                  Remover
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
                {busy(c.coreId) ? 'Baixando…' : 'Instalar'}
              </Button>
            )}
          </div>
        ))}
      </div>
    </>
  )
}
