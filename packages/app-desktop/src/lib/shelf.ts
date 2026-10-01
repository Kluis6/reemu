/**
 * Regras de dimensão da prateleira/grade de jogos — compartilhadas entre o CSS
 * (Griffel, em `styles/xbox.ts`) e o JS que decide QUANTOS cards renderizar pra
 * encher a linha (`hooks/useShelfCapacity`).
 *
 * O card tem largura fixa em epx (`CARD_W`, ver styles/metrics.ts): 6 por
 * fileira numa tela 16:9, a regra dos "no máximo 6 cliques de borda a borda"
 * do guia de TV da Microsoft. Quem cresce com a resolução é o zoom da tela
 * inteira (lib/uiScale.ts); numa tela mais larga que 16:9 cabem mais cards.
 */
import { CARD_GAP, CARD_W } from "../styles/metrics";

export const SHELF_GAP = CARD_GAP;
/** Padding horizontal do `.shelf` (cada lado) — folga pro anel de foco do
 *  1º/último card não ser cortado pelo `overflow` do scroller (ver
 *  `useShelfStyles.shelf`/`.wrap` em `styles/xbox.ts`). `clientWidth` do
 *  `.shelf` inclui esse padding; subtraído antes de calcular capacidade/
 *  preenchimento, senão a conta "acha" 2×isto de espaço a mais pros cards
 *  do que realmente existe — sobrando um resto na borda direita.
 */
export const SHELF_PAD = 8;

/** Largura pronta pro `width` / `grid-template-columns`. */
export const cardSizeCss = `${CARD_W}px`;

/** Quantos cards cabem numa prateleira de `shelfWidth` epx (mínimo 1). */
export function shelfCapacity(shelfWidth: number): number {
  return Math.max(1, Math.floor((shelfWidth + SHELF_GAP) / (CARD_W + SHELF_GAP)));
}

/**
 * Largura de card que faz os `cap` cards (já calculados por `shelfCapacity`)
 * + seus gaps preencherem `shelfWidth` de ponta a ponta — em vez de deixar a
 * folga que sobra do `floor()` como espaço morto na borda direita da
 * prateleira (o que descolava a última coluna de card do fim da topbar/
 * relógio). Só estica pra cima do nominal (nunca encolhe abaixo de `CARD_W`).
 */
export function shelfFillWidth(shelfWidth: number, cap: number): number {
  if (cap <= 0) return CARD_W;
  const filled = (shelfWidth - (cap - 1) * SHELF_GAP) / cap;
  // Largura fracionária, truncada em centésimos: a fila NUNCA passa da
  // prateleira. O `Math.ceil` de antes estourava até 1 px por card — a
  // prateleira virava rolável e, a cada foco pelo controle, o
  // `scrollIntoView` deslocava a fila inteira pra um lado e pro outro
  // (visto no Windows, 2026-09-25). Sobra no máximo ~0,01 px por card.
  return Math.floor(Math.max(CARD_W, filled) * 100) / 100;
}
