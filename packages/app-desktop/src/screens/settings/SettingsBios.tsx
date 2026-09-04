import { Badge, Body1, Button, Caption1, Spinner, makeStyles, tokens } from '@fluentui/react-components'
import {
  CheckmarkCircleFilled,
  DeleteRegular,
  DocumentArrowUpRegular,
  WarningFilled,
} from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  importBiosFile,
  listBiosStatus,
  pickBiosFile,
  removeBiosFile,
  type BiosStatus,
} from '../../lib/tauri'
import { sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  system: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
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

const SYSTEM_LABEL: Record<string, string> = {
  psx: 'PlayStation',
  saturn: 'Saturn',
  dreamcast: 'Dreamcast',
  arcade: 'Arcade (FBNeo)',
}

type Key = { systemId: string; filename: string }
const sameKey = (a?: Key, b?: Key) => a?.systemId === b?.systemId && a?.filename === b?.filename

export function SettingsBios() {
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const bios = useQuery({ queryKey: ['bios-status'], queryFn: listBiosStatus, retry: false })

  const refresh = () => qc.invalidateQueries({ queryKey: ['bios-status'] })

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
      push(sysToast(`BIOS importado: ${filename}`, 'Success'))
    },
    onError: (e) => push(sysToast(`Falha ao importar: ${e}`, 'Error')),
  })
  const doRemove = useMutation({
    mutationFn: (v: Key) => removeBiosFile(v.systemId, v.filename),
    onSuccess: refresh,
    onError: (e) => push(sysToast(`Falha ao remover: ${e}`, 'Error')),
  })

  if (bios.isLoading) return <Spinner label="Conferindo pasta de sistema…" />
  if (bios.isError) return <Body1>Indisponível.</Body1>

  const bySystem = new Map<string, BiosStatus[]>()
  for (const b of bios.data ?? []) {
    const list = bySystem.get(b.systemId) ?? []
    list.push(b)
    bySystem.set(b.systemId, list)
  }

  return (
    <div className={styles.root}>
      <Caption1>
        Arquivos de sistema que alguns cores exigem além da ROM (PS1, Saturn, Dreamcast, Arcade).
        O ReEmu <strong>nunca baixa BIOS</strong> — são copyright da fabricante; importe um arquivo
        que você já possui legalmente.
      </Caption1>
      {[...bySystem.entries()].map(([systemId, files]) => (
        <div key={systemId} className={styles.system}>
          <Body1>
            <strong>{(SYSTEM_LABEL[systemId] ?? systemId).toUpperCase()}</strong>
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
                          obrigatório
                        </Badge>
                      )}
                    </Body1>
                    <Caption1>{f.note}</Caption1>
                  </span>
                  <span style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                    {!f.present && (
                      <Badge appearance="tint" color={f.required ? 'danger' : 'informative'}>
                        faltando
                      </Badge>
                    )}
                    {f.present && f.hashOk === false && (
                      <Badge appearance="tint" color="warning" icon={<WarningFilled />}>
                        presente, hash não bate
                      </Badge>
                    )}
                    {f.present && f.hashOk !== false && (
                      <Badge appearance="tint" color="success" icon={<CheckmarkCircleFilled />}>
                        presente
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
                        Remover
                      </Button>
                    ) : (
                      <Button
                        size="small"
                        appearance="primary"
                        icon={<DocumentArrowUpRegular />}
                        disabled={busy}
                        onClick={() => doImport.mutate(key)}
                      >
                        {busy ? 'Importando…' : 'Importar…'}
                      </Button>
                    )}
                  </span>
                </div>
              )
            })}
          </div>
        </div>
      ))}
    </div>
  )
}
