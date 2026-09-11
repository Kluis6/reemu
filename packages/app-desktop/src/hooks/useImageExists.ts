import { useEffect, useState } from "react";

// Cache por sessão — várias instâncias (Splash + AppLogo) checando a mesma
// URL não disparam vários GETs nem "piscam" fora de sincronia.
const cache = new Map<string, boolean>();

/**
 * `true`/`false`/`undefined` (ainda checando) se `src` carrega. Pré-carrega
 * com um `Image()` fora do DOM — evita montar um `<img>` "quebrado" (o
 * ícone de imagem ausente do navegador pisca na tela até o `onError`
 * reagir). Enquanto não sabe, quem chama deve mostrar um fallback.
 */
export function useImageExists(src: string): boolean | undefined {
  const [ok, setOk] = useState<boolean | undefined>(() => cache.get(src));

  useEffect(() => {
    // já resolvido (por este hook ou outra instância) — nada a fazer
    if (cache.has(src)) return;
    let cancelled = false;
    const img = new Image();
    img.onload = () => {
      if (cancelled) return;
      cache.set(src, true);
      setOk(true);
    };
    img.onerror = () => {
      if (cancelled) return;
      cache.set(src, false);
      setOk(false);
    };
    img.src = src;
    return () => {
      cancelled = true;
    };
  }, [src]);

  return ok;
}
