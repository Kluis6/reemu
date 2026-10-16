import { Button, Text, Tooltip, makeStyles, tokens } from '@fluentui/react-components'
import { ChevronRightRegular } from '@fluentui/react-icons'
import type { ReactNode } from 'react'
import {
  ICON,
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
    // Do fim do título à caixa do botão: 18 epx, como no app Xbox (print a
    // 3840×2160, "Jogar com mouse e teclado": 57 px a 3,07 px por epx).
    columnGap: '18px',
  },
  title: {
    // Título de seção do app Xbox (medido, ver `metrics.ts`): 26, semibold.
    // Só texto: quem abre a prateleira é o botão ao lado (`seeAll`).
    fontSize: `${SECTION_TITLE}px`,
    // título (26 px): Segoe UI Variable Display, como na rampa do Windows 11
    fontFamily: "var(--reemuFontDisplay)",
    fontWeight: tokens.fontWeightSemibold,
    lineHeight: `${SECTION_TITLE_LINE}px`,
    // `<Text as="h2">`: sem a margem padrão do navegador (0,83em).
    margin: 0,
  },
  // Botão quadrado só com o chevron, logo depois do título, como no app
  // Xbox ("Jogar com mouse e teclado  [>]", dica "Mostrar tudo"): caixa de
  // 24×24 epx com o `ChevronRight` de 20 (traço de 6×12 epx, o mesmo do
  // Xbox), medidos no print a 3840×2160.
  seeAll: {
    // A linha centraliza as caixas, mas o meio das maiúsculas do título fica
    // ~1,7 px abaixo do meio da linha de 32 (Segoe UI: ascendente 1,079 em,
    // maiúsculas 0,7 em). No print do Xbox o centro do chevron coincide com
    // o das maiúsculas — desce 2 px.
    position: 'relative',
    top: '2px',
    minWidth: '24px !important',
    maxWidth: '24px !important',
    width: '24px',
    height: '24px',
    padding: '0 !important',
    // `appearance="subtle"`: sem fundo em repouso (como no Xbox). Hover e
    // foco (teclado/controle) com o MESMO fundo dos outros botões do tema
    // (voltar, tela cheia, filtros: `colorNeutralBackground2` no hover/foco,
    // `…Pressed` ao clicar) — pedido do usuário.
    '&:hover, &:focus, &:focus-visible': {
      backgroundColor: `${tokens.colorNeutralBackground2Hover} !important`,
      color: `${tokens.colorNeutralForeground1} !important`,
    },
    ':active': {
      backgroundColor: `${tokens.colorNeutralBackground2Pressed} !important`,
    },
    // O slot de ícone do <Button> é fixo em 20 no tamanho medium; garante.
    '& .fui-Button__icon': { fontSize: `${ICON}px`, width: `${ICON}px`, height: `${ICON}px` },
  },
  seeAllIcon: { fontSize: `${ICON}px` },
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
              appearance="subtle"
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
