import {
  Children,
  isValidElement,
  type CSSProperties,
  type ReactNode,
} from "react";
import { useShelfCapacity } from "../hooks/useShelfCapacity";
import { shelfCapacity, shelfFillWidth } from "../lib/shelf";
import { useShelfStyles } from "../styles/xbox";

/**
 * Prateleira horizontal estilo Xbox. Mede a própria largura e mostra só os
 * cards que cabem numa linha (mais o `more` no fim, quando a lista foi
 * cortada). Sem número fixo: enche a linha em qualquer tela. Se ainda assim
 * transbordar (janela minúscula), rola no eixo X.
 */
export function Shelf({
  children,
  more,
  fill = true,
}: {
  children: ReactNode;
  /** Célula fixa no fim (ex.: card "ver todos"). Só aparece se a lista coube
   *  cortada; ocupa uma vaga da capacidade. */
  more?: ReactNode;
  /** `false` = mostra todos os filhos (não corta pra caber). */
  fill?: boolean;
}) {
  const s = useShelfStyles();
  const [ref, shelfWidth] = useShelfCapacity();

  const items = Children.toArray(children).filter(isValidElement);
  const viewport = typeof window !== "undefined" ? window.innerWidth : 1920;
  const cap =
    shelfWidth > 0 ? shelfCapacity(shelfWidth, viewport) : items.length || 1;
  const room = more ? Math.max(1, cap - 1) : cap;
  const truncated = fill && items.length > room;
  const shown = truncated ? items.slice(0, room) : items;
  const tail = truncated ? more : null;
  const renderedCount = shown.length + (tail ? 1 : 0);

  // Largura de card que preenche a linha de ponta a ponta (até a borda
  // direita do container — mesma borda onde termina a topbar/relógio), com
  // base em quantos cards ESTA linha vai renderizar de verdade (não a
  // capacidade bruta da largura — uma linha com menos itens que `cap`, ex.
  // "Continuar jogando" com só 3 jogos, não deve esticar os cards pra
  // preencher 8 vagas vazias). Só estica quando a linha cabe inteira
  // (`renderedCount <= cap`); se `fill={false}` deixou mais itens do que
  // cabem, a linha rola no eixo X e esticar só pioraria — cai no `clamp()`
  // estático (fallback da CSS var abaixo).
  const cardPx =
    shelfWidth > 0 && renderedCount > 0 && renderedCount <= cap
      ? shelfFillWidth(shelfWidth, viewport, renderedCount)
      : 0;

  const style: CSSProperties | undefined =
    cardPx > 0
      ? ({ ["--reemuCardW" as string]: `${cardPx}px` } as CSSProperties)
      : undefined;

  return (
    <div className={s.wrap}>
      <div ref={ref} className={s.shelf} style={style}>
        {shown}
        {tail}
      </div>
    </div>
  );
}
