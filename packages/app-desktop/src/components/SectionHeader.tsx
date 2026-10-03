import { Button, Text, Tooltip, makeStyles, tokens } from '@fluentui/react-components'
import { ChevronRightRegular } from '@fluentui/react-icons'
import type { ReactNode } from 'react'
import {
  SECTION_SUB,
  SECTION_SUB_GAP,
  SECTION_SUB_LINE,
  SECTION_TITLE,
  SECTION_TITLE_GAP,
  SECTION_TITLE_LINE,
} from '../styles/metrics'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  // Espaço até os cards, como no app Xbox (metrics.ts).
  root: { marginBottom: `${SECTION_TITLE_GAP}px` },
  row: {
    display: 'flex',
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalM,
  },
  title: {
    // Título de seção do app Xbox (medido, ver `metrics.ts`): 26, semibold.
    // Só texto: quem abre a prateleira é o botão ao lado (`seeAll`).
    fontSize: `${SECTION_TITLE}px`,
    fontWeight: tokens.fontWeightSemibold,
    lineHeight: `${SECTION_TITLE_LINE}px`,
    // `<Text as="h2">`: sem a margem padrão do navegador (0,83em).
    margin: 0,
  },
  // Botão quadrado só com o chevron, logo depois do título, como no app
  // Xbox ("Jogos principais pagos  [>]", dica "Mostrar tudo").
  seeAll: {
    // A linha já centraliza as caixas (`alignItems: center`), mas as letras
    // do título ficam abaixo do meio da caixa de 32: o centro do botão caía
    // 2 px acima do centro das maiúsculas. No app Xbox fica 1 px acima
    // (medido no print, "Jogos principais pagos") — desce 1 px.
    position: 'relative',
    top: '1px',
    minWidth: '24px !important',
    width: '24px',
    height: '24px',
    padding: '0 !important',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: `${tokens.colorNeutralBackground3} !important`,
    border: `1px solid ${tokens.colorNeutralBackground3} !important`,
    color: tokens.colorNeutralForeground2,
    ':hover': {
      backgroundColor: `${tokens.colorNeutralBackground3Hover} !important`,
      border: `1px solid ${tokens.colorNeutralBackground3Hover} !important`,
      color: tokens.colorNeutralForeground1,
    },
    ':active': {
      backgroundColor: `${tokens.colorNeutralBackground3Pressed} !important`,
    },
  },
  seeAllIcon: { fontSize: '16px' },
  right: { marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalS },
  sub: {
    marginTop: `${SECTION_SUB_GAP}px`,
    fontSize: `${SECTION_SUB}px`,
    lineHeight: `${SECTION_SUB_LINE}px`,
    color: tokens.colorNeutralForeground3,
  },
})

/**
 * Cabeçalho de seção no modelo modo XBOX: título grande (só texto) e, com
 * `onSeeAll`, um botão quadrado com chevron `>` logo depois (dica "Mostrar
 * tudo"); uma linha de subtítulo embaixo, e um slot opcional à direita
 * (contagem etc.).
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
        <Text as={as} className={s.title}>
          {title}
        </Text>
        {onSeeAll && (
          <Tooltip content={t('shell2.showAll')} relationship="description">
            <Button
              appearance="secondary"
              className={s.seeAll}
              onClick={onSeeAll}
              aria-label={seeAllLabel ?? t('shell2.seeAll', { title })}
              icon={<ChevronRightRegular className={s.seeAllIcon} />}
            />
          </Tooltip>
        )}
        {right && <div className={s.right}>{right}</div>}
      </div>
      {subtitle && <div className={s.sub}>{subtitle}</div>}
    </div>
  )
}
