import { Text, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { ChevronRightRegular } from '@fluentui/react-icons'
import type { ReactNode } from 'react'

const useStyles = makeStyles({
  root: { marginBottom: '18px' },
  row: {
    display: 'flex',
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalM,
  },
  titleBtn: {
    display: 'inline-flex',
    alignItems: 'center',
    columnGap: '4px',
    border: 'none',
    backgroundColor: 'transparent',
    padding: 0,
    margin: 0,
    color: 'inherit',
    cursor: 'pointer',
    borderRadius: tokens.borderRadiusMedium,
    outlineOffset: '4px',
  },
  titlePlain: { cursor: 'default' },
  title: {
    fontSize: 'clamp(18px, 1.35vw, 28px)',
    fontWeight: tokens.fontWeightBold,
    lineHeight: 1.15,
  },
  chevron: {
    fontSize: '20px',
    color: tokens.colorNeutralForeground3,
    transitionProperty: 'transform, color',
    transitionDuration: '150ms',
    transitionTimingFunction: tokens.curveEasyEase,
    'button:hover > &, button:focus-visible > &': {
      transform: 'translateX(3px)',
      color: tokens.colorNeutralForeground1,
    },
  },
  right: { marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalS },
  sub: {
    marginTop: '3px',
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground3,
  },
})

/**
 * Cabeçalho de seção no modelo modo XBOX: título grande com um chevron `>`
 * logo depois (o conjunto todo é clicável quando `onSeeAll` é dado), uma
 * linha de subtítulo embaixo, e um slot opcional à direita (contagem etc.).
 */
export function SectionHeader({
  title,
  subtitle,
  onSeeAll,
  seeAllLabel,
  right,
  as = 'h2',
}: {
  title: string
  subtitle?: string
  onSeeAll?: () => void
  seeAllLabel?: string
  right?: ReactNode
  as?: 'h2' | 'h3'
}) {
  const s = useStyles()
  return (
    <div className={s.root}>
      <div className={s.row}>
        {onSeeAll ? (
          <button
            type="button"
            className={s.titleBtn}
            onClick={onSeeAll}
            aria-label={seeAllLabel ?? `Ver tudo — ${title}`}
          >
            <Text as={as} className={s.title}>
              {title}
            </Text>
            <ChevronRightRegular className={s.chevron} />
          </button>
        ) : (
          <Text as={as} className={mergeClasses(s.title, s.titlePlain)}>
            {title}
          </Text>
        )}
        {right && <div className={s.right}>{right}</div>}
      </div>
      {subtitle && <div className={s.sub}>{subtitle}</div>}
    </div>
  )
}
