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

  // Largura de card que preenche a linha de ponta a ponta (até a borda
  // direita do container — mesma borda onde termina a topbar/relógio).
  // Baseada em `cap` (quantos cards CABEM na largura disponível, igual em
  // qualquer prateleira da página — só depende da largura/viewport, não do
  // conteúdo), NÃO em `renderedCount` (quantos ESTA prateleira de fato
  // mostra). Já tentamos basear em `renderedCount`: cada prateleira acabava
  // com um tamanho de card diferente da vizinha (uma com 3 jogos "enchia" a
  // linha esticando pra um tamanho, outra com 6 pra outro), e no extremo —
  // 1 jogo só — o card sozinho virava do tamanho da prateleira INTEIRA.
  // Usar `cap` fixa o mesmo tamanho pra toda prateleira da tela, cheia ou
  // não; uma prateleira com poucos itens só deixa espaço vazio à direita
  // em vez de esticar os poucos cards que tem — mesmo comportamento do
  // "Continuar jogando" antes de qualquer um desses ajustes.
  const cardPx = shelfWidth > 0 ? shelfFillWidth(shelfWidth, viewport, cap) : 0;

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
