import { useClock } from "../hooks/useClock";
import { useShellStyles } from "../styles/xbox";

/**
 * Isolado num componente próprio: o tick de 30s só re-renderiza este `<span>`,
 * não o `AppShell` inteiro (que teria a Library/Home embaixo dele
 * reconciliando de novo a cada tick à toa).
 */
export function Clock() {
  const s = useShellStyles();
  const clock = useClock();
  return <span className={s.clock}>{clock}</span>;
}
