import { Body1, makeStyles, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router-dom'
import { CoreOptions } from '../../components/CoreOptions'
import { LoadingState } from '../../components/EmptyState'
import { listCoreCatalog, listInstalledCores } from '../../lib/tauri'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalXXL },
  // rótulo à esquerda, valor à direita, como em Sistema › Hardware
  info: {
    display: 'grid',
    gridTemplateColumns: 'minmax(140px, max-content) minmax(0, 1fr)',
    columnGap: tokens.spacingHorizontalXXL,
    rowGap: tokens.spacingVerticalS,
    margin: 0,
  },
  label: { color: tokens.colorNeutralForeground3 },
  value: { margin: 0, overflowWrap: 'anywhere' },
})

/**
 * Configurações › Cores › um core: as informações dele e as opções que valem para
 * todos os jogos que o usam. O ajuste feito na página de um jogo vence estas.
 */
export function SettingsCoreDetail() {
  const { t } = useTranslation()
  const s = useStyles()
  const { coreId = '' } = useParams()
  const catalog = useQuery({ queryKey: ['core-catalog'], queryFn: listCoreCatalog, retry: false })
  const installed = useQuery({
    queryKey: ['installed-cores'],
    queryFn: listInstalledCores,
    retry: false,
  })

  if (catalog.isLoading || installed.isLoading) return <LoadingState />
  const entry = catalog.data?.find((c) => c.coreId === coreId)
  const core = installed.data?.find((c) => c.coreId === coreId)
  if (!entry && !core) return <Body1>{t('cores.detail.notFound')}</Body1>

  const rows: [string, string | null | undefined][] = [
    [t('cores.detail.version'), core?.version || t('cores.noVersion')],
    [t('cores.detail.systems'), entry?.systems],
    [t('cores.detail.license'), entry?.license],
    [t('cores.detail.render'), core?.renderBackend],
    [t('cores.detail.extensions'), core?.extensions.join(', ')],
  ]

  return (
    <div className={s.root}>
      <dl className={s.info}>
        {rows
          .filter(([, value]) => value)
          .map(([label, value]) => (
            <div key={label} style={{ display: 'contents' }}>
              <dt className={s.label}>{label}</dt>
              <dd className={s.value}>{value}</dd>
            </div>
          ))}
      </dl>
      <CoreOptions coreId={coreId} />
    </div>
  )
}
