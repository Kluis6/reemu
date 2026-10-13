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
  // 12 entre os cards: o mesmo espaço da grade do hub das Configurações.
  list: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  // `<a>`: ganha o anel de foco global do app (controle/teclado).
  // Grade: ícone · nome · chevron na 1ª linha (centrados com o NOME — pedido
  // do usuário), descrição embaixo do nome; o conjunto no meio do card.
  card: {
    display: 'grid',
    gridTemplateColumns: 'auto minmax(0, 1fr) auto',
    alignItems: 'center',
    alignContent: 'center',
    columnGap: tokens.spacingHorizontalL,
    rowGap: '2px',
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
  icon: {
    gridColumn: 1,
    gridRow: 1,
    display: 'flex',
    fontSize: '24px',
    color: tokens.colorNeutralForeground2,
  },
  title: { gridColumn: 2, gridRow: 1 },
  desc: { gridColumn: 2, gridRow: 2, color: tokens.colorNeutralForeground3 },
  chevron: { gridColumn: 3, gridRow: 1, fontSize: '16px', color: tokens.colorNeutralForeground2 },
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
          <Body1 className={s.title}>{it.title}</Body1>
          <Caption1 className={s.desc}>{it.description}</Caption1>
          <ChevronRightRegular className={s.chevron} />
        </Link>
      ))}
    </nav>
  )
}
