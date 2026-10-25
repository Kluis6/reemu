import {
  Spinner,
  Subtitle1,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

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
  // `--reemuBrandText`, não `colorBrandForeground1`: o tom padrão de marca
  // falha contraste AA contra os neutros customizados do tema claro (ver
  // `styles/themes.ts::ReEmuTokens.reemuBrandText`).
  art: { color: "var(--reemuBrandText)", opacity: 0.85 },
  title: { color: tokens.colorNeutralForeground1 },
  action: { marginTop: tokens.spacingVerticalS },
  // `size="large"` (página inicial sem jogos, pedido do usuário): no meio da
  // altura útil da página, ilustração e textos maiores. A altura útil é a da
  // janela menos o topo da página e a margem de baixo do shell.
  rootLg: {
    minHeight: 'calc(100vh - var(--reemuPageTop, 116px) - 100px)',
    justifyContent: 'center',
    rowGap: tokens.spacingVerticalL,
    paddingBlock: 0,
  },
  artLg: { fontSize: '112px', lineHeight: 1, '& svg': { display: 'block' } },
  titleLg: {
    fontSize: tokens.fontSizeHero800,
    lineHeight: tokens.lineHeightHero800,
    fontWeight: tokens.fontWeightSemibold,
  },
  textLg: { fontSize: tokens.fontSizeBase500, lineHeight: tokens.lineHeightBase500 },
  actionLg: { marginTop: tokens.spacingVerticalM },
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
  size = 'medium',
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
  /** `large`: ocupa a página, no meio, com tudo maior (página inicial vazia). */
  size?: 'medium' | 'large'
}) {
  const s = useStyles()
  const lg = size === 'large'
  return (
    <div className={mergeClasses(s.root, lg && s.rootLg)}>
      <div className={mergeClasses(art ? s.art : s.icon, lg && s.artLg)} aria-hidden>
        {art ?? icon}
      </div>
      <Subtitle1 as="h2" className={mergeClasses(s.title, lg && s.titleLg)}>
        {title}
      </Subtitle1>
      {children && <Text className={lg ? s.textLg : undefined}>{children}</Text>}
      {action && <div className={mergeClasses(s.action, lg && s.actionLg)}>{action}</div>}
    </div>
  )
}

/** Spinner centralizado — mesmo espaçamento em todas as telas. */
export function LoadingState({ label }: { label?: string }) {
  const { t } = useTranslation()
  const s = useStyles()
  return (
    <div className={s.loading}>
      <Spinner label={label ?? t('common.loading')} />
    </div>
  )
}
