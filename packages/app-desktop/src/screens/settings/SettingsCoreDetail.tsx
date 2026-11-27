import { Body1, makeStyles, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useParams } from 'react-router-dom'
import { CoreOptions } from '../../components/CoreOptions'
import { LoadingState } from '../../components/EmptyState'
import { listCoreCatalog, listInstalledCores } from '../../lib/tauri'
import { CoreInfo } from './CoreInfo'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalXXL },
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

  return (
    <div className={s.root}>
      <CoreInfo entry={entry} core={core} />
      <CoreOptions coreId={coreId} />
    </div>
  )
}
