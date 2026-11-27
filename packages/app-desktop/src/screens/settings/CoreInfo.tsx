import {
  Badge, Caption1, Dialog, DialogBody, DialogContent, DialogSurface, DialogTitle,
  makeStyles, tokens,
} from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { CloseButton } from '../../components/CloseButton'
import { LoadingState } from '../../components/EmptyState'
import { DIALOG_FADE_ONLY } from '../../lib/motion'
import { listInstalledCores, type CatalogCore, type InstalledCore } from '../../lib/tauri'

const useStyles = makeStyles({
  info: {
    display: 'grid',
    gridTemplateColumns: 'minmax(0, min(42%, 180px)) minmax(0, 1fr)',
    columnGap: tokens.spacingHorizontalL,
    rowGap: tokens.spacingVerticalM,
    margin: 0,
  },
  label: { color: tokens.colorNeutralForeground3, overflowWrap: 'anywhere' },
  value: { margin: 0, overflowWrap: 'anywhere' },
  status: { display: 'flex', flexWrap: 'wrap', gap: tokens.spacingHorizontalS },
  content: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalL },
})

export function CoreInfo({ entry, core, updateAvailable = false }: {
  entry?: CatalogCore
  core?: InstalledCore
  updateAvailable?: boolean
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const installed = entry?.installed ?? Boolean(core)
  const expectedRender = entry?.hw === 'opengl' ? 'OpenGL'
    : entry?.hw === 'vulkan' ? 'Vulkan'
      : entry?.hw === 'software' ? t('cores.detail.software') : undefined

  const rows: [string, string | null | undefined][] = [
    [t('cores.detail.version'), core && (core.version || t('cores.noVersion'))],
    [t('cores.detail.systems'), entry?.systems || core?.systems.join(', ')],
    [t('cores.detail.license'), entry?.license],
    [t('cores.detail.expectedRender'), expectedRender],
    [t('cores.detail.render'), core?.renderBackend],
    [t('cores.detail.extensions'), core?.extensions.join(', ')],
  ]

  return (
    <dl className={s.info}>
      <dt className={s.label}>{t('cores.detail.status')}</dt>
      <dd className={s.value}>
        <div className={s.status}>
          <Badge appearance="tint" color={installed ? 'success' : 'informative'}>
            {t(installed ? 'cores.installedBadge' : 'cores.detail.notInstalled')}
          </Badge>
          {installed && updateAvailable && (
            <Badge appearance="tint" color="brand">{t('cores.updateBadge')}</Badge>
          )}
        </div>
      </dd>
      {rows.filter(([, value]) => value).map(([label, value]) => (
        <div key={label} style={{ display: 'contents' }}>
          <dt className={s.label}>{label}</dt>
          <dd className={s.value}>{value}</dd>
        </div>
      ))}
    </dl>
  )
}

export function CoreInfoDialog({ entry, updateAvailable, onClose }: {
  entry: CatalogCore
  updateAvailable: boolean
  onClose: () => void
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const installed = useQuery({
    queryKey: ['installed-cores'],
    queryFn: listInstalledCores,
    enabled: entry.installed,
    retry: false,
  })
  const core = entry.installed ? installed.data?.find((c) => c.coreId === entry.coreId) : undefined

  return (
    <Dialog open onOpenChange={(_, data) => { if (!data.open) onClose() }} surfaceMotion={DIALOG_FADE_ONLY}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle action={<CloseButton onClick={onClose} />}>{entry.name}</DialogTitle>
          <DialogContent className={s.content}>
            <CoreInfo entry={entry} core={core} updateAvailable={updateAvailable} />
            {entry.installed && installed.isLoading && <LoadingState label={t('cores.reading')} />}
            {entry.installed && installed.isError && <Caption1 role="status">{t('cores.loadFailed')}</Caption1>}
          </DialogContent>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  )
}
