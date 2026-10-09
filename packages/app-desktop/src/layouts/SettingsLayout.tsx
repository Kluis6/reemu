import {
  Title2,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { ChevronRightRegular } from "@fluentui/react-icons";
import { Fragment } from "react";
import { useTranslation } from "react-i18next";
import { Link, Outlet, useLocation } from "react-router-dom";
import { RouteTransition } from "../components/RouteTransition";
import { useGamepadStore } from "../stores/useGamepadStore";
import * as M from "../styles/metrics";
import { SETTINGS_SECTIONS } from "./settingsSections";

// Em `/settings` fica só o título e o hub de cards (`SettingsHome`); numa
// categoria, o caminho "Configurações › Categoria" no topo (como nas
// Configurações do Windows, com os níveis de cima clicáveis) e o conteúdo
// dentro de um card que rola por dentro. A página ocupa exatamente a altura da
// tela (do topo do conteúdo até a margem de baixo da `.scroll`), então ela
// mesma não rola. `vh` já está em px da interface: o tamanho vem do zoom
// nativo do webview (`uiScale.ts`).
const useStyles = makeStyles({
  // O hub e o card da categoria descem até a área segura de baixo (32 px da
  // borda). A `.scroll` reserva 88 px embaixo pra barra de dicas do
  // controle, mas ela só ocupa o canto direito; a margem negativa devolve os
  // 56 de diferença pra `.scroll` não ganhar rolagem.
  root: {
    height: `calc(100vh - var(--reemuPageTop, ${M.PAGE_TOP}px) - ${M.SAFE_Y}px)`,
    marginBottom: `-${M.PAGE_PAD_B - M.SAFE_Y}px`,
    minHeight: "240px",
  },
  // Com controle conectado a barra de dicas aparece (canto inferior
  // direito, fixa): aí a página para acima dela, na margem que a `.scroll`
  // já reserva, pra barra não cobrir o pé dos cards.
  withHints: {
    height: `calc(100vh - var(--reemuPageTop, ${M.PAGE_TOP}px) - ${M.PAGE_PAD_B}px)`,
    marginBottom: 0,
  },
  page: {
    height: "100%",
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalXL,
  },
  title: { whiteSpace: "nowrap" },
  crumbs: {
    display: "flex",
    alignItems: "center",
    flexWrap: "wrap",
    columnGap: tokens.spacingHorizontalS,
    minWidth: 0,
  },
  crumbLink: {
    color: tokens.colorNeutralForeground3,
    textDecorationLine: "none",
    borderRadius: tokens.borderRadiusMedium,
    ":hover": { color: tokens.colorNeutralForeground1 },
  },
  crumbSep: {
    color: tokens.colorNeutralForeground3,
    fontSize: "20px",
    flexShrink: 0,
  },
  // Card do conteúdo: `Background1` com sombra baixa; os de dentro usam
  // `Background2`, redefinido abaixo pra ter contraste em qualquer tema.
  card: {
    flexGrow: 1,
    minHeight: 0,
    display: "flex",
    // `Background1`, um pouco mais fechado nos temas claros (`liftTokens`)
    backgroundColor: "var(--reemuSurface1)",
    // Sem borda; a separação do fundo é a sombra baixa (Fluent 2 ›
    // Elevation: "low-elevation shadows" pra cards).
    borderRadius: tokens.borderRadiusMedium,
    boxShadow: tokens.shadow4,
    overflow: "hidden",
    // Camadas de dentro (cards, linhas, campos): os tokens do Fluent viram
    // os `reemuSurface*` do tema — um passo fixo e sempre mais claro que o
    // fundo do card, em qualquer tema (ver `liftTokens` em themes.ts). Os
    // componentes do Fluent leem esses mesmos tokens (variáveis CSS), então
    // tudo aqui dentro acompanha.
    "--colorNeutralBackground2": "var(--reemuSurface2)",
    "--colorNeutralBackground2Hover": "var(--reemuSurface2Hover)",
    "--colorNeutralBackground2Pressed": "var(--reemuSurface2Pressed)",
    "--colorNeutralBackground2Selected": "var(--reemuSurface2Selected)",
    // campos (`filled-darker`) e superfícies de 3º nível
    "--colorNeutralBackground3": "var(--reemuSurface3)",
    "--colorNeutralBackground3Hover": "var(--reemuSurface3Hover)",
    "--colorNeutralBackground3Pressed": "var(--reemuSurface3Pressed)",
    "--colorNeutralBackground3Selected": "var(--reemuSurface3Selected)",
  },
  // Área que rola, dentro do card. Mesmo scrollbar da `.scroll` do shell.
  pane: {
    flexGrow: 1,
    minWidth: 0,
    overflowY: "auto",
    scrollbarGutter: "stable",
    padding: tokens.spacingHorizontalXXL,
    paddingRight: `calc(${tokens.spacingHorizontalXXL} - ${M.SCROLLBAR_W}px)`,
    "::-webkit-scrollbar": { width: `${M.SCROLLBAR_W}px` },
    "::-webkit-scrollbar-thumb": {
      backgroundColor: tokens.colorNeutralStroke2,
    },
  },
  content: { minWidth: 0, maxWidth: "640px" },
  // Abas de lista em grade (Aparência, Cores, BIOS…) usam a largura toda —
  // 640 é bom pra formulário, mas deixava as colunas espremidas.
  wide: { maxWidth: "none" },
});

const WIDE_TABS = new Set([
  "aparencia",
  "cores",
  "bios",
  "controllers",
  "video",
]);

export function SettingsLayout() {
  const { t } = useTranslation();
  const styles = useStyles();
  const { pathname } = useLocation();
  // mesma condição da barra de dicas (`ButtonHints`)
  const hints = useGamepadStore((st) => st.devices.length > 0);
  // ["", "settings", categoria?, subseção?]
  const [, , catKey, sub] = pathname.split("/");
  const section = SETTINGS_SECTIONS.find((s) => s.key === catKey);

  // Caminho do topo: cada nível acima do atual é um link.
  const crumbs: { label: string; to?: string }[] = [
    { label: t("settings.title"), to: "/settings" },
  ];
  if (section) {
    const label = t(`settings.tabs.${section.id}`);
    if (section.key === "video" && (sub === "shaders" || sub === "molduras")) {
      crumbs.push({ label, to: "/settings/video" });
      crumbs.push({
        label: sub === "shaders" ? t("video.tabShaders") : t("video.tabBezels"),
      });
    } else {
      crumbs.push({ label });
    }
  }

  return (
    <div className={mergeClasses(styles.root, hints && styles.withHints)}>
      {/* Remonta a cada rota: a página nova entra com a transição do app
          (e o conteúdo começa do topo). */}
      <RouteTransition routeKey={pathname} className={styles.page}>
        {section ? (
          <>
            <nav className={styles.crumbs} aria-label={t("settings.title")}>
              {crumbs.map((c, i) => (
                <Fragment key={i}>
                  {i > 0 && <ChevronRightRegular className={styles.crumbSep} />}
                  {c.to ? (
                    <Link to={c.to} className={styles.crumbLink}>
                      <Title2 className={styles.title}>{c.label}</Title2>
                    </Link>
                  ) : (
                    <Title2 className={styles.title} aria-current="page">
                      {c.label}
                    </Title2>
                  )}
                </Fragment>
              ))}
            </nav>
            <div className={styles.card}>
              <div className={styles.pane}>
                <div
                  className={mergeClasses(
                    styles.content,
                    WIDE_TABS.has(section.key) && styles.wide,
                  )}
                >
                  <Outlet />
                </div>
              </div>
            </div>
          </>
        ) : (
          <>
            <Title2 className={styles.title}>{t("settings.title")}</Title2>
            <Outlet />
          </>
        )}
      </RouteTransition>
    </div>
  );
}
