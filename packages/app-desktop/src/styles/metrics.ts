/**
 * Métricas do "modo Xbox", em epx (pixel efetivo). Com a escala de
 * lib/uiScale.ts, 1 px do CSS = 1 epx e a tela lógica tem pelo menos
 * 960×540 — a mesma do Xbox (1080p a 200%). Por isso os valores aqui são
 * fixos: crescem com a tela pelo zoom, nunca por `vw`.
 *
 * Fonte das regras: Microsoft, "Designing for Xbox and TV" e "Gamepad and
 * remote control interactions" (ver docs/design/xbox-mode-reference.md §Escala).
 * - Área segura da TV: 48 epx nas laterais, 27 epx em cima e embaixo.
 * - Alvo interativo: no mínimo 32 epx de altura.
 * - Texto: 15 epx o principal, 12 epx o secundário (tokens do tema).
 * - De uma borda a outra, no máximo 6 cliques — daí os 6 cards por fileira
 *   numa tela 16:9.
 */

/** Área segura da TV (overscan). */
export const SAFE_X = 48;
export const SAFE_Y = 27;

/** Altura mínima de um alvo interativo. */
export const TARGET_MIN = 32;

/** Ícone padrão do Fluent/Windows. */
export const ICON = 20;

/** Rail de navegação (ícones). Fundo vai até a borda; os itens têm 40 epx
 *  (acima do mínimo de 32) e o conteúdo começa depois dela. */
export const RAIL_W = 64;
export const RAIL_ITEM = 40;

/** Topbar flutuante: começa na área segura de cima, controles de 32 epx. */
export const TOPBAR_TOP = SAFE_Y;
/** Folga que a área de rolagem reserva pra topbar: topo seguro + controle
 *  + respiro. */
export const TOPBAR_CLEARANCE = TOPBAR_TOP + TARGET_MIN + 16;

/** Padding lateral do conteúdo: à esquerda, a rail já afasta da borda
 *  (64 + 24 = 88 ≥ 48); à direita, a área segura. */
export const PAGE_PAD_L = 24;
export const PAGE_PAD_R = SAFE_X;

/** Barra de rolagem da área de conteúdo. */
export const SCROLLBAR_W = 6;

/** Folga embaixo pra barra de dicas (área segura + dica + respiro). */
export const PAGE_PAD_B = SAFE_Y + TARGET_MIN + 24;

/** Espaço entre cards de jogo. */
export const CARD_GAP = 12;

/** Largura nominal de um card: 6 por fileira numa tela 16:9 (regra dos 6
 *  cliques). Conteúdo = 960 − rail − paddings − barra de rolagem. */
export const CARD_W = Math.floor(
  (960 - RAIL_W - PAGE_PAD_L - PAGE_PAD_R - SCROLLBAR_W - 5 * CARD_GAP) / 6,
);

/** Raios (escala do Fluent). */
export const RADIUS = 8;
export const RADIUS_LG = 12;
