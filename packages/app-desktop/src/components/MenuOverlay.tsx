import {
  Tab,
  TabList,
  Text,
  Title2,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useState } from 'react'
import { useFocusStore } from '../stores/useFocusStore'
import { Library } from '../screens/Library'
import { Settings } from '../screens/Settings'

const useStyles = makeStyles({
  // Sempre sobreposto à surface do jogo. Quando `GameFocused`, some (o jogo
  // continua rodando atrás). Nunca esconde a webview — só deixa de captar.
  scrim: {
    position: 'fixed',
    inset: 0,
    background: 'rgba(0, 0, 0, 0.55)',
    backdropFilter: 'blur(2px)',
    display: 'flex',
    justifyContent: 'center',
    padding: tokens.spacingVerticalXXL,
    overflowY: 'auto',
  },
  panel: {
    width: '100%',
    maxWidth: '880px',
    background: tokens.colorNeutralBackground1,
    borderRadius: tokens.borderRadiusXLarge,
    boxShadow: tokens.shadow28,
    padding: tokens.spacingVerticalXL,
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
    height: 'fit-content',
  },
  header: { display: 'flex', alignItems: 'baseline', justifyContent: 'space-between' },
})

type Screen = 'library' | 'settings'

export function MenuOverlay() {
  const styles = useStyles()
  const focus = useFocusStore((s) => s.focus)
  const [screen, setScreen] = useState<Screen>('library')

  if (focus !== 'MenuFocused') return null

  return (
    <div className={styles.scrim}>
      <div className={styles.panel}>
        <div className={styles.header}>
          <Title2>ReEmu</Title2>
          <Text size={200}>Toggle do menu = hotkey (padrão configurável)</Text>
        </div>
        <TabList selectedValue={screen} onTabSelect={(_, d) => setScreen(d.value as Screen)}>
          <Tab value="library">Biblioteca</Tab>
          <Tab value="settings">Configurações</Tab>
        </TabList>
        {screen === 'library' ? <Library /> : <Settings />}
      </div>
    </div>
  )
}
