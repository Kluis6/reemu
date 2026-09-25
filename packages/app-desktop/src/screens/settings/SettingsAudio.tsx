import { Body1, Button, Field, Input, Switch, makeStyles, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { LoadingState } from '../../components/EmptyState'
import { errorToast, sysToast } from '../../lib/toast'
import { getAudioConfig, updateAudioConfig, type AudioConfig } from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  section: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM, maxWidth: '440px' },
})

export function SettingsAudio() {
  const { t } = useTranslation()
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
      push(sysToast(t('audio.saved'), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'saveAudioConfig')),
  })

  if (isLoading) return <LoadingState />
  if (isError || !draft)
    return <Body1>{t('audio.unavailable')}</Body1>

  return (
    <div className={styles.section}>
      <Field label={t('audio.drc')} hint={t('audio.drcHint')}>
        <Switch
          checked={draft.rateControlEnabled}
          onChange={(_, d) => set('rateControlEnabled', d.checked)}
        />
      </Field>
      <Field label={t('audio.delta')} hint={t('audio.deltaHint')}>
        <Input
          type="number"
          step={0.001}
          value={String(draft.rateControlDelta)}
          onChange={(_, d) => set('rateControlDelta', Number(d.value))}
        />
      </Field>
      <Field label={t('audio.device')}>
        <Input
          value={draft.outputDeviceId ?? ''}
          placeholder={t('audio.systemDefault')}
          onChange={(_, d) => set('outputDeviceId', d.value === '' ? null : d.value)}
        />
      </Field>
      <Button
        appearance="primary"
        disabled={save.isPending || Object.keys(edits).length === 0}
        onClick={() => save.mutate(draft)}
      >
        {t('common.save')}
      </Button>
    </div>
  )
}
