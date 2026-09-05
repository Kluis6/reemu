import { type ReactNode } from "react";
import { useShelfStyles } from "../styles/xbox";

/**
 * Prateleira horizontal estilo Xbox (scroll-snap + chevron de "próxima
 * página"). Usada nas faixas curadas da Início ("Continuar jogando",
 * "Adicionados recentemente"…).
 */
export function Shelf({ children }: { children: ReactNode }) {
  const s = useShelfStyles();
  return (
    <div className={s.wrap}>
      <div className={s.shelf}>{children}</div>
    </div>
  );
}
