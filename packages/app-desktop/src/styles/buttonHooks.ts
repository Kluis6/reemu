import type { CSSProperties } from 'react'

/**
 * Aparência única dos botões do ReEmu (pedido do usuário, 2026-11-14: "todos
 * os botões do app" como os da página do jogo — preenchidos na cor do tema,
 * sem borda, em qualquer tema e modo; o tamanho de cada um continua o dele).
 *
 * Vai pelo gancho oficial do Fluent pra estilizar componentes no app inteiro
 * (`FluentProvider` › `customStyleHooks_unstable`, que o `Button` chama depois
 * dos estilos dele — `useCustomStyleHook_unstable('useButtonStyles_unstable')`
 * em `@fluentui/react-button`). Em vez de pôr classe (que venceria as classes
 * próprias de cada tela), o gancho só redireciona, NO botão, as variáveis que
 * o `secondary` do Fluent usa: o fundo `colorNeutralBackground1*` passa a ser o
 * preenchimento translúcido `reemuControlFill*` (`liftTokens`, styles/themes.ts:
 * sempre um passo acima do fundo atrás, inclusive dentro de uma linha) e a borda
 * `colorNeutralStroke1*` fica transparente. Quem já define o próprio fundo
 * (ex.: o "Remover" das linhas, no `Background3`) continua como está.
 * `subtle`, `transparent`, `outline` e `primary` ficam como o Fluent desenha.
 */
const SECONDARY_VARS = {
  '--colorNeutralBackground1': 'var(--reemuControlFill)',
  '--colorNeutralBackground1Hover': 'var(--reemuControlFillHover)',
  '--colorNeutralBackground1Pressed': 'var(--reemuControlFillPressed)',
  '--colorNeutralStroke1': 'transparent',
  '--colorNeutralStroke1Hover': 'transparent',
  '--colorNeutralStroke1Pressed': 'transparent',
} as CSSProperties

type ButtonLikeState = {
  appearance?: string
  root: { style?: CSSProperties }
}

function filledSecondary(state: unknown) {
  const s = state as ButtonLikeState
  if (s.appearance !== 'secondary') return
  s.root.style = { ...SECONDARY_VARS, ...s.root.style }
}

export const BUTTON_STYLE_HOOKS = {
  useButtonStyles_unstable: filledSecondary,
  useToggleButtonStyles_unstable: filledSecondary,
  useMenuButtonStyles_unstable: filledSecondary,
}
