import { Body1, Button, Caption1, makeStyles, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { errorToast } from '../../lib/toast'
import {
  clearSystemHotkey,
  describeRawInput,
  listSystemHotkeys,
  type RawInputEvent,
  type SystemActionKey,
} from '../../lib/tauri'
import { useBindingCaptureStore } from '../../stores/useBindingCaptureStore'
import { useToastStore } from '../../stores/useToastStore'
import { Trans, useTranslation } from 'react-i18next'

// Rótulo traduzido na renderização: `hotkeys.actions.<key>`.
const HOTKEY_ACTIONS = ['toggle_menu_overlay', 'quick_save', 'quick_load'] as const satisfies readonly SystemActionKey[]

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


export function SettingsHotkeys() {
  const { t } = useTranslation()
  const triggerText = (tr: RawInputEvent[]) =>
    tr.length === 0 ? t('hotkeys.notSet') : tr.map(describeRawInput).join(' + ')
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
    onError: (e) => push(errorToast(e, 'clearHotkey')),
  })

  return (
    <div className={styles.root}>
      <Caption1>
        <Trans i18nKey="hotkeys.intro" components={{ kbd: <kbd /> }} />
      </Caption1>
      {HOTKEY_ACTIONS.map((key) => {
        const label = t(`hotkeys.actions.${key}`)
        return (
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
              {t('hotkeys.reset')}
            </Button>
            <Button
              size="small"
              appearance="subtle"
              disabled={clearHotkey.isPending || triggerFor(key).length === 0}
              onClick={() => clearHotkey.mutate(key)}
            >
              {t('hotkeys.clear')}
            </Button>
          </span>
        </div>
        )
      })}
    </div>
  )
}
