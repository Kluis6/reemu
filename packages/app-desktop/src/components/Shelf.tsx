import { mergeClasses } from "@fluentui/react-components";
import { type ReactNode } from "react";
import { useShelfStyles } from "../styles/xbox";

/**
 * Prateleira horizontal estilo Xbox (scroll-snap + chevron de "próxima
 * página"). Usada nas faixas curadas da Início ("Continuar jogando",
 * "Adicionados recentemente"…).
 */
export function Shelf({
  children,
  start,
}: {
  children: ReactNode;
  /** Alinha os cards à esquerda (poucos itens) em vez de espalhar. */
  start?: boolean;
}) {
  const s = useShelfStyles();
  return (
    <div className={s.wrap}>
      <div className={mergeClasses(s.shelf, start && s.shelfStart)}>
        {children}
      </div>
    </div>
  );
}
