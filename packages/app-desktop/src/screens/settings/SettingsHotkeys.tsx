import { Body1, Button, Caption1, makeStyles, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { sysToast } from '../../lib/toast'
import {
  clearSystemHotkey,
  describeRawInput,
  listSystemHotkeys,
  type RawInputEvent,
  type SystemActionKey,
} from '../../lib/tauri'
import { useBindingCaptureStore } from '../../stores/useBindingCaptureStore'
import { useToastStore } from '../../stores/useToastStore'

const HOTKEY_ACTIONS: { key: SystemActionKey; label: string }[] = [
  { key: 'toggle_menu_overlay', label: 'Abrir/fechar menu' },
  { key: 'quick_save', label: 'Save state rápido' },
  { key: 'quick_load', label: 'Load state rápido' },
]

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM, maxWidth: '520px' },
  row: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalM,
  },
  label: { display: 'flex', flexDirection: 'column' },
  actions: { display: 'flex', gap: tokens.spacingHorizontalXS },
})

const triggerText = (t: RawInputEvent[]) =>
  t.length === 0 ? 'não definido' : t.map(describeRawInput).join(' + ')

export function SettingsHotkeys() {
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const beginCapture = useBindingCaptureStore((s) => s.begin)

  const hotkeys = useQuery({ queryKey: ['system-hotkeys'], queryFn: listSystemHotkeys, retry: false })
  const triggerFor = (key: SystemActionKey): RawInputEvent[] =>
    hotkeys.data?.find((b) => b.action === key)?.trigger ?? []

  const clearHotkey = useMutation({
    mutationFn: (key: SystemActionKey) => clearSystemHotkey(key),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['system-hotkeys'] }),
    onError: (e) => push(sysToast(`Falha ao limpar: ${e}`, 'Error')),
  })

  return (
    <div className={styles.root}>
      <Caption1>
        Combinação hold+press: segure a primeira tecla/botão e aperte outra.{' '}
        <kbd>Esc</kbd> sempre abre/fecha o menu no jogo (rede de segurança, não editável).
      </Caption1>
      {HOTKEY_ACTIONS.map(({ key, label }) => (
        <div key={key} className={styles.row}>
          <span className={styles.label}>
            <Body1>{label}</Body1>
            <Caption1>{triggerText(triggerFor(key))}</Caption1>
          </span>
          <span className={styles.actions}>
            <Button
              size="small"
              onClick={() => beginCapture({ target: 'system_hotkey', targetKey: key, label })}
            >
              Redefinir
            </Button>
            <Button
              size="small"
              appearance="subtle"
              disabled={clearHotkey.isPending || triggerFor(key).length === 0}
              onClick={() => clearHotkey.mutate(key)}
            >
              Limpar
            </Button>
          </span>
        </div>
      ))}
    </div>
  )
}
