import { Tooltip } from "@fluentui/react-components";
import { JoystickRegular } from "@fluentui/react-icons";
import { useGamepadStore } from "../stores/useGamepadStore";
import { useShellStyles } from "../styles/xbox";

/**
 * Ícone na topbar, ao lado do relógio — só aparece com ≥1 controle
 * conectado (`useGamepadStatus`, montado uma vez no `RootLayout`, mantém
 * `useGamepadStore` em dia). O toast de "conectado" é disparado lá; aqui é
 * só o indicador persistente.
 */
export function GamepadStatus() {
  const s = useShellStyles();
  const devices = useGamepadStore((st) => st.devices);
  if (devices.length === 0) return null;

  const label =
    devices.length === 1
      ? `Controle conectado: ${devices[0].name}`
      : `${devices.length} controles conectados`;

  return (
    <Tooltip content={label} relationship="label">
      <span className={s.gamepadStatus} aria-label={label}>
        <JoystickRegular />
      </span>
    </Tooltip>
  );
}
