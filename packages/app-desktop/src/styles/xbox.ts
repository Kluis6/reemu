
/**
 * Linguagem visual do "modo Xbox" (ver docs/design/xbox-mode-reference.md e
 * docs/design/fluent2.md) em **Griffel** (`makeStyles` + `tokens` do Fluent 2),
 * substituindo o antigo `xbox.css`.
 *
 * Cor de marca, elevações e fundo da casca vêm do TEMA (ver styles/themes.ts):
 * tokens Fluent (`colorBrand*`, `colorNeutralBackground*` já escurecidos) +
 * tokens custom `--reemu*`. Trocar de tema reajusta tudo. Só o que não é cor
 * (raio 12/16, rail 68px) fica em `shell`.
 *
 * CUIDADO (WebKitGTK, ver src-tauri/src/main.rs): nada de `backdrop-filter`
 * nem `radial-gradient` multicamada em elemento `position: fixed`.
 */
import { makeStyles, shorthands, tokens } from "@fluentui/react-components";
import { cardSizeCss, SHELF_GAP, SHELF_PAD } from "../lib/shelf";
import * as M from "./metrics";

// Só os valores NÃO-cor do "console look". Cor de marca, elevações e o fundo
// da casca vêm do tema (tokens Fluent + tokens custom `--reemu*`, ver
// styles/themes.ts) — trocar de tema reajusta tudo.
//
// Medidas em epx (styles/metrics.ts): a tela lógica tem sempre pelo menos
// 1366×768, como o app Xbox do Windows, e cresce com a resolução pelo zoom da tela inteira
// (lib/uiScale.ts) — por isso nada aqui usa `vw`.
export const shell = {
  radius: `${M.RADIUS}px`,
  radiusLg: `${M.RADIUS_LG}px`,
  railW: `${M.RAIL_W}px`,
};

// Gradiente de superfície elevada (cartões, hero) a partir dos neutros do tema.
const elevGradient = `linear-gradient(135deg, ${tokens.colorNeutralBackground4}, ${tokens.colorNeutralBackground3})`;

// Largura de um card (a mesma que o JS usa pra contar quantos cabem — ver
// lib/shelf.ts): 6 por fileira numa tela 16:9.
const gameCardSize = cardSizeCss;

// Distância do anel de foco até o elemento (o anel tem 3 epx).
const FOCUS_OFFSET = 4;

// Largura do scrollbar customizado da `.scroll` (`::-webkit-scrollbar`
// abaixo) — a `.topbar` compensa exatamente esse valor no próprio padding.
const SCROLLBAR_W = M.SCROLLBAR_W;

// Folga vertical pra limpar a `.topbar` flutuante. Publicada como
// `--reemuTopbarH` em `.app` (ver abaixo) porque páginas com hero "colado no
// topo" (RomDetail) cancelam exatamente esse valor via margin negativo.
const TOPBAR_CLEARANCE = `${M.TOPBAR_CLEARANCE}px`;

// Padding lateral compartilhado por `.topbar`/`.scroll` — um hero de sangria
// total (RomDetail) cancela exatamente esse valor com margin negativo. A
// direita fecha na área segura da TV (48 epx da borda): a `.scroll` perde
// `SCROLLBAR_W` pro próprio gutter, a `.topbar` não — por isso dois nomes.
const PAGE_PAD_L = `${M.PAGE_PAD_L}px`;
const SCROLL_PAD_R = `${M.PAGE_PAD_R - SCROLLBAR_W}px`;
// A topbar é mais larga que o conteúdo, como no app Xbox (ver metrics.ts).
const TOPBAR_PAD_L = `${M.TOPBAR_PAD_L}px`;
const TOPBAR_PAD_R = `${M.TOPBAR_PAD_R}px`;

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
    // Largura da rail como var CSS (quem precisa alinhar com ela lê daqui).
    ["--reemuRailW" as string]: shell.railW,
    // Folga vertical que a `.scroll` reserva pra topbar flutuante não
    // cobrir o início do conteúdo (topbar é `position:absolute` por cima).
    // Publicada aqui pra páginas com hero "colado no topo" (RomDetail)
    // poderem cancelar essa folga com margin negativo em vez de duplicar
    // o valor (ver `.hero` em `useDetailStyles`).
    ["--reemuTopbarH" as string]: TOPBAR_CLEARANCE,
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
      // 4 epx: o anel respira em volta do item (pedido do usuário). Na rail
      // ele ocupa 4 + 3 = 7 de cada lado do item de 50 e cabe nos 9 que
      // sobram dos 68.
      outlineOffset: `${FOCUS_OFFSET}px`,
    },
    // Abas (`<Tab>` do Fluent): são `<button>` sem `tabindex`, então a regra
    // de cima não pega — valia o anel do próprio Fluent, um `box-shadow`
    // branco (`colorStrokeFocus2`) em vez da cor de destaque do tema. Mesmo
    // anel do resto do app; o do Fluent sai.
    "& .fui-Tab:focus": {
      outlineWidth: "3px",
      outlineStyle: "solid",
      outlineColor: tokens.colorBrandStroke1,
      outlineOffset: "2px",
    },
    "& .fui-Tab[data-fui-focus-visible]": {
      boxShadow: "none",
    },
    // Botões do Fluent: o foco de teclado deles (`[data-fui-focus-visible]`)
    // é uma borda + `box-shadow` rente ao botão, sem afastamento. Troca pelo
    // mesmo anel de cima. `!important`: a regra do Fluent é classe+atributo.
    "& .fui-Button[data-fui-focus-visible]": {
      // Griffel não aceita a abreviada `borderColor`; `shorthands` expande nas
      // quatro bordas.
      ...shorthands.borderColor("transparent !important"),
      boxShadow: "none !important",
      outline: `3px solid ${tokens.colorBrandStroke1} !important`,
      outlineOffset: `${FOCUS_OFFSET}px !important`,
    },
  },

  rail: {
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    // Itens a cada 56 epx, avatar centrado em y = 31 (app Xbox, metrics.ts).
    rowGap: `${M.RAIL_GAP}px`,
    paddingTop: `${M.RAIL_TOP}px`,
    paddingBottom: `${M.RAIL_TOP}px`,
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
    width: "28px",
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
    // 50×46 epx, o retângulo do item ativo no app Xbox.
    width: `${M.RAIL_ITEM_W}px !important`,
    height: `${M.RAIL_ITEM_H}px !important`,
    minWidth: "0 !important",
    maxWidth: "none !important",
    padding: "0 !important",
    display: "grid",
    alignItems: "center",
    justifyItems: "center",
    flex: "none",
    color: tokens.colorNeutralForeground3,
    textDecorationLine: "none",
    // Ícone de 24 epx, como na rail do app Xbox (múltiplo de 4: o SVG não
    // borra).
    fontSize: `${M.RAIL_ICON}px`,
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
    // Cresce pelo TAMANHO (redesenha nítido), não por `scale` (que amplia a
    // imagem já rasterizada — borrava no Windows): 24 → ~29 epx.
    "& svg": {
      fontSize: "1em",
      transitionProperty: "font-size, transform",
      transitionDuration: "220ms",
      transitionTimingFunction: tokens.curveEasyEase,
    },
    "&:hover svg, &:focus svg, &:focus-visible svg": {
      fontSize: "1.2em",
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
    // fui-Button__icon) — não acompanha o fontSize do railItem (nem o
    // aumento no foco) como o ícone cru do <NavLink>. Reajusta pra igualar.
    "& .fui-Button__icon": {
      fontSize: "1em",
      width: "1em",
      height: "1em",
    },
  },
  railBrand: {
    // 7 + rowGap (10): o 1º item começa em y = 64, como no app Xbox.
    marginBottom: "7px",
    cursor: "pointer",
    padding: 0,
    border: "none",
    backgroundColor: "transparent",
    borderRadius: tokens.borderRadiusCircular,
    lineHeight: 0,
    outlineOffset: `${FOCUS_OFFSET}px`,
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
    columnGap: "12px",
    // Controles a partir de y = 32; altura total = TOPBAR_CLEARANCE.
    paddingTop: `${M.TOPBAR_TOP}px`,
    paddingBottom: "16px",
    paddingLeft: TOPBAR_PAD_L,
    // Inclui o SCROLLBAR_W: a `.scroll` reserva essa faixa pro scrollbar
    // próprio (`scrollbarGutter: "stable"`) — a topbar não rola, então não
    // perde essa faixa sozinha. Assim o relógio termina na mesma borda que o
    // conteúdo (hero/cards).
    paddingRight: TOPBAR_PAD_R,
    boxSizing: "border-box",
    flexShrink: 0,
    backgroundColor: "transparent",
    backgroundImage: "none",
    border: "none",
    boxShadow: "none",
  },
  topbarSpacer: { flexGrow: 1 },
  // Botões de ícone "soltos" da topbar/rail (Voltar, Tela cheia, Adicionar
  // ROM, Gerenciar biblioteca…) — cor/borda continuam no `navBtn` local de
  // cada tela; aqui só width/height/fontSize: o alvo mínimo de 32 epx, um
  // degrau abaixo do `railItem` (40). `!important`: o `<Button>` do Fluent
  // injeta width/height/padding próprios do tamanho "medium" depois da nossa
  // classe.
  navIconBtn: {
    width: `${M.TARGET_MIN}px !important`,
    height: `${M.TARGET_MIN}px !important`,
    minWidth: "0 !important",
    // O Fluent injeta `max-width: 32px` sozinho em `<Button icon>` sem
    // texto (pra travar o botão "quadrado") — sem isto, `max-width` vence
    // o `width` acima (regra do CSS, independe de `!important`) e o botão
    // nunca cresce além de 32px.
    maxWidth: `${M.TARGET_MIN}px !important`,
    padding: "0 !important",
    fontSize: "16px !important",
    // O slot de ícone do <Button> tem tamanho fixo (20px) — não acompanha o
    // fontSize acima sozinho (mesmo caso do `railQuit`, ver abaixo).
    "& .fui-Button__icon": {
      fontSize: "1em",
      width: "1em",
      height: "1em",
    },
  },
  // Ícone um pouco maior no mesmo botão (voltar, adicionar ROM, gerenciar
  // biblioteca) — vem depois de `navIconBtn` no mergeClasses.
  navIconLg: {
    fontSize: `${M.ICON}px !important`,
  },
  // Avatar do perfil (rail): um degrau abaixo do `railItem`, como sempre
  // foi em relação aos ícones de navegação abaixo dele.
  railAvatarSize: {
    width: "32px !important",
    height: "32px !important",
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
    // 500 epx, como a busca do app Xbox — centralizada na área à direita da
    // rail.
    width: `${M.SEARCH_W}px`,
    maxWidth: "calc(100% - 160px)",
    backgroundColor: `${tokens.colorNeutralBackground2} !important`,
    "& input:focus": {
      outline: "none !important",
    },
  },
  clock: {
    color: tokens.colorNeutralForeground3,
    fontVariantNumeric: "tabular-nums",
    fontSize: tokens.fontSizeBase300,
    fontWeight: 600,
    lineHeight: 1,
    letterSpacing: "0.01em",
  },
  gamepadStatus: {
    display: "flex",
    alignItems: "center",
    color: tokens.colorNeutralForeground3,
    fontSize: `${M.ICON}px`,
  },
  scroll: {
    scrollBehavior: "smooth",
    flexGrow: 1,
    minWidth: 0,
    overflowY: "auto",
    scrollbarGutter: "stable",
    boxSizing: "border-box",
    // Limpa a altura da `.topbar` flutuante.
    paddingTop: `var(--reemuTopbarH, ${TOPBAR_CLEARANCE})`,
    // Mesmo padding da `.topbar` — o conteúdo alinha com o botão de voltar
    // (esquerda) e o fim do relógio (direita).
    paddingLeft: PAGE_PAD_L,
    paddingRight: SCROLL_PAD_R,
    // Área segura de baixo + a barra de dicas, que não pode cobrir nada.
    paddingBottom: `${M.PAGE_PAD_B}px`,
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
    // `backwards`: ver RouteTransition (texto nítido depois da entrada)
    animationFillMode: "backwards",
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
    animationFillMode: "backwards",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none",
      opacity: 1,
    },
  },
});

/** Seções, cabeçalho, toolbar/chips, grade, estado vazio, gerenciar bibliotecas. */
export const useBrowseStyles = makeStyles({
  // Do fim de uma fileira ao título da próxima (app Xbox, metrics.ts).
  section: { marginTop: `${M.SECTION_GAP}px` },

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
    //
    // Mínimo = o MESMO tamanho do card das prateleiras da tela inicial
    // (`cardSizeCss`, 6 por fileira em 16:9).
    //
    // `auto-fill` (não `auto-fit`): com a linha cheia as colunas continuam
    // esticando até a borda; numa grade rala as colunas vazias ficam
    // reservadas e o card mantém o tamanho da prateleira — com `auto-fit`, 3
    // jogos viravam cards de 1/3 da tela cada, fora de escala com o resto do
    // app (revisão de UI, 2026-09-25).
    gridTemplateColumns: `repeat(auto-fill, minmax(${gameCardSize}, 1fr))`,
    rowGap: `${SHELF_GAP}px`,
    columnGap: `${SHELF_GAP}px`,
    "& > *": { width: "100%", minWidth: 0 },
  },

  toolbar: {
    display: "flex",
    alignItems: "center",
    columnGap: "10px",
    rowGap: "10px",
    marginTop: "6px",
    // Mesmo respiro do `rowGap` da grade (20px): com os 4px de antes a
    // primeira linha de cards ficava colada na linha de filtros, enquanto
    // acima dela as tabs tinham 24px de folga.
    marginBottom: `${SHELF_GAP}px`,
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
    width: `calc(100% + ${2 * SHELF_PAD}px)`,
    maxWidth: "none",
    marginLeft: `-${SHELF_PAD}px`,
    marginRight: `-${SHELF_PAD}px`,
    // Compensa o padding do `.shelf` (folga pro anel de foco não ser
    // cortado pelo `overflow` do scroller — o 1º/último card de cada linha
    // só tem essa margem pra respirar, os do meio ainda têm o SHELF_GAP).
    marginTop: "-14px",
    marginBottom: "-14px",
  },
  // Prateleira que só mostra o que cabe (`fill`): não rola. `clip` (não
  // `hidden`) não cria contêiner de rolagem — o `scrollIntoView` do foco por
  // controle não tem o que deslocar, e o Y continua visível pro anel de foco
  // e o zoom da capa. Sem isto a fila "dançava" a cada foco (Windows,
  // 2026-09-25). `!important`: ganha do `overflowX: auto` de `.shelf`.
  shelfFit: {
    overflowX: "clip !important" as "clip",
    overflowY: "visible !important" as "visible",
    scrollSnapType: "none",
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
      // encher a linha exatamente) tem prioridade; o nominal fica só de
      // fallback até a 1ª medição do `ResizeObserver` resolver (1º paint) ou
      // se JS estiver desligado.
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
    // Afasta mais o anel de foco (o global do `.app` usa 4px) — com o zoom
    // da imagem por baixo, rente ficava apertado. `!important`: precisa
    // ganhar do `.app [tabindex]:focus`, que tem mais specificity.
    "&:focus, &:focus-visible": {
      outlineOffset: `${FOCUS_OFFSET + 2}px !important`,
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
    // No canto da área segura da TV (48 × 27 epx da borda).
    right: `${M.SAFE_X}px`,
    bottom: `${M.SAFE_Y}px`,
    display: "flex",
    columnGap: "12px",
    paddingTop: "6px",
    paddingBottom: "6px",
    paddingLeft: "12px",
    paddingRight: "12px",
    // Mesmo raio do card (`useCardStyles.card`, `borderRadiusMedium`) — era
    // `borderRadiusCircular` (pílula).
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: "rgba(0, 0, 0, 0.6)",
    border: "none",
    // Texto secundário: mínimo de 12 epx do guia de TV.
    fontSize: "13px",
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
    fontSize: "12px",
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

// Página do jogo: altura dos botões do hero (o app Xbox usa 55) e o
// degradê que dissolve a arte do hero no fundo da página.
const HERO_BTN_H = 56;
const HERO_FADE = "linear-gradient(180deg, #000 0%, #000 70%, transparent 100%)";

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
    // Colado no topo da página (atrás da topbar flutuante, igual à
    // referência de Store/app Xbox) — cancela a `paddingTop` que a
    // `.scroll` reserva pra topbar (`--reemuTopbarH`) com margin negativo
    // em vez de deixar aquele vão em branco acima do hero.
    marginTop: "calc(-1 * var(--reemuTopbarH, 0px))",
    // Sangria total nos lados também — cancela o padding lateral da
    // `.scroll` (mesmos `PAGE_PAD_L`/`SCROLL_PAD_R` que ela usa) pra o
    // hero ocupar a largura inteira da página, rente à rail de um lado e
    // à borda da janela do outro (`width:auto` + margin negativo já
    // estica sozinho, sem precisar declarar `width` à mão). `heroBody`
    // devolve esse respiro pro conteúdo (ícone/título) não ficar colado
    // na rail.
    marginLeft: `calc(-1 * ${PAGE_PAD_L})`,
    marginRight: `calc(-1 * ${SCROLL_PAD_R})`,
    // Mais alto ainda (pedido do usuário) — a imagem precisa continuar
    // visível por trás da faixa de tabs, não só encostar nela. Calibrado
    // junto com `.tabsOverlap` (folga sobrando abaixo da linha de botões
    // continua maior que o quanto a faixa sobe, então não colide).
    // Cabe topbar + capa + ações + aviso de core e ainda sobra a faixa que
    // as abas sobrepõem (`.tabsOverlap`). 420 epx: a arte do app Xbox se
    // dissolve no fundo por volta de y = 420 (1366×768).
    minHeight: "420px",
    maxHeight: "72vh",
    // Sem raio: hero de sangria total (encosta na rail e na borda da
    // janela) não tem mais canto pra arredondar, igual à referência.
    borderRadius: 0,
    overflowX: "hidden",
    overflowY: "hidden",
    display: "flex",
    // Modelo "página de produto de loja" (não mais "hub Xbox" com texto
    // ancorado embaixo): ícone + título + botões ficam no TOPO do hero, o
    // resto do banner só é pano de fundo decorativo.
    alignItems: "flex-start",
    backgroundImage: elevGradient,
    // Arte, véu e fundo somem juntos no fundo da página em vez de terminar
    // num corte reto, como na página de jogo do app Xbox. A máscara vai no
    // hero inteiro: nos filhos o fundo do próprio hero continuava marcando
    // a borda. Os botões terminam bem antes do início do degradê.
    maskImage: HERO_FADE,
    WebkitMaskImage: HERO_FADE,
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
    // A arte de fundo (plano de fundo do jogo) continua dominando o meio
    // do hero, com 3 camadas de gradiente (mais escuras que a 1ª versão)
    // garantindo contraste nas duas pontas: canto SUPERIOR-esquerdo
    // (ícone/título) e faixa DE BAIXO (onde a seção de tabs sobrepõe —
    // ver `.tabsOverlap` — sem um fundo sólido atrás dela, é este
    // gradiente que sustenta a legibilidade ali).
    backgroundImage: [
      "linear-gradient(90deg, rgba(0,0,0,0.45) 0%, transparent 45%)",
      // Reforçada (pedido do usuário) — mais opaca rente à borda de baixo
      // e alcança mais alto, acompanhando o `.tabsOverlap` maior abaixo.
      "linear-gradient(0deg, rgba(0,0,0,0.92) 0%, rgba(0,0,0,0.6) 20%, rgba(0,0,0,0.22) 38%, transparent 55%)",
      "linear-gradient(180deg, rgba(0,0,0,0.6) 0%, rgba(0,0,0,0.22) 32%, transparent 58%)",
    ].join(", "),
  },
  heroBody: {
    position: "relative",
    zIndex: 1,
    display: "flex",
    flexDirection: "column",
    // Capa → ações: 16 epx, como no app Xbox.
    rowGap: "16px",
    // Esquerda usa o MESMO padding da página (`PAGE_PAD_L`, o `.hero` pai
    // cancelou com margin negativo) — ícone/título alinham com o resto do
    // conteúdo (tabs, título das seções) em vez de ficar colado na rail.
    paddingLeft: PAGE_PAD_L,
    paddingRight: "26px",
    paddingBottom: "26px",
    // O hero agora cola no topo (por baixo da topbar flutuante — ver
    // `marginTop` negativo em `.hero`) — o próprio conteúdo (ícone/título)
    // precisa dessa folga de volta, senão nasce escondido atrás da busca/
    // relógio. A capa começa logo abaixo dela (y = 80; 79 no app Xbox).
    paddingTop: "var(--reemuTopbarH, 0px)",
    maxWidth: "min(85%, 900px)",
  },
  heroHeader: {
    display: "flex",
    flexDirection: "row",
    alignItems: "center",
    columnGap: "16px",
  },
  heroIcon: {
    flexShrink: 0,
    // 128 epx, cantos de 4, como a capa da página de jogo do app Xbox.
    width: "128px",
    height: "128px",
    borderRadius: tokens.borderRadiusMedium,
    objectFit: "cover",
    // Sem borda (pedido do usuário) — a sombra já separa a capa do banner.
    // Token da escala de elevação do Fluent 2, não um rgba fixo: a capa é um
    // "card without edge", nível 16 (fluent2.microsoft.design/elevation), e
    // o token acompanha o tema claro/escuro.
    boxShadow: tokens.shadow16,
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
    // Título da página de jogo do app Xbox: ~36 epx, semibold.
    fontSize: "36px",
    fontWeight: tokens.fontWeightSemibold,
    lineHeight: 1.2,
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
    // 16 epx entre os botões, como no app Xbox.
    columnGap: "16px",
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
      outlineOffset: `${FOCUS_OFFSET}px !important`,
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
  // Favoritar/Editar/Informações: icon-only, quadrados de 56 epx (a altura
  // do "Jogar" ao lado; o "…" do app Xbox tem 56×55). `max-width` porque o
  // Fluent injeta um próprio que vence o `width` mesmo com `!important`.
  heroActionBtn: {
    width: `${HERO_BTN_H}px !important`,
    height: `${HERO_BTN_H}px !important`,
    minWidth: "0 !important",
    maxWidth: `${HERO_BTN_H}px !important`,
    fontSize: `${M.ICON}px !important`,
    "& .fui-Button__icon": {
      fontSize: "1em",
      width: "1em",
      height: "1em",
    },
  },
  // Linha de tempo de jogo abaixo das ações (página de jogo do app Xbox:
  // texto de 16 com ícone, logo abaixo dos botões). Branco translúcido:
  // fica sempre sobre o véu escuro do hero.
  heroStats: {
    display: "flex",
    alignItems: "center",
    columnGap: "8px",
    fontSize: tokens.fontSizeBase400,
    color: "rgba(255, 255, 255, 0.82)",
  },
  heroStatsIcon: { fontSize: `${M.ICON}px`, flexShrink: 0 },
  // Coração de favorito preenchido: cor de marca (mesmo token do "Nintendo
  // 64" no kicker do hero) em vez do branco padrão dos outros ícones —
  // destaca visualmente que está favoritado.
  favIconOn: { color: "var(--reemuBrandText)" },
  // Botão "Jogar": 180×56 epx com texto de 14 regular, como o
  // "Reproduzir" do app Xbox (180×55).
  playBtn: {
    minWidth: "180px !important",
    height: `${HERO_BTN_H}px !important`,
    fontSize: `${tokens.fontSizeBase300} !important`,
    fontWeight: `${tokens.fontWeightRegular} !important`,
  },
  // Remover: cor de perigo no ícone. Mesmo espaçamento das outras ações
  // (a margem extra que o separava deixava os ícones desiguais).
  dangerBtn: {
    color: `${tokens.colorPaletteRedForeground1} !important`,
  },
  // 2º toque pendente: botão vermelho cheio — a confirmação fica visível
  // sem depender do tooltip.
  dangerConfirm: {
    backgroundColor: `${tokens.colorPaletteRedBackground3} !important`,
    color: "#ffffff !important",
  },
  noCoreBar: {
    marginTop: "14px",
    maxWidth: "720px",
  },
  section: { display: "flex", flexDirection: "column", rowGap: "10px" },
  // Puxa a seção de tabs (Emulador/Save states/Shader) por cima da borda
  // de baixo do hero — pedido do usuário, estilo "card flutuante"
  // (Netflix/Steam). `zIndex:10` garante que fica por cima da arte do
  // hero (que não declara z-index próprio, mas isto blinda contra
  // qualquer ordem de pintura).
  tabsOverlap: {
    position: "relative",
    zIndex: 10,
    // Sobe mais que antes (pedido do usuário — as tabs devem ficar um
    // pouco POR CIMA da imagem, não só encostadas nela). A imagem é bem
    // mais alta que a versão original (`.hero` acima) e o gradiente de
    // baixo foi reforçado junto (`.heroScrim`), então ainda sobra folga
    // abaixo da linha de botões — não colide (conferido até em janela
    // 1600×700, o caso mais apertado testado).
    marginTop: "-56px",
  },
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
    // Borda na MESMA cor do fundo (pedido do usuário) — fica "sem borda"
    // visualmente, sem tirar a propriedade (mantém a caixa com o mesmo
    // tamanho de antes, sem o realce de contorno).
    border: `1px solid ${tokens.colorNeutralBackground2}`,
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
  // Seções da gaveta (Sobre o jogo / Descrição / Na sua biblioteca).
  infoSection: {
    display: "flex",
    flexDirection: "column",
    rowGap: "10px",
    paddingTop: "4px",
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  infoHeading: {
    margin: "10px 0 2px",
    fontSize: tokens.fontSizeBase400,
    fontWeight: tokens.fontWeightSemibold,
    color: tokens.colorNeutralForeground1,
  },
  // Lista rótulo → valor em duas colunas (rótulos alinhados à esquerda).
  infoList: {
    margin: 0,
    display: "grid",
    gridTemplateColumns: "minmax(110px, 34%) 1fr",
    columnGap: "16px",
    rowGap: "12px",
    alignItems: "baseline",
  },
  // 12 px no mínimo — o guia de TV da Microsoft pede ≥12 px até pro texto
  // secundário (antes era fontSizeBase100, 10 px).
  infoLabel: {
    fontSize: tokens.fontSizeBase200,
    color: tokens.colorNeutralForeground3,
  },
  infoValue: {
    margin: 0,
    fontSize: tokens.fontSizeBase300,
    color: tokens.colorNeutralForeground1,
    overflowWrap: "anywhere",
  },
  infoTags: {
    display: "flex",
    flexWrap: "wrap",
    gap: "6px",
  },
  infoPara: {
    margin: 0,
    fontSize: tokens.fontSizeBase300,
    lineHeight: tokens.lineHeightBase400,
    color: tokens.colorNeutralForeground2,
  },
  infoFileName: {
    display: "block",
    fontWeight: tokens.fontWeightSemibold,
  },
  infoFileDir: {
    display: "block",
    marginTop: "2px",
    fontSize: tokens.fontSizeBase200,
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
    // `:focus-visible` aí. `!important`: o `.r1f29ykk[data-fui-focus-visible]`
    // do próprio Button (ver abaixo) tem mais especificidade que isto
    // (classe+atributo vs classe+pseudo) e ganharia o `outline` sem isso.
    "& a:focus, & button:focus": {
      outline: `3px solid ${tokens.colorBrandStroke1} !important`,
      outlineOffset: `${FOCUS_OFFSET}px !important`,
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
