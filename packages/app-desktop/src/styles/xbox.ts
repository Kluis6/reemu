
/**
 * Linguagem visual do "modo Xbox" (ver docs/design/xbox-mode-reference.md e
 * docs/design/fluent2.md) em **Griffel** (`makeStyles` + `tokens` do Fluent 2),
 * substituindo o antigo `xbox.css`.
 *
 * Cor de marca, elevações e fundo da casca vêm do TEMA (ver styles/themes.ts):
 * tokens Fluent (`colorBrand*`, `colorNeutralBackground*` já escurecidos) +
 * tokens custom `--reemu*`. Trocar de tema reajusta tudo. Só o que não é cor
 * (raio 12/16, rail 64px) fica em `shell`.
 *
 * CUIDADO (WebKitGTK, ver src-tauri/src/main.rs): nada de `backdrop-filter`
 * nem `radial-gradient` multicamada em elemento `position: fixed`.
 */
import { makeStyles, tokens } from "@fluentui/react-components";
import { cardSizeCss, SHELF_GAP } from "../lib/shelf";

// Só os valores NÃO-cor do "console look". Cor de marca, elevações e o fundo
// da casca vêm do tema (tokens Fluent + tokens custom `--reemu*`, ver
// styles/themes.ts) — trocar de tema reajusta tudo.
const shell = {
  radius: "12px",
  radiusLg: "16px",
  // Rail cresce em telas largas (fixo em 72px ficava minúsculo em 4K).
  railW: "clamp(72px, 5vw, 108px)",
};

// Gradiente de superfície elevada (cartões, hero) a partir dos neutros do tema.
const elevGradient = `linear-gradient(135deg, ${tokens.colorNeutralBackground4}, ${tokens.colorNeutralBackground3})`;

// Largura de um card (mesma fórmula do JS que conta quantos cabem — ver
// lib/shelf.ts). Fluida: ~148px em janela estreita, até 248px em telas largas.
const gameCardSize = cardSizeCss;

/** Casca: app / rail / topbar / área de rolagem + anel de foco global. */
export const useShellStyles = makeStyles({
  app: {
    position: "fixed",
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    // Altura + linha explícitas: o WebKitGTK não estica a linha implícita (`auto`)
    // de um grid `position: fixed` de forma confiável — sem isto, em monitores
    // altos o rail e o `.main` colapsam pra altura do conteúdo e sobra um vazio
    // escuro embaixo ("tela preta").
    height: "100vh",
    // Fallback literal: se o `--reemuAppBg` do FluentProvider não resolver, a
    // área de conteúdo cai pra este quase-preto (mesmo tom do rail).
    backgroundColor: "#0b0b0d",
    backgroundImage:
      "var(--reemuAppBg, linear-gradient(180deg, #121214 0%, #0b0b0d 45%))",
    color: tokens.colorNeutralForeground1,
    fontFamily: tokens.fontFamilyBase,
    display: "grid",
    gridTemplateColumns: `${shell.railW} 1fr`,
    gridTemplateRows: "minmax(0, 1fr)",
    overflowX: "hidden",
    overflowY: "hidden",
    // brilho de canto (1 camada só — multicamada quebra o WebKitGTK)
    "::before": {
      content: '""',
      position: "absolute",
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      backgroundImage:
        "radial-gradient(760px 420px at 8% -8%, var(--reemuAccentSoft), transparent 70%)",
      pointerEvents: "none",
    },
    // Anel de foco forte pra navegação por controle. `:focus` (não só
    // `:focus-visible`): o pulso do gamepad vem de um evento Tauri, sem
    // keydown, então o WebKitGTK não marca `:focus-visible` no `.focus()`
    // programático — e o usuário não via onde estava o foco.
    "& a:focus, & input:focus, & [tabindex]:focus": {
      outlineWidth: "2px",
      outlineStyle: "solid",
      outlineColor: tokens.colorNeutralForeground1,
      outlineOffset: "3px",
    },
  },

  rail: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    rowGap: "14px",
    paddingTop: "14px",
    paddingBottom: "14px",
    borderRight: "none",
    // Mesmo quase-preto do fundo das páginas (estilo app do Xbox — rail e
    // conteúdo no mesmo tom). Literal, imune a variação de tema.
    backgroundColor: "#0b0b0d",
  },
  railSpacer: { flexGrow: 1 },
  railSep: {
    width: "26px",
    height: "1px",
    backgroundColor: tokens.colorNeutralStroke1,
    marginTop: "6px",
    marginBottom: "6px",
  },
  railItem: {
    position: "relative",
    width: "clamp(44px, 3.4vw, 64px)",
    height: "clamp(44px, 3.4vw, 64px)",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    flex: "none",
    color: tokens.colorNeutralForeground3,
    textDecorationLine: "none",
    fontSize: "clamp(22px, 1.7vw, 32px)",
    border: "none",
    backgroundColor: "transparent",
    cursor: "pointer",
    transitionProperty: "background-color, color, transform, box-shadow",
    transitionDuration: "160ms",
    transitionTimingFunction: tokens.curveDecelerateMid,
    ":hover": {
      backgroundColor: tokens.colorNeutralBackground3,
      color: tokens.colorNeutralForeground1,
      transform: "scale(1.06)",
    },
    '&[aria-current="page"]': {
      backgroundColor: "#3a3a3f",
      color: tokens.colorNeutralForeground1,
    },
  },
  railQuit: {
    ":hover": {
      backgroundColor: "rgba(220, 60, 60, 0.18)",
      color: "#ff8a8a",
    },
  },
  railBrand: {
    marginBottom: "6px",
    cursor: "pointer",
  },

  main: {
    position: "relative",
    display: "flex",
    flexDirection: "column",
    minWidth: 0,
    minHeight: 0,
  },
  topbar: {
    position: "absolute",
    top: 0,
    right: 0,
    left: 0,
    zIndex: 2,
    display: "flex",
    alignItems: "center",
    columnGap: "14px",
    paddingTop: "14px",
    paddingBottom: "14px",
    paddingLeft: "clamp(14px, 3.5vw, 32px)",
    paddingRight: "clamp(16px, 4vw, 42px)",
    boxSizing: "border-box",
    flexShrink: 0,
    backgroundColor: "transparent",
    backgroundImage: "none",
    border: "none",
    boxShadow: "none",
  },
  topbarSpacer: { flexGrow: 1 },
  iconBtn: {
    width: "38px",
    height: "38px",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    borderRadius: shell.radius,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground1,
    fontSize: "17px",
    cursor: "pointer",
    ":hover": { backgroundColor: tokens.colorNeutralBackground3 },
    ":disabled": { opacity: 0.4, cursor: "default" },
  },
  // pílula de busca centralizada, independente do conteúdo lateral
  // Só posicionamento (pílula centralizada, independente do conteúdo lateral) —
  // o chrome do campo (borda, fundo, foco) vem do `<SearchBox>` do Fluent.
  search: {
    position: "absolute",
    left: "50%",
    transform: "translateX(-50%)",
    width: "clamp(200px, 42vw, 780px)",
    maxWidth: "calc(100% - 160px)",
    borderRadius: "20px",
  },
  clock: {
    color: tokens.colorNeutralForeground3,
    fontVariantNumeric: "tabular-nums",
    fontSize: "clamp(15px, 1vw, 22px)",
    fontWeight: 600,
    lineHeight: 1,
    letterSpacing: "0.01em",
  },
  scroll: {
    scrollBehavior: "smooth",
    flexGrow: 1,
    minWidth: 0,
    overflowY: "auto",
    scrollbarGutter: "stable",
    boxSizing: "border-box",
    paddingTop: "clamp(72px, 9vw, 132px)",
    paddingLeft: "clamp(14px, 3vw, 56px)",
    paddingRight: "clamp(14px, 3vw, 56px)",
    paddingBottom: "100px",
    "::-webkit-scrollbar": { width: "10px" },
    "::-webkit-scrollbar-thumb": {
      backgroundColor: tokens.colorNeutralStroke2,
      borderRadius: "999px",
    },
  },
});

/** Hero grande no topo da Início. */
export const useHeroStyles = makeStyles({
  hero: {
    position: "relative",
    display: "block",
    width: "100%",
    minWidth: 0,
    height: "clamp(220px, 26vw, 520px)",
    border: "none",
    padding: 0,
    borderRadius: shell.radiusLg,
    overflowX: "hidden",
    overflowY: "hidden",
    cursor: "pointer",
    backgroundImage: elevGradient,
    color: "inherit",
    marginTop: "8px",
    marginBottom: "4px",
    transitionProperty: "transform, box-shadow",
    transitionDuration: "200ms",
    transitionTimingFunction: tokens.curveDecelerateMid,
    "&:hover, &:focus": {
      transform: "scale(1.006)",
      boxShadow: "0 20px 48px rgba(0, 0, 0, 0.45)",
    },
    "& img": {
      position: "absolute",
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      width: "100%",
      height: "100%",
      objectFit: "cover",
      // zoom de entrada sutil (uma vez)
      animationName: {
        from: { transform: "scale(1.07)" },
        to: { transform: "scale(1)" },
      },
      animationDuration: "760ms",
      animationTimingFunction: tokens.curveDecelerateMax,
      animationFillMode: "both",
      "@media (prefers-reduced-motion: reduce)": { animationName: "none" },
    },
    "&::after": {
      content: '""',
      position: "absolute",
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      backgroundImage:
        "linear-gradient(90deg, rgba(0,0,0,0.78) 0%, rgba(0,0,0,0.15) 55%, transparent 75%), linear-gradient(0deg, rgba(0,0,0,0.5), transparent 40%)",
    },
  },
  body: {
    position: "absolute",
    left: "28px",
    bottom: "22px",
    zIndex: 1,
    maxWidth: "60%",
    textAlign: "left",
    animationName: {
      from: { opacity: 0, transform: "translateY(12px)" },
      to: { opacity: 1, transform: "translateY(0)" },
    },
    animationDuration: "460ms",
    animationDelay: "90ms",
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: "both",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none",
      opacity: 1,
    },
  },
  kicker: {
    fontSize: tokens.fontSizeBase200,
    fontWeight: 700,
    letterSpacing: "0.08em",
    textTransform: "uppercase",
    color: tokens.colorBrandForeground1,
  },
  title: {
    fontSize: "clamp(20px, 2.4vw, 44px)",
    fontWeight: 800,
    lineHeight: 1.15,
    marginTop: "4px",
    marginBottom: "2px",
  },
  sub: {
    fontSize: "clamp(12px, 0.9vw, 18px)",
    color: tokens.colorNeutralForeground3,
  },
});

/** Animações de entrada compartilhadas (estilo Xbox full-screen: fade + subida
 *  suave, com stagger por índice via `animationDelay` inline). */
export const useMotionStyles = makeStyles({
  riseIn: {
    animationName: {
      from: { opacity: 0, transform: "translateY(16px)" },
      to: { opacity: 1, transform: "translateY(0)" },
    },
    animationDuration: "380ms",
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: "both",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none",
      opacity: 1,
      transform: "none",
    },
  },
  fadeIn: {
    animationName: { from: { opacity: 0 }, to: { opacity: 1 } },
    animationDuration: "300ms",
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: "both",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none",
      opacity: 1,
    },
  },
});

/** Seções, cabeçalho, toolbar/chips, grade, estado vazio, gerenciar bibliotecas. */
export const useBrowseStyles = makeStyles({
  section: { marginTop: "28px" },
  sectionHead: {
    display: "flex",
    alignItems: "baseline",
    columnGap: "8px",
    marginBottom: "22px",
  },
  sectionTitle: {
    fontSize: "clamp(18px, 1.35vw, 28px)",
    fontWeight: 700,
    margin: 0,
  },
  sectionChevron: {
    color: tokens.colorNeutralForeground3,
    fontSize: "15px",
    border: "none",
    backgroundColor: "transparent",
    cursor: "pointer",
    padding: 0,
  },
  sectionSub: {
    fontSize: tokens.fontSizeBase100,
    color: tokens.colorNeutralForeground3,
    marginTop: "2px",
    marginBottom: "12px",
  },

  grid: {
    display: "grid",
    // Card fluido (`gameCardSize` escala com o viewport) — mesmo tamanho que os
    // da prateleira. `auto-fill` reflui de 2 colunas (janela estreita) a
    // dezenas (4K), sempre alinhado à esquerda.
    gridTemplateColumns: `repeat(auto-fill, ${gameCardSize})`,
    justifyContent: "start",
    rowGap: "18px",
    columnGap: `${SHELF_GAP}px`,
    "& > *": { width: "100%", minWidth: 0 },
  },

  toolbar: {
    display: "flex",
    alignItems: "center",
    columnGap: "10px",
    rowGap: "10px",
    marginTop: "6px",
    marginBottom: "4px",
    flexWrap: "wrap",
  },
  chip: {
    display: "inline-flex",
    alignItems: "center",
    columnGap: "6px",
    height: "32px",
    paddingLeft: "12px",
    paddingRight: "12px",
    borderRadius: "999px",
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: "rgba(255, 255, 255, 0.04)",
    color: tokens.colorNeutralForeground1,
    fontSize: tokens.fontSizeBase200,
    cursor: "pointer",
    ":hover": { backgroundColor: tokens.colorNeutralBackground3 },
  },
  chipOn: {
    border: `1px solid ${tokens.colorBrandStroke1}`,
    backgroundColor: "var(--reemuAccentSoft)",
  },
  count: {
    color: tokens.colorNeutralForeground3,
    fontSize: tokens.fontSizeBase200,
  },

  empty: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    rowGap: "10px",
    textAlign: "center",
    paddingTop: "72px",
    paddingBottom: "72px",
    paddingLeft: "24px",
    paddingRight: "24px",
    color: tokens.colorNeutralForeground3,
    "& h2": {
      color: tokens.colorNeutralForeground1,
      fontSize: "18px",
      margin: 0,
    },
  },
  emptyIcon: { fontSize: "44px", opacity: 0.5 },

  libManage: {
    marginTop: "14px",
    paddingTop: "12px",
    paddingBottom: "12px",
    paddingLeft: "14px",
    paddingRight: "14px",
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    borderRadius: shell.radius,
    backgroundColor: "rgba(255, 255, 255, 0.02)",
    maxWidth: "640px",
  },
  libRow: {
    display: "flex",
    alignItems: "center",
    columnGap: "16px",
    paddingTop: "8px",
    paddingBottom: "8px",
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
    ":first-of-type": { borderTop: "none" },
  },
  libPath: {
    flexGrow: 1,
    minWidth: 0,
    fontSize: tokens.fontSizeBase100,
    overflowX: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});

/** Prateleira horizontal (faixas curadas da Início). */
export const useShelfStyles = makeStyles({
  wrap: {
    position: "relative",
    minWidth: 0,
    width: "calc(100% + 8px)",
    maxWidth: "none",
    marginLeft: "-4px",
    marginRight: "-4px",
    // Compensa o padding vertical do `.shelf` (folga pro card crescer no foco
    // sem ser cortado pelo `overflow` do scroller).
    marginTop: "-14px",
    marginBottom: "-14px",
  },
  shelf: {
    display: "flex",
    minWidth: 0,
    width: "100%",
    maxWidth: "none",
    boxSizing: "border-box",
    // Sempre alinhado à esquerda: quem controla o preenchimento da linha é a
    // quantidade de cards (Shelf mede a largura), não `space-between` — que com
    // poucos itens abria buracos gigantes e jogava o último card pra fora.
    justifyContent: "flex-start",
    columnGap: `${SHELF_GAP}px`,
    overflowX: "auto",
    scrollSnapType: "x proximity",
    scrollBehavior: "smooth",
    scrollbarWidth: "none",
    // Foco por controle centraliza o card na faixa (moveFocus faz o
    // scrollIntoView; isto dá a margem).
    scrollPaddingLeft: "48px",
    scrollPaddingRight: "48px",
    paddingTop: "14px",
    paddingBottom: "14px",
    paddingLeft: "4px",
    paddingRight: "4px",
    "::-webkit-scrollbar": { display: "none" },
    "& > *": {
      width: gameCardSize,
      flexBasis: gameCardSize,
      scrollSnapAlign: "start",
      flexShrink: 0,
      flexGrow: 0,
    },
  },
});

/** Cartão-capa em retrato. */
export const useCardStyles = makeStyles({
  card: {
    width: "100%",
    aspectRatio: "1 / 1",
    display: "block",
    border: "none",
    backgroundColor: "transparent",
    // mesmo raio da arte → o anel de foco arredonda igual ao card visível
    borderRadius: shell.radius,
    padding: 0,
    cursor: "pointer",
    textAlign: "left",
    color: "inherit",
    // O tile INTEIRO cresce no hover/foco (estilo Xbox full-screen) — os
    // vizinhos não mexem (transform não reflui). `will-change` evita hitch.
    willChange: "transform",
    transformOrigin: "center",
    transitionProperty: "transform, outline-color, outline-offset",
    transitionDuration: "160ms",
    transitionTimingFunction: tokens.curveDecelerateMid,
    "&:hover, &:focus, &:focus-visible": {
      transform: "scale(1.055)",
      zIndex: 2,
    },
    "&:hover [data-art] img, &:focus [data-art] img": {
      transform: "scale(1.06)",
    },
    "&:hover [data-art], &:focus [data-art]": {
      boxShadow: "0 14px 34px rgba(0, 0, 0, 0.5)",
    },
    "@media (prefers-reduced-motion: reduce)": {
      transitionProperty: "outline-color, outline-offset",
      "&:hover, &:focus, &:focus-visible": { transform: "none" },
    },
  },
  art: {
    position: "relative",
    width: "100%",
    height: "100%",
    borderRadius: shell.radius,
    overflowX: "hidden",
    overflowY: "hidden",
    backgroundImage: elevGradient,
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    boxShadow: "0 6px 14px rgba(0, 0, 0, 0.22)",
    transitionProperty: "box-shadow",
    transitionDuration: "180ms",
    transitionTimingFunction: tokens.curveEasyEase,
    "& img": {
      width: "100%",
      height: "100%",
      objectFit: "cover",
      transitionProperty: "transform",
      transitionDuration: "220ms",
      transitionTimingFunction: tokens.curveEasyEase,
    },
  },
  badge: {
    position: "absolute",
    left: "7px",
    bottom: "7px",
    paddingTop: "3px",
    paddingBottom: "3px",
    paddingLeft: "7px",
    paddingRight: "7px",
    borderRadius: "999px",
    fontSize: "9px",
    fontWeight: 700,
    letterSpacing: "0.02em",
    textTransform: "uppercase",
    backgroundColor: "rgba(0, 0, 0, 0.62)",
    color: tokens.colorNeutralForeground1,
  },
});

/** Barra de dicas de botão do controle (canto inferior direito). */
export const useHintStyles = makeStyles({
  hints: {
    position: "fixed",
    right: "22px",
    bottom: "16px",
    display: "flex",
    columnGap: "16px",
    paddingTop: "8px",
    paddingBottom: "8px",
    paddingLeft: "16px",
    paddingRight: "16px",
    borderRadius: "999px",
    backgroundColor: "rgba(0, 0, 0, 0.6)",
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground1,
    zIndex: 50,
    pointerEvents: "none",
  },
  hint: { display: "flex", alignItems: "center", columnGap: "6px" },
  glyph: {
    width: "20px",
    height: "20px",
    borderRadius: "50%",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    fontSize: tokens.fontSizeBase100,
    fontWeight: 700,
    color: "var(--reemuOnBrand)",
  },
  a: { backgroundColor: "#16c60c" },
  b: { backgroundColor: "#e74856" },
  x: { backgroundColor: "#0078d4" },
  y: { backgroundColor: "#fce100" },
  menu: {
    backgroundColor: "transparent",
    border: `1px solid ${tokens.colorNeutralForeground3}`,
    color: tokens.colorNeutralForeground1,
    fontSize: "11px",
  },
});

/** Página de detalhe do jogo (RomDetail) — estilo "página de jogo" do Xbox:
 *  hero com arte, título grande, botão Jogar, seções em painéis. */
export const useDetailStyles = makeStyles({
  root: {
    display: "flex",
    flexDirection: "column",
    rowGap: "20px",
    paddingBottom: "48px",
    maxWidth: "860px",
    opacity: 1,
    transform: "translateY(0)",
    transitionProperty: "opacity, transform",
    transitionDuration: "240ms",
    transitionTimingFunction: tokens.curveEasyEase,
    '&[data-loading="true"]': {
      opacity: 0.72,
      transform: "translateY(6px)",
    },
  },
  back: {
    alignSelf: "flex-start",
    display: "inline-flex",
    alignItems: "center",
    columnGap: "6px",
    height: "32px",
    paddingLeft: "10px",
    paddingRight: "14px",
    borderRadius: "999px",
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: "rgba(255, 255, 255, 0.04)",
    color: tokens.colorNeutralForeground1,
    fontSize: tokens.fontSizeBase200,
    cursor: "pointer",
    ":hover": { backgroundColor: tokens.colorNeutralBackground3 },
  },
  hero: {
    position: "relative",
    minHeight: "280px",
    borderRadius: shell.radiusLg,
    overflowX: "hidden",
    overflowY: "hidden",
    display: "flex",
    alignItems: "flex-end",
    backgroundImage: elevGradient,
  },
  heroArt: {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    width: "100%",
    height: "100%",
    objectFit: "cover",
  },
  heroScrim: {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    backgroundImage:
      "linear-gradient(90deg, rgba(0,0,0,0.82) 0%, rgba(0,0,0,0.35) 55%, rgba(0,0,0,0.1) 100%), linear-gradient(0deg, rgba(0,0,0,0.7), transparent 55%)",
  },
  heroBody: {
    position: "relative",
    zIndex: 1,
    padding: "26px",
    maxWidth: "72%",
  },
  title: {
    fontSize: "clamp(24px, 3vw, 40px)",
    fontWeight: 800,
    lineHeight: 1.1,
    margin: 0,
  },
  badges: {
    display: "flex",
    columnGap: "8px",
    rowGap: "6px",
    marginTop: "10px",
    flexWrap: "wrap",
  },
  badge: {
    paddingTop: "3px",
    paddingBottom: "3px",
    paddingLeft: "10px",
    paddingRight: "10px",
    borderRadius: "999px",
    fontSize: tokens.fontSizeBase200,
    fontWeight: 600,
    textTransform: "uppercase",
    letterSpacing: "0.02em",
    backgroundColor: "rgba(0, 0, 0, 0.55)",
    border: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  desc: {
    marginTop: "12px",
    fontSize: tokens.fontSizeBase300,
    lineHeight: 1.5,
    color: tokens.colorNeutralForeground2,
    display: "-webkit-box",
    WebkitLineClamp: 4,
    WebkitBoxOrient: "vertical",
    overflowX: "hidden",
    overflowY: "hidden",
  },
  path: {
    marginTop: "10px",
    fontSize: tokens.fontSizeBase100,
    color: tokens.colorNeutralForeground3,
  },
  actions: {
    display: "flex",
    columnGap: "12px",
    rowGap: "10px",
    alignItems: "end",
    flexWrap: "wrap",
  },
  section: { display: "flex", flexDirection: "column", rowGap: "10px" },
  sectionTitle: { fontSize: "16px", fontWeight: 700, margin: 0 },
  panel: {
    display: "flex",
    flexDirection: "column",
    rowGap: "12px",
    paddingTop: "14px",
    paddingBottom: "14px",
    paddingLeft: "16px",
    paddingRight: "16px",
    borderRadius: shell.radius,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: "rgba(255, 255, 255, 0.02)",
    maxWidth: "560px",
  },
  field: { display: "flex", flexDirection: "column", rowGap: "4px" },
  stateRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    columnGap: "10px",
    paddingTop: "8px",
    paddingBottom: "8px",
    paddingLeft: "12px",
    paddingRight: "12px",
    borderRadius: shell.radius,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  hint: {
    fontSize: tokens.fontSizeBase100,
    color: tokens.colorNeutralForeground3,
  },
});

/** Menu de pausa da PlayScreen (fora do `.xb-app`, então tem seu próprio
 *  anel de foco pra navegação por controle). */
export const usePauseStyles = makeStyles({
  scrim: {
    position: "fixed",
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    backgroundColor: "rgba(0, 0, 0, 0.62)",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    zIndex: 100,
    // Anel de foco no `:focus` (não só `:focus-visible`): a navegação por
    // controle foca via `.focus()` sem keydown e o WebKitGTK não marca
    // `:focus-visible` aí. O `<Button>` do Fluent só mostra o dele no
    // `:focus-visible`, então aqui a gente reforça.
    "& a:focus, & button:focus": {
      outlineWidth: "2px",
      outlineStyle: "solid",
      outlineColor: tokens.colorNeutralForeground1,
      outlineOffset: "3px",
    },
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    rowGap: "8px",
    minWidth: "280px",
    paddingTop: "22px",
    paddingBottom: "22px",
    paddingLeft: "22px",
    paddingRight: "22px",
    borderRadius: shell.radiusLg,
    backgroundColor: tokens.colorNeutralBackground2,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    color: tokens.colorNeutralForeground1,
  },
  title: { fontSize: "18px", fontWeight: 700, margin: "0 0 6px" },
  states: {
    display: "flex",
    flexDirection: "column",
    rowGap: "4px",
    maxHeight: "220px",
    overflowY: "auto",
    marginTop: "2px",
    marginBottom: "2px",
  },
  stateRow: {
    display: "flex",
    alignItems: "center",
    columnGap: "10px",
    padding: "6px",
    borderRadius: shell.radius,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: "rgba(255,255,255,0.03)",
    color: tokens.colorNeutralForeground1,
    cursor: "pointer",
    textAlign: "left",
    font: "inherit",
    ":hover": { backgroundColor: tokens.colorNeutralBackground3 },
  },
  stateLabel: {
    fontSize: tokens.fontSizeBase200,
    lineHeight: 1.3,
    minWidth: 0,
  },
  stateDate: {
    fontSize: tokens.fontSizeBase100,
    color: tokens.colorNeutralForeground3,
  },
});
