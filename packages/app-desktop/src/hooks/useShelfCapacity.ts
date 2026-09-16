import { useEffect, useRef, useState } from "react";
import { SHELF_PAD } from "../lib/shelf";

/**
 * Observa a largura de um elemento (a prateleira) e devolve em px, ao vivo
 * (`ResizeObserver` + resize da janela) — já descontado o padding próprio do
 * `.shelf` (`SHELF_PAD` de cada lado, ver `lib/shelf.ts`), que `clientWidth`
 * inclui mas não é espaço disponível pros cards. Só a largura — quantos
 * cards cabem e a largura que cada um deve ter pra encher a linha dependem
 * de quantos itens o `Shelf` REALMENTE vai renderizar (depois de aplicar
 * corte/`more`), que só ele sabe; por isso essa conta mora em `Shelf.tsx`,
 * não aqui (ver `lib/shelf.ts::shelfCapacity`/`shelfFillWidth`).
 */
export function useShelfCapacity(): readonly [
  React.RefObject<HTMLDivElement | null>,
  number,
] {
  const ref = useRef<HTMLDivElement | null>(null);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () =>
      setWidth(Math.max(0, el.clientWidth - SHELF_PAD * 2));
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, []);

  return [ref, width] as const;
}
