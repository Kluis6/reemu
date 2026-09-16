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
export const CARD_MIN = 150;
export const CARD_MAX = 445;
/** ~1/8.5 da largura da janela → mantém ~8 cards por prateleira em qualquer
 *  tela; o card cresce até 445px em 4K real (3840px) — teto anterior (320)
 *  travava em ~2.7K e já ficava pequeno em 4K de verdade. */
export const CARD_VW = 0.116;
export const SHELF_GAP = 16;
/** Padding horizontal do `.shelf` (cada lado) — folga pro anel de foco do
 *  1º/último card não ser cortado pelo `overflow` do scroller (ver
 *  `useShelfStyles.shelf`/`.wrap` em `styles/xbox.ts`). `clientWidth` do
 *  `.shelf` inclui esse padding; subtraído antes de calcular capacidade/
 *  preenchimento, senão a conta "acha" 2×isto de espaço a mais pros cards
 *  do que realmente existe — sobrando um resto na borda direita.
 */
export const SHELF_PAD = 10;

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

/**
 * Largura de card que faz os `cap` cards (já calculados por `shelfCapacity`)
 * + seus gaps preencherem `shelfWidth` de ponta a ponta — em vez de deixar a
 * folga que sobra do `floor()` como espaço morto na borda direita da
 * prateleira (o que descolava a última coluna de card do fim da topbar/
 * relógio). Só estica pra cima do nominal (nunca encolhe abaixo de
 * `cardWidthPx`) — SEM teto em `CARD_MAX`: perto de 4K o nominal já bate
 * nesse teto, e limitar aí reabriria exatamente a folga que isto existe pra
 * fechar (medido: ~240px de vão numa prateleira de 7 cards em 3840px). O
 * alinhamento com a borda tem prioridade sobre o teto de tamanho antigo.
 */
export function shelfFillWidth(
  shelfWidth: number,
  viewport: number,
  cap: number,
): number {
  const nominal = cardWidthPx(viewport);
  if (cap <= 0) return nominal;
  const filled = (shelfWidth - (cap - 1) * SHELF_GAP) / cap;
  // `ceil`, não `round`: o resto do arredondamento de cada card some no
  // scroller (`.shelf` já rola no X se preciso), então prefere sobrar
  // 1 px pra dentro (ou nada) a faltar alguns px pra fora — é o lado que
  // mantém a última borda alinhada com o fim da topbar/relógio.
  return Math.ceil(Math.max(nominal, filled));
}
