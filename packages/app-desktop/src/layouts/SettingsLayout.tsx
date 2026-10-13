import {
  Breadcrumb,
  BreadcrumbButton,
  BreadcrumbDivider,
  BreadcrumbItem,
  Title2,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
import { Fragment } from "react";
import { useTranslation } from "react-i18next";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { RouteTransition } from "../components/RouteTransition";
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
  // 56 de diferença pra `.scroll` não ganhar rolagem. Com controle
  // conectado a altura é a mesma: a barra de dicas flutua por cima do canto
  // inferior direito do card, como no app Xbox (pedido do usuário,
  // 2026-10-11 — antes a página encolhia 56 px pra ela).
  root: {
    height: `calc(100vh - var(--reemuPageTop, ${M.PAGE_TOP}px) - ${M.SAFE_Y}px)`,
    marginBottom: `-${M.PAGE_PAD_B - M.SAFE_Y}px`,
    minHeight: "240px",
  },
  page: {
    height: "100%",
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalXL,
  },
  title: { whiteSpace: "nowrap" },
  // Caminho "Configurações › Vídeo › Molduras" com o `Breadcrumb` do Fluent 2
  // (nav + lista ordenada, divisores fora do foco, `aria-current` no último
  // — fluent2.microsoft.design/components/web/react/core/breadcrumb/usage),
  // no tamanho de título como nas Configurações do Windows 11 (o maior
  // tamanho do Fluent é 16 px). O texto do 1º item começa na borda do card
  // de baixo, como no Windows: o botão tem padding, então a lista volta o
  // mesmo tanto (medido).
  crumbs: {
    marginLeft: "calc(-1 * var(--reemuCrumbPad, 8px))",
    "& ol": { flexWrap: "nowrap" },
  },
  crumbButton: {
    fontSize: tokens.fontSizeHero700,
    fontFamily: "var(--reemuFontDisplay)",
    lineHeight: tokens.lineHeightHero700,
    fontWeight: tokens.fontWeightSemibold,
    height: "auto",
    whiteSpace: "nowrap",
    color: tokens.colorNeutralForeground3,
    ":hover": { color: tokens.colorNeutralForeground1 },
  },
  crumbCurrent: { color: `${tokens.colorNeutralForeground1} !important` },
  crumbSep: {
    color: tokens.colorNeutralForeground3,
    "& svg": { fontSize: "20px", width: "20px", height: "20px" },
  },
  // Card do conteúdo: `Background1` com sombra baixa; os de dentro usam
  // `Background2`, redefinido abaixo pra ter contraste em qualquer tema.
  card: {
    flexGrow: 1,
    minHeight: 0,
    display: "flex",
    // `Background1`, um pouco mais fechado nos temas claros (`liftTokens`)
    backgroundColor: "var(--reemuSurface1)",
    // Sombra baixa (Fluent 2 › Elevation: `shadow4` em cards) e traço de
    // 1 px: no escuro a sombra quase some sobre fundo escuro, e "Windows
    // uses strokes instead of key shadows to outline an object" (mesma
    // página) — o card das Configurações do Windows 11 também tem traço.
    borderRadius: tokens.borderRadiusMedium,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
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
  // Página que se ajusta à altura útil do card em vez de rolar (Shaders: a
  // lista e a prévia descem até o padding de baixo).
  fill: { height: "100%" },
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
  const navigate = useNavigate();
  // mesma condição da barra de dicas (`ButtonHints`)
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
    <div className={styles.root}>
      {/* Remonta a cada rota: a página nova entra com a transição do app
          (e o conteúdo começa do topo). */}
      <RouteTransition routeKey={pathname} className={styles.page}>
        {section ? (
          <>
            <Breadcrumb
              className={styles.crumbs}
              size="large"
              aria-label={t("settings.title")}
            >
              {crumbs.map((c, i) => (
                <Fragment key={i}>
                  {i > 0 && <BreadcrumbDivider className={styles.crumbSep} />}
                  <BreadcrumbItem>
                    {c.to ? (
                      <BreadcrumbButton
                        className={styles.crumbButton}
                        onClick={() => navigate(c.to!)}
                      >
                        {c.label}
                      </BreadcrumbButton>
                    ) : (
                      <BreadcrumbButton
                        className={mergeClasses(styles.crumbButton, styles.crumbCurrent)}
                        current
                      >
                        {c.label}
                      </BreadcrumbButton>
                    )}
                  </BreadcrumbItem>
                </Fragment>
              ))}
            </Breadcrumb>
            <div className={styles.card}>
              <div className={styles.pane}>
                <div
                  className={mergeClasses(
                    styles.content,
                    WIDE_TABS.has(section.key) && styles.wide,
                    section.key === "video" && sub === "shaders" && styles.fill,
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
