import { useEffect, useRef, useState } from "react";
import { shelfCapacity } from "../lib/shelf";

/**
 * Observa a largura de um elemento e devolve quantos cards de jogo cabem nele
 * agora. A prateleira usa isto pra renderizar só o suficiente pra encher a
 * linha — em qualquer breakpoint, sem número fixo.
 */
export function useShelfCapacity(): readonly [
  React.RefObject<HTMLDivElement | null>,
  number,
] {
  const ref = useRef<HTMLDivElement | null>(null);
  const [cap, setCap] = useState(8);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const w = el.clientWidth;
      if (w > 0) setCap(shelfCapacity(w, window.innerWidth));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, []);

  return [ref, cap] as const;
}
