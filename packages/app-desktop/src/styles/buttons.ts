import { makeStyles, tokens } from '@fluentui/react-components'

/** Contorno de campo/botão que passa de 3:1 contra a superfície em todos os
 *  temas — a mesma cor da borda dos campos no `index.css` (`reemuFieldStroke`
 *  no alto contraste). */
export const STROKE = `var(--reemuFieldStroke, color-mix(in srgb, ${tokens.colorNeutralForeground1} 50%, transparent))`

/**
 * Botões dos modais no padrão do ReEmu. Sobre a superfície de um Dialog o
 * `secondary` do Fluent tem o mesmo fundo dela e quase some (Cancelar,
 * Voltar, Fechar — prints do usuário); com o preenchimento neutro `Background3`
 * e o contorno visível ele aparece como os botões das páginas. O primário
 * continua o do Fluent.
 */
export const useModalButtonStyles = makeStyles({
  // Mesmo fundo dos seletores de core (`SettingBlock`), sem cor na borda.
  block: {
    backgroundColor: tokens.colorNeutralBackground2,
    borderTopColor: "transparent",
    borderRightColor: "transparent",
    borderBottomColor: "transparent",
    borderLeftColor: "transparent",
    ":hover": {
      backgroundColor: tokens.colorNeutralBackground2Hover,
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
    ":hover:active": {
      backgroundColor: tokens.colorNeutralBackground2Pressed,
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
  },
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
  // O mesmo preenchimento, sem contorno (Gerenciar biblioteca, pedido do
  // usuário, 2026-11-06). Borda de 1 px transparente, não `none`: o tamanho
  // não muda e o foco continua no anel de fora (index.css).
  secondaryFlat: {
    backgroundColor: tokens.colorNeutralBackground3,
    borderTopColor: "transparent",
    borderRightColor: "transparent",
    borderBottomColor: "transparent",
    borderLeftColor: "transparent",
    ':hover': {
      backgroundColor: tokens.colorNeutralBackground3Hover,
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
    ':hover:active': {
      backgroundColor: tokens.colorNeutralBackground3Pressed,
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
  },
})
