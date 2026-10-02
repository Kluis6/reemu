import { Field, Switch, makeStyles, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { LoadingState } from '../../components/EmptyState'
import {
  getSystemSettings,
  setAutostart,
  setMinimizeToTray,
  setStartFullscreen,
  type SystemSettings,
} from '../../lib/tauri'
import { errorToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: tokens.spacingVerticalXL,
  },
})

type Key = keyof SystemSettings

// Cada interruptor e o comando que grava ele.
const SAVE: Record<Key, (enabled: boolean) => Promise<void>> = {
  autostart: setAutostart,
  startFullscreen: setStartFullscreen,
  minimizeToTray: setMinimizeToTray,
}

/** Configurações › Sistema: iniciar com o sistema, abrir em tela cheia e
 *  minimizar para a bandeja (ver `src-tauri/src/system.rs`). */
export function SettingsSystem() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const settings = useQuery({
    queryKey: ['system-settings'],
    queryFn: getSystemSettings,
    retry: false,
  })
  const save = useMutation({
    mutationFn: ({ key, value }: { key: Key; value: boolean }) => SAVE[key](value),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['system-settings'] }),
    onError: (e) => push(errorToast(e, 'saveSystemSettings')),
  })

  if (settings.isLoading) return <LoadingState />

  const toggle = (key: Key, label: string, hint: string) => (
    <Field label={label} hint={hint}>
      <Switch
        checked={settings.data?.[key] ?? false}
        disabled={!settings.data || save.isPending}
        onChange={(_, d) => save.mutate({ key, value: d.checked })}
      />
    </Field>
  )

  return (
    <div className={s.root}>
      {toggle('autostart', t('system.autostart'), t('system.autostartHint'))}
      {toggle('startFullscreen', t('system.startFullscreen'), t('system.startFullscreenHint'))}
      {toggle('minimizeToTray', t('system.minimizeToTray'), t('system.minimizeToTrayHint'))}
    </div>
  )
}
