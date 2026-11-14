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
  // Preenchimento de controle (`reemuControlFill*`, translúcido: sempre um
  // passo acima do fundo atrás — 2026-11-14), sem cor na borda; o mesmo dos
  // seletores (`SettingBlock`).
  block: {
    backgroundColor: "var(--reemuControlFill)",
    borderTopColor: "transparent",
    borderRightColor: "transparent",
    borderBottomColor: "transparent",
    borderLeftColor: "transparent",
    ":hover": {
      backgroundColor: "var(--reemuControlFillHover)",
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
    ":hover:active": {
      backgroundColor: "var(--reemuControlFillPressed)",
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
  },
  // Sem contorno desde 2026-11-14 (todos os botões preenchidos e sem borda,
  // `buttonHooks.ts`), no preenchimento translúcido de controle.
  secondary: {
    backgroundColor: "var(--reemuControlFill)",
    // longhands: o atalho `borderColor` do Griffel some sem aviso
    borderTopColor: "transparent",
    borderRightColor: "transparent",
    borderBottomColor: "transparent",
    borderLeftColor: "transparent",
    ':hover': { backgroundColor: "var(--reemuControlFillHover)" },
    ':hover:active': { backgroundColor: "var(--reemuControlFillPressed)" },
  },
  // O mesmo preenchimento, sem contorno (Gerenciar biblioteca, pedido do
  // usuário, 2026-11-06). Borda de 1 px transparente, não `none`: o tamanho
  // não muda e o foco continua no anel de fora (index.css).
  secondaryFlat: {
    backgroundColor: "var(--reemuControlFill)",
    borderTopColor: "transparent",
    borderRightColor: "transparent",
    borderBottomColor: "transparent",
    borderLeftColor: "transparent",
    ':hover': {
      backgroundColor: "var(--reemuControlFillHover)",
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
    ':hover:active': {
      backgroundColor: "var(--reemuControlFillPressed)",
      borderTopColor: "transparent",
      borderRightColor: "transparent",
      borderBottomColor: "transparent",
      borderLeftColor: "transparent",
    },
  },
})
