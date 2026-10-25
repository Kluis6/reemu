import { makeStyles, tokens } from '@fluentui/react-components'

/** Contorno de campo/botão que passa de 3:1 contra a superfície em todos os
 *  temas — a mesma cor da borda dos campos no `index.css` (`reemuFieldStroke`
 *  no alto contraste). */
const STROKE = `var(--reemuFieldStroke, color-mix(in srgb, ${tokens.colorNeutralForeground1} 50%, transparent))`

/**
 * Botões dos modais no padrão do ReEmu. Sobre a superfície de um Dialog o
 * `secondary` do Fluent tem o mesmo fundo dela e quase some (Cancelar,
 * Voltar, Fechar — prints do usuário); com o preenchimento neutro `Background3`
 * e o contorno visível ele aparece como os botões das páginas. O primário
 * continua o do Fluent.
 */
export const useModalButtonStyles = makeStyles({
  secondary: {
    backgroundColor: tokens.colorNeutralBackground3,
    // longhands: o atalho `borderColor` do Griffel some sem aviso
    borderTopColor: STROKE,
    borderRightColor: STROKE,
    borderBottomColor: STROKE,
    borderLeftColor: STROKE,
    ':hover': { backgroundColor: tokens.colorNeutralBackground3Hover },
    ':hover:active': { backgroundColor: tokens.colorNeutralBackground3Pressed },
  },
})
