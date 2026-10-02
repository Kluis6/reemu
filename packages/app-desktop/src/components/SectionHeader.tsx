import { Button, Text, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { ChevronRightRegular } from '@fluentui/react-icons'
import type { ReactNode } from 'react'
import { SECTION_TITLE, SECTION_TITLE_GAP } from '../styles/metrics'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  // Espaço até os cards, como no app Xbox (metrics.ts).
  root: { marginBottom: `${SECTION_TITLE_GAP}px` },
  row: {
    display: 'flex',
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalM,
  },
  // `<Button appearance="transparent">` por baixo — zera o tamanho/padding
  // padrão dele (feito pra rótulo de botão normal, não pra um título grande)
  // e mantém só o hover/focus nativos do Fluent.
  titleBtn: {
    minWidth: 'auto !important',
    height: 'auto !important',
    padding: '0 !important',
    columnGap: '4px !important',
    margin: 0,
    color: 'inherit',
    borderRadius: tokens.borderRadiusMedium,
    outlineOffset: '6px',
  },
  titlePlain: { cursor: 'default' },
  title: {
    // Título de seção do app Xbox ("Voltar a jogar"): 24 epx, semibold.
    fontSize: `${SECTION_TITLE}px`,
    fontWeight: tokens.fontWeightSemibold,
    lineHeight: 1.15,
    // `<Text as="h2">`: sem a margem padrão do navegador (0,83em).
    margin: 0,
  },
  chevron: {
    // Um degrau abaixo do título ao lado.
    fontSize: '20px',
    color: tokens.colorNeutralForeground3,
    transitionProperty: 'transform, color',
    transitionDuration: '150ms',
    transitionTimingFunction: tokens.curveEasyEase,
    // Descendente, não filho direto — o ícone do `<Button icon=.../>` fica
    // dentro de um `span.fui-Button__icon`, não direto no `<button>`.
    'button:hover &, button:focus-visible &': {
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
  const { t } = useTranslation()
  const s = useStyles()
  return (
    <div className={s.root}>
      <div className={s.row}>
        {onSeeAll ? (
          <Button
            appearance="transparent"
            className={s.titleBtn}
            onClick={onSeeAll}
            aria-label={seeAllLabel ?? t('shell2.seeAll', { title })}
            icon={<ChevronRightRegular className={s.chevron} />}
            iconPosition="after"
          >
            <Text as={as} className={s.title}>
              {title}
            </Text>
          </Button>
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
