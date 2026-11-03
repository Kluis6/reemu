import { Button } from "@fluentui/react-components";
import { DismissRegular } from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";
import { useGamepadStore } from "../stores/useGamepadStore";

/**
 * O "X" de fechar de diálogos e drawers. Só aparece sem controle conectado
 * (pedido do usuário, 2026-11-03): com controle, o B já fecha e a barra de
 * dicas mostra isso, como no Xbox; o X só servia de parada a mais pro foco.
 * Mesma condição da barra de dicas (`ButtonHints`).
 */
export function CloseButton({
  onClick,
  label,
  size,
  tabIndex,
}: {
  onClick: () => void;
  label?: string;
  size?: "small" | "medium" | "large";
  tabIndex?: number;
}) {
  const { t } = useTranslation();
  const pad = useGamepadStore((st) => st.devices.length > 0);
  if (pad) return null;
  return (
    <Button
      appearance="subtle"
      icon={<DismissRegular />}
      aria-label={label ?? t("common.close")}
      size={size}
      tabIndex={tabIndex}
      onClick={onClick}
    />
  );
}
