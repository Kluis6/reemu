import {
  Tab,
  TabList,
  Title2,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { RouteTransition } from "../components/RouteTransition";
import * as M from "../styles/metrics";

// Folga pro anel de foco (3 + 4 de afastamento) não ser cortado pela borda
// da área que rola.
const RING = 8;

// Modelo das Configurações do Xbox: categorias numa lista vertical à
// esquerda, conteúdo à direita. Dez abas lado a lado não cabem na tela de
// referência e a lista vertical é o que o controle navega melhor
// (cima/baixo escolhe a categoria, direita entra no conteúdo).
// Organização das Configurações do Windows: a coluna da esquerda (título +
// categorias) fica parada e só o conteúdo rola, numa área própria cortada no
// topo da página (y = 116) — nada passa por cima do título nem por baixo da
// busca. A página ocupa exatamente a altura da tela (do topo do conteúdo até a
// margem de baixo da `.scroll`), então a página em si não rola. `vh` já está
// em px da interface: o tamanho vem do zoom nativo do webview (`uiScale.ts`).
const useStyles = makeStyles({
  root: {
    display: "grid",
    gridTemplateColumns: "220px minmax(0, 1fr)",
    columnGap: tokens.spacingHorizontalXXXL,
    height: `calc(100vh - var(--reemuPageTop, ${M.PAGE_TOP}px) - ${M.PAGE_PAD_B}px)`,
    minHeight: "240px",
  },
  aside: {
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalL,
    minHeight: 0,
  },
  title: { whiteSpace: "nowrap" },
  nav: {
    // Com a interface grande a lista pode passar da altura: rola sozinha.
    overflowY: "auto",
    minHeight: 0,
    paddingLeft: `${RING}px`,
    paddingRight: `${RING}px`,
    paddingBottom: `${RING}px`,
    marginLeft: `-${RING}px`,
    marginRight: `-${RING}px`,
    scrollbarWidth: "none",
    // Abas maiores (`size="large"`) e afastadas: alvo mais fácil de acertar
    // com o controle e de ler de longe.
    rowGap: tokens.spacingVerticalS,
    // Rótulos alinhados à esquerda: o Tab reserva a largura do rótulo em
    // negrito (selecionado) e centraliza o texto normal dentro dela.
    "& .fui-Tab__content": { textAlign: "left" },
  },
  // Área que rola. O conteúdo começa na altura da lista (título + espaço);
  // ao rolar, some no topo da página. Mesmo scrollbar da `.scroll` do shell.
  pane: {
    minWidth: 0,
    overflowY: "auto",
    scrollbarGutter: "stable",
    paddingTop: `calc(${tokens.lineHeightHero800} + ${tokens.spacingVerticalL})`,
    paddingLeft: `${RING}px`,
    paddingRight: `${RING}px`,
    paddingBottom: `${RING}px`,
    marginLeft: `-${RING}px`,
    "::-webkit-scrollbar": { width: `${M.SCROLLBAR_W}px` },
    "::-webkit-scrollbar-thumb": {
      backgroundColor: tokens.colorNeutralStroke2,
    },
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
  // Categoria nova começa do topo (a área que rola é a mesma entre elas).
  const paneRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (paneRef.current) paneRef.current.scrollTop = 0;
  }, [current]);

  return (
    <div className={styles.root}>
      <div className={styles.aside}>
        <Title2 className={styles.title}>{t("settings.title")}</Title2>
        <TabList
          className={styles.nav}
          vertical
          size="large"
          selectedValue={current}
          onTabSelect={(_, d) => navigate(`/settings/${d.value}`)}
        >
          {TABS.map((tab) => (
            <Tab key={tab.key} value={tab.key}>
              {t(tab.label)}
            </Tab>
          ))}
        </TabList>
      </div>
      <div className={styles.pane} ref={paneRef}>
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
    </div>
  );
}
