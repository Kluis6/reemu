import { makeStyles, tokens } from "@fluentui/react-components";
import type { ReactNode } from "react";

const useStyles = makeStyles({
  fade: {
    animationName: {
      from: { opacity: 0, transform: "translateY(10px) scale(0.994)" },
      to: { opacity: 1, transform: "translateY(0) scale(1)" },
    },
    animationDuration: "240ms",
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: "both",
    "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
  },
});

/**
 * Fade + slide sutil ao trocar de rota. Remonta o conteúdo (via `key`) e roda a
 * animação de entrada. `routeKey` = o que identifica a "tela" (o pathname
 * inteiro, ou só o grupo `/settings` pra não remontar o layout de
 * configurações a cada sub-aba).
 */
export function RouteTransition({
  routeKey,
  children,
}: {
  routeKey: string;
  children: ReactNode;
}) {
  const s = useStyles();
  return (
    <div key={routeKey} className={s.fade}>
      {children}
    </div>
  );
}
