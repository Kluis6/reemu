import { Tab, TabList, Title2, makeStyles, tokens } from '@fluentui/react-components'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL, maxWidth: '640px' },
})

const TABS = [
  { key: 'audio', label: 'Áudio' },
  { key: 'video', label: 'Vídeo' },
  { key: 'metadata', label: 'Metadata' },
  { key: 'hotkeys', label: 'Atalhos' },
  { key: 'controllers', label: 'Controles' },
  { key: 'cores', label: 'Cores' },
  { key: 'bios', label: 'BIOS' },
]

export function SettingsLayout() {
  const styles = useStyles()
  const navigate = useNavigate()
  const { pathname } = useLocation()
  const current = TABS.find((t) => pathname.endsWith(`/${t.key}`))?.key ?? 'audio'

  return (
    <div className={styles.root}>
      <Title2>Configurações</Title2>
      <TabList
        selectedValue={current}
        onTabSelect={(_, d) => navigate(`/settings/${d.value}`)}
      >
        {TABS.map((t) => (
          <Tab key={t.key} value={t.key}>
            {t.label}
          </Tab>
        ))}
      </TabList>
      <Outlet />
    </div>
  )
}
