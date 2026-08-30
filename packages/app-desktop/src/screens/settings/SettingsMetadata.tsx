import {
  Body1,
  Button,
  Caption1,
  Field,
  Image,
  Input,
  ProgressBar,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkRegular, DismissRegular, SearchRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { sysToast } from '../../lib/toast'
import {
  cancelMetadataScan,
  getMetadataConfig,
  listPendingMatches,
  metadataScanProgress,
  resolvePendingMatch,
  setMetadataConfig,
  startMetadataScan,
  type MetadataConfig,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL, maxWidth: '520px' },
  form: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS, maxWidth: '360px' },
  pending: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
  row: {
    display: 'flex',
    gap: tokens.spacingHorizontalM,
    alignItems: 'center',
    padding: tokens.spacingVerticalS,
    borderRadius: tokens.borderRadiusMedium,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  cover: { width: '36px', height: '48px', objectFit: 'cover', flexShrink: 0, borderRadius: '3px' },
  grow: { flexGrow: 1, minWidth: 0 },
  dim: { color: tokens.colorNeutralForeground3 },
})

export function SettingsMetadata() {
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const cfg = useQuery({ queryKey: ['metadata-config'], queryFn: getMetadataConfig, retry: false })
  const [form, setForm] = useState<MetadataConfig | null>(null)
  useEffect(() => {
    if (cfg.data && !form) setForm(cfg.data)
  }, [cfg.data, form])

  const progress = useQuery({
    queryKey: ['metadata-progress'],
    queryFn: metadataScanProgress,
    refetchInterval: (q) => (q.state.data?.running ? 700 : false),
    retry: false,
  })
  const running = progress.data?.running ?? false

  const pending = useQuery({
    queryKey: ['pending-matches'],
    queryFn: listPendingMatches,
    retry: false,
  })
  // recarrega a fila de pendências quando o scan termina
  useEffect(() => {
    if (!running) {
      pending.refetch()
      qc.invalidateQueries({ queryKey: ['roms'] })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [running])

  const save = useMutation({
    mutationFn: (c: MetadataConfig) => setMetadataConfig(c),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['metadata-config'] })
      push(sysToast('Configuração salva.', 'Success'))
    },
    onError: (e) => push(sysToast(`Falha: ${e}`, 'Error')),
  })
  const scan = useMutation({
    mutationFn: () => startMetadataScan(),
    onSuccess: () => progress.refetch(),
    onError: (e) => push(sysToast(`Falha: ${e}`, 'Error')),
  })
  const resolve = useMutation({
    mutationFn: ({ romId, accept }: { romId: string; accept: boolean }) =>
      resolvePendingMatch(romId, accept),
    onSuccess: () => {
      pending.refetch()
      qc.invalidateQueries({ queryKey: ['roms'] })
    },
    onError: (e) => push(sysToast(`Falha: ${e}`, 'Error')),
  })

  if (cfg.isLoading) return <Spinner label="Carregando…" />
  if (cfg.isError || !form) return <Body1>Config de metadata indisponível (sem banco?).</Body1>

  const p = progress.data

  return (
    <div className={s.root}>
      <Caption1>
        Busca título, descrição, ano e gênero por hash (CRC32) no{' '}
        <strong>ScreenScraper</strong>. Só match de hash exato entra sozinho — o
        resto vai pra revisão abaixo. Uma conta grátis em screenscraper.fr
        aumenta bastante o limite de requisições.
      </Caption1>

      <div className={s.form}>
        <Field label="Usuário ScreenScraper (opcional)">
          <Input
            value={form.screenscraperUser ?? ''}
            onChange={(_, d) => setForm({ ...form, screenscraperUser: d.value || null })}
          />
        </Field>
        <Field label="Senha ScreenScraper (opcional)">
          <Input
            type="password"
            value={form.screenscraperPassword ?? ''}
            onChange={(_, d) => setForm({ ...form, screenscraperPassword: d.value || null })}
          />
        </Field>
        <Button
          appearance="primary"
          disabled={save.isPending}
          onClick={() => save.mutate(form)}
        >
          Salvar
        </Button>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <div style={{ display: 'flex', gap: 8 }}>
          <Button
            icon={<SearchRegular />}
            disabled={running || scan.isPending}
            onClick={() => scan.mutate()}
          >
            {running ? 'Escaneando…' : 'Escanear metadata da biblioteca'}
          </Button>
          {running && (
            <Button appearance="subtle" onClick={() => void cancelMetadataScan()}>
              Cancelar
            </Button>
          )}
        </div>
        {p && (p.running || p.done > 0) && (
          <>
            <ProgressBar value={p.total ? p.done / p.total : undefined} />
            <Caption1 className={s.dim}>
              {p.done}/{p.total} — {p.auto} automáticas · {p.pending} p/ revisão · {p.failed} falha
            </Caption1>
          </>
        )}
      </div>

      {(pending.data?.length ?? 0) > 0 && (
        <div className={s.pending}>
          <Caption1>Revisar ({pending.data!.length}) — correspondências por nome, não por hash</Caption1>
          {pending.data!.map((m) => (
            <div key={m.romId} className={s.row}>
              {m.coverUrl && <Image className={s.cover} src={m.coverUrl} alt="" />}
              <div className={s.grow}>
                <Body1>{m.title}</Body1>
                <Caption1 className={s.dim}>
                  {m.fileStem}
                  {m.releaseDate ? ` · ${m.releaseDate}` : ''}
                  {m.genre ? ` · ${m.genre}` : ''}
                </Caption1>
              </div>
              <Button
                size="small"
                icon={<CheckmarkRegular />}
                disabled={resolve.isPending}
                onClick={() => resolve.mutate({ romId: m.romId, accept: true })}
              >
                Aceitar
              </Button>
              <Button
                size="small"
                appearance="subtle"
                icon={<DismissRegular />}
                disabled={resolve.isPending}
                onClick={() => resolve.mutate({ romId: m.romId, accept: false })}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
