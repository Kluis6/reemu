import {
  Tab,
  TabList,
  Title2,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { useTranslation } from "react-i18next";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { RouteTransition } from "../components/RouteTransition";

const useStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    gap: tokens.spacingVerticalL,
    maxWidth: "640px",
  },
  // Abas de lista em grade (Aparência, Cores, BIOS) usam a largura da tela — 640px é bom pra
  // formulário, mas deixava as colunas de cores espremidas.
  wide: { maxWidth: "1400px" },
});

const WIDE_TABS = new Set(["aparencia", "cores", "bios"]);

// `key` = trecho da rota; `label` = chave de tradução.
const TABS = [
  { key: "perfil", label: "settings.tabs.profile" },
  { key: "aparencia", label: "settings.tabs.appearance" },
  { key: "biblioteca", label: "settings.tabs.library" },
  { key: "audio", label: "settings.tabs.audio" },
  { key: "video", label: "settings.tabs.video" },
  { key: "metadata", label: "settings.tabs.metadata" },
  { key: "hotkeys", label: "settings.tabs.hotkeys" },
  { key: "controllers", label: "settings.tabs.controllers" },
  { key: "cores", label: "settings.tabs.cores" },
  { key: "bios", label: "settings.tabs.bios" },
] as const;

export function SettingsLayout() {
  const { t } = useTranslation();
  const styles = useStyles();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const current =
    TABS.find((tab) => pathname.endsWith(`/${tab.key}`))?.key ?? "audio";

  return (
    <div
      className={mergeClasses(
        styles.root,
        WIDE_TABS.has(current) && styles.wide,
      )}
    >
      <Title2>{t("settings.title")}</Title2>
      <TabList
        selectedValue={current}
        onTabSelect={(_, d) => navigate(`/settings/${d.value}`)}
      >
        {TABS.map((tab) => (
          <Tab key={tab.key} value={tab.key}>
            {t(tab.label)}
          </Tab>
        ))}
      </TabList>
      <RouteTransition routeKey={current}>
        <Outlet />
      </RouteTransition>
    </div>
  );
}
