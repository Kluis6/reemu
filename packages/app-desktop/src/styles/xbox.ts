
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
  // Rail estreito estilo modo XBOX; cresce com a tela. Teto antigo (80px)
  // travava em ~2222px de viewport — em 4K (3840px) ficava minúsculo. Mesma
  // inclinação (3.6vw), teto estendido pra continuar crescendo até 4K real.
  railW: "clamp(56px, 3.6vw, 138px)",
};

// Gradiente de superfície elevada (cartões, hero) a partir dos neutros do tema.
const elevGradient = `linear-gradient(135deg, ${tokens.colorNeutralBackground4}, ${tokens.colorNeutralBackground3})`;

// Largura de um card (mesma fórmula do JS que conta quantos cabem — ver
// lib/shelf.ts). Fluida: ~150px em janela estreita, até 320px em 2.7K+.
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
    // Cor de marca (verde no tema padrão/claro) — igual ao destaque do item
    // selecionado no dashboard Xbox de verdade, não um cinza neutro.
    "& a:focus, & input:focus, & [tabindex]:focus": {
      outlineWidth: "3px",
      outlineStyle: "solid",
      outlineColor: tokens.colorBrandStroke1,
      outlineOffset: "2px",
    },
  },

  rail: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    rowGap: "clamp(6px, 0.9vw, 35px)",
    paddingTop: "clamp(10px, 1.4vw, 54px)",
    paddingBottom: "clamp(10px, 1.4vw, 54px)",
    borderRight: "none",
    // Hierarquia visual (Fluent 2): chrome de navegação persistente fica um
    // tom ACIMA do conteúdo (`colorNeutralBackground1`), não no mesmo tom —
    // é o que separa a rail (estrutural, sempre visível) da área de
    // conteúdo (rola por baixo). Mesmo tom que os cards/paineis já usam
    // (`useCardStyles`, diálogos), então o rail lê como "uma superfície",
    // igual a eles.
    backgroundColor: tokens.colorNeutralBackground2,
  },
  railSpacer: { flexGrow: 1 },
  railSep: {
    width: "clamp(18px, 1.8vw, 69px)",
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
    // Piso 44px (não 38px): fluent2.microsoft.design/layout — alvo mínimo de
    // toque/clique pra web. Numa janela 1366-1920px (a faixa "XX-large" da
    // própria doc, bem comum) o `2.7vw` sozinho ficava abaixo disso. Teto
    // estendido de 48 pra 104px — travava bem antes de 4K (~1778px de
    // viewport) e ficava minúsculo num monitor/TV grande.
    width: "clamp(44px, 2.7vw, 104px) !important",
    height: "clamp(44px, 2.7vw, 104px) !important",
    minWidth: "0 !important",
    maxWidth: "none !important",
    padding: "0 !important",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    flex: "none",
    color: tokens.colorNeutralForeground3,
    textDecorationLine: "none",
    fontSize: "clamp(19px, 1.4vw, 54px)",
    border: "none",
    backgroundColor: "transparent",
    cursor: "pointer",
    transitionProperty: "background-color, color",
    transitionDuration: "150ms",
    transitionTimingFunction: tokens.curveEasyEase,
    ":hover": {
      backgroundColor: tokens.colorNeutralBackground3,
      color: tokens.colorNeutralForeground1,
    },
    '&[aria-current="page"]': {
      backgroundColor: "var(--reemuActiveBg)",
      color: "var(--reemuActiveFg)",
    },
    // Zoom só no glifo (não na pílula inteira) — cresce suave no hover/foco
    // e volta sozinho ao sair, via transition no próprio ícone.
    "& svg": {
      transitionProperty: "transform",
      transitionDuration: "220ms",
      transitionTimingFunction: tokens.curveEasyEase,
    },
    "&:hover svg, &:focus svg, &:focus-visible svg": {
      transform: "scale(1.18)",
    },
    // Feedback de clique: encolhe (zoom out) no instante do toque/clique —
    // Griffel prioriza o bucket `:active` acima de `:hover`/`:focus`, então
    // isto já ganha deles sem precisar de `!important`.
    "&:active svg": {
      transform: "scale(0.85)",
    },
    "@media (prefers-reduced-motion: reduce)": {
      "& svg": { transitionProperty: "none" },
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
    columnGap: "clamp(8px, 1.4vw, 54px)",
    paddingTop: "clamp(10px, 1.4vw, 54px)",
    paddingBottom: "clamp(10px, 1.4vw, 54px)",
    paddingLeft: "clamp(12px, 3vw, 115px)",
    paddingRight: "clamp(14px, 3.5vw, 134px)",
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
  // Posicionamento (pílula centralizada, independente do conteúdo lateral) —
  // borda e raio ficam com o padrão do `<SearchBox>` do Fluent
  // (`appearance="filled-darker"` já pega essas do tema sozinho); só o FUNDO
  // é sobrescrito abaixo pro mesmo tom da sidebar (`colorNeutralBackground2`)
  // em vez do `colorNeutralBackground3` que "filled-darker" traria.
  //
  // O foco em si o Fluent já resolve sozinho, SEM outline: o `<Input>`
  // (base do SearchBox) marca `:focus-within{ outline: 2px solid
  // transparent }` de propósito e desenha o realce como uma barrinha
  // animada embaixo (`::after`, cor `colorCompoundBrandStroke`, já do
  // tema). O `<input>` cru lá dentro não tem cantos arredondados — a
  // regra global `.app "& input:focus"` (pro resto do app) desenhava um
  // outline colorido normal EM CIMA disso, quadrado, por cima do fundo
  // arredondado do SearchBox. Desliga essa regra global só aqui.
  search: {
    position: "absolute",
    left: "50%",
    transform: "translateX(-50%)",
    // Teto subiu de 780 pra 1100px — mas não pra 42vw cheio (~1613px em 4K):
    // uma busca centralizada não deve virar a largura da tela toda, só
    // acompanhar o crescimento geral em vez de travar em telas grandes.
    width: "clamp(200px, 42vw, 1100px)",
    maxWidth: "calc(100% - 160px)",
    backgroundColor: `${tokens.colorNeutralBackground2} !important`,
    "& input:focus": {
      outline: "none !important",
    },
  },
  clock: {
    color: tokens.colorNeutralForeground3,
    fontVariantNumeric: "tabular-nums",
    fontSize: "clamp(15px, 1vw, 38px)",
    fontWeight: 600,
    lineHeight: 1,
    letterSpacing: "0.01em",
  },
  gamepadStatus: {
    display: "flex",
    alignItems: "center",
    color: tokens.colorNeutralForeground3,
    fontSize: "clamp(16px, 1.1vw, 42px)",
  },
  scroll: {
    scrollBehavior: "smooth",
    flexGrow: 1,
    minWidth: 0,
    overflowY: "auto",
    scrollbarGutter: "stable",
    boxSizing: "border-box",
    // Só precisa limpar a altura da `.topbar` (padding + conteúdo, este sem
    // `clamp` — os botões da Fluent não escalam) com uma folga; não faz
    // sentido crescer no mesmo 6.5vw até 4K cheio (viraria vão vazio enorme).
    paddingTop: "clamp(60px, 6.5vw, 172px)",
    // Mesmo valor do padding da `.topbar` — o conteúdo alinha exatamente com
    // o botão de voltar (esquerda) e o fim do relógio (direita).
    paddingLeft: "clamp(12px, 3vw, 115px)",
    paddingRight: "clamp(14px, 3.5vw, 134px)",
    paddingBottom: "96px",
    "::-webkit-scrollbar": { width: "10px" },
    "::-webkit-scrollbar-thumb": {
      backgroundColor: tokens.colorNeutralStroke2,
      borderRadius: tokens.borderRadiusCircular,
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
    borderRadius: tokens.borderRadiusCircular,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: "var(--reemuFillWeak)",
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
    backgroundColor: "var(--reemuFillWeak)",
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
    // Raio padrão do Fluent (`--fui-Card--border-radius` no tamanho
    // "medium" resolve pra isto) — não um valor customizado.
    borderRadius: tokens.borderRadiusMedium,
    // SEM overflow:hidden aqui — quem clipa a arte/zoom é o `.art`
    // (embaixo), não o card inteiro.
    padding: 0,
    cursor: "pointer",
    textAlign: "left",
    color: "inherit",
    // O <Card> do Fluent desenha o próprio "anel" de foco via `::after` COM
    // BORDA (`[data-fui-focus-visible]`/`[data-fui-focus-within]`, ver
    // @fluentui/react-card useCardStyles) — some daqui. O anel que fica é o
    // `outline` global do `.app` (`& [tabindex]:focus`), o mesmo usado em
    // todo o resto do app pra navegação por controle. `!important` porque o
    // Card injeta essa regra DEPOIS da nossa (perde no empate de ordem).
    "&[data-fui-focus-visible]::after, &[data-fui-focus-within]::after": {
      border: "none !important",
    },
    // Afasta mais o anel de foco (o global do `.app` usa 2px) — com o zoom
    // da imagem por baixo, rente ficava apertado. `!important`: precisa
    // ganhar do `.app [tabindex]:focus`, que tem mais specificity.
    "&:focus, &:focus-visible": {
      outlineOffset: "4px !important",
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
    borderRadius: tokens.borderRadiusMedium,
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
    // Cantos de baixo arredondados igual à arte — o card não clipa
    // (`overflow` fica na `.art`), então quem tem que ficar arredondado
    // por conta própria é cada camada. Raio padrão do Fluent, igual ao
    // `.card`/`.art`.
    borderBottomLeftRadius: tokens.borderRadiusMedium,
    borderBottomRightRadius: tokens.borderRadiusMedium,
    display: "flex",
    flexDirection: "column",
    rowGap: "1px",
    paddingTop: "18px",
    paddingBottom: "9px",
    paddingLeft: "10px",
    paddingRight: "10px",
    backgroundImage:
      "linear-gradient(0deg, rgba(0,0,0,0.96) 0%, rgba(0,0,0,0.65) 55%, transparent 100%)",
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
    color: "#ffffff",
    whiteSpace: "nowrap",
    overflowX: "hidden",
    textOverflow: "ellipsis",
  },
});

/** Barra de dicas de botão do controle (canto inferior direito). */
export const useHintStyles = makeStyles({
  // Chip HUD sempre escuro, IMUNE ao tema (igual overlay de botão de
  // controle em qualquer console/jogo — inclusive no próprio Xbox, os
  // glifos de botão ficam sobre um fundo escuro translúcido mesmo com o
  // dashboard no modo claro). Por isso o texto é branco literal, não um
  // token que inverteria com o tema e ficaria ilegível (branco no claro).
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
    // Mesmo raio do card (`useCardStyles.card`, `borderRadiusMedium`) — era
    // `borderRadiusCircular` (pílula).
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: "rgba(0, 0, 0, 0.6)",
    border: "none",
    fontSize: tokens.fontSizeBase200,
    color: "#ffffff",
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
    border: "1px solid rgba(255, 255, 255, 0.4)",
    color: "#ffffff",
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
    maxWidth: "min(72%, 640px)",
  },
  platform: {
    display: "block",
    fontSize: tokens.fontSizeBase300,
    fontWeight: 600,
    letterSpacing: "0.02em",
    color: "var(--reemuBrandText)",
    marginBottom: "4px",
  },
  title: {
    // Teto subiu de 40 pra 64px (não os 115px que 3vw daria em 4K cheio —
    // um título de jogo não precisa ficar do tamanho de um outdoor).
    fontSize: "clamp(24px, 3vw, 64px)",
    fontWeight: 800,
    lineHeight: 1.1,
    margin: 0,
    // Sempre branco: o hero tem `heroScrim` escuro por baixo em qualquer
    // tema (claro ou escuro) — a cor do tema (`colorNeutralForeground1`)
    // ficaria ilegível no tema claro.
    color: "#fff",
  },
  badges: {
    display: "flex",
    columnGap: "8px",
    rowGap: "6px",
    marginTop: "10px",
    flexWrap: "wrap",
  },
  // fluent2.microsoft.design/typography: "use sentence case, nunca all caps".
  badge: {
    paddingTop: "3px",
    paddingBottom: "3px",
    paddingLeft: "10px",
    paddingRight: "10px",
    borderRadius: tokens.borderRadiusCircular,
    fontSize: tokens.fontSizeBase200,
    fontWeight: 600,
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
  actions: {
    display: "flex",
    columnGap: "12px",
    rowGap: "10px",
    alignItems: "end",
    flexWrap: "wrap",
    // O `[tabindex]:focus` global do `.app` (useShellStyles) não cobre
    // `<button>` puro (o `<Button>` do Fluent não recebe `tabindex` — só
    // `a`/`input`/elementos com `tabindex` explícito ganham o anel dali).
    // Mesmo par de regras já usado em `usePauseStyles.scrim` (que também
    // precisa do seu próprio anel): apaga o "halo" nativo do Fluent
    // (`boxShadow`/`border-color` via `[data-fui-focus-visible]` na própria
    // raiz `.fui-Button`, não um `::after` como o Card) e desenha o anel
    // padrão do reemu direto no `:focus` (não só `:focus-visible`: o pulso
    // do gamepad foca via `.focus()` sem keydown).
    "& .fui-Button[data-fui-focus-visible]": {
      border: "1px solid transparent !important",
      boxShadow: "none !important",
    },
    "& button:focus": {
      outline: `3px solid ${tokens.colorBrandStroke1} !important`,
      outlineOffset: "2px !important",
    },
  },
  // Favoritar/Editar/Informações: `appearance="secondary"` sem a borda
  // visível do Fluent. `1px solid transparent` (NÃO `border: none`) sempre
  // — inclusive em repouso, não só no foco. O botão "Jogar" ao lado
  // (`appearance="primary"`) já reserva 1px de borda transparente o tempo
  // todo; com `border: none` aqui, esses 3 botões ficavam 2px menores que o
  // "Jogar" em repouso, e a regra de foco em `.actions` (acima) reintroduzia
  // o 1px só quando focados — essa mudança de tamanho no exato instante do
  // foco/desfoco era o "piscar" percebido. Reservando o mesmo 1px sempre, o
  // tamanho fica idêntico ao "Jogar" em qualquer estado.
  noBorderButton: {
    border: "1px solid transparent !important",
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
    // Sólido (não `--reemuFillWeak`, que é translúcido) — e sem `maxWidth`:
    // acompanha a largura do hero/ações acima, como o resto da página.
    backgroundColor: tokens.colorNeutralBackground2,
  },
  field: { display: "flex", flexDirection: "column", rowGap: "4px" },
  // Mesma matemática do grid de 3 colunas do `CoreOptions` (`repeat(3,
  // minmax(0, 1fr))` + `columnGap: spacingHorizontalM`) — alinha a borda
  // direita do seletor de core com a da 1ª coluna das opções do core.
  coreField: {
    width: `calc((100% - 2 * ${tokens.spacingHorizontalM}) / 3)`,
    minWidth: "200px",
  },
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
  // 1/3 da largura da tela — nenhum dos presets do Drawer (`size`) bate com
  // isso, então sobrescreve a CSS var interna que o Fluent usa pra largura.
  infoDrawer: {
    ["--fui-Drawer--size" as string]: "33.34vw",
  },
  infoBody: {
    display: "flex",
    flexDirection: "column",
    rowGap: "16px",
    paddingBottom: "24px",
  },
  infoCover: {
    width: "100%",
    borderRadius: shell.radius,
    aspectRatio: "16 / 9",
    objectFit: "cover",
    backgroundColor: tokens.colorNeutralBackground3,
  },
  infoRow: {
    display: "flex",
    flexDirection: "column",
    rowGap: "2px",
  },
  infoLabel: {
    fontSize: tokens.fontSizeBase100,
    color: tokens.colorNeutralForeground3,
  },
  infoValue: {
    fontSize: tokens.fontSizeBase300,
    wordBreak: "break-word",
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
    // `:focus-visible` aí. `!important`: o `.r1f29ykk[data-fui-focus-visible]`
    // do próprio Button (ver abaixo) tem mais especificidade que isto
    // (classe+atributo vs classe+pseudo) e ganharia o `outline` sem isso.
    "& a:focus, & button:focus": {
      outline: `3px solid ${tokens.colorBrandStroke1} !important`,
      outlineOffset: "2px !important",
    },
    // O <Button> do Fluent NÃO usa `::after` pra isso — muda a própria
    // `border-color` e desenha um `box-shadow` inset direto no elemento
    // quando `[data-fui-focus-visible]` (raiz `.fui-Button`, não um
    // pseudo). Neutraliza os dois; o anel que sobra é só o outline acima.
    "& .fui-Button[data-fui-focus-visible]": {
      border: "1px solid transparent !important",
      boxShadow: "none !important",
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
    // Raio padrão do <Button> do Fluent (não `shell.radius`) — pedido
    // direto pra não destoar do resto dos botões do menu.
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: "var(--reemuFillWeak)",
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
