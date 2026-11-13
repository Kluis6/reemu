import { useEffect, useRef, useState, type MutableRefObject } from "react";
import { createFrameRenderer, type FrameRenderer } from "../../lib/frameRenderer";
import {
  decorationImage,
  FRAME_HEADER,
  jsLog,
  pollFrame,
  setVideoViewport,
} from "../../lib/tauri";
import { useFocusStore } from "../../stores/useFocusStore";

export interface CanvasDeco {
  gen: number;
  w: number;
  h: number;
}

/**
 * Vídeo no modo canvas (sem a surface nativa): dois loops desacoplados. O de
 * **fetch** puxa o frame RGBA do core por IPC e guarda o mais recente numa
 * ref; o de **paint** (rAF, sincronizado com o vblank) pinta o último frame no
 * canvas. Assim a latência do IPC não trava a pintura, e ao pausar o canvas
 * segura o último frame (freeze) — o fetch cai pra ~8 Hz. A moldura vem à
 * parte (`decoration_image`) e é pintada num canvas por cima.
 *
 * `running` = jogo carregado e sem vídeo nativo. `setAspect` recebe a
 * proporção de exibição quando o quadro muda de tamanho ou de proporção.
 */
export function useCanvasVideo({
  running,
  nativeVideo,
  declaredAspect,
  setAspect,
}: {
  running: boolean;
  nativeVideo: boolean;
  declaredAspect: MutableRefObject<number>;
  setAspect: (a: number) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  // Moldura no modo canvas: dimensões pro palco; a imagem fica na ref e é
  // pintada no canvas de cima quando ele monta.
  const [deco, setDeco] = useState<CanvasDeco | null>(null);
  const decoPixels = useRef<Uint8ClampedArray<ArrayBuffer> | null>(null);
  const decoCanvasRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = decoCanvasRef.current;
    const px = decoPixels.current;
    if (!deco || !c || !px) return;
    c.width = deco.w;
    c.height = deco.h;
    c.getContext("2d")?.putImageData(new ImageData(px, deco.w, deco.h), 0, 0);
  }, [deco]);
  // Sem moldura: tira o posicionamento que o paint aplicou no canvas do jogo.
  useEffect(() => {
    if (deco) return;
    const c = canvasRef.current;
    if (c) for (const k of ["left", "top", "width", "height"] as const) c.style[k] = "";
  }, [deco]);

  // Informa ao backend o tamanho REAL (pixels físicos) da área do jogo — é o
  // viewport dos shaders. Sem isso o shader rodava em 3× a nativa e o canvas
  // reduzia o resultado: máscara de CRT virava moiré e o quadro gigante pelo
  // IPC derrubava o desempenho.
  useEffect(() => {
    if (nativeVideo) return;
    const c = canvasRef.current;
    if (!c) return;
    let last = "";
    const send = (w: number, h: number) => {
      const key = `${w}x${h}`;
      if (w < 1 || h < 1 || key === last) return;
      last = key;
      void setVideoViewport(w, h).catch(() => {});
    };
    const ro = new ResizeObserver((entries) => {
      const dp = entries[0]?.devicePixelContentBoxSize?.[0];
      if (dp) {
        send(Math.round(dp.inlineSize), Math.round(dp.blockSize));
      } else {
        const r = c.getBoundingClientRect();
        const dpr = window.devicePixelRatio || 1;
        send(Math.round(r.width * dpr), Math.round(r.height * dpr));
      }
    });
    try {
      ro.observe(c, { box: "device-pixel-content-box" });
    } catch {
      ro.observe(c); // motor sem esse modo: cai no cálculo por devicePixelRatio
    }
    return () => ro.disconnect();
    // `running`: o canvas só existe depois que o jogo carrega
  }, [nativeVideo, deco, running]);

  // O loop de fetch olha o foco por ref (não re-monta o efeito a cada pausa).
  const focus = useFocusStore((s) => s.focus);
  const pausedRef = useRef(false);
  useEffect(() => {
    pausedRef.current = focus === "MenuFocused";
  }, [focus]);

  useEffect(() => {
    if (!running || nativeVideo) return;
    let alive = true;
    let raf = 0;
    // `fresh`: chegou frame novo desde o último desenho — sem frame novo não
    // redesenha (o canvas continua mostrando o último).
    const latest = {
      px: null as Uint8Array<ArrayBuffer> | null,
      w: 0,
      h: 0,
      fresh: false,
      // moldura: geração (0 = nenhuma) e retângulo do jogo em NDC
      decoGen: 0,
      rect: [0, 0, 1, 1] as [number, number, number, number],
      // zoom da moldura em volta do centro (integer scaling; 1 = sem)
      zoom: 1,
      // proporção de exibição do quadro (muda com `SET_GEOMETRY` em runtime)
      aspect: 0,
    };
    let appliedAspect = 0;
    let renderer: FrameRenderer | null = null;
    let shownDecoGen = 0;
    let fetchingDeco = false;
    let appliedRect = "";

    // Moldura nova (ou nenhuma): busca a imagem uma vez e troca o palco.
    const syncDeco = async (gen: number) => {
      if (gen === shownDecoGen || fetchingDeco) return;
      if (gen === 0) {
        shownDecoGen = 0;
        decoPixels.current = null;
        setDeco(null);
        return;
      }
      fetchingDeco = true;
      try {
        const buf = await decorationImage();
        if (!alive || buf.byteLength < 12) return;
        const dv = new DataView(buf);
        const [g, w, h] = [dv.getUint32(0, true), dv.getUint32(4, true), dv.getUint32(8, true)];
        if (buf.byteLength < 12 + w * h * 4) return;
        decoPixels.current = new Uint8ClampedArray(buf, 12, w * h * 4);
        shownDecoGen = g;
        setDeco({ gen: g, w, h });
      } catch {
        /* tenta de novo no próximo quadro */
      } finally {
        fetchingDeco = false;
      }
    };

    void (async () => {
      while (alive) {
        let got = false;
        try {
          const buf = await pollFrame();
          if (buf.byteLength > FRAME_HEADER) {
            const dv = new DataView(buf);
            const w = dv.getUint32(0, true);
            const h = dv.getUint32(4, true);
            const need = w * h * 4;
            if (w > 0 && h > 0 && buf.byteLength >= FRAME_HEADER + need) {
              // View sobre o MESMO ArrayBuffer (sem `.slice()`, que copia o
              // frame inteiro) — `Uint8Array` não tem restrição de
              // alinhamento, então o offset do cabeçalho é seguro.
              latest.px = new Uint8Array(buf, FRAME_HEADER, need);
              latest.w = w;
              latest.h = h;
              latest.decoGen = dv.getUint32(8, true);
              latest.rect = [
                dv.getFloat32(12, true),
                dv.getFloat32(16, true),
                dv.getFloat32(20, true),
                dv.getFloat32(24, true),
              ];
              latest.aspect = dv.getFloat32(28, true);
              latest.zoom = dv.getFloat32(32, true) || 1;
              latest.fresh = true;
              got = true;
              void syncDeco(latest.decoGen);
            }
          }
        } catch {
          /* frame perdido, tenta no próximo */
        }
        // Pausado: ~8Hz. Rodando: se acabou de pegar um frame, o próximo só sai
        // em ~16ms — espera ~11ms antes de pollar de novo (menos IPC = menos
        // hitch). Se veio vazio, pollar rápido pra não perder o próximo.
        const delay = pausedRef.current ? 120 : got ? 11 : 2;
        await new Promise((r) => setTimeout(r, delay));
      }
    })();

    const paint = () => {
      const c = canvasRef.current;
      if (c && latest.px && latest.fresh) {
        if (!renderer) {
          // WebGL se der, senão 2D (`lib/frameRenderer.ts`).
          renderer = createFrameRenderer(c);
          if (renderer) jsLog("info", `canvas de vídeo: ${renderer.kind}`);
          else jsLog("error", "canvas de vídeo: sem contexto WebGL nem 2D — jogo sem imagem");
        }
        if (c.width !== latest.w || c.height !== latest.h || latest.aspect !== appliedAspect) {
          // Proporção do próprio quadro quando vem (o core pode mudar em
          // runtime); senão a declarada no carregamento.
          appliedAspect = latest.aspect;
          const declared = latest.aspect > 0 ? latest.aspect : declaredAspect.current;
          const pixels = latest.w / Math.max(1, latest.h);
          // orientação bate → AR declarada (PAR ok); senão frame rotacionado.
          setAspect(declared >= 1 === pixels >= 1 ? declared : pixels);
        }
        if (latest.decoGen !== 0 && shownDecoGen === latest.decoGen) {
          // retângulo NDC do Rust → % do palco (só mexe no DOM se mudou)
          const [cx, cy, hw, hh] = latest.rect;
          const key = `${latest.rect.join()}|${latest.zoom}`;
          if (key !== appliedRect) {
            appliedRect = key;
            const d = decoCanvasRef.current;
            if (d) d.style.transform = latest.zoom === 1 ? "" : `scale(${latest.zoom})`;
            c.style.left = `${((cx - hw + 1) / 2) * 100}%`;
            c.style.top = `${((1 - cy - hh) / 2) * 100}%`;
            c.style.width = `${hw * 100}%`;
            c.style.height = `${hh * 100}%`;
          }
        } else {
          appliedRect = "";
        }
        renderer?.draw(latest.px, latest.w, latest.h);
        latest.fresh = false;
      }
      if (alive) raf = requestAnimationFrame(paint);
    };
    raf = requestAnimationFrame(paint);

    return () => {
      alive = false;
      cancelAnimationFrame(raf);
      renderer?.dispose();
    };
  }, [running, nativeVideo, declaredAspect, setAspect]);

  return { canvasRef, decoCanvasRef, deco };
}
