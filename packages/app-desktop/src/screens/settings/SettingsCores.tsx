import {
  Badge,
  Body1,
  Button,
  Caption1,
  Input,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkCircleFilled, DeveloperBoardRegular } from '@fluentui/react-icons'
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

// Largura do botão da direita (Baixar/Remover), igual em todas as linhas —
// como o "Abrir" da Biblioteca da Microsoft Store.
const ACTION_W = '120px'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  // Busca: o campo com largura de leitura e a contagem ao lado.
  filterRow: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
    '& .fui-Input': { width: 'min(420px, 100%)' },
  },
  // Lista em linhas, como a Biblioteca da Microsoft Store (print do usuário):
  // uma coluna, cada core numa faixa da largura toda.
  list: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  // ícone + nome | sistemas | situação | botão
  row: {
    display: 'grid',
    gridTemplateColumns: `minmax(0, 2fr) minmax(0, 1.4fr) minmax(0, 0.8fr) ${ACTION_W}`,
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
    '@media (max-width: 900px)': { gridTemplateColumns: `minmax(0, 1fr) ${ACTION_W}` },
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
    backgroundColor: tokens.colorNeutralBackground3,
    color: 'var(--reemuBrandAccent)',
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
  action: { width: ACTION_W, minWidth: '0 !important', maxWidth: 'none !important' },
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
      <div className={styles.filterRow}>
        <Input
          // sem o anel de foco (pedido do usuário; ver `reemu-no-ring` no index.css)
          className="reemu-no-ring"
          appearance="filled-darker"
          placeholder={t('cores.filter')}
          value={filter}
          onChange={(_, d) => setFilter(d.value)}
        />
        <Caption1>
          {t('cores.count', { shown: sorted.length, total: catalog.data?.length ?? 0 })}
        </Caption1>
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
              {c.installed && (
                <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                  {t('cores.installedBadge')}
                </Badge>
              )}
            </span>
            {c.installed ? (
              // Remover: secondary, só o texto (pedido do usuário)
              <Button
                className={styles.action}
                appearance="secondary"
                disabled={busy(c.coreId)}
                onClick={() => uninstall.mutate(c.coreId)}
              >
                {t('common.remove')}
              </Button>
            ) : (
              <DownloadButton
                className={styles.action}
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
