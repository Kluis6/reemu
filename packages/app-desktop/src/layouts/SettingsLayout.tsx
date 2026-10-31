import {
  Breadcrumb,
  BreadcrumbButton,
  BreadcrumbItem,
  makeStyles,
  mergeClasses,
  tokens,
} from "@fluentui/react-components";
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
    height: `calc(100vh - var(--reemuOverscan, 0) * 1vh - var(--reemuPageTop, ${M.PAGE_TOP}px) - ${M.SAFE_Y}px)`,
    marginBottom: `-${M.PAGE_PAD_B - M.SAFE_Y}px`,
    minHeight: "240px",
  },
  page: {
    height: "100%",
    display: "flex",
    flexDirection: "column",
    rowGap: tokens.spacingVerticalXL,
  },
  // Caminho "Configurações › Vídeo › Molduras" com o `Breadcrumb` do Fluent 2
  // (nav + lista ordenada, divisores fora do foco, `aria-current` no último
  // — fluent2.microsoft.design/components/web/react/core/breadcrumb/usage),
  // no tamanho de título como nas Configurações do Windows 11 (o maior
  // tamanho do Fluent é 16 px). O texto do 1º item começa na borda do card
  // de baixo, como no Windows: o botão tem padding, então a lista volta o
  // mesmo tanto (medido).
  crumbs: {
    // 9 = 1 de borda + 8 de padding do botão (medido): o texto do 1º item
    // fica na mesma coluna da borda dos cards (com 8 sobrava 1 px)
    marginLeft: "-9px",
    // sem o chevron do Fluent (pedido do usuário): os níveis se separam pelo
    // espaço e pela cor, como no título das configurações do console
    "& ol": { flexWrap: "nowrap", columnGap: tokens.spacingHorizontalS },
  },
  crumbButton: {
    fontSize: tokens.fontSizeHero700,
    fontFamily: "var(--reemuFontDisplay)",
    lineHeight: tokens.lineHeightHero700,
    fontWeight: tokens.fontWeightSemibold,
    height: "auto",
    // sem o padding vertical do botão: a caixa tem a altura do texto e a
    // distância até os cards é só o `rowGap` da página (20, XL) — com ele
    // sobravam 9 px a mais embaixo do título (pedido do usuário)
    paddingTop: "0 !important",
    paddingBottom: "0 !important",
    whiteSpace: "nowrap",
    color: tokens.colorNeutralForeground3,
    ":hover": { color: tokens.colorNeutralForeground1 },
  },
  crumbCurrent: { color: `${tokens.colorNeutralForeground1} !important` },
  bodyPage: {
    flexGrow: 1,
    minWidth: 0,
    minHeight: 0,
    display: "flex",
    flexDirection: "column",
  },
  // Card do conteúdo: `Background1` com sombra baixa; os de dentro usam
  // `Background2`, redefinido abaixo pra ter contraste em qualquer tema.
  card: {
    flexGrow: 1,
    minHeight: 0,
    display: "flex",
    // `Background1`, um pouco mais fechado nos temas claros (`liftTokens`)
    backgroundColor: "var(--reemuSurface1)",
    // Sem borda e sem sombra (pedido do usuário, como no modo XBOX): o card
    // se destaca da página só pela cor (a superfície, `reemuSurface1`).
    borderRadius: tokens.borderRadiusMedium,
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
    // O fim da lista sobe além da barra de dicas, que flutua sobre o card.
    paddingBottom: `${M.PAGE_PAD_B - M.SAFE_Y + 24}px`,
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
  // Perfil em duas colunas (campos | avatares)
  "perfil",
  "audio",
  "metadata",
  "aparencia",
  "cores",
  "bios",
  "controllers",
  "video",
]);

/** Categorias que abrem numa grade de cards (subcategorias). */
const HUB_SECTIONS = new Set(["aparencia", "video", "sistema"]);

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
  // Na entrada (`/settings`) é só "Configurações", já como item atual: o
  // mesmo componente nas duas telas, então o título e o começo dos cards
  // ficam no mesmo lugar (antes o hub usava um `Title2` 8 px mais alto e os
  // cards subiam 18 px — pedido do usuário).
  const crumbs: { label: string; to?: string }[] = [
    { label: t("settings.title"), to: section ? "/settings" : undefined },
  ];
  if (section) {
    const label = t(`settings.tabs.${section.id}`);
    if (section.key === "video" && (sub === "shaders" || sub === "molduras")) {
      crumbs.push({ label, to: "/settings/video" });
      crumbs.push({
        label: sub === "shaders" ? t("video.tabShaders") : t("video.tabBezels"),
      });
    } else if (section.key === "aparencia" && (sub === "temas" || sub === "papel-de-parede")) {
      crumbs.push({ label, to: "/settings/aparencia" });
      crumbs.push({
        label: sub === "temas" ? t("appearance.theme.title") : t("appearance.wallpaper.title"),
      });
    } else if (
      section.key === "sistema" &&
      (sub === "opcoes" || sub === "hardware" || sub === "sobre")
    ) {
      crumbs.push({ label, to: "/settings/sistema" });
      crumbs.push({
        label:
          sub === "opcoes"
            ? t("system.options")
            : sub === "hardware"
              ? t("system.hardware.title")
              : t("about.tab"),
      });
    } else {
      crumbs.push({ label });
    }
  }

  return (
    <div className={styles.root}>
      <div className={styles.page}>
        <Breadcrumb
          className={styles.crumbs}
          // com o controle, B volta; o foco entra direto nas opções
          data-nav-skip
          size="large"
          aria-label={t("settings.title")}
        >
          {crumbs.map((c, i) => (
              <BreadcrumbItem key={i}>
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
          ))}
        </Breadcrumb>
        {section ? (
          <>
            {/* Remonta a cada rota: a página nova entra com a transição do app
                (e o conteúdo começa do topo). */}
            <RouteTransition routeKey={pathname} className={styles.bodyPage}>
              {/* Entrada de uma subcategoria (Aparência, Decorações): os cards
                  das opções direto na página, sem o card geral — ele só
                  aparece dentro da opção escolhida. */}
              {HUB_SECTIONS.has(section.key) && !sub ? (
                <Outlet />
              ) : (
                <div className={styles.card}>
                  <div className={styles.pane}>
                    <div
                      className={mergeClasses(
                        styles.content,
                        WIDE_TABS.has(section.key) && styles.wide,
                        // Sistema › Opções e › Informações e licenças: seções
                        // em cards (largura toda, até os 1120 do próprio
                        // SettingsSystem; as licenças viram duas colunas)
                        section.key === "sistema" &&
                          (sub === "opcoes" || sub === "sobre") &&
                          styles.wide,
                        section.key === "video" && sub === "shaders" && styles.fill,
                      )}
                    >
                      <Outlet />
                    </div>
                  </div>
                </div>
              )}
            </RouteTransition>
          </>
        ) : (
          <RouteTransition routeKey={pathname} className={styles.bodyPage}>
            <Outlet />
          </RouteTransition>
        )}
      </div>
    </div>
  );
}
