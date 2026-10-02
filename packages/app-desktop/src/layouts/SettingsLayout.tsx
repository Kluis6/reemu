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

// Modelo das Configurações do Xbox: categorias numa lista vertical à
// esquerda, conteúdo à direita. Dez abas lado a lado não cabem na tela de
// referência e a lista vertical é o que o controle navega melhor
// (cima/baixo escolhe a categoria, direita entra no conteúdo).
const useStyles = makeStyles({
  root: {
    display: "grid",
    gridTemplateColumns: "200px minmax(0, 1fr)",
    columnGap: tokens.spacingHorizontalXXXL,
    rowGap: tokens.spacingVerticalL,
    alignItems: "start",
  },
  title: { gridColumn: "1 / -1" },
  nav: {
    position: "sticky",
    top: 0,
    // Rótulos alinhados à esquerda: o Tab reserva a largura do rótulo em
    // negrito (selecionado) e centraliza o texto normal dentro dela.
    "& .fui-Tab__content": { textAlign: "left" },
  },
  content: { minWidth: 0, maxWidth: "640px" },
  // Abas de lista em grade (Aparência, Cores, BIOS) usam a largura toda — 640
  // é bom pra formulário, mas deixava as colunas de cores espremidas.
  wide: { maxWidth: "none" },
});

const WIDE_TABS = new Set(["aparencia", "cores", "bios", "controllers"]);

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
  { key: "sistema", label: "settings.tabs.system" },
] as const;

export function SettingsLayout() {
  const { t } = useTranslation();
  const styles = useStyles();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const current =
    TABS.find((tab) => pathname.endsWith(`/${tab.key}`))?.key ?? "audio";

  return (
    <div className={styles.root}>
      <Title2 className={styles.title}>{t("settings.title")}</Title2>
      <TabList
        className={styles.nav}
        vertical
        selectedValue={current}
        onTabSelect={(_, d) => navigate(`/settings/${d.value}`)}
      >
        {TABS.map((tab) => (
          <Tab key={tab.key} value={tab.key}>
            {t(tab.label)}
          </Tab>
        ))}
      </TabList>
      <div
        className={mergeClasses(
          styles.content,
          WIDE_TABS.has(current) && styles.wide,
        )}
      >
        <RouteTransition routeKey={current}>
          <Outlet />
        </RouteTransition>
      </div>
    </div>
  );
}
