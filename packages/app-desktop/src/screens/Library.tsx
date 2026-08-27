import {
  Badge,
  Body1,
  Button,
  Caption1,
  Card,
  CardHeader,
  Input,
  Spinner,
  Subtitle2,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { listRoms, scanLibrary } from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  scanRow: { display: 'flex', gap: tokens.spacingHorizontalS, alignItems: 'end' },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))',
    gap: tokens.spacingHorizontalM,
  },
})

export function Library() {
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const [dir, setDir] = useState('')

  const roms = useQuery({ queryKey: ['roms'], queryFn: listRoms, retry: false })

  const scan = useMutation({
    mutationFn: (path: string) => scanLibrary(path),
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: ['roms'] })
      push({
        id: crypto.randomUUID(),
        message: `Scan: ${r.added} adicionada(s), ${r.skippedKnown} já conhecida(s), ${r.skippedUnrecognized} ignorada(s).`,
        variant: r.errors > 0 ? 'Warning' : 'Success',
        durationMs: 4000,
        source: 'System',
      })
    },
    onError: (e) =>
      push({
        id: crypto.randomUUID(),
        message: `Scan falhou: ${e}`,
        variant: 'Error',
        durationMs: 5000,
        source: 'System',
      }),
  })

  return (
    <div className={styles.root}>
      <Subtitle2>Biblioteca</Subtitle2>
      <div className={styles.scanRow}>
        <Input
          style={{ flex: 1 }}
          value={dir}
          placeholder="/caminho/para/roms"
          onChange={(_, d) => setDir(d.value)}
        />
        <Button
          appearance="primary"
          disabled={!dir || scan.isPending}
          onClick={() => scan.mutate(dir)}
        >
          {scan.isPending ? 'Escaneando…' : 'Escanear'}
        </Button>
      </div>

      {roms.isLoading && <Spinner label="Carregando…" />}
      {roms.isError && <Body1>Biblioteca indisponível (backend sem banco).</Body1>}
      {roms.data?.length === 0 && (
        <Caption1>Nenhuma ROM ainda — aponte um diretório acima e escaneie.</Caption1>
      )}

      <div className={styles.grid}>
        {roms.data?.map((r) => (
          <Card key={r.id} appearance="filled-alternative">
            <CardHeader
              header={<Body1><strong>{r.title}</strong></Body1>}
              description={<Badge appearance="tint">{r.systemId}</Badge>}
            />
          </Card>
        ))}
      </div>
    </div>
  )
}
