/**
 * Regras de dimensão da prateleira/grade de jogos — compartilhadas entre o CSS
 * (Griffel, em `styles/xbox.ts`) e o JS que decide QUANTOS cards renderizar pra
 * encher a linha (`hooks/useShelfCapacity`). Nada de número fixo de cards: a
 * prateleira puxa o suficiente pra preencher a largura disponível em qualquer
 * tela (de janela estreita a 4K).
 *
 * Um card: largura fluida entre MIN e MAX, escalando com a largura da janela —
 * a MESMA fórmula nos dois lados (o `clamp()` do CSS e o `cardWidthPx` do JS).
 */
export const CARD_MIN = 148;
export const CARD_MAX = 248;
export const CARD_VW = 0.132;
export const SHELF_GAP = 14;

/** `clamp()` pronto pro `width` / `grid-template-columns`. */
export const cardSizeCss = `clamp(${CARD_MIN}px, ${CARD_VW * 100}vw, ${CARD_MAX}px)`;

/** Largura resolvida de um card, em px, pra uma largura de viewport. */
export function cardWidthPx(viewport: number): number {
  return Math.round(Math.min(CARD_MAX, Math.max(CARD_MIN, viewport * CARD_VW)));
}

/** Quantos cards cabem numa prateleira de `shelfWidth` px (mínimo 1). */
export function shelfCapacity(shelfWidth: number, viewport: number): number {
  const card = cardWidthPx(viewport);
  return Math.max(1, Math.floor((shelfWidth + SHELF_GAP) / (card + SHELF_GAP)));
}
