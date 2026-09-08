import { Children, isValidElement, type ReactNode } from "react";
import { useShelfCapacity } from "../hooks/useShelfCapacity";
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
  const [ref, cap] = useShelfCapacity();

  const items = Children.toArray(children).filter(isValidElement);
  const room = more ? Math.max(1, cap - 1) : cap;
  const truncated = fill && items.length > room;
  const shown = truncated ? items.slice(0, room) : items;
  const tail = truncated ? more : null;

  return (
    <div className={s.wrap}>
      <div ref={ref} className={s.shelf}>
        {shown}
        {tail}
      </div>
    </div>
  );
}
