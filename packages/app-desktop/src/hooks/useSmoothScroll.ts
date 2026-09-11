import { useEffect, type RefObject } from "react";

/**
 * Rolagem suave estilo console no container de conteúdo: intercepta o `wheel`
 * e faz o `scrollTop` perseguir um alvo com easing (lerp por rAF). Trackpad e
 * arrastar a barra continuam nativos — o alvo ressincroniza quando algo
 * externo mexe no scroll. Respeita `prefers-reduced-motion`.
 */
export function useSmoothScroll(ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    let target = el.scrollTop;
    let raf = 0;
    let running = false;

    const tick = () => {
      const cur = el.scrollTop;
      const diff = target - cur;
      if (Math.abs(diff) < 0.5) {
        el.scrollTop = target;
        running = false;
        return;
      }
      el.scrollTop = cur + diff * 0.18;
      raf = requestAnimationFrame(tick);
    };

    const onWheel = (e: WheelEvent) => {
      if (e.ctrlKey) return; // zoom
      const max = el.scrollHeight - el.clientHeight;
      if (max <= 0) return;
      // gesto novo (não estamos animando) → parte de onde o scroll está agora
      if (!running) target = el.scrollTop;
      const unit =
        e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? el.clientHeight : 1;
      const next = target + e.deltaY * unit;
      // deixa o overscroll no topo/base ir pro navegador (ex.: gesto de voltar)
      if ((next < 0 && target <= 0) || (next > max && target >= max)) return;
      e.preventDefault();
      target = Math.max(0, Math.min(max, next));
      if (!running) {
        running = true;
        raf = requestAnimationFrame(tick);
      }
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      el.removeEventListener("wheel", onWheel);
      cancelAnimationFrame(raf);
    };
  }, [ref]);
}
