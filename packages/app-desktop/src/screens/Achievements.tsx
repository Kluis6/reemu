import { TrophyRegular } from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";
import { EmptyState } from "../components/EmptyState";

/** Conquistas do jogador. Fica fora da barra lateral: entra pelo drawer de perfil. */
export function Achievements() {
  const { t } = useTranslation();
  return (
    <EmptyState size="large" icon={<TrophyRegular />} title={t("nav.achievements")}>
      {t("play.achievementsSoon")}
    </EmptyState>
  );
}
