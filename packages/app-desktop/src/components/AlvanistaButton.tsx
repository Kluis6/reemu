import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useTranslation } from 'react-i18next'
import symbol from '../assets/alvanista-symbol.svg'

// Cores do botão de login do Alvanista (identidade do parceiro).
const BLUE = '#0094D3'
const BLUE_HOVER = '#005C82'

const useStyles = makeStyles({
  base: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    columnGap: tokens.spacingHorizontalS,
    boxSizing: 'border-box',
    minHeight: '36px',
    paddingTop: tokens.spacingVerticalS,
    paddingBottom: tokens.spacingVerticalS,
    paddingLeft: tokens.spacingHorizontalM,
    paddingRight: tokens.spacingHorizontalL,
    borderRadius: '0.65rem',
    border: `1px solid ${BLUE}`,
    fontFamily: tokens.fontFamilyBase,
    fontSize: tokens.fontSizeBase300,
    fontWeight: tokens.fontWeightSemibold,
    cursor: 'pointer',
    ':disabled': { opacity: 0.55, cursor: 'not-allowed' },
    // Foco de teclado: o anel padrão do ReEmu (3 px a 3 px, na cor de
    // destaque do tema), que o `index.css` só dá aos botões do Fluent; o
    // brilho por fora já vem de lá (regra 4 do foco), e o foco do controle
    // também (regra 2).
    // Propriedades separadas: o atalho `outline` com uma variável CSS dentro
    // não se expande certo no CSSOM do happy-dom (as três partes viravam a
    // variável), e o teste lê o CSS por ele.
    ':focus-visible': {
      outlineWidth: '3px',
      outlineStyle: 'solid',
      outlineColor: tokens.colorBrandStroke1,
      outlineOffset: '3px',
    },
  },
  solid: {
    backgroundColor: BLUE,
    color: '#ffffff',
    ':hover:not(:disabled)': { backgroundColor: BLUE_HOVER, borderTopColor: BLUE_HOVER, borderRightColor: BLUE_HOVER, borderBottomColor: BLUE_HOVER, borderLeftColor: BLUE_HOVER },
  },
  outline: {
    backgroundColor: 'transparent',
    color: BLUE,
    ':hover:not(:disabled)': { backgroundColor: BLUE_HOVER, color: '#ffffff', borderTopColor: BLUE_HOVER, borderRightColor: BLUE_HOVER, borderBottomColor: BLUE_HOVER, borderLeftColor: BLUE_HOVER },
  },
  symbol: { width: '20px', height: '20px', flexShrink: 0 },
})

/**
 * Botão "Entrar com Alvanista": azul `#0094D3` (hover `#005C82`), texto
 * branco, símbolo à esquerda. `variant="outline"` é a versão vazada.
 */
export function AlvanistaButton({
  onClick,
  disabled = false,
  variant = 'solid',
  className,
}: {
  onClick: () => void
  disabled?: boolean
  variant?: 'solid' | 'outline'
  className?: string
}) {
  const { t } = useTranslation()
  const s = useStyles()
  return (
    <button
      type="button"
      className={mergeClasses(s.base, variant === 'solid' ? s.solid : s.outline, className)}
      data-variant={variant}
      disabled={disabled}
      onClick={onClick}
    >
      <img className={s.symbol} src={symbol} alt="" aria-hidden="true" draggable={false} />
      <span>{t('alvanista.signIn')}</span>
    </button>
  )
}
