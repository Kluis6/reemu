import { useEffect, useRef, useState } from "react";
import { pauseBackgroundUrl } from "../../lib/tauri";

/**
 * Menu de pausa no vídeo nativo: fica montado durante a animação de saída
 * (`mounted`) e usa de fundo o print do jogo capturado pelo Rust (`bg`). Com o
 * overlay (`REEMU_WIN_OVERLAY`) o jogo continua visível atrás e o print não é
 * buscado.
 */
export function usePauseBackground(menuOpen: boolean, nativeVideo: boolean, overlay: boolean) {
  const [closing, setClosing] = useState(false);
  const wasOpen = useRef(false);
  const [bg, setBg] = useState<string | null>(null);
  useEffect(() => {
    if (menuOpen) {
      wasOpen.current = true;
      if (!nativeVideo || overlay) return;
      let url: string | null = null;
      // pequeno atraso: o Rust captura ~2 ticks depois do foco trocar.
      const t = setTimeout(() => {
        void pauseBackgroundUrl().then((u) => {
          url = u;
          setBg(u);
        });
      }, 60);
      return () => {
        clearTimeout(t);
        if (url) URL.revokeObjectURL(url);
      };
    }
    if (!wasOpen.current) return; // fechou sem nunca ter aberto (mount)
    wasOpen.current = false;
    setClosing(true);
    const t = setTimeout(() => {
      setClosing(false);
      setBg((u) => {
        if (u) URL.revokeObjectURL(u);
        return null;
      });
    }, 340);
    // 340: além da saída do drawer (`small`, durationGentle = 250 ms), a
    // camada opaca do vídeo nativo fica até a janela do jogo voltar
    // (`VideoMenu::Closing(20)`, ~300 ms) — sair antes mostrava o branco.
    return () => clearTimeout(t);
  }, [menuOpen, nativeVideo, overlay]);

  return { mounted: menuOpen || closing, bg };
}
