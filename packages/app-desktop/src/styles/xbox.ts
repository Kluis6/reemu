
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
export const shell = {
  radius: "12px",
  radiusLg: "16px",
  // Rail estreito estilo modo XBOX; cresce um pouco em telas largas.
  railW: "clamp(56px, 3.6vw, 80px)",
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
    rowGap: "clamp(6px, 0.9vw, 12px)",
    paddingTop: "clamp(10px, 1.4vw, 16px)",
    paddingBottom: "clamp(10px, 1.4vw, 16px)",
    borderRight: "none",
    // Mesmo quase-preto do fundo das páginas (estilo app do Xbox — rail e
    // conteúdo no mesmo tom). Literal, imune a variação de tema.
    backgroundColor: "#0b0b0d",
  },
  railSpacer: { flexGrow: 1 },
  railSep: {
    width: "clamp(18px, 1.8vw, 26px)",
    height: "1px",
    backgroundColor: tokens.colorNeutralStroke1,
    marginTop: "4px",
    marginBottom: "4px",
  },
  railItem: {
    position: "relative",
    // !important: essa classe também vai num <Button> do Fluent (botão de
    // fechar) — o Button tem width/height/padding/minWidth próprios (do
    // tamanho "medium" default) que competiam com isso e deixavam ele fora
    // de proporção com o <NavLink> (um <a> puro, sem essa disputa).
    width: "clamp(38px, 2.7vw, 48px) !important",
    height: "clamp(38px, 2.7vw, 48px) !important",
    minWidth: "0 !important",
    maxWidth: "none !important",
    padding: "0 !important",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    flex: "none",
    color: tokens.colorNeutralForeground3,
    textDecorationLine: "none",
    fontSize: "clamp(19px, 1.4vw, 24px)",
    border: "none",
    backgroundColor: "transparent",
    cursor: "pointer",
    transitionProperty: "background-color, color, transform",
    transitionDuration: "150ms",
    transitionTimingFunction: tokens.curveEasyEase,
    ":hover": {
      backgroundColor: tokens.colorNeutralBackground3,
      color: tokens.colorNeutralForeground1,
      transform: "scale(1.03)",
    },
    '&[aria-current="page"]': {
      backgroundColor: "#3a3a3f",
      color: tokens.colorNeutralForeground1,
    },
  },
  railQuit: {
    // O slot de ícone do <Button> do Fluent tem tamanho FIXO (20px, ver
    // fui-Button__icon) — não acompanha o fontSize clamp do railItem como o
    // ícone cru do <NavLink> acompanha. Reajusta pra escalar igual.
    "& .fui-Button__icon": {
      fontSize: "1em",
      width: "1em",
      height: "1em",
    },
    ":hover": {
      backgroundColor: "rgba(220, 60, 60, 0.18)",
      color: "#ff8a8a",
    },
  },
  railBrand: {
    marginBottom: "6px",
    cursor: "pointer",
    padding: 0,
    border: "none",
    backgroundColor: "transparent",
    borderRadius: tokens.borderRadiusCircular,
    lineHeight: 0,
    outlineOffset: "2px",
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
    columnGap: "clamp(8px, 1.4vw, 14px)",
    paddingTop: "clamp(10px, 1.4vw, 16px)",
    paddingBottom: "clamp(10px, 1.4vw, 16px)",
    paddingLeft: "clamp(12px, 3vw, 28px)",
    paddingRight: "clamp(14px, 3.5vw, 36px)",
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
    paddingTop: "clamp(60px, 6.5vw, 92px)",
    // Mesmo valor do padding da `.topbar` — o conteúdo alinha exatamente com
    // o botão de voltar (esquerda) e o fim do relógio (direita).
    paddingLeft: "clamp(12px, 3vw, 28px)",
    paddingRight: "clamp(14px, 3.5vw, 36px)",
    paddingBottom: "96px",
    "::-webkit-scrollbar": { width: "10px" },
    "::-webkit-scrollbar-thumb": {
      backgroundColor: tokens.colorNeutralStroke2,
      borderRadius: "999px",
    },
  },
});

/** Animações de entrada compartilhadas (estilo Xbox full-screen: fade + subida
 *  suave, com stagger por índice via `animationDelay` inline). */
export const useMotionStyles = makeStyles({
  riseIn: {
    animationName: {
      from: { opacity: 0, transform: "translateY(10px)" },
      to: { opacity: 1, transform: "translateY(0)" },
    },
    animationDuration: "260ms",
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
    animationDuration: "200ms",
    animationTimingFunction: tokens.curveEasyEase,
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
    width: "calc(100% + 20px)",
    maxWidth: "none",
    marginLeft: "-10px",
    marginRight: "-10px",
    // Compensa o padding do `.shelf` (folga pro anel de foco não ser
    // cortado pelo `overflow` do scroller — o 1º/último card de cada linha
    // só tem essa margem pra respirar, os do meio ainda têm o SHELF_GAP).
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
    alignItems: "flex-start",
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
    paddingLeft: "10px",
    paddingRight: "10px",
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

/** Cartão de jogo — arte + título + plataforma embaixo (modelo modo XBOX). */
export const useCardStyles = makeStyles({
  // O card É a imagem (quadrado). Sem escala no card — o zoom é na imagem.
  // O nome fica sobre a imagem e só aparece no hover/foco.
  card: {
    position: "relative",
    width: "100%",
    aspectRatio: "1 / 1",
    display: "block",
    border: "none",
    backgroundColor: "transparent",
    borderRadius: shell.radius,
    // SEM overflow:hidden aqui — o WebKitGTK corta o próprio outline de foco
    // quando ele "vaza" pra fora de um elemento com overflow:hidden nele
    // mesmo (o anel ficava com pedaço cortado). Quem clipa a arte/zoom
    // agora é o `.art` (embaixo), não o card inteiro.
    padding: 0,
    cursor: "pointer",
    textAlign: "left",
    color: "inherit",
    transitionProperty: "outline-color, outline-offset",
    transitionDuration: "170ms",
    transitionTimingFunction: tokens.curveEasyEase,
    // O <Card> do Fluent desenha o próprio "anel" de foco via `::after` COM
    // BORDA (`[data-fui-focus-visible]`/`[data-fui-focus-within]`, ver
    // @fluentui/react-card useCardStyles) — some daqui. O anel que fica é o
    // `outline` global do `.app` (`& [tabindex]:focus`), o mesmo usado em
    // todo o resto do app pra navegação por controle. `!important` porque o
    // Card injeta essa regra DEPOIS da nossa (perde no empate de ordem).
    "&[data-fui-focus-visible]::after, &[data-fui-focus-within]::after": {
      border: "none !important",
    },
    // Afasta mais o anel de foco (o global do `.app` usa 3px) — com o zoom
    // da imagem por baixo, rente ficava apertado. `!important`: precisa
    // ganhar do `.app [tabindex]:focus`, que tem mais specificity.
    "&:focus, &:focus-visible": {
      outlineOffset: "6px !important",
    },
    "&:hover, &:focus-within, &:focus-visible": {
      zIndex: 2,
    },
    // zoom da imagem (não do card) — sutil, não um "salto"
    "&:hover [data-art] img, &:focus-within [data-art] img": {
      transform: "scale(1.045)",
    },
    // revela o nome sobreposto
    "&:hover [data-meta], &:focus-within [data-meta]": {
      opacity: 1,
      transform: "translateY(0)",
    },
    "@media (prefers-reduced-motion: reduce)": {
      "&:hover [data-art] img, &:focus-within [data-art] img": {
        transform: "none",
      },
    },
  },
  art: {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    borderRadius: shell.radius,
    overflowX: "hidden",
    overflowY: "hidden",
    backgroundImage: elevGradient,
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    "& img": {
      width: "100%",
      height: "100%",
      objectFit: "cover",
      objectPosition: "center",
      // Some até o onLoad disparar — uma capa remota ainda decodificando
      // (linha por linha) podia aparecer com uma faixa escura no topo por
      // um instante; agora só fica visível já pronta.
      opacity: 0,
      transitionProperty: "transform, opacity",
      // Zoom suave mas sem arrastar: nem o "salto" de antes nem lento demais.
      transitionDuration: "320ms, 220ms",
      transitionTimingFunction: tokens.curveEasyEase,
      "&[data-loaded]": { opacity: 1 },
    },
  },
  meta: {
    position: "absolute",
    left: 0,
    right: 0,
    bottom: 0,
    // Cantos de baixo arredondados igual à arte — o card não clipa mais
    // (era o que cortava o anel de foco), então quem tem que ficar
    // arredondado por conta própria é cada camada.
    borderBottomLeftRadius: shell.radius,
    borderBottomRightRadius: shell.radius,
    display: "flex",
    flexDirection: "column",
    rowGap: "1px",
    paddingTop: "18px",
    paddingBottom: "9px",
    paddingLeft: "10px",
    paddingRight: "10px",
    backgroundImage:
      "linear-gradient(0deg, rgba(0,0,0,0.9) 0%, rgba(0,0,0,0.5) 55%, transparent 100%)",
    opacity: 0,
    transform: "translateY(6px)",
    transitionProperty: "opacity, transform",
    transitionDuration: "200ms",
    transitionTimingFunction: tokens.curveEasyEase,
    pointerEvents: "none",
    "@media (prefers-reduced-motion: reduce)": { transform: "none" },
  },
  titleText: {
    fontSize: tokens.fontSizeBase200,
    fontWeight: tokens.fontWeightSemibold,
    lineHeight: 1.2,
    color: "#ffffff",
    display: "-webkit-box",
    WebkitLineClamp: 2,
    WebkitBoxOrient: "vertical",
    overflowX: "hidden",
    overflowY: "hidden",
  },
  subText: {
    fontSize: tokens.fontSizeBase100,
    color: "rgba(255, 255, 255, 0.72)",
    whiteSpace: "nowrap",
    overflowX: "hidden",
    textOverflow: "ellipsis",
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
