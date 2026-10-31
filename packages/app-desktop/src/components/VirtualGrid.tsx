import {
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type Ref,
} from "react";
import { NAV_CONTENT } from "../lib/focusNav";
import { SHELF_GAP } from "../lib/shelf";
import { VIRTUAL_MIN_ITEMS, gridColumns, gridWindow } from "../lib/virtualGrid";
import { CARD_W, PAGE_TOP } from "../styles/metrics";

export interface VirtualGridHandle {
  /** Rola até o item e põe o foco nele. */
  focusIndex: (index: number) => void;
}

/** Os cards da grade: entre eles o Fluent põe guardas de foco (`<i>`), sem tamanho. */
const cardsOf = (grid: HTMLElement) =>
  grid.querySelectorAll<HTMLElement>(":scope > .fui-Card");

/**
 * Grade de jogos que, em lista grande, só mantém no DOM o que está perto da tela.
 * Com 3.000 jogos, abrir a página levava 1,9 s e cada movimento do foco ~95 ms.
 * O foco geométrico (`focusNav`) continua igual: as fileiras a mais garantem o vizinho.
 */
export function VirtualGrid<T>({
  items,
  render,
  className,
  handle,
}: {
  items: readonly T[];
  render: (item: T) => ReactNode;
  className?: string;
  handle?: Ref<VirtualGridHandle>;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const virtual = items.length >= VIRTUAL_MIN_ITEMS;
  const [view, setView] = useState({ cols: 1, rowHeight: 0, top: 0, height: 0 });

  useLayoutEffect(() => {
    const grid = ref.current;
    const scroller = grid?.closest<HTMLElement>(NAV_CONTENT);
    if (!virtual || !grid || !scroller) return;
    let frame = 0;
    const measure = () => {
      frame = 0;
      const style = getComputedStyle(grid);
      const width = grid.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      const cols = gridColumns(width, CARD_W, SHELF_GAP);
      const cell = cardsOf(grid)[0]?.getBoundingClientRect().height ?? 0;
      const top = scroller.getBoundingClientRect().top - grid.getBoundingClientRect().top;
      setView((v) => {
        const next = {
          cols,
          rowHeight: cell > 0 ? cell + SHELF_GAP : v.rowHeight,
          top: Math.max(0, top),
          height: scroller.clientHeight,
        };
        return next.cols === v.cols &&
          next.rowHeight === v.rowHeight &&
          next.top === v.top &&
          next.height === v.height
          ? v
          : next;
      });
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(measure);
    };
    measure();
    const ro = new ResizeObserver(schedule);
    ro.observe(grid);
    scroller.addEventListener("scroll", schedule, { passive: true });
    return () => {
      ro.disconnect();
      scroller.removeEventListener("scroll", schedule);
      cancelAnimationFrame(frame);
    };
  }, [virtual]);

  // Antes da 1ª medição, só o começo da lista: o suficiente para medir um card.
  const win =
    virtual && view.rowHeight > 0
      ? gridWindow(items.length, view.cols, view.rowHeight, view.top, view.height)
      : virtual
        ? { start: 0, end: Math.min(items.length, 60), padTop: 0, padBottom: 0 }
        : { start: 0, end: items.length, padTop: 0, padBottom: 0 };

  const pending = useRef<number | null>(null);
  const focusPending = () => {
    const index = pending.current;
    const grid = ref.current;
    if (index === null || !grid || index < win.start || index >= win.end) return;
    const target = cardsOf(grid)[index - win.start];
    if (!target) return;
    pending.current = null;
    target.focus({ preventScroll: true });
  };
  useEffect(focusPending);

  useImperativeHandle(handle, () => ({
    focusIndex: (index) => {
      const grid = ref.current;
      const scroller = grid?.closest<HTMLElement>(NAV_CONTENT);
      if (!grid || !scroller) return;
      pending.current = index;
      if (virtual && view.rowHeight > 0) {
        const row = Math.floor(index / view.cols);
        const gridTop =
          grid.getBoundingClientRect().top -
          scroller.getBoundingClientRect().top +
          scroller.scrollTop;
        // abaixo da barra superior, que flutua sobre o conteúdo
        scroller.scrollTop = gridTop + row * view.rowHeight - PAGE_TOP;
      } else {
        cardsOf(grid)[index]?.scrollIntoView({ block: "center" });
      }
      focusPending();
    },
  }));

  return (
    <div
      ref={ref}
      className={className}
      data-virtual-grid={virtual ? "" : undefined}
      style={virtual ? { paddingTop: win.padTop, paddingBottom: win.padBottom } : undefined}
    >
      {items.slice(win.start, win.end).map(render)}
    </div>
  );
}
