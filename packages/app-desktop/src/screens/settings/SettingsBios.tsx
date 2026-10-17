import {
  Badge,
  Body1,
  Button,
  Caption1,
  SearchBox,
  Switch,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  CheckmarkCircleFilled,
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
  ppssppAssetsInstalled,
  removeBiosFile,
  type BiosStatus,
} from '../../lib/tauri'
import { platformLabel } from '../../lib/platform'
import { LoadingState } from '../../components/EmptyState'
import { errorToast, sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'
import { biosNote } from '../../lib/backendText'
import { DownloadButton } from '../../components/DownloadButton'
import { Trans, useTranslation } from 'react-i18next'

// Colunas fixas (iguais em todas as linhas): situação e botão, como na
// lista de Cores.
const STATUS_W = '210px'
const ACTION_W = '120px'

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
    gridTemplateColumns: `minmax(0, 1fr) ${STATUS_W} ${ACTION_W}`,
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalL,
    padding: tokens.spacingHorizontalM,
    boxSizing: 'border-box',
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
    // coluna estreita: a situação encolhe pro tamanho do selo
    '@media (max-width: 760px)': { gridTemplateColumns: `minmax(0, 1fr) auto ${ACTION_W}` },
  },
  lead: { display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalM, minWidth: 0 },
  // ícone de BIOS (peça de quebra-cabeça, o do card no hub) num quadrado
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
  meta: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
    minWidth: 0,
    overflowWrap: 'anywhere',
  },
  note: { color: tokens.colorNeutralForeground3 },
  // selo da situação: não quebra nem vaza da pílula
  status: { minWidth: 0, whiteSpace: 'nowrap' },
  action: { width: ACTION_W, minWidth: '0 !important', maxWidth: 'none !important' },
})

type Key = { systemId: string; filename: string }
/** O que aparece no lugar do nome: a pasta, numa entrada "qualquer um destes". */
const shownName = (f: BiosStatus) => (f.anyOfFolder ? `${f.anyOfFolder}/` : f.filename)
const sameKey = (a?: Key, b?: Key) => a?.systemId === b?.systemId && a?.filename === b?.filename

export function SettingsBios() {
  const { t } = useTranslation()
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const bios = useQuery({ queryKey: ['bios-status'], queryFn: listBiosStatus, retry: false })

  const refresh = () => qc.invalidateQueries({ queryKey: ['bios-status'] })
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
          className={styles.search}
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
      </div>
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
                const busy =
                  (doImport.isPending && sameKey(doImport.variables, key)) ||
                  (doRemove.isPending && sameKey(doRemove.variables, key))
                return (
                  <div key={f.filename} className={styles.row}>
                    <span className={styles.lead}>
                      <span className={styles.tile} aria-hidden>
                        <PuzzlePieceRegular />
                      </span>
                      <span className={styles.meta}>
                        <Body1>
                          <code>{shownName(f)}</code>
                          {f.required && (
                            <Badge appearance="tint" color="danger" style={{ marginLeft: 8 }}>
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
                      {!f.present && (
                        <Badge appearance="tint" color={f.required ? 'danger' : 'informative'}>
                          {t('bios.missing')}
                        </Badge>
                      )}
                      {f.present && f.hashOk === false && (
                        <Badge appearance="tint" color="warning" icon={<WarningFilled />}>
                          {t('bios.hashMismatch')}
                        </Badge>
                      )}
                      {f.present && f.hashOk !== false && (
                        <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                          {t('bios.present')}
                        </Badge>
                      )}
                    </span>
                    {/* Importar em primary e Remover em secondary, só com o
                        texto (como na lista de Cores) */}
                    {f.present ? (
                      <Button
                        className={styles.action}
                        appearance="secondary"
                        disabled={busy}
                        onClick={() => doRemove.mutate(key)}
                      >
                        {t('common.remove')}
                      </Button>
                    ) : (
                      <Button
                        className={styles.action}
                        appearance="primary"
                        disabled={busy}
                        onClick={() => doImport.mutate({ ...key, label: shownName(f) })}
                      >
                        {busy ? t('bios.importing') : t('bios.import')}
                      </Button>
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
              <span className={styles.tile} aria-hidden>
                <PuzzlePieceRegular />
              </span>
              <span className={styles.meta}>
                <Body1>
                  <code>PPSSPP/</code>
                </Body1>
                <Caption1 className={styles.note}>{t('bios.ppssppNote')}</Caption1>
              </span>
            </span>
            <span className={styles.status}>
              {ppsspp.data && (
                <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                  {t('bios.installed')}
                </Badge>
              )}
            </span>
            {/* baixar em primary; já instalado, atualizar em secondary */}
            <DownloadButton
              className={styles.action}
              appearance={ppsspp.data ? 'secondary' : 'primary'}
              label={getPpsspp.isPending ? t('bios.downloading') : ppsspp.data ? t('bios.update') : t('bios.download')}
              busy={getPpsspp.isPending}
              onClick={() => getPpsspp.mutate()}
            />
          </div>
        </div>
        )}
      </div>
    </div>
  )
}
