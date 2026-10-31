import { Tooltip } from "@fluentui/react-components";
import { Wifi1Regular, WifiOffRegular } from "@fluentui/react-icons";
import { useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { TOOLTIP_POSITION } from "../lib/tooltip";
import { useShellStyles } from "../styles/xbox";

function subscribe(onChange: () => void) {
  window.addEventListener("online", onChange);
  window.addEventListener("offline", onChange);
  return () => {
    window.removeEventListener("online", onChange);
    window.removeEventListener("offline", onChange);
  };
}

/**
 * Ícone na topbar, ao lado do relógio. Vem de `navigator.onLine`, sem consultar
 * servidor nenhum: diz se o sistema tem rede, não garante que a internet responde.
 * O estado muda o desenho do ícone, não só a cor, e o nome fica no tooltip.
 */
export function NetworkStatus() {
  const { t } = useTranslation();
  const s = useShellStyles();
  const online = useSyncExternalStore(subscribe, () => navigator.onLine);
  const label = t(online ? "shell.networkOnline" : "shell.networkOffline");

  return (
    <Tooltip positioning={TOOLTIP_POSITION} content={label} relationship="label">
      <span className={s.gamepadStatus} role="img" aria-label={label}>
        {online ? <Wifi1Regular /> : <WifiOffRegular />}
      </span>
    </Tooltip>
  );
}
