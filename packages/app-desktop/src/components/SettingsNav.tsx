import {
  Body1,
  Caption1,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { ChevronRightRegular } from '@fluentui/react-icons'
import type { ReactElement } from 'react'
import { Link } from 'react-router-dom'

// Navegação dentro de uma categoria das Configurações, como nas
// Configurações do Windows: a categoria lista as subseções em cards com uma
// seta (`SettingsLinkList`); o caminho no topo ("Configurações › Vídeo ›
// Shaders") é do `SettingsLayout`.

const useStyles = makeStyles({
  list: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
  // `<a>`: ganha o anel de foco global do app (controle/teclado).
  card: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalL,
    minHeight: '68px',
    padding: `0 ${tokens.spacingHorizontalL}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
    color: tokens.colorNeutralForeground1,
    textDecorationLine: 'none',
    boxSizing: 'border-box',
    ':hover': { backgroundColor: tokens.colorNeutralBackground2Hover },
    ':active': { backgroundColor: tokens.colorNeutralBackground2Pressed },
  },
  icon: { fontSize: '24px', flexShrink: 0, color: tokens.colorNeutralForeground2 },
  text: { display: 'flex', flexDirection: 'column', gap: '2px', flexGrow: 1, minWidth: 0 },
  desc: { color: tokens.colorNeutralForeground3 },
  chevron: { fontSize: '16px', flexShrink: 0, color: tokens.colorNeutralForeground2 },
})

export interface SettingsLink {
  to: string
  icon: ReactElement
  title: string
  description: string
}

/** Lista de subseções de uma categoria, cada uma num card com seta. */
export function SettingsLinkList({ items }: { items: SettingsLink[] }) {
  const s = useStyles()
  return (
    <nav className={s.list}>
      {items.map((it) => (
        <Link key={it.to} to={it.to} className={s.card}>
          <span className={s.icon}>{it.icon}</span>
          <span className={s.text}>
            <Body1>{it.title}</Body1>
            <Caption1 className={s.desc}>{it.description}</Caption1>
          </span>
          <ChevronRightRegular className={s.chevron} />
        </Link>
      ))}
    </nav>
  )
}
