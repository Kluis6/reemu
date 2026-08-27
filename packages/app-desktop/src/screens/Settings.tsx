import {
  Body1,
  Button,
  Field,
  Input,
  Spinner,
  Switch,
  Title3,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { getAudioConfig, updateAudioConfig, type AudioConfig } from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL, maxWidth: '440px' },
  section: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
})

const toast = (message: string, variant: 'Success' | 'Error') => ({
  id: crypto.randomUUID(),
  message,
  variant,
  durationMs: variant === 'Error' ? 4000 : 2500,
  source: 'System' as const,
})

export function Settings() {
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)

  const { data, isLoading, isError } = useQuery({
    queryKey: ['audio-config'],
    queryFn: getAudioConfig,
    retry: false,
  })

  // Só as edições ficam em state; o resto vem da query (sem effect de sync).
  const [edits, setEdits] = useState<Partial<AudioConfig>>({})
  const draft = data ? { ...data, ...edits } : null
  const set = <K extends keyof AudioConfig>(key: K, value: AudioConfig[K]) =>
    setEdits((e) => ({ ...e, [key]: value }))

  const save = useMutation({
    mutationFn: (c: AudioConfig) => updateAudioConfig(c),
    onSuccess: () => {
      setEdits({})
      qc.invalidateQueries({ queryKey: ['audio-config'] })
      push(toast('Configuração de áudio salva.', 'Success'))
    },
    onError: (e) => push(toast(`Falha ao salvar: ${e}`, 'Error')),
  })

  if (isLoading) return <Spinner label="Carregando configurações…" />
  if (isError || !draft)
    return <Body1>Configurações indisponíveis (backend sem banco de dados).</Body1>

  return (
    <div className={styles.root}>
      <Title3>Áudio</Title3>
      <div className={styles.section}>
        <Field label="Dynamic Rate Control" hint="Ajusta o resample em tempo real pra sincronia A/V.">
          <Switch
            checked={draft.rateControlEnabled}
            onChange={(_, d) => set('rateControlEnabled', d.checked)}
          />
        </Field>
        <Field label="Margem de ajuste (delta)" hint="0.005 = ±0,5%">
          <Input
            type="number"
            step={0.001}
            value={String(draft.rateControlDelta)}
            onChange={(_, d) => set('rateControlDelta', Number(d.value))}
          />
        </Field>
        <Field label="Dispositivo de saída (ID do SO)">
          <Input
            value={draft.outputDeviceId ?? ''}
            placeholder="padrão do sistema"
            onChange={(_, d) => set('outputDeviceId', d.value === '' ? null : d.value)}
          />
        </Field>
      </div>
      <Button
        appearance="primary"
        disabled={save.isPending || Object.keys(edits).length === 0}
        onClick={() => save.mutate(draft)}
      >
        Salvar
      </Button>
    </div>
  )
}
