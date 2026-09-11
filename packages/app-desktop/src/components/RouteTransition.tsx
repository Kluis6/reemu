import { makeStyles, mergeClasses, tokens } from "@fluentui/react-components";
import type { ReactNode } from "react";
import { useNavigationType } from "react-router-dom";

const enterFwd = {
  from: { opacity: 0, transform: "translateY(14px)" },
  to: { opacity: 1, transform: "translateY(0)" },
};
const enterBack = {
  from: { opacity: 0, transform: "translateX(-16px)" },
  to: { opacity: 1, transform: "translateX(0)" },
};

const useStyles = makeStyles({
  layer: {
    animationDuration: "400ms",
    animationTimingFunction: tokens.curveDecelerateMax,
    animationFillMode: "both",
    "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
  },
  fwd: { animationName: enterFwd },
  back: { animationName: enterBack },
});

/**
 * Transição de rota estilo modo XBOX: a tela nova entra com fade + subida (ou
 * deslizando da esquerda quando é "voltar"). Remonta o conteúdo via `key` e
 * roda a animação de entrada.
 *
 * `routeKey` identifica a "tela" (pathname, ou `/settings` pra não remontar o
 * layout de configurações a cada sub-aba).
 */
export function RouteTransition({
  routeKey,
  children,
}: {
  routeKey: string;
  children: ReactNode;
}) {
  const s = useStyles();
  const back = useNavigationType() === "POP";
  return (
    <div key={routeKey} className={mergeClasses(s.layer, back ? s.back : s.fwd)}>
      {children}
    </div>
  );
}
