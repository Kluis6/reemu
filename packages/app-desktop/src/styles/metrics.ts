/**
 * Métricas do "modo Xbox", em epx (pixel efetivo). Com a escala de
 * lib/uiScale.ts, 1 px do CSS = 1 epx e a tela lógica tem pelo menos
 * 1366×768: a mesma em que o app Xbox do Windows desenha 1 px = 1 px. Por
 * isso os valores aqui são fixos: crescem com a tela pelo zoom, nunca por
 * `vw`.
 *
 * Os números saem das capturas do app Xbox do Windows em 1366×768 (início,
 * Game Pass e menu do perfil, 2026-10-02) — ver
 * docs/design/xbox-mode-reference.md §Escala. Das regras de TV da Microsoft
 * ("Designing for Xbox and TV") ficam o alvo mínimo de 32 epx e os 6 cards
 * por fileira; a área segura e a rampa de texto passam a ser as do app.
 */

/** Margem lateral do conteúdo: 47 epx dos dois lados, como os cards do
 *  Xbox (da rail até o 1º card e do último card até a borda). */
export const SAFE_X = 47;
/** Topo dos controles da topbar (busca, voltar) — y = 32 no Xbox. */
export const SAFE_Y = 32;

/** Altura mínima de um alvo interativo (a busca do Xbox tem 31). */
export const TARGET_MIN = 32;

/** Ícone padrão do Fluent/Windows. */
export const ICON = 20;

/** Rail de navegação: 68 epx; o item ativo é um retângulo de 50×46 e os
 *  itens se repetem a cada 56 (46 + 10 de espaço). Ícones de 24. */
export const RAIL_W = 68;
export const RAIL_ITEM_W = 50;
export const RAIL_ITEM_H = 46;
export const RAIL_GAP = 10;
export const RAIL_ICON = 24;
/** Avatar do perfil no topo da rail (centro em y = 31). */
export const RAIL_TOP = 15;

/** Topbar flutuante: controles de 32 epx a partir de y = 32. */
export const TOPBAR_TOP = SAFE_Y;
/** Folga que a área de rolagem reserva pra topbar: topo + controle +
 *  respiro. */
export const TOPBAR_CLEARANCE = TOPBAR_TOP + TARGET_MIN + 16;
/** A topbar é mais larga que o conteúdo: o "voltar" começa 17 epx depois
 *  da rail; à direita, como no modo XBOX do app (com relógio), o texto do
 *  relógio termina a ~18 da borda (print a 1366×768: 1348) — fora do modo
 *  XBOX, sem relógio, o último botão do Xbox termina a 36. */
export const TOPBAR_PAD_L = 17;
export const TOPBAR_PAD_R = 13;
/** Largura da busca, centralizada na área à direita da rail. */
export const SEARCH_W = 500;

/** Padding lateral do conteúdo (a barra de rolagem fica dentro do da
 *  direita). */
export const PAGE_PAD_L = SAFE_X;
export const PAGE_PAD_R = SAFE_X;

/** Barra de rolagem da área de conteúdo. */
export const SCROLLBAR_W = 6;

/** Folga embaixo pra barra de dicas (margem + dica + respiro). */
export const PAGE_PAD_B = SAFE_Y + TARGET_MIN + 24;

/** Espaço entre cards de jogo. */
export const CARD_GAP = 20;

/** Largura nominal de um card: 6 por fileira na tela de referência (184
 *  epx, como no Xbox). Conteúdo = 1366 − rail − paddings = 1204. O `- 1` é
 *  folga pro arredondamento do zoom: a prateleira estica o card até
 *  encher a linha (lib/shelf.ts), então ele volta a 184. */
export const CARD_W =
  Math.floor((1366 - RAIL_W - PAGE_PAD_L - PAGE_PAD_R - 5 * CARD_GAP) / 6) - 1;

/** Cabeçalho de prateleira, medido num print do app Xbox na tela de
 *  referência (1366×768, "Jogos principais pagos"), com a largura do texto
 *  conferida contra a Segoe UI Variable:
 *  - título 26 semibold (268 px medidos × 271 calculados), subtítulo 16
 *    regular (377 × 371);
 *  - linha de base do título → do subtítulo: 24; do subtítulo → topo dos
 *    cards: ~27; fim dos cards → linha de base do próximo título: 91.
 *  Com as métricas da Segoe UI (ascendente 1,079 em, descendente 0,251 em),
 *  as alturas de linha e margens abaixo reproduzem essas distâncias. */
export const SECTION_TITLE = 26;
export const SECTION_TITLE_LINE = 32;
export const SECTION_SUB = 16;
export const SECTION_SUB_LINE = 20;
/** Do título ao subtítulo (caixa a caixa). */
export const SECTION_SUB_GAP = 2;
/** Do fim do cabeçalho ao topo dos cards. */
export const SECTION_TITLE_GAP = 24;
/** Do fim de uma fileira ao topo do próximo cabeçalho. */
export const SECTION_GAP = 64;

/** Raios (escala do Fluent). */
export const RADIUS = 8;
export const RADIUS_LG = 12;
