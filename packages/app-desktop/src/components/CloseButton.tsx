import { Button } from "@fluentui/react-components";
import { DismissRegular } from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";

/**
 * Botão de fechar de diálogos e drawers. Permanece sempre visível mesmo com
 * controle conectado: quem alterna entre controle e mouse (ou usa um controle
 * sem o botão B mapeado) nunca fica preso numa tela cheia esperando o Esc.
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
  return (
    <Button
      appearance="subtle"
      icon={<DismissRegular />}
      aria-label={label ?? t("common.close")}
      size={size}
      tabIndex={tabIndex}
      data-overlay-close=""
      onClick={onClick}
    />
  );
}
