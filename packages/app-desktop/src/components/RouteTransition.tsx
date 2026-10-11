import { makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import type { ReactNode } from "react";

const useStyles = makeStyles({
  // Fluent 2 › Motion › Top level: "For navigating between major
  // destinations, use a quick fade. Avoid sliding large elements" — a tela
  // inteira só faz fade (antes subia 14 px, ou vinha da esquerda no
  // "voltar"). Duração e curva do `Fade` do Fluent
  // (@fluentui/react-motion-components-preview: `durationNormal`,
  // `curveEasyEase`).
  layer: {
    animationName: { from: { opacity: 0 }, to: { opacity: 1 } },
    animationDuration: tokens.durationNormal,
    animationTimingFunction: tokens.curveEasyEase,
    // `backwards`, não `both`: com `both` o estado final ficava aplicado pra
    // sempre — a tela inteira presa numa camada composta, e o WebView2 troca
    // o ClearType do texto por antialias em cinza (letras serrilhadas no
    // Windows, 2026-09-25). O quadro final é o estado natural, então ao
    // terminar nada muda na tela.
    animationFillMode: "backwards",
    "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
  },
});

/**
 * Transição de rota: a tela nova entra com um fade rápido. Remonta o
 * conteúdo via `key` e roda a animação de entrada.
 *
 * `routeKey` identifica a "tela" (pathname, ou `/settings` pra não remontar o
 * layout de configurações a cada sub-aba).
 */
export function RouteTransition({
  routeKey,
  className,
  children,
}: {
  routeKey: string;
  className?: string;
  children: ReactNode;
}) {
  const s = useStyles();
  return (
    <div key={routeKey} className={mergeClasses(s.layer, className)}>
      {children}
    </div>
  );
}
