import {
  Spinner,
  Subtitle1,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import type { ReactNode } from 'react'

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    rowGap: tokens.spacingVerticalM,
    textAlign: 'center',
    paddingBlock: '72px',
    paddingInline: tokens.spacingHorizontalXXL,
    color: tokens.colorNeutralForeground3,
  },
  icon: { fontSize: '44px', lineHeight: 1, opacity: 0.5 },
  art: { color: tokens.colorBrandForeground1, opacity: 0.85 },
  title: { color: tokens.colorNeutralForeground1 },
  action: { marginTop: tokens.spacingVerticalS },
  loading: {
    display: 'flex',
    justifyContent: 'center',
    paddingBlock: '48px',
  },
})

/**
 * Estado vazio padrão das telas (biblioteca vazia, sem favoritos, busca sem
 * resultado, erro de backend…). A Fluent não tem um componente pra isso —
 * este é o único lugar que define o visual, pra não divergir entre telas.
 */
export function EmptyState({
  icon,
  art,
  title,
  children,
  action,
}: {
  /** Emoji/ícone pequeno (fallback). Ignorado se `art` for dado. */
  icon?: ReactNode
  /** Ilustração de linha (`components/EmptyArt`) — no lugar do `icon`. */
  art?: ReactNode
  title: string
  /** Uma frase curta explicando o que fazer. */
  children?: ReactNode
  /** Botão de ação (ex.: "Adicionar ROMs…"). */
  action?: ReactNode
}) {
  const s = useStyles()
  return (
    <div className={s.root}>
      <div className={art ? s.art : s.icon} aria-hidden>
        {art ?? icon}
      </div>
      <Subtitle1 as="h2" className={s.title}>
        {title}
      </Subtitle1>
      {children && <Text>{children}</Text>}
      {action && <div className={s.action}>{action}</div>}
    </div>
  )
}

/** Spinner centralizado — mesmo espaçamento em todas as telas. */
export function LoadingState({ label }: { label?: string }) {
  const s = useStyles()
  return (
    <div className={s.loading}>
      <Spinner label={label ?? 'Carregando…'} />
    </div>
  )
}
