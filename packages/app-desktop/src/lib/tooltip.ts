import type { PositioningShorthand } from '@fluentui/react-components'

/**
 * Posição de todos os tooltips do app (pedido do usuário, 2026-10-29): à
 * direita do componente, centralizado na altura dele e um pouco afastado.
 * `after` = depois do alvo no eixo horizontal; sem espaço (botões na borda
 * direita da tela) o posicionamento do Fluent vira pro outro lado sozinho.
 */
export const TOOLTIP_POSITION: PositioningShorthand = {
  position: 'after',
  align: 'center',
  offset: 10,
}
