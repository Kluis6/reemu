
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
import { CARD_MIN, cardSizeCss, SHELF_GAP, SHELF_PAD } from "../lib/shelf";

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

// Largura do scrollbar customizado da `.scroll` (`::-webkit-scrollbar`
// abaixo) — extraída pra constante porque a `.topbar` precisa compensar
// exatamente esse valor no próprio padding (ver comentário em `.topbar`).
const SCROLLBAR_W = 10;

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
    // Publica a largura da rail como var CSS — `.topbar`/`.scroll` derivam o
    // padding lateral disso (`calc()`) em vez de um `clamp()` próprio e
    // independente. Antes os dois cresciam com fórmulas diferentes (rail:
    // piso 56px/3.6vw; padding: piso 12px/3vw) e só ficavam proporcionais
    // entre si acima de ~1600px de largura — numa janela de notebook comum
    // (1366-1440px), a rail já tinha travado no piso mas o padding do
    // conteúdo continuava encolhendo, e o respiro entre os dois ficava
    // desproporcional (conteúdo "descolava" da rail ao redimensionar).
    // Amarrado direto na rail, o gutter agora escala junto em qualquer
    // largura.
    ["--reemuRailW" as string]: shell.railW,
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
    // Derivado da largura real da rail (`--reemuRailW`, publicada em
    // `.app`), não de um `clamp()` independente — ver comentário em `.app`.
    // Mesma proporção que o valor antigo tinha nas telas grandes (5/6 ≈
    // 115/138 em 4K), agora válida em QUALQUER largura.
    paddingLeft: "calc(var(--reemuRailW) * 0.8333)",
    // +10px (SCROLLBAR_W): a `.scroll` reserva essa faixa pro scrollbar
    // próprio (`scrollbarGutter: "stable"` + `::-webkit-scrollbar` de
    // 10px abaixo) — a topbar não rola, então não perde essa faixa
    // sozinha. Sem compensar aqui, o relógio ficava ~10px à direita de
    // onde o conteúdo (hero/cards) realmente termina.
    paddingRight: `calc(var(--reemuRailW) * 0.9722 + ${SCROLLBAR_W}px)`,
    boxSizing: "border-box",
    flexShrink: 0,
    backgroundColor: "transparent",
    backgroundImage: "none",
    border: "none",
    boxShadow: "none",
  },
  topbarSpacer: { flexGrow: 1 },
  // Tamanho fluido pros botões de ícone "soltos" da topbar/rail (Voltar,
  // Tela cheia, Adicionar ROM, Gerenciar biblioteca…) — cor/borda continuam
  // no `navBtn` local de cada tela; aqui só width/height/fontSize, na MESMA
  // curva do `railItem` (rail ao lado) só que um degrau menor, pra não
  // ficarem do mesmo tamanho do ícone de navegação principal. Sem isto eles
  // ficam no tamanho fixo "medium" do Fluent (32px) em qualquer resolução —
  // minúsculos ao lado da rail em telas grandes/4K (medido: 32px em 1920px
  // E em 3840px, contra 52px→104px da rail). `!important`: o `<Button>` do
  // Fluent injeta width/height/padding próprios do tamanho "medium" depois
  // da nossa classe.
  navIconBtn: {
    width: "clamp(32px, 1.7vw, 78px) !important",
    height: "clamp(32px, 1.7vw, 78px) !important",
    minWidth: "0 !important",
    // O Fluent injeta `max-width: 32px` sozinho em `<Button icon>` sem
    // texto (pra travar o botão "quadrado") — sem isto, `max-width` vence
    // o `width` acima (regra do CSS, independe de `!important`) e o botão
    // nunca cresce além de 32px.
    maxWidth: "clamp(32px, 1.7vw, 78px) !important",
    padding: "0 !important",
    fontSize: "clamp(14px, 0.85vw, 34px) !important",
    // O slot de ícone do <Button> tem tamanho fixo (20px) — não acompanha o
    // fontSize acima sozinho (mesmo caso do `railQuit`, ver abaixo).
    "& .fui-Button__icon": {
      fontSize: "1em",
      width: "1em",
      height: "1em",
    },
  },
  // Mesma ideia pro avatar do perfil (rail): o `<Avatar>` do Fluent só
  // aceita tamanhos discretos via prop (`size`), que viram width/height em
  // px cru — nunca acompanham a tela sozinhos. Override aqui, curva um
  // degrau abaixo do `railItem` (o avatar sempre foi menor que os ícones de
  // navegação abaixo dele).
  railAvatarSize: {
    width: "clamp(32px, 1.6vw, 84px) !important",
    height: "clamp(32px, 1.6vw, 84px) !important",
  },
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
    // Mesma fórmula do padding da `.topbar` (derivada de `--reemuRailW`,
    // ver `.app`) — o conteúdo alinha exatamente com o botão de voltar
    // (esquerda) e o fim do relógio (direita) em QUALQUER largura, não só
    // acima de ~1600px.
    paddingLeft: "calc(var(--reemuRailW) * 0.8333)",
    paddingRight: "calc(var(--reemuRailW) * 0.9722)",
    paddingBottom: "96px",
    "::-webkit-scrollbar": { width: `${SCROLLBAR_W}px` },
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
    // `auto-fit` + `minmax(MIN, 1fr)`: o número de colunas que cabem ainda
    // varia com a tela (de 2 na janela estreita a dezenas em 4K), mas cada
    // coluna ESTICA pra dividir a largura toda da linha — sem isto
    // (`auto-fill` + card de largura fixa em `vw`), a última coluna quase
    // nunca batia exatamente na borda direita do container (mesma borda
    // onde termina a topbar/relógio), sobrando uma faixa morta variável. Sem
    // teto: numa grade rala (poucos favoritos, poucos jogos de uma
    // plataforma) os cards crescem pra preencher mesmo assim — é o
    // comportamento "dinâmico" pedido, prioriza alinhar com a borda a manter
    // um teto de tamanho fixo.
    gridTemplateColumns: `repeat(auto-fit, minmax(${CARD_MIN}px, 1fr))`,
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
    paddingLeft: `${SHELF_PAD}px`,
    paddingRight: `${SHELF_PAD}px`,
    "::-webkit-scrollbar": { display: "none" },
    "& > *": {
      // `--reemuCardW` (px, calculado por `useShelfCapacity`/`Shelf.tsx` pra
      // encher a linha exatamente) tem prioridade; o `clamp()` estático fica
      // só de fallback até a 1ª medição do `ResizeObserver` resolver (1º
      // paint) ou se JS estiver desligado.
      width: `var(--reemuCardW, ${gameCardSize})`,
      flexBasis: `var(--reemuCardW, ${gameCardSize})`,
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
    // Mesma curva de canto usada pela topbar (`clamp(12px, 3vw, 115px)` /
    // `clamp(14px, 3.5vw, 134px)`) num degrau menor — HUD fixo, não precisa
    // acompanhar 1:1, só não ficar minúsculo em 4K.
    right: "clamp(14px, 1.2vw, 44px)",
    bottom: "clamp(10px, 1vw, 32px)",
    display: "flex",
    columnGap: "clamp(10px, 1vw, 32px)",
    paddingTop: "clamp(6px, 0.5vw, 16px)",
    paddingBottom: "clamp(6px, 0.5vw, 16px)",
    paddingLeft: "clamp(12px, 1vw, 32px)",
    paddingRight: "clamp(12px, 1vw, 32px)",
    // Mesmo raio do card (`useCardStyles.card`, `borderRadiusMedium`) — era
    // `borderRadiusCircular` (pílula).
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: "rgba(0, 0, 0, 0.6)",
    border: "none",
    fontSize: "clamp(13px, 0.85vw, 30px)",
    color: "#ffffff",
    zIndex: 50,
    pointerEvents: "none",
  },
  hint: { display: "flex", alignItems: "center", columnGap: "6px" },
  glyph: {
    width: "clamp(20px, 1.2vw, 44px)",
    height: "clamp(20px, 1.2vw, 44px)",
    borderRadius: "50%",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    fontSize: "clamp(11px, 0.7vw, 26px)",
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
    // Fluido (mesma ideia do `HeroCarousel` da Início, um degrau menor) —
    // sem isto o hero do RomDetail ficava baixo/apertado no 4K enquanto o
    // título por cima já crescia até 64px via `clamp()` (título e moldura
    // descasando).
    minHeight: "clamp(220px, 22vw, 720px)",
    borderRadius: shell.radiusLg,
    overflowX: "hidden",
    overflowY: "hidden",
    display: "flex",
    // Modelo "página de produto de loja" (não mais "hub Xbox" com texto
    // ancorado embaixo): ícone + título + botões ficam no TOPO do hero, o
    // resto do banner só é pano de fundo decorativo.
    alignItems: "flex-start",
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
    // Escurece o canto SUPERIOR-esquerdo (onde ícone/título/botões ficam
    // agora) — invertido do modelo antigo (escurecia embaixo).
    backgroundImage:
      "linear-gradient(90deg, rgba(0,0,0,0.82) 0%, rgba(0,0,0,0.35) 55%, rgba(0,0,0,0.1) 100%), linear-gradient(180deg, rgba(0,0,0,0.75), transparent 60%)",
  },
  heroBody: {
    position: "relative",
    zIndex: 1,
    display: "flex",
    flexDirection: "column",
    rowGap: "clamp(14px, 1.6vw, 28px)",
    padding: "26px",
    maxWidth: "min(85%, 760px)",
  },
  heroHeader: {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    columnGap: "clamp(16px, 1.6vw, 32px)",
  },
  heroIcon: {
    flexShrink: 0,
    width: "clamp(72px, 8.5vw, 168px)",
    height: "clamp(72px, 8.5vw, 168px)",
    borderRadius: shell.radius,
    objectFit: "cover",
    border: "1px solid rgba(255, 255, 255, 0.15)",
    boxShadow: "0 6px 20px rgba(0, 0, 0, 0.45)",
    backgroundColor: tokens.colorNeutralBackground3,
  },
  heroTitleCol: {
    display: "flex",
    flexDirection: "column",
    minWidth: 0,
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
    // Um degrau menor que o antigo (24-64px): agora divide a linha com o
    // ícone em vez de ser o único elemento da faixa — não precisa mais
    // carregar sozinho a escala do hero inteiro.
    fontSize: "clamp(20px, 2.3vw, 46px)",
    fontWeight: 800,
    lineHeight: 1.15,
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
    marginTop: "8px",
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
  // Descrição: fora do hero agora (modelo "página de loja" — banner só
  // com ícone/título/ações, sinopse vem depois, sobre o fundo normal da
  // página) — `.root` já dá o espaçamento (`rowGap`) entre ela e o hero.
  desc: {
    fontSize: tokens.fontSizeBase300,
    lineHeight: 1.5,
    color: tokens.colorNeutralForeground2,
    display: "-webkit-box",
    WebkitLineClamp: 4,
    WebkitBoxOrient: "vertical",
    overflowX: "hidden",
    overflowY: "hidden",
    maxWidth: "min(85%, 760px)",
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
  // Favoritar/Editar/Informações são `size="large"` icon-only — o Fluent
  // trava esse tamanho em px cru (~40px) que não acompanha o título do hero
  // ao lado (`clamp(24px, 3vw, 64px)`, crescia sozinho até 64px em 4K
  // enquanto estes ficavam do tamanho de uma tela FHD). Mesmo truque do
  // `navIconBtn` da topbar: `max-width` porque o Fluent injeta um próprio
  // que vence o `width` mesmo com `!important` (regra de box model).
  heroActionBtn: {
    width: "clamp(40px, 2.1vw, 96px) !important",
    height: "clamp(40px, 2.1vw, 96px) !important",
    minWidth: "0 !important",
    maxWidth: "clamp(40px, 2.1vw, 96px) !important",
    fontSize: "clamp(18px, 1vw, 44px) !important",
    "& .fui-Button__icon": {
      fontSize: "1em",
      width: "1em",
      height: "1em",
    },
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
