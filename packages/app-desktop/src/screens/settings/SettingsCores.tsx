import {
  Badge,
  Body1,
  Button,
  Caption1,
  Input,
  Text,
  makeStyles,
  tokens,
  mergeClasses,
} from '@fluentui/react-components'
import {
  CheckmarkCircleFilled,
  DeleteRegular,
  DeveloperBoardRegular,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { LoadingState } from '../../components/EmptyState'
import { errorToast, sysToast } from '../../lib/toast'
import {
  downloadCore,
  listCoreCatalog,
  removeCore,
  type CatalogCore,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { DownloadButton } from '../../components/DownloadButton'
import { Trans, useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  // abas no estilo do app; 24 até o conteúdo (12 do `gap` + 12)
  // 3 colunas (uma só em janela bem estreita).
  list: {
    display: 'grid',
    gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
    gap: tokens.spacingVerticalM,
    '@media (max-width: 640px)': { gridTemplateColumns: 'minmax(0, 1fr)' },
  },
  // Catálogo: quantas colunas couberem na largura disponível, cada uma com
  // pelo menos 420 — ícone + nome/sistemas (~160) + "instalado" e Remover
  // (~190) sem espremer o texto. Acompanha a janela e o tamanho da
  // interface: 2 colunas no padrão, 1 com a interface grande.
  catalogList: {
    gridTemplateColumns: 'repeat(auto-fill, minmax(420px, 1fr))',
    '@media (max-width: 640px)': { gridTemplateColumns: 'minmax(0, 1fr)' },
  },
  // Linha da busca no mesmo grid do catálogo: o campo tem a largura de uma
  // coluna e a contagem fica na seguinte (embaixo, com uma coluna só).
  filterRow: { alignItems: 'center' },
  // mensagem de lista vazia ocupa a linha inteira do grid
  fullRow: { gridColumn: '1 / -1' },
  // ícone | nome e sistema (cresce) | botão
  row: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
  },
  // `minWidth: 0` deixa o texto quebrar dentro da coluna em vez de empurrar
  // o botão pra fora do card.
  meta: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
    minWidth: 0,
    flexGrow: 1,
    overflowWrap: 'anywhere',
  },
  // Mesmo ícone de Cores no menu das Configurações.
  icon: { fontSize: '24px', flexShrink: 0, color: tokens.colorNeutralForeground2 },
})

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
      <div className={mergeClasses(styles.list, styles.catalogList, styles.filterRow)}>
        <Input
          appearance="filled-darker"
          placeholder={t('cores.filter')}
          value={filter}
          onChange={(_, d) => setFilter(d.value)}
        />
        <Caption1>
          {t('cores.count', { shown: sorted.length, total: catalog.data?.length ?? 0 })}
        </Caption1>
      </div>
      <div className={mergeClasses(styles.list, styles.catalogList)}>
        {sorted.length === 0 && <Caption1 className={styles.fullRow}>{t('cores.noneFound')}</Caption1>}
        {sorted.map((c: CatalogCore) => (
          <div key={c.coreId} className={styles.row}>
            <DeveloperBoardRegular className={styles.icon} />
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
                  appearance="subtle"
                  icon={<DeleteRegular />}
                  disabled={busy(c.coreId)}
                  onClick={() => uninstall.mutate(c.coreId)}
                >
                  {t('common.remove')}
                </Button>
              </span>
            ) : (
              <DownloadButton
                appearance="primary"
                label={busy(c.coreId) ? t('cores.downloading') : t('cores.install')}
                busy={busy(c.coreId)}
                onClick={() => install.mutate(c.coreId)}
              />
            )}
          </div>
        ))}
      </div>
    </>
  )
}
