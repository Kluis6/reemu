import { Badge, Button, Caption1, makeStyles, tokens } from '@fluentui/react-components'
import { MenuOverlay } from './components/MenuOverlay'
import { ToastLayer } from './components/ToastLayer'
import { useFocusBridge } from './hooks/useFocusBridge'
import { toggleFocus } from './lib/tauri'
import { useFocusStore } from './stores/useFocusStore'
import { useToastStore } from './stores/useToastStore'

const useStyles = makeStyles({
  // Fundo transparente: a surface nativa do jogo aparece por trás (Win/macOS,
  // e Linux/X11 via child window — ver src-tauri/src/video.rs).
  hud: {
    position: 'fixed',
    top: tokens.spacingVerticalM,
    left: tokens.spacingHorizontalM,
    display: 'flex',
    gap: tokens.spacingHorizontalS,
    alignItems: 'center',
    padding: `${tokens.spacingVerticalXS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    background: tokens.colorNeutralBackground1,
    boxShadow: tokens.shadow8,
  },
})

export default function App() {
  const styles = useStyles()
  useFocusBridge()
  const focus = useFocusStore((s) => s.focus)
  const setFocus = useFocusStore((s) => s.setFocus)
  const push = useToastStore((s) => s.push)

  const onToggle = () => {
    // Fora do Tauri, alterna localmente só pra ver a UI.
    toggleFocus()
      .then(setFocus)
      .catch(() => setFocus(focus === 'MenuFocused' ? 'GameFocused' : 'MenuFocused'))
  }

  return (
    <>
      <div className={styles.hud}>
        <Caption1><strong>ReEmu</strong></Caption1>
        <Badge appearance="tint" color={focus === 'MenuFocused' ? 'warning' : 'success'}>
          {focus}
        </Badge>
        <Button size="small" appearance="subtle" onClick={onToggle}>
          {focus === 'MenuFocused' ? 'Voltar ao jogo' : 'Menu'}
        </Button>
        <Button
          size="small"
          appearance="subtle"
          onClick={() =>
            push({
              id: crypto.randomUUID(),
              message: 'Toast de teste.',
              variant: 'Info',
              durationMs: 3000,
              source: 'System',
            })
          }
        >
          Toast
        </Button>
      </div>

      <MenuOverlay />
      <ToastLayer />
    </>
  )
}
