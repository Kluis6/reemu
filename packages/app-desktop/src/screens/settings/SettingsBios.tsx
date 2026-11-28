import {
  Badge,
  Body1,
  Button,
  Caption1,
  MessageBar,
  MessageBarBody,
  SearchBox,
  Spinner,
  Switch,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowSyncRegular,
  ArrowUploadRegular,
  CheckmarkCircleFilled,
  CloudArrowDownRegular,
  DeleteRegular,
  PuzzlePieceRegular,
  WarningFilled,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import {
  downloadPpssppAssets,
  importBiosFile,
  listBiosStatus,
  listRoms,
  pickBiosFile,
  pickFolder,
  ppssppAssetsInstalled,
  removeBiosFile,
  scanBiosFolder,
  type BiosStatus,
} from '../../lib/tauri'
import { platformLabel } from '../../lib/platform'
import { LoadingState } from '../../components/EmptyState'
import { errorToast, sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'
import { biosNote } from '../../lib/backendText'
import { LIST_ICON_ACTION_WIDTH, ListIconAction } from '../../components/ListIconAction'
import { useSettingsListStyles } from '../../styles/settingsList'
import { Trans, useTranslation } from 'react-i18next'

// Colunas fixas (iguais em todas as linhas): situação e botão, como na
// lista de Cores.
const STATUS_W = '210px'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  // busca + "só da minha biblioteca" + contagem, numa linha que quebra
  filters: {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    gap: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalL}`,
  },
  search: { width: '320px', maxWidth: '100%' },
  count: { color: tokens.colorNeutralForeground3 },
  scanAction: { marginInlineStart: 'auto' },
  scanReport: { display: 'flex', flexDirection: 'column', overflowWrap: 'anywhere' },
  // Um bloco por sistema numa coluna só; 2 COLUNAS CORRIDAS só em tela
  // grande (≥ 1600 px úteis: interface Compacta ou monitor ultrawide — num
  // 16:9 o zoom da interface mantém ~1366). Colunas corridas e não grade:
  // cada coluna empilha os blocos direto, sem o buraco que a grade deixava
  // embaixo do bloco menor.
  groups: {
    columnCount: 1,
    columnGap: tokens.spacingHorizontalXL,
    '@media (min-width: 1600px)': { columnCount: 2 },
  },
  // `inline-flex` + largura cheia: bloco inline nunca é partido entre duas
  // colunas (mais garantido que só `breakInside` no WebKitGTK). O espaço
  // entre blocos vem da margem, já que `gap` não vale entre itens de coluna.
  system: {
    display: 'inline-flex',
    flexDirection: 'column',
    width: '100%',
    gap: tokens.spacingVerticalS,
    breakInside: 'avoid',
    marginBottom: tokens.spacingVerticalXL,
  },
  list: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  // Linhas como as da lista de Cores (Biblioteca da Microsoft Store): ícone
  // num quadrado + arquivo e nota | situação | botão de largura fixa, com o
  // mesmo padding nos 4 lados.
  row: {
    display: 'grid',
    gridTemplateColumns: `minmax(0, 1fr) ${STATUS_W} ${LIST_ICON_ACTION_WIDTH}`,
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalL,
    padding: tokens.spacingHorizontalM,
    boxSizing: 'border-box',
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
    // coluna estreita: a situação encolhe pro tamanho do selo
    '@media (max-width: 760px)': { gridTemplateColumns: `minmax(0, 1fr) auto ${LIST_ICON_ACTION_WIDTH}` },
  },
  lead: { display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalM, minWidth: 0 },
  meta: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
    minWidth: 0,
    overflowWrap: 'anywhere',
  },
  note: { color: tokens.colorNeutralForeground3 },
  // Status e avisos ficam na mesma coluna.
  status: { minWidth: 0, whiteSpace: 'nowrap' },
})

type Key = { systemId: string; filename: string }
/** O que aparece no lugar do nome: a pasta, numa entrada "qualquer um destes". */
const shownName = (f: BiosStatus) => (f.anyOfFolder ? `${f.anyOfFolder}/` : f.filename)
const sameKey = (a?: Key, b?: Key) => a?.systemId === b?.systemId && a?.filename === b?.filename

export function SettingsBios() {
  const { t } = useTranslation()
  const styles = useStyles()
  const itemStyles = useSettingsListStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const bios = useQuery({ queryKey: ['bios-status'], queryFn: () => listBiosStatus(), retry: false })

  const refresh = () => qc.invalidateQueries({ queryKey: ['bios-status'] })
  const scan = useMutation({
    mutationFn: async () => {
      const path = await pickFolder(t('bios.scanPickFolder'))
      return path ? scanBiosFolder(path) : null
    },
    onSuccess: (result) => {
      if (result?.imported) refresh()
    },
    onError: (e) => push(errorToast(e, 'importBios')),
  })
  const ppsspp = useQuery({ queryKey: ['ppsspp-assets'], queryFn: ppssppAssetsInstalled, retry: false })
  const getPpsspp = useMutation({
    mutationFn: downloadPpssppAssets,
    onSuccess: (n) => {
      qc.invalidateQueries({ queryKey: ['ppsspp-assets'] })
      push(sysToast(t('bios.ppssppInstalled', { count: n }), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'downloadFiles')),
  })

  // O picker abre dentro do `mutationFn` — `isPending`/`variables` cobrem o
  // diálogo nativo + a cópia, então o botão fica "ocupado" o tempo todo.
  const doImport = useMutation({
    mutationFn: async ({ systemId, filename }: Key & { label: string }) => {
      const path = await pickBiosFile()
      if (!path) return false // cancelado no diálogo
      await importBiosFile(systemId, filename, path)
      return true
    },
    onSuccess: (imported, { label }) => {
      if (!imported) return
      refresh()
      push(sysToast(t('bios.imported', { file: label }), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'importBios')),
  })
  const doRemove = useMutation({
    mutationFn: (v: Key) => removeBiosFile(v.systemId, v.filename),
    onSuccess: refresh,
    onError: (e) => push(errorToast(e, 'removeBios')),
  })

  // Filtros: a lista cobre ~35 sistemas. Por padrão mostra só os que têm
  // jogo na biblioteca (se ela tiver algum); a busca casa com o nome do
  // sistema, o arquivo ou a nota.
  const roms = useQuery({ queryKey: ['roms'], queryFn: listRoms, retry: false })
  const mine = new Set((roms.data ?? []).map((r) => r.systemId))
  const [onlyMineChoice, setOnlyMine] = useState<boolean | null>(null)
  const onlyMine = onlyMineChoice ?? mine.size > 0
  const [query, setQuery] = useState('')
  const q = query.trim().toLowerCase()
  const shows = (systemId: string, texts: string[]) =>
    (!onlyMine || mine.has(systemId)) &&
    (!q || [platformLabel(systemId), ...texts].some((x) => x.toLowerCase().includes(q)))

  if (bios.isLoading) return <LoadingState label={t('bios.checking')} />
  if (bios.isError) return <Body1>{t('bios.unavailable')}</Body1>

  const bySystem = new Map<string, BiosStatus[]>()
  for (const b of bios.data ?? []) {
    const list = bySystem.get(b.systemId) ?? []
    list.push(b)
    bySystem.set(b.systemId, list)
  }

  const allSystems = [...bySystem.keys(), 'psp']
  const visible = [...bySystem.entries()].filter(([systemId, files]) =>
    shows(
      systemId,
      files.flatMap((f) => [shownName(f), biosNote(t, f.anyOfFolder ?? f.filename, f.note)]),
    ),
  )
  const showPsp = shows('psp', ['PPSSPP', t('bios.ppssppNote')])
  const shownCount = visible.length + (showPsp ? 1 : 0)

  return (
    <div className={styles.root}>
      <Caption1>
        <Trans i18nKey="bios.intro" components={{ b: <Text as="strong" weight="semibold" /> }} />
      </Caption1>
      <div className={styles.filters}>
        <SearchBox
          // sem anel de foco (pedido do usuário) e com fundo/borda visíveis
          // em qualquer tema (`reemu-list-search`, index.css)
          className={mergeClasses('reemu-no-ring reemu-list-search', styles.search)}
          appearance="filled-darker"
          placeholder={t('bios.search')}
          aria-label={t('bios.search')}
          value={query}
          onChange={(_, d) => setQuery(d.value)}
        />
        <Switch
          label={t('bios.onlyMine')}
          checked={onlyMine}
          disabled={mine.size === 0}
          onChange={(_, d) => setOnlyMine(d.checked)}
        />
        <Caption1 className={styles.count}>
          {t('bios.showing', { shown: shownCount, total: allSystems.length })}
        </Caption1>
        <Button
          className={styles.scanAction}
          appearance="secondary"
          disabled={scan.isPending || doImport.isPending || doRemove.isPending}
          onClick={() => scan.mutate()}
        >
          {t('bios.scanFolder')}
        </Button>
      </div>
      {scan.isPending && <Spinner size="tiny" label={t('bios.scanning')} />}
      {scan.data && (
        <MessageBar intent={scan.data.review.length || scan.data.unreadable ? 'warning' : 'info'}>
          <MessageBarBody>
            <div className={styles.scanReport}>
              <Text>
                {scan.data.imported > 0
                  ? t('bios.scanImported', { count: scan.data.imported })
                  : t('bios.scanNoImports')}
              </Text>
              {scan.data.skipped > 0 && <Caption1>{t('bios.scanSkipped', { count: scan.data.skipped })}</Caption1>}
              {scan.data.unreadable > 0 && <Caption1>{t('bios.scanUnreadable', { count: scan.data.unreadable })}</Caption1>}
              {scan.data.review.length > 0 && (
                <Caption1>{t('bios.scanReview', { files: scan.data.review.join(', ') })}</Caption1>
              )}
            </div>
          </MessageBarBody>
        </MessageBar>
      )}
      {shownCount === 0 && <Caption1>{t('bios.noneFound')}</Caption1>}
      <div className={styles.groups}>
        {visible.map(([systemId, files]) => (
          <div key={systemId} className={styles.system}>
            <Body1>
              <Text as="strong" weight="semibold">{platformLabel(systemId).toUpperCase()}</Text>
            </Body1>
            <div className={styles.list}>
              {files.map((f) => {
                const key: Key = { systemId, filename: f.filename }
                const ready = f.present && f.hashOk !== false
                const busy =
                  (doImport.isPending && sameKey(doImport.variables, key)) ||
                  (doRemove.isPending && sameKey(doRemove.variables, key))
                return (
                  <div key={f.filename} className={styles.row}>
                    <span className={styles.lead}>
                      <span className={itemStyles.tile} aria-hidden>
                        <PuzzlePieceRegular />
                      </span>
                      <span className={styles.meta}>
                        <Body1 className={itemStyles.titleLine}>
                          <span className={itemStyles.titleName}>
                            <code>{shownName(f)}</code>
                            {ready && <CheckmarkCircleFilled className={itemStyles.installedCheck} aria-hidden />}
                          </span>
                          {f.required && (
                            <Badge appearance="tint" color="danger">
                              {t('bios.required')}
                            </Badge>
                          )}
                        </Body1>
                        <Caption1 className={styles.note}>
                          {biosNote(t, f.anyOfFolder ?? f.filename, f.note)}
                        </Caption1>
                      </span>
                    </span>
                    <span className={styles.status}>
                      {!f.present && <Body1>{t('bios.missing')}</Body1>}
                      {f.present && f.hashOk === false && (
                        <Badge appearance="tint" color="warning" icon={<WarningFilled />}>
                          {t('bios.hashMismatch')}
                        </Badge>
                      )}
                      {ready && <Body1>{t('bios.present')}</Body1>}
                    </span>
                    {f.present ? (
                      <ListIconAction
                        label={t('common.remove')}
                        icon={busy ? <Spinner size="tiny" /> : <DeleteRegular />}
                        disabled={busy || scan.isPending}
                        onClick={() => doRemove.mutate(key)}
                      />
                    ) : (
                      <ListIconAction
                        primary
                        label={busy ? t('bios.importing') : t('bios.import')}
                        icon={busy ? <Spinner size="tiny" /> : <ArrowUploadRegular />}
                        disabled={busy || scan.isPending}
                        onClick={() => doImport.mutate({ ...key, label: shownName(f) })}
                      />
                    )}
                  </div>
                )
              })}
            </div>
          </div>
        ))}
        {showPsp && (
        <div className={styles.system}>
          <Body1>
            <Text as="strong" weight="semibold">{platformLabel('psp').toUpperCase()}</Text>
          </Body1>
          <div className={styles.row}>
            <span className={styles.lead}>
              <span className={itemStyles.tile} aria-hidden>
                <PuzzlePieceRegular />
              </span>
              <span className={styles.meta}>
                <Body1 className={itemStyles.titleName}>
                  <code>PPSSPP/</code>
                  {ppsspp.data && <CheckmarkCircleFilled className={itemStyles.installedCheck} aria-hidden />}
                </Body1>
                <Caption1 className={styles.note}>{t('bios.ppssppNote')}</Caption1>
              </span>
            </span>
            <span className={styles.status}>
              {ppsspp.data && <Body1>{t('bios.installed')}</Body1>}
            </span>
            {/* baixar em primary; já instalado, atualizar em secondary */}
            <ListIconAction
              primary={!ppsspp.data}
              label={getPpsspp.isPending ? t('bios.downloading') : ppsspp.data ? t('bios.update') : t('bios.download')}
              icon={getPpsspp.isPending ? <Spinner size="tiny" /> : ppsspp.data ? <ArrowSyncRegular /> : <CloudArrowDownRegular />}
              disabled={getPpsspp.isPending}
              onClick={() => getPpsspp.mutate()}
            />
          </div>
        </div>
        )}
      </div>
    </div>
  )
}
