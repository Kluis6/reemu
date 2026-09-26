import { Badge, Body1, Button, Caption1, Text, makeStyles, tokens } from '@fluentui/react-components'
import {
  ArrowDownloadRegular,
  CheckmarkCircleFilled,
  DeleteRegular,
  DocumentArrowUpRegular,
  WarningFilled,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  downloadPpssppAssets,
  importBiosFile,
  listBiosStatus,
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
import { Trans, useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  // Um bloco por sistema em COLUNAS CORRIDAS (2, ou 3 em tela larga): cada
  // coluna empilha os blocos direto um embaixo do outro. Em grade, a linha
  // inteira tomava a altura do maior bloco (PlayStation, 3 arquivos) e o
  // vizinho menor (Saturn, 1) ficava com um buraco embaixo.
  groups: {
    columnCount: 2,
    columnGap: tokens.spacingHorizontalXL,
    '@media (min-width: 1600px)': { columnCount: 3 },
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
  list: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  row: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalM,
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground2,
  },
  meta: { display: 'flex', flexDirection: 'column', gap: '2px', minWidth: 0, overflowWrap: 'anywhere' },
  // Selos + botão: nunca encolhem nem quebram — quem quebra linha é a nota
  // à esquerda (sem isto, "presente, hash não bate" vazava da pílula e o
  // "Remover" era cortado nas colunas estreitas).
  actions: {
    display: 'flex',
    gap: tokens.spacingHorizontalS,
    alignItems: 'center',
    flexShrink: 0,
    whiteSpace: 'nowrap',
  },
})

type Key = { systemId: string; filename: string }
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
    mutationFn: async ({ systemId, filename }: Key) => {
      const path = await pickBiosFile()
      if (!path) return false // cancelado no diálogo
      await importBiosFile(systemId, filename, path)
      return true
    },
    onSuccess: (imported, { filename }) => {
      if (!imported) return
      refresh()
      push(sysToast(t('bios.imported', { file: filename }), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'importBios')),
  })
  const doRemove = useMutation({
    mutationFn: (v: Key) => removeBiosFile(v.systemId, v.filename),
    onSuccess: refresh,
    onError: (e) => push(errorToast(e, 'removeBios')),
  })

  if (bios.isLoading) return <LoadingState label={t('bios.checking')} />
  if (bios.isError) return <Body1>{t('bios.unavailable')}</Body1>

  const bySystem = new Map<string, BiosStatus[]>()
  for (const b of bios.data ?? []) {
    const list = bySystem.get(b.systemId) ?? []
    list.push(b)
    bySystem.set(b.systemId, list)
  }

  return (
    <div className={styles.root}>
      <Caption1>
        <Trans i18nKey="bios.intro" components={{ b: <Text as="strong" weight="semibold" /> }} />
      </Caption1>
      <div className={styles.groups}>
        {[...bySystem.entries()].map(([systemId, files]) => (
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
                    <span className={styles.meta}>
                      <Body1>
                        <code>{f.filename}</code>
                        {f.required && (
                          <Badge appearance="tint" color="danger" style={{ marginLeft: 8 }}>
                            {t('bios.required')}
                          </Badge>
                        )}
                      </Body1>
                      <Caption1>{biosNote(t, f.filename, f.note)}</Caption1>
                    </span>
                    <span className={styles.actions}>
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
                      {f.present ? (
                        <Button
                          size="small"
                          appearance="subtle"
                          icon={<DeleteRegular />}
                          disabled={busy}
                          onClick={() => doRemove.mutate(key)}
                        >
                          {t('common.remove')}
                        </Button>
                      ) : (
                        <Button
                          size="small"
                          appearance="primary"
                          icon={<DocumentArrowUpRegular />}
                          disabled={busy}
                          onClick={() => doImport.mutate(key)}
                        >
                          {busy ? t('bios.importing') : t('bios.import')}
                        </Button>
                      )}
                    </span>
                  </div>
                )
              })}
            </div>
          </div>
        ))}
        <div className={styles.system}>
          <Body1>
            <Text as="strong" weight="semibold">{platformLabel('psp').toUpperCase()}</Text>
          </Body1>
          <div className={styles.row}>
            <span className={styles.meta}>
              <Body1>
                <code>PPSSPP/</code>
              </Body1>
              <Caption1>{t('bios.ppssppNote')}</Caption1>
            </span>
            <span className={styles.actions}>
              {ppsspp.data && (
                <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                  {t('bios.installed')}
                </Badge>
              )}
              <Button
                size="small"
                appearance={ppsspp.data ? 'subtle' : 'primary'}
                icon={<ArrowDownloadRegular />}
                disabled={getPpsspp.isPending}
                onClick={() => getPpsspp.mutate()}
              >
                {getPpsspp.isPending ? t('bios.downloading') : ppsspp.data ? t('bios.update') : t('bios.download')}
              </Button>
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}
