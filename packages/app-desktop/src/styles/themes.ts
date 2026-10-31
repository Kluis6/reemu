/**
 * Temas de cor do ReEmu (Fluent 2 / Griffel).
 *
 * Cada tema = uma rampa de marca (`BrandVariants`, 16 tons) passada por
 * `createDarkTheme` OU `createLightTheme` (`make(ramp, mode)`), mais:
 *  - `consoleDark`/`consoleLight`: neutros mais "de console" que os temas web
 *    padrão do Fluent (a rampa neutra web é clara/pálida demais pra uma tela
 *    cheia tipo TV — era o motivo do antigo objeto `brand` em `xbox.ts`).
 *    Calibrados olhando screenshot real do dashboard Xbox Series S/X e do
 *    "modo XBOX" no PC, nos dois modos.
 *  - tokens **custom** `reemu*`: o `FluentProvider` emite toda chave do objeto
 *    de tema como CSS var (`--reemuAppBg`, …), então o Griffel referencia com
 *    `var(--reemuAppBg)`. Todo token `reemu*` tem que ter um valor pros DOIS
 *    modos em `make()` — é isso que faz o tema claro não "vazar" preto por
 *    baixo (ver `reemuVeil`).
 *
 * Regenerar rampas: Fluent Theme Designer
 * (https://react.fluentui.dev/?path=/docs/theme-theme-designer--docs) ou o
 * script HSL em docs/design/fluent2.md. Semente ~tom 80.
 */
import {
  createDarkTheme,
  createLightTheme,
  teamsHighContrastTheme,
  type BrandVariants,
  type Theme,
} from "@fluentui/react-components";

// ---------------------------------------------------------------- rampas ----

/** ReEmu — o verde neon da logo e do site (`#3DDC2F`, o `--color-green` de
 *  site/src/input.css) no tom 90; o resto da rampa segue a mesma curva de
 *  luminosidade das outras (ver `RAMP_L`), no matiz dele (115°). */
const reemuGreen: BrandVariants = {
  10: "#070E06",
  20: "#0C1E0B",
  30: "#12340E",
  40: "#154C10",
  50: "#196912",
  60: "#1F8316",
  70: "#26A21B",
  80: "#2DC020",
  90: "#3DDC2F",
  100: "#53E047",
  110: "#6DE562",
  120: "#89EA80",
  130: "#A8ECA2",
  140: "#C6EFC2",
  150: "#DFF3DD",
  160: "#F2F9F1",
};

const xboxGreen: BrandVariants = {
  10: "#090D07",
  20: "#111B0D",
  30: "#1B2F13",
  40: "#254418",
  50: "#2F5E1C",
  60: "#3A7920",
  70: "#469825",
  80: "#52B72A",
  90: "#64D138",
  100: "#78D552",
  110: "#8DD86E",
  120: "#A4DD8D",
  130: "#BCE2AC",
  140: "#D2E9C9",
  150: "#E5F0E0",
  160: "#F4F8F2",
};

/** Verde do dashboard "Blades" do Xbox 360 clássico (2005) — mais amarelado
 *  que o `xboxGreen` (Series S/X moderno): o brilho por trás da blade
 *  selecionada e o logo antigo do Xbox usavam esse verde-limão ("Apple
 *  Green" #7EB900), não o verde puro atual (#107C10). Sem tema claro — o
 *  Blades nunca teve um "modo claro", era preto/verde sempre. */
const xboxClassico: BrandVariants = {
  10: "#0C0F05",
  20: "#192108",
  30: "#2A3A09",
  40: "#3C5507",
  50: "#527605",
  60: "#669306",
  70: "#7EB508",
  80: "#95D709",
  90: "#ADF514",
  100: "#B7F631",
  110: "#C2F84F",
  120: "#CEF971",
  130: "#D8F797",
  140: "#E3F5BC",
  150: "#EDF6DA",
  160: "#F7FAF0",
};

/** Vermelho PlayStation (o "Spanish Red" da marca oficial — logo/símbolo). */
const ps1Red: BrandVariants = {
  10: "#0F0507",
  20: "#21080C",
  30: "#3A0911",
  40: "#550714",
  50: "#760517",
  60: "#93061D",
  70: "#B50824",
  80: "#D7092A",
  90: "#F51439",
  100: "#F63151",
  110: "#F84F6A",
  120: "#F97187",
  130: "#F797A6",
  140: "#F5BCC5",
  150: "#F6DADF",
  160: "#FAF0F2",
};

/** Manchas do fundo do tema "PlayStation Clássico": teal, azul e amarelo
 *  oficiais da marca (junto do vermelho da rampa acima) — base cinza,
 *  destaque multicor no degradê, como pedido. */
// ------------------------------------------------------ paletas do fundo ----
//
// 4 brilhos difusos, um por canto, com MATIZES diferentes (não 4 tons da
// mesma rampa) — o fundo do dashboard do Xbox (Series/PC): ciano no alto à
// esquerda, rosado no alto à direita, verde embaixo à direita, azul fundo
// embaixo à esquerda. Posição fixa por slot (`components/AnimatedBackground`):
//   bg1 = alto/esquerda · bg2 = baixo/direita · bg3 = baixo/esquerda · bg4 = alto/direita

export interface BgPalette {
  bg1: string;
  bg2: string;
  bg3: string;
  bg4: string;
}

/** Logo do ReEmu: órbitas verdes à esquerda e azuis (`#1E8BFF`) à direita,
 *  como o circuito do site — verde no alto/esquerda e embaixo/esquerda
 *  (mais fundo), azul no alto/direita e embaixo/direita (mais fundo). */
// Tons baixos: o site é quase preto com brilho suave (orbs a ~24% de opacidade).
const reemuBg: BgPalette = { bg1: "#1B6516", bg2: "#0E3C78", bg3: "#0C3A18", bg4: "#13508F" };
/** O da imagem de referência do dashboard do Xbox. */
const xboxGreenBg: BgPalette = { bg1: "#1E7F74", bg2: "#2E9E4F", bg3: "#1B3F5C", bg4: "#8A3A4A" };
/** Blades (2005): lima + oliva, com âmbar no canto quente. */
const xboxClassicoBg: BgPalette = { bg1: "#5A8410", bg2: "#7FAF14", bg3: "#2C4A12", bg4: "#C47A12" };
const psBlueBg: BgPalette = { bg1: "#0A6FC2", bg2: "#2FA7C9", bg3: "#1C2F7A", bg4: "#6A3FB5" };
/** As 4 cores do logo do PlayStation original (amarelo e vermelho um tom
 *  abaixo — cheios, eles gritavam no fundo). */
const ps1Bg: BgPalette = { bg1: "#00AC9F", bg2: "#2E6DB4", bg3: "#C9A200", bg4: "#B0122E" };

/** Alva — o azul do Alvanista (alvanista.com, `--color-brand-500` #0094D3
 *  no tom 80; `-600` #005C82 no 60, `-700` #004060 no 40, `-300` #33B5E5
 *  no 100, tirados do CSS do site em 2026-09-25). */
const alvaBlue: BrandVariants = {
  10: "#001018",
  20: "#001C2B",
  30: "#00283E",
  40: "#004060",
  50: "#004C70",
  60: "#005C82",
  70: "#0078AA",
  80: "#0094D3",
  90: "#1AA3DB",
  100: "#33B5E5",
  110: "#5AC3EA",
  120: "#7ED0EE",
  130: "#A0DCF2",
  140: "#C0E8F6",
  150: "#DDF3FA",
  160: "#F2FAFD",
};

/** Os 4 acentos do Alvanista: azul de marca, verde-água (`secondary`),
 *  roxo (`--neon-purple`, o gradiente principal no escuro) e laranja
 *  (`accent`). */
const alvaBg: BgPalette = { bg1: "#0094D3", bg2: "#00C398", bg3: "#7C45F5", bg4: "#FF8B3E" };

/** Super Nintendo (modelo americano) — o roxo-azulado das chaves
 *  POWER/RESET no tom 80, indo até o lavanda dos botões X/Y (tom 130). */
const snesPurple: BrandVariants = {
  10: "#0D0B24",
  20: "#16133A",
  30: "#1F1B52",
  40: "#29236A",
  50: "#332C82",
  60: "#3E3599",
  70: "#4B41B4",
  80: "#5B4FCF",
  90: "#6C61D6",
  100: "#7E74DC",
  110: "#9088E2",
  120: "#A39CE8",
  130: "#B6B0ED",
  140: "#C9C4F2",
  150: "#DCD9F7",
  160: "#EFEDFB",
};

/** Só as cores do console, sem o arco-íris do Super Famicom: lavanda dos
 *  botões X/Y (alto/esquerda), roxo dos botões A/B (baixo/direita), cinza
 *  das partes rebaixadas do corpo (baixo/esquerda) e o roxo-azulado das
 *  chaves POWER/RESET (alto/direita). */
const snesBg: BgPalette = { bg1: "#8F88D6", bg2: "#4A3D9E", bg3: "#8C8A98", bg4: "#5B4FCF" };

/** Escuro: o grafite do direcional e dos botões SELECT/START. Claro: o
 *  cinza-lavanda do corpo do console, com o cinza mais escuro das partes
 *  rebaixadas nas superfícies de baixo. */
const snesDark = {
  colorNeutralBackground1: "#1D1C23",
  colorNeutralBackground1Hover: "#27262E",
  colorNeutralBackground1Pressed: "#18171D",
  colorNeutralBackground1Selected: "#33313C",
  colorNeutralBackground2: "#25242C",
  colorNeutralBackground3: "#2F2D37",
  colorNeutralBackground4: "#3A3843",
  colorNeutralForeground1: "#F3F2F7",
  colorNeutralForeground2: "#CAC8D4",
  colorNeutralForeground3: "#9C99A9",
  colorNeutralStroke1: "rgba(255, 255, 255, 0.12)",
  colorNeutralStroke2: "rgba(255, 255, 255, 0.07)",
} satisfies Partial<Theme>;

const snesLight = {
  colorNeutralBackground1: "#DCDAE2",
  colorNeutralBackground1Hover: "#D1CFD8",
  colorNeutralBackground1Pressed: "#C7C5CF",
  colorNeutralBackground1Selected: "#C7C5CF",
  colorNeutralBackground2: "#E9E8EE",
  colorNeutralBackground3: "#CFCDD6",
  colorNeutralBackground4: "#BFBDC8",
  colorNeutralForeground1: "#1D1C23",
  colorNeutralForeground2: "#3D3B47",
  colorNeutralForeground3: "#5C5968",
  colorNeutralStroke1: "rgba(0, 0, 0, 0.16)",
  colorNeutralStroke2: "rgba(0, 0, 0, 0.09)",
} satisfies Partial<Theme>;

/** Neutros do Alvanista — as variáveis `--bg-dominant`, `--surface-*`,
 *  `--text-*` e `--border-*` do site, escuro (`:root`) e claro
 *  (`[data-theme=light]`). */
const alvaDark = {
  colorNeutralBackground1: "#121212",
  colorNeutralBackground1Hover: "#1E1F22",
  colorNeutralBackground1Pressed: "#0E0E0E",
  colorNeutralBackground1Selected: "#2E3035",
  colorNeutralBackground2: "#1E1F22",
  colorNeutralBackground3: "#2E3035",
  colorNeutralBackground4: "#3A3B40",
  colorNeutralForeground1: "#FFFFFF",
  colorNeutralForeground2: "#C8CAD0",
  colorNeutralForeground3: "#9A9DA5",
  colorNeutralStroke1: "rgba(255, 255, 255, 0.12)",
  colorNeutralStroke2: "rgba(255, 255, 255, 0.07)",
} satisfies Partial<Theme>;

const alvaLight = {
  colorNeutralBackground1: "#F0F2F5",
  colorNeutralBackground1Hover: "#E4ECF2",
  colorNeutralBackground1Pressed: "#F0F2F5",
  colorNeutralBackground1Selected: "#E4ECF2",
  colorNeutralBackground2: "#FFFFFF",
  colorNeutralBackground3: "#F4F7FA",
  colorNeutralBackground4: "#E4ECF2",
  colorNeutralForeground1: "#121212",
  colorNeutralForeground2: "#1E1F22",
  colorNeutralForeground3: "#6B7280",
  colorNeutralStroke1: "rgba(0, 0, 0, 0.12)",
  colorNeutralStroke2: "rgba(0, 0, 0, 0.06)",
} satisfies Partial<Theme>;

/** Neutros do site do ReEmu (site/src/input.css): preto esverdeado
 *  (`#030605`), superfícies `#0a100e`/`#111a17`, borda `#1b2a25`, texto
 *  `#eef4f1` e secundário `#9aaba4`. Só escuro — a logo é neon sobre preto e
 *  o site também não tem modo claro. */
const reemuDark = {
  colorNeutralBackground1: "#0A100E",
  colorNeutralBackground1Hover: "#111A17",
  colorNeutralBackground1Pressed: "#070B0A",
  colorNeutralBackground1Selected: "#1B2A25",
  colorNeutralBackground2: "#0E1613",
  colorNeutralBackground3: "#16211D",
  colorNeutralBackground4: "#1E2C27",
  colorNeutralForeground1: "#EEF4F1",
  colorNeutralForeground2: "#C4D0CB",
  colorNeutralForeground3: "#9AABA4",
  colorNeutralStroke1: "#1B2A25",
  colorNeutralStroke2: "rgba(238, 244, 241, 0.07)",
} satisfies Partial<Theme>;

/** Azul PlayStation (o acento do dashboard PS4/PS5). */
const psBlue: BrandVariants = {
  10: "#020C14",
  20: "#041A29",
  30: "#052842",
  40: "#05375B",
  50: "#064674",
  60: "#05558D",
  70: "#0667AC",
  80: "#0A7BCB",
  90: "#128EE7",
  100: "#379DE6",
  110: "#60ABE0",
  120: "#86B9DE",
  130: "#A9C8DF",
  140: "#C8D8E4",
  150: "#DFE6EB",
  160: "#F3F4F5",
};

/** Simple Vibes — só brancos e cinzas: a "marca" é um cinza neutro (matiz
 *  220, 4% de saturação, na mesma curva de luminosidade do Personalizado),
 *  então botões e seleção ficam em grafite, sem cor. */
const simpleGray: BrandVariants = {
  10: "#0A0A0A",
  20: "#141415",
  30: "#202123",
  40: "#2C2D30",
  50: "#3B3D40",
  60: "#494B50",
  70: "#5B5D62",
  80: "#6B6E74",
  90: "#808389",
  100: "#8F9298",
  110: "#A0A2A7",
  120: "#B2B4B8",
  130: "#C5C6C9",
  140: "#D7D8DA",
  150: "#E7E8E9",
  160: "#F5F5F5",
};

/** Casca cinza bem clara e superfícies brancas (como as Configurações do
 *  Windows no claro). Fundo de uma cor só: o próprio `Background1`. */
const simpleLight = {
  colorNeutralBackground1: "#F0F0F1",
  colorNeutralBackground1Hover: "#E5E5E7",
  colorNeutralBackground1Pressed: "#DCDCDF",
  colorNeutralBackground1Selected: "#DCDCDF",
  colorNeutralBackground2: "#FFFFFF",
  colorNeutralBackground3: "#F7F7F8",
  colorNeutralBackground4: "#E4E4E6",
  colorNeutralForeground1: "#1B1B1D",
  colorNeutralForeground2: "#3E3E43",
  colorNeutralForeground3: "#626268",
  colorNeutralStroke1: "rgba(0, 0, 0, 0.12)",
  colorNeutralStroke2: "rgba(0, 0, 0, 0.06)",
} satisfies Partial<Theme>;

/** Serene — branco como cor de destaque, como o modo XBOX do app Xbox
 *  (aba selecionada, chip ativo e anel de foco brancos; print de
 *  2026-10-11). Tons 10–50 são cinzas escuros (fundos "de marca" sutis,
 *  `colorBrandBackground2`, `colorBrandStroke2`) e do 60 pra cima vai pro
 *  branco: `colorBrandBackground` (70), o texto/ícone de marca (100) e o
 *  traço de foco/seleção (100) ficam brancos no escuro (mapeamento do
 *  `createDarkTheme`, @fluentui/tokens `alias/darkColor`). */
const sereneWhite: BrandVariants = {
  10: "#0B0C0E",
  20: "#16181B",
  30: "#202328",
  40: "#2B2F35",
  50: "#3A3F46",
  60: "#C9CCD1",
  70: "#E9E9E9",
  80: "#F2F2F3",
  90: "#D7DADE",
  100: "#FFFFFF",
  110: "#FFFFFF",
  120: "#FFFFFF",
  130: "#FFFFFF",
  140: "#FFFFFF",
  150: "#FFFFFF",
  160: "#FFFFFF",
};

/** Neutros do Serene nas proporções do modo XBOX (print a 3840×2160):
 *  página #1A1B1E e superfícies (cards, rail, botões) #25282C, um degrau
 *  mais claro — Fluent 2 › Color: "Use lighter neutrals on surfaces to
 *  highlight areas of primary focus". Levemente azulados. Fundo de uma cor
 *  só: o próprio `Background1`. */
const sereneDark = {
  colorNeutralBackground1: "#1A1C20",
  colorNeutralBackground1Hover: "#22252A",
  colorNeutralBackground1Pressed: "#16181B",
  colorNeutralBackground1Selected: "#2A2E33",
  colorNeutralBackground2: "#262A2F",
  colorNeutralBackground2Hover: "#2F3339",
  colorNeutralBackground2Pressed: "#212428",
  colorNeutralBackground2Selected: "#353A40",
  colorNeutralBackground3: "#30353B",
  colorNeutralBackground3Hover: "#393E45",
  colorNeutralBackground3Pressed: "#2B2F35",
  colorNeutralBackground3Selected: "#3F444C",
  colorNeutralBackground4: "#3B4047",
  colorNeutralForeground1: "#F2F3F5",
  colorNeutralForeground2: "#C8CDD3",
  // AA também sobre os campos (#3B4148), não só sobre a página
  colorNeutralForeground3: "#A9B0B9",
  // borda dos controles: a dos filtros do Xbox (#66707F sobre #3E434B)
  colorNeutralStroke1: "rgba(255, 255, 255, 0.20)",
  colorNeutralStroke2: "rgba(255, 255, 255, 0.10)",
  // Branco como destaque: o pressionado do botão primário seria o tom 40
  // (escuro) — fica um cinza claro; e o texto sobre o branco é escuro
  // (chip "Instalado" do Xbox: preto sobre #E9E9E9).
  colorBrandBackgroundPressed: "#C9CCD1",
  colorNeutralForegroundOnBrand: "#111316",
} satisfies Partial<Theme>;

/** Camadas do card das Configurações no Serene: com o fundo liso (sem os
 *  brilhos coloridos), o card na cor da página só se separava pela sombra.
 *  Cada camada sobe um degrau de ~1,18:1 sobre a de baixo — a mesma
 *  distância página → superfície do modo XBOX (#1A1B1E → #25282C): card na
 *  cor das superfícies (`Background2`), linhas e campos acima dele
 *  (Fluent 2 › Color: "Use lighter neutrals on surfaces"). */
const sereneLift: ReEmuLiftTokens = {
  reemuSurface1: "#262A2F",
  reemuSurface2: "#31363C",
  reemuSurface2Hover: "#383D44",
  reemuSurface2Pressed: "#2C3036",
  reemuSurface2Selected: "#3D434A",
  reemuSurface3: "#3B4148",
  reemuSurface3Hover: "#42484F",
  reemuSurface3Pressed: "#363B42",
  reemuSurface3Selected: "#474D55",
};

/** Dream Sky — o menu do Dreamcast: céu azul com nuvens ao fundo ("a blue
 *  background that looks like the sky", com Play/File/Music/Settings) e a
 *  espiral laranja do logo (versão americana; a europeia é azul). Sem
 *  códigos oficiais publicados: a rampa é o laranja da espiral (matiz 26)
 *  na mesma curva do "Personalizado". */
const dreamOrange: BrandVariants = {
  10: "#0F0905",
  20: "#211308",
  30: "#3A1E09",
  40: "#552907",
  50: "#763605",
  60: "#934306",
  70: "#B55308",
  80: "#D76209",
  90: "#F57614",
  100: "#F68631",
  110: "#F8984F",
  120: "#F9AC71",
  130: "#F7C097",
  140: "#F5D5BC",
  150: "#F6E6DA",
  160: "#FAF4F0",
};

/** Céu do menu nos cantos (azul alto/esquerda, azul-claro de nuvem
 *  alto/direita, azul fundo embaixo/esquerda) e a espiral laranja
 *  embaixo/direita. */
const dreamBg: BgPalette = { bg1: "#2F7FD0", bg2: "#D76209", bg3: "#163F7A", bg4: "#7FB8E8" };

/** Escuro: céu noturno, azul-marinho. */
const dreamDark = {
  colorNeutralBackground1: "#0D1828",
  colorNeutralBackground1Hover: "#132136",
  colorNeutralBackground1Pressed: "#0A1220",
  colorNeutralBackground1Selected: "#1A2A42",
  colorNeutralBackground2: "#142238",
  colorNeutralBackground3: "#1B2D48",
  colorNeutralBackground4: "#243957",
  colorNeutralForeground1: "#EEF4FB",
  colorNeutralForeground2: "#C3D2E4",
  colorNeutralForeground3: "#9AAFC8",
  colorNeutralStroke1: "rgba(255, 255, 255, 0.14)",
  colorNeutralStroke2: "rgba(255, 255, 255, 0.08)",
} satisfies Partial<Theme>;

/** Claro: o céu claro do menu, com superfícies brancas (o branco do
 *  console). */
const dreamLight = {
  colorNeutralBackground1: "#DCEAF7",
  colorNeutralBackground1Hover: "#D0E2F3",
  colorNeutralBackground1Pressed: "#C4DAEF",
  colorNeutralBackground1Selected: "#C4DAEF",
  colorNeutralBackground2: "#FFFFFF",
  colorNeutralBackground3: "#EEF5FC",
  colorNeutralBackground4: "#CFE1F2",
  colorNeutralForeground1: "#0E1B2C",
  colorNeutralForeground2: "#24364D",
  colorNeutralForeground3: "#4A5E77",
  colorNeutralStroke1: "rgba(14, 27, 44, 0.16)",
  colorNeutralStroke2: "rgba(14, 27, 44, 0.08)",
} satisfies Partial<Theme>;

// --------------------------------------------------------------- fábrica ----

/** Neutros do "console look" — casam com o antigo `brand` (bg/bgElev/bgElev2)
 *  pra não mudar nada visualmente na migração. */
const consoleDark = {
  colorNeutralBackground1: "#0b0b0d",
  colorNeutralBackground1Hover: "#17171b",
  colorNeutralBackground1Pressed: "#0b0b0d",
  colorNeutralBackground1Selected: "#1f1f24",
  colorNeutralBackground2: "#14141a",
  colorNeutralBackground3: "#17171b",
  colorNeutralBackground4: "#1f1f24",
} satisfies Partial<Theme>;

/** Neutros do "console look" claro — calibrado com screenshots reais do
 *  dashboard Xbox Series S/X e do "modo XBOX" no PC (guia/conquistas, gestor
 *  de armazenamento): o fundo NÃO é branco, é um cinza médio-claro; quem é
 *  branco são os cartões/painéis por cima (tile de jogo, popup de
 *  conquista) — o mesmo contraste "cartão claro sobre fundo cinza" do tema
 *  escuro (lá é "cartão cinza sobre fundo quase-preto"), só invertido. */
const consoleLight = {
  colorNeutralBackground1: "#e2e3e6",
  colorNeutralBackground1Hover: "#d4d6da",
  colorNeutralBackground1Pressed: "#e2e3e6",
  colorNeutralBackground1Selected: "#cbcdd2",
  colorNeutralBackground2: "#ffffff",
  colorNeutralBackground3: "#eceef1",
  colorNeutralBackground4: "#d6d8dc",
} satisfies Partial<Theme>;

/** Tokens custom do ReEmu, emitidos como `--reemu*` pelo `FluentProvider`. */
/** Camadas dentro de um card `colorNeutralBackground1` (o card das
 *  Configurações): linhas/cards (`Surface2*`) e campos (`Surface3*`). Ver
 *  `liftTokens`. */
export interface ReEmuLiftTokens {
  /** Fundo do próprio card. */
  reemuSurface1: string;
  reemuSurface2: string;
  reemuSurface2Hover: string;
  reemuSurface2Pressed: string;
  reemuSurface2Selected: string;
  reemuSurface3: string;
  reemuSurface3Hover: string;
  reemuSurface3Pressed: string;
  reemuSurface3Selected: string;
}

/**
 * Fluent 2 › Color: "Use lighter neutrals on surfaces" — a camada de dentro
 * é mais CLARA que a de fora. As escalas de neutros dos temas não garantem
 * isso (em vários o Background2 fica a 4–8% do Background1, e nos claros o
 * Background3 é mais escuro), então as camadas saem de um passo fixo a
 * partir do fundo do card:
 * - escuro / alto contraste: misturando a cor do texto (clareia);
 * - claro: misturando branco (clareia), como os cards brancos sobre cinza
 *   das Configurações do Windows. Hover/pressionado ficam um pouco mais
 *   escuros que o repouso (no Fluent o controle escurece ao interagir).
 * `var(--…)`: resolve no elemento, com o tema que estiver aplicado.
 */
function liftTokens(light: boolean): ReEmuLiftTokens {
  // Escuro: as camadas de dentro CLAREIAM a partir do card (misturando a cor
  // do texto). Claro: o card é a superfície branca e as camadas de dentro
  // descem em direção à página (`--reemuPage`) — o mesmo sentido do hover do
  // Fluent no claro. Sem borda nem sombra no card (como no modo XBOX), a
  // separação é só a cor.
  const toward = light ? "var(--reemuPage)" : "var(--colorNeutralForeground1)";
  const mix = (pct: number) =>
    `color-mix(in srgb, var(--colorNeutralBackground1), ${toward} ${pct}%)`;
  // [repouso, hover, pressionado, selecionado]
  const rows = light ? [55, 70, 85, 65] : [8, 12, 6, 14];
  const fields = light ? [30, 45, 60, 40] : [14, 18, 12, 20];
  return {
    reemuSurface1: "var(--colorNeutralBackground1)",
    reemuSurface2: mix(rows[0]),
    reemuSurface2Hover: mix(rows[1]),
    reemuSurface2Pressed: mix(rows[2]),
    reemuSurface2Selected: mix(rows[3]),
    reemuSurface3: mix(fields[0]),
    reemuSurface3Hover: mix(fields[1]),
    reemuSurface3Pressed: mix(fields[2]),
    reemuSurface3Selected: mix(fields[3]),
  };
}

export interface ReEmuTokens extends ReEmuLiftTokens {
  /** Cor da PÁGINA (o fundo atrás de tudo). No Fluent o `Background1` é a
   *  superfície mais à frente (menus, diálogos, campos, botões — branco no
   *  claro, o cinza mais claro no escuro) e os números maiores vão pro
   *  fundo; aqui o `Background1` do tema vira essa superfície e a página
   *  fica neste token — ver `surfaceTokens`. */
  reemuPage: string;
  /** Fundo da casca (`.app`) — cinza neutro suave, sem matiz. */
  reemuAppBg: string;
  /** Realce translúcido neutro (brilho de canto, chip ativo). */
  reemuAccentSoft: string;
  /** Superfície sólida — igual ao fundo do item ativo da sidebar; usada
   * nos botões de ícone da topbar e de ação da biblioteca. */
  reemuSurfaceSoft: string;
  /** Preenchimento sólido de marca (círculo do rail, glifos de dica). */
  reemuBrandSolid: string;
  /** Cor de texto legível sobre `reemuBrandSolid`. */
  reemuOnBrand: string;
  /** Brilhos do fundo (`components/AnimatedBackground`), um por canto —
   *  ver `BgPalette`. */
  reemuBg1: string;
  reemuBg2: string;
  reemuBg3: string;
  reemuBg4: string;
  /** Véu leve e uniforme sobre os brilhos (o `reemuVeil`, que escurece as
   *  bordas, é pro papel de parede — nos brilhos ele apagaria justamente os
   *  cantos, onde fica a cor). */
  reemuGlowVeil: string;
  /** Véu sobre as manchas do fundo (`components/AnimatedBackground`) — escuro
   *  no tema escuro (abafa a cor, mantém tudo "moody"), CLARO no tema claro
   *  (abafa a cor pro lado do branco). Sem isto o fundo de um tema claro
   *  ficaria escuro de qualquer jeito (o véu cobre tudo por cima). */
  reemuVeil: string;
  /** Fundo do item ativo no rail (`[aria-current="page"]`) — mais forte que
   *  o hover (`colorNeutralBackground3`), pra destacar onde o usuário está.
   *  No claro é o verde de marca (como no guia/gestor de armazenamento do
   *  Xbox de verdade — o item selecionado vira uma pílula verde solida). */
  reemuActiveBg: string;
  /** Cor do texto/ícone sobre `reemuActiveBg` — sempre branco (a pílula
   *  ativa é sempre escura o bastante pra pedir texto claro, nos dois
   *  modos: cinza escuro no tema escuro, verde saturado no claro). */
  reemuActiveFg: string;
  /** Preenchimento quase-imperceptível usado em chips/painéis que ficam
   *  direto sobre o fundo da casca (não sobre uma arte) — branco translúcido
   *  no escuro, preto translúcido no claro. */
  reemuFillWeak: string;
  /** Cor de marca pra TEXTO/ÍCONE sobre o fundo neutro da casca (não sobre
   *  uma superfície colorida — pra isso é `reemuOnBrand`). `tokens.
   *  colorBrandForeground1` (tom 80/100 da rampa) falha WCAG AA (< 3:1,
   *  chega a ficar em ~2:1) contra os neutros customizados do tema claro
   *  pra praticamente toda rampa cadastrada (confirmado calculando o
   *  contraste real das 4 rampas) — rampas mais claras/amareladas
   *  (Xbox Clássico) falham ainda pior. Tom 40 da rampa passa em AA (>6:1)
   *  em todas; no escuro tom 90 já é legível de sobra, mantém perto do que
   *  `colorBrandForeground1` já mostrava. Usar em vez de
   *  `colorBrandForeground1` sempre que a cor de marca for aplicada como
   *  `color` de texto/ícone direto sobre o fundo da casca. */
  reemuBrandText: string;
  /** Cor de destaque pra ÍCONES e INDICADORES (favorito, aba ativa, foco,
   *  switch/rádio/checkbox marcados, borda de item selecionado): o tom
   *  primário do tema (80 da rampa, o mesmo do `reemuBrandSolid` e do
   *  botão primário no claro) sempre que ele tem contraste de 3:1 com as
   *  superfícies (WCAG 1.4.11, componentes não-texto); senão o tom mais
   *  próximo que tenha — mais claro no escuro, mais escuro no claro. Ver
   *  `pickAccent`. */
  reemuBrandAccent: string;
  /** Superfície dos cards de jogo sem capa e do card 2×2 das prateleiras
   *  (degradê do topo pra base) e o traço deles. Os `Background3/4` dos
   *  temas ficavam a 1,04–1,20:1 da página (cards sumiam, sobretudo nos
   *  claros) — ver `cardSurface`. */
  reemuCardTop: string;
  reemuCardBottom: string;
  reemuCardStroke: string;
  /** Fonte dos títulos (≥ 20 px): Segoe UI Variable Display — ver
   *  `segoeVariable`. */
  reemuFontDisplay: string;
  /** Borda de campo e de botão secundário de modal (`index.css`,
   *  `styles/buttons.ts`). Sem ela vale a cor do texto do tema a 50% (≥ 3:1
   *  contra a superfície, medido nos temas); só o alto contraste define, pra
   *  manter o traço branco dele. */
  reemuFieldStroke?: string;
}

export type ReEmuTheme = Theme & ReEmuTokens;


/**
 * Fonte do Windows 11: Segoe UI Variable (learn.microsoft.com/windows/apps/
 * design/signature-experiences/typography — "When using HTML, optical
 * scaling is also automatic, but you will need to specify the Segoe UI
 * Variable font in CSS"). No Chromium/WebView2 o nome da família é por
 * instância: "Segoe UI Variable Text" (corpo, até 18 px) e "Segoe UI
 * Variable Display" (subtítulo 20 px e títulos — a rampa do Windows usa
 * "Display semibold" do Subtitle pra cima); "Segoe UI Variable" sozinho não
 * casa (medido com `CSS.getPlatformFontsForNode`). Antes vem a pilha do
 * Fluent ("Segoe UI" estática do Windows 10 …). Sem Segoe (Linux), entra a
 * Selawik — fonte aberta da Microsoft com as métricas da Segoe UI, embutida
 * no app (index.css, `@font-face`) — e só depois o resto da pilha do Fluent.
 */
function segoeVariable(base: string): { text: string; display: string } {
  const rest = `'Segoe UI', 'Selawik', ${base}`;
  return {
    text: `'Segoe UI Variable Text', ${rest}`,
    // Títulos como no dashboard do Xbox Series: DIN, que no Windows é a Bahnschrift
    // (identificada pela aparência, sem fonte escrita).
    display: `'Bahnschrift', 'Segoe UI Variable Display', ${rest}`,
  };
}

/** Mistura `a` com `b` na proporção `p` (0–1), em sRGB. */
function mixHex(a: string, b: string, p: number): string {
  const ch = (h: string, i: number) => parseInt(h.slice(1 + i * 2, 3 + i * 2), 16)
  const out = [0, 1, 2].map((i) => Math.round(ch(a, i) * (1 - p) + ch(b, i) * p))
  return `#${out.map((v) => v.toString(16).padStart(2, "0")).join("")}`.toUpperCase()
}

/**
 * Superfície dos cards (jogo sem capa, 2×2 das prateleiras) a partir da
 * página. Fluent 2 › Color: "Use lighter neutrals on surfaces" — no ESCURO
 * o card é a página clareada pela cor do texto (14% no topo, 8% na base:
 * ~1,4 e ~1,2:1 sobre a página); no CLARO é a superfície branca do tema
 * (`Background2`) indo um pouco pro tom da página. SEM traço (pedido do
 * usuário, comparando com o modo XBOX do app Xbox: os cards se destacam só
 * pela cor, #25282C sobre a página #1A1B1E, sem borda) — só o alto
 * contraste mantém o traço.
 */
function cardSurface(
  light: boolean,
  page: string,
  surface: string,
  text: string,
): Pick<ReEmuTokens, "reemuCardTop" | "reemuCardBottom" | "reemuCardStroke"> {
  if (!HEX.test(page)) {
    return { reemuCardTop: page, reemuCardBottom: page, reemuCardStroke: "transparent" };
  }
  if (light) {
    const top = HEX.test(surface) ? surface : "#FFFFFF";
    return {
      reemuCardTop: top,
      reemuCardBottom: mixHex(top, page, 0.35),
      reemuCardStroke: "transparent",
    };
  }
  const ink = HEX.test(text) ? text : "#FFFFFF";
  return {
    reemuCardTop: mixHex(page, ink, 0.14),
    reemuCardBottom: mixHex(page, ink, 0.08),
    reemuCardStroke: "transparent",
  };
}

/**
 * Elevação como no Fluent e no modo XBOX do app Xbox: a página atrás e UMA
 * superfície um degrau acima pra tudo que fica sobre ela — no Xbox, página
 * #1A1B1E e superfície #25282C (cards, rail, menu suspenso, busca), ~1,2:1,
 * sem borda nem sombra; o item em foco/selecionado clareia mais um degrau
 * (#4E535B no menu). No Fluent o `colorNeutralBackground1` é essa
 * superfície (os componentes — Menu, Popover, Dialog, Input, Button
 * secondary, Tooltip — pintam o fundo com ele; branco no claro, grey[16] no
 * escuro, e os números maiores vão pro fundo). Os temas do ReEmu tinham o
 * `Background1` como PÁGINA, então menus/diálogos saíam da cor da página.
 *
 * Superfície: o `Background2` do tema quando ele já fica ≥ 1,15:1 da página
 * (claro: ≥ 1,08:1 e mais claro que ela); senão a página clareada pela cor
 * do texto (escuro) ou puxada pro branco (claro) até isso. O `Background2`
 * vira a MESMA superfície (o app usa ele em cards, rail e botões) e o 3 e o
 * 4 são os degraus de realce acima dela — uma escada só em todos os temas.
 */
function surfaceTokens(
  light: boolean,
  page: string,
  authored: string,
  text: string,
): Partial<Theme> {
  if (!HEX.test(page)) return {};
  const ink = HEX.test(text) ? text : light ? "#000000" : "#FFFFFF";
  let s = authored;
  const lighter = HEX.test(s) && luminance(s) > luminance(page);
  if (light) {
    if (!(lighter && contrastRatio(s, page) >= 1.08)) s = mixHex(page, "#FFFFFF", 0.75);
  } else if (!(lighter && contrastRatio(s, page) >= 1.15)) {
    s = page;
    for (let p = 0.04; p <= 0.2 && contrastRatio(s, page) < 1.18; p += 0.01) s = mixHex(page, ink, p);
  }
  // Escada única (Xbox: página #1A1B1E → superfície #25282C → filtros
  // #3E434B → selecionado #4E535B). Escuro: cada degrau clareia pela cor do
  // texto. Claro: a superfície é branca e os realces escurecem (o hover do
  // Fluent no claro escurece) — o 4 fica um pouco mais escuro que a PÁGINA,
  // pra aba/hover sobre a página também aparecer.
  const l1 = s;
  // degrau até ter `min` de contraste com `ref` (e nunca menos que `start`)
  const stepFrom = (from: string, ref: string, start: number, min: number) => {
    let c = mixHex(from, ink, start);
    for (let p = start; p <= 0.3 && contrastRatio(c, ref) < min; p += 0.01) c = mixHex(from, ink, p);
    return c;
  };
  const l2 = light ? stepFrom(mixHex(s, page, 0.55), s, 0, 1.1) : mixHex(s, ink, 0.1);
  const l3 = light ? stepFrom(page, page, 0.06, 1.1) : mixHex(s, ink, 0.17);
  const states = (base: string, up: number) =>
    light
      ? { hover: mixHex(base, ink, 0.04), pressed: mixHex(base, ink, 0.08), selected: mixHex(base, ink, 0.06) }
      : { hover: mixHex(base, ink, up), pressed: mixHex(base, page, 0.35), selected: mixHex(base, ink, up + 0.04) };
  const s1 = states(l1, 0.08);
  const s2 = states(l2, 0.07);
  const s3 = states(l3, 0.07);
  return {
    // superfície: menus, diálogos, campos (Fluent) e cards/rail/botões (app)
    colorNeutralBackground1: l1,
    colorNeutralBackground1Hover: s1.hover,
    colorNeutralBackground1Pressed: s1.pressed,
    colorNeutralBackground1Selected: s1.selected,
    colorNeutralBackground2: l1,
    colorNeutralBackground2Hover: s1.hover,
    colorNeutralBackground2Pressed: s1.pressed,
    colorNeutralBackground2Selected: s1.selected,
    // realce sobre a superfície (filtros, chips, campos, células)
    colorNeutralBackground3: l2,
    colorNeutralBackground3Hover: s2.hover,
    colorNeutralBackground3Pressed: s2.pressed,
    colorNeutralBackground3Selected: s2.selected,
    // realce maior (hover de aba, item em destaque)
    colorNeutralBackground4: l3,
    colorNeutralBackground4Hover: s3.hover,
    colorNeutralBackground4Pressed: s3.pressed,
    colorNeutralBackground4Selected: s3.selected,
  };
}

/** Luminância relativa WCAG 2.x de `#rrggbb`. */
function luminance(hex: string): number {
  const n = hex.replace("#", "");
  const [r, g, b] = [0, 2, 4].map((i) => {
    const c = parseInt(n.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Razão de contraste WCAG 2.x entre duas cores `#rrggbb`. */
export function contrastRatio(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const HEX = /^#[0-9a-fA-F]{6}$/;

/**
 * Tom de destaque pra ícones/indicadores: começa no 80 (o primário do tema)
 * e anda pela rampa — pro claro no escuro, pro escuro no claro — até ter
 * 3:1 com todas as `surfaces` (WCAG 2.2, 1.4.11 "Non-text Contrast"). No
 * modo escuro o Fluent usa o tom 100 pra isso (`createDarkTheme`), que em
 * várias rampas fica desbotado, longe da cor do tema.
 */
function pickAccent(ramp: BrandVariants, light: boolean, surfaces: string[]): string {
  const steps = (light ? [80, 70, 60, 50, 40, 30] : [80, 90, 100, 110, 120, 130]) as (keyof BrandVariants)[];
  const bgs = surfaces.filter((c) => HEX.test(c));
  for (const k of steps) {
    if (bgs.every((bg) => contrastRatio(ramp[k], bg) >= 3)) return ramp[k];
  }
  return ramp[steps[steps.length - 1]];
}

/** Tokens do Fluent que pintam indicadores com a marca, apontados pro
 *  destaque (`pickAccent`). Texto de marca (`colorBrandForeground*`,
 *  links) fica como está: precisa de 4,5:1, não 3:1. */
function accentTokens(accent: string): Partial<Theme> {
  return {
    // switch/checkbox marcados, indicador da aba (Fluent), traço de foco e
    // de item selecionado, bolinha do rádio
    colorCompoundBrandBackground: accent,
    colorCompoundBrandBackgroundHover: accent,
    colorCompoundBrandBackgroundPressed: accent,
    colorCompoundBrandStroke: accent,
    colorCompoundBrandStrokeHover: accent,
    colorCompoundBrandStrokePressed: accent,
    colorCompoundBrandForeground1: accent,
    colorCompoundBrandForeground1Hover: accent,
    colorCompoundBrandForeground1Pressed: accent,
    colorBrandStroke1: accent,
  };
}

function readableOn(hex: string): string {
  const n = hex.replace("#", "");
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(n.slice(i, i + 2), 16));
  // brilho percebido (YIQ) — > 128 pede texto escuro.
  return (r * 299 + g * 587 + b * 114) / 1000 > 128 ? "#0b0b0d" : "#ffffff";
}

/** Ajustes de um tema com identidade própria (ex.: Alva) por cima do
 *  "console look" padrão. */
interface ThemeOverrides {
  /** Neutros no lugar de `consoleDark`/`consoleLight`. */
  neutrals?: Partial<Theme>;
  /** Fundo da casca (`reemuAppBg`). */
  appBg?: string;
  /** Pílula do item ativo no rail (`reemuActiveBg`). */
  activeBg?: string;
  /** Fundo de uma cor só: sem os brilhos coloridos nos cantos (os 4 viram
   *  esta cor, como no alto contraste) e casca lisa. */
  solidBg?: string;
  /** Texto de marca (`reemuBrandText`) quando o tom padrão não passa AA. */
  brandText?: string;
  /** Camadas do card das Configurações no lugar das derivadas
   *  (`liftTokens`). */
  lift?: ReEmuLiftTokens;
}

/** Tipografia do app Xbox do Windows: corpo de 14 epx (o `Base300` padrão
 *  do Fluent, ex.: "Pesquisar jogos…", "Compre e jogue agora" nas capturas
 *  em 1366×768). Só o menor (`Base100`, 10 no Fluent) sobe pra 12 — o app
 *  não usa texto abaixo disso. */
const tvType: Partial<Theme> = {
  fontSizeBase100: "12px",
  lineHeightBase100: "16px",
};

function make(
  ramp: BrandVariants,
  mode: "dark" | "light",
  bg: BgPalette,
  o: ThemeOverrides = {},
): ReEmuTheme {
  const light = mode === "light";
  const t: ReEmuTheme = {
    ...(light ? createLightTheme(ramp) : createDarkTheme(ramp)),
    ...(o.neutrals ?? (light ? consoleLight : consoleDark)),
    ...tvType,
    // Um raio só no app inteiro: 4 px (o `borderRadiusMedium` do Fluent),
    // inclusive nos componentes do Fluent que usam Small/Large/XLarge
    // (diálogos, menus, cards). Circular (avatar, switch) fica como é.
    borderRadiusSmall: "4px",
    borderRadiusMedium: "4px",
    borderRadiusLarge: "4px",
    borderRadiusXLarge: "4px",
    // Fundo da casca: o stop mais "fraco" do gradiente é sempre igual ao
    // `colorNeutralBackground1` (o rail usa esse mesmo tom — ver `xbox.ts`),
    // só o outro stop clareia (escuro) ou clareia mais ainda (claro).
    reemuAppBg: light
      ? "linear-gradient(180deg, #ececee 0%, #e2e3e6 45%)"
      : "linear-gradient(180deg, #121214 0%, #0b0b0d 45%)",
    reemuAccentSoft: light ? "rgba(0, 0, 0, 0.045)" : "rgba(255, 255, 255, 0.05)",
    // Botão/pílula "sólida" (ícones da topbar, ação da biblioteca): no
    // escuro é um cinza médio sobre o quase-preto; no claro o equivalente é
    // BRANCO sobre o cinza da casca (mesmo idioma "cartão claro flutuando"
    // das telas de jogos/apps do Xbox — os tiles também são brancos).
    reemuSurfaceSoft: light ? "#ffffff" : "#5f6368",
    reemuBrandSolid: ramp[80],
    reemuOnBrand: readableOn(ramp[80]),
    reemuBg1: bg.bg1,
    reemuBg2: bg.bg2,
    reemuBg3: bg.bg3,
    reemuBg4: bg.bg4,
    // Escuro: só abafa um pouco (o fundo continua quase preto e os cantos
    // brilham). Claro: puxa bastante pro cinza da casca — cor cheia num
    // fundo claro fica berrante.
    reemuGlowVeil: light ? "rgba(236, 238, 241, 0.62)" : "rgba(9, 9, 12, 0.1)",
    // Radial (não mais vertical): centro bem mais transparente — deixa o
    // papel de parede aparecer no meio da tela — e as bordas/cantos (onde
    // ficam as manchas de cor do tema) mantêm a força de antes. Camada
    // ÚNICA de gradiente (WebKitGTK quebra com radial-gradient multicamada
    // num elemento `position: fixed` — ver comentário no `.app`).
    reemuVeil: light
      ? "radial-gradient(ellipse at center, rgba(255, 255, 255, 0) 0%, rgba(255, 255, 255, 0.9) 100%)"
      : "radial-gradient(ellipse at center, rgba(9, 9, 12, 0) 0%, rgba(9, 9, 12, 0.82) 100%)",
    // Verde de marca (tom 70 — mais fechado que o 80 padrão, fica igual ao
    // verde da pílula de seleção nos screenshots) só no claro; no escuro
    // continua o cinza neutro que já existia.
    reemuActiveBg: light ? ramp[70] : "#3a3a3f",
    reemuActiveFg: "#ffffff",
    reemuFillWeak: light ? "rgba(0, 0, 0, 0.04)" : "rgba(255, 255, 255, 0.04)",
    // Ver doc de `ReEmuTokens.reemuBrandText` — tom mais fechado que
    // `colorBrandForeground1` pra garantir AA (>4.5:1) contra os neutros
    // customizados do tema claro em qualquer rampa cadastrada.
    reemuBrandText: light ? ramp[40] : ramp[90],
    reemuBrandAccent: "",
    reemuCardTop: "",
    reemuCardBottom: "",
    reemuCardStroke: "",
    reemuFontDisplay: "",
    reemuPage: "",
    ...liftTokens(light),
  };
  if (o.appBg) t.reemuAppBg = o.appBg;
  if (o.activeBg) t.reemuActiveBg = o.activeBg;
  if (o.brandText) t.reemuBrandText = o.brandText;
  if (o.lift) Object.assign(t, o.lift);
  // Destaque de ícones/indicadores contra a página e as superfícies (rail,
  // cards, botões) — depois dos overrides de neutros.
  // Página = o `Background1` que o tema definiu; o `Background1` do Fluent
  // vira a superfície um degrau acima (`surfaceTokens`).
  const page = t.colorNeutralBackground1;
  t.reemuPage = page;
  Object.assign(t, surfaceTokens(light, page, t.colorNeutralBackground2, t.colorNeutralForeground1));
  t.reemuBrandAccent = pickAccent(ramp, light, [
    page,
    t.colorNeutralBackground1,
    t.colorNeutralBackground2,
    t.colorNeutralBackground3,
  ]);
  Object.assign(t, accentTokens(t.reemuBrandAccent));
  const fonts = segoeVariable(t.fontFamilyBase);
  t.fontFamilyBase = fonts.text;
  t.reemuFontDisplay = fonts.display;
  // Depois dos neutros finais (overrides incluídos).
  Object.assign(
    t,
    cardSurface(light, page, t.colorNeutralBackground2, t.colorNeutralForeground1),
  );
  if (o.solidBg) {
    const c = o.solidBg;
    t.reemuBg1 = t.reemuBg2 = t.reemuBg3 = t.reemuBg4 = c;
    t.reemuGlowVeil = c;
    // `backgroundImage` na casca (xbox.ts): gradiente de uma cor só
    t.reemuAppBg = `linear-gradient(${c}, ${c})`;
  }
  return t;
}

/**
 * Alto contraste: o tema de alto contraste da Fluent (preto/branco puros,
 * foco em ciano) + os tokens do ReEmu no mesmo esquema — sem as manchas de
 * cor do fundo animado nem o véu, que só atrapalham a leitura aqui. O teste
 * em `themes.test.ts` confere os pares texto/fundo em AAA (7:1).
 */
function makeHighContrast(): ReEmuTheme {
  const t = teamsHighContrastTheme;
  const bg = t.colorNeutralBackground1;
  return {
    ...t,
    reemuAppBg: bg,
    reemuAccentSoft: "rgba(255, 255, 255, 0.12)",
    reemuSurfaceSoft: bg,
    reemuBrandSolid: t.colorBrandBackground,
    reemuOnBrand: t.colorNeutralForegroundOnBrand,
    reemuBg1: bg,
    reemuBg2: bg,
    reemuBg3: bg,
    reemuBg4: bg,
    reemuGlowVeil: `linear-gradient(${bg}, ${bg})`,
    reemuVeil: `linear-gradient(${bg}, ${bg})`,
    reemuActiveBg: t.colorBrandBackground,
    reemuActiveFg: t.colorNeutralForegroundOnBrand,
    reemuFillWeak: "rgba(255, 255, 255, 0.08)",
    reemuBrandText: t.colorBrandForeground1,
    reemuBrandAccent: t.colorBrandStroke1,
    // alto contraste: card preto com traço branco
    reemuCardTop: bg,
    reemuCardBottom: bg,
    reemuCardStroke: t.colorNeutralStroke1,
    reemuFieldStroke: t.colorNeutralStroke1,
    fontFamilyBase: segoeVariable(t.fontFamilyBase).text,
    reemuFontDisplay: segoeVariable(t.fontFamilyBase).display,
    reemuPage: bg,
    ...liftTokens(false),
  };
}

// -------------------------------------------------------------- registro ----

export type ThemeId =
  | "reemu"
  | "xbox-green"
  | "xbox-classico"
  | "ps-blue"
  | "ps1"
  | "claro"
  | "ps-blue-claro"
  | "ps1-claro"
  | "alva"
  | "alva-claro"
  | "snes"
  | "snes-claro"
  | "dream"
  | "dream-claro"
  | "simple-vibes"
  | "serene"
  | "alto-contraste";

// "Roxo"/"Âmbar" (e seus pares "-claro") foram removidos: eram só uma rampa
// de cor simples, sem identidade nenhuma — redundante agora que o
// "Personalizado" deixa escolher qualquer matiz (ver seção abaixo). Os temas
// que sobram são todos "de marca" (Xbox, PlayStation).
export const THEMES: Record<ThemeId, { label: string; theme: ReEmuTheme }> = {
  // Identidade do próprio ReEmu (logo + site) — o tema padrão. Os nomes
// exibidos dos temas são próprios, sem marcas de consoles (ver os `names` em
// i18n/locales); os IDs continuam os mesmos, que é o que fica salvo.
  reemu: {
    label: "Neon Circuit",
    theme: make(reemuGreen, "dark", reemuBg, {
      neutrals: reemuDark,
      appBg: "linear-gradient(180deg, #08100C 0%, #030605 45%)",
      activeBg: "#16211D",
    }),
  },
  "xbox-green": { label: "Green Modern", theme: make(xboxGreen, "dark", xboxGreenBg) },
  // Sem par "-claro" de propósito — o dashboard Blades nunca teve modo claro.
  "xbox-classico": { label: "Lime Classic", theme: make(xboxClassico, "dark", xboxClassicoBg) },
  "ps-blue": { label: "Blue Modern", theme: make(psBlue, "dark", psBlueBg) },
  ps1: { label: "Crimson Retro", theme: make(ps1Red, "dark", ps1Bg) },
  // Modo claro do dashboard Xbox (Series S/X e "modo XBOX" no PC): fundo
  // branco/cinza bem claro — só a luminosidade da casca inverte, a marca
  // não muda. Uma variante claro por rampa, mesmo par light/dark que o
  // verde já tinha.
  claro: { label: "Green Modern (claro)", theme: make(xboxGreen, "light", xboxGreenBg) },
  "ps-blue-claro": { label: "Blue Modern (claro)", theme: make(psBlue, "light", psBlueBg) },
  "ps1-claro": {
    label: "Crimson Retro (claro)",
    theme: make(ps1Red, "light", ps1Bg),
  },
  // Alva: cores do Alvanista (alvanista.com), nos dois modos do site. No
  // claro a pílula ativa é o azul de marca, como a navegação de lá.
  alva: {
    label: "Alva",
    theme: make(alvaBlue, "dark", alvaBg, {
      neutrals: alvaDark,
      appBg: "linear-gradient(180deg, #121212 0%, #0E0E0E 45%)",
      activeBg: "#2E3035",
    }),
  },
  "alva-claro": {
    label: "Alva (claro)",
    theme: make(alvaBlue, "light", alvaBg, {
      neutrals: alvaLight,
      appBg: "linear-gradient(180deg, #F4F7FA 0%, #F0F2F5 45%)",
      activeBg: alvaBlue[80],
    }),
  },
  snes: {
    label: "Purple Mistique",
    theme: make(snesPurple, "dark", snesBg, {
      neutrals: snesDark,
      appBg: "linear-gradient(180deg, #1D1C23 0%, #15141A 45%)",
      activeBg: "#3A3845",
    }),
  },
  "snes-claro": {
    label: "Purple Mistique (claro)",
    theme: make(snesPurple, "light", snesBg, {
      neutrals: snesLight,
      appBg: "linear-gradient(180deg, #E4E2EA 0%, #D9D7E0 45%)",
      activeBg: snesPurple[80],
    }),
  },
  // Dream Sky: o menu do Dreamcast (céu azul + espiral laranja).
  dream: {
    label: "Dream Sky",
    theme: make(dreamOrange, "dark", dreamBg, {
      neutrals: dreamDark,
      appBg: "linear-gradient(180deg, #12223A 0%, #0D1828 45%)",
      activeBg: "#24395A",
    }),
  },
  "dream-claro": {
    label: "Dream Sky (claro)",
    theme: make(dreamOrange, "light", dreamBg, {
      neutrals: dreamLight,
      appBg: "linear-gradient(180deg, #E8F2FB 0%, #DCEAF7 45%)",
      activeBg: dreamOrange[70],
    }),
  },
  // Fundo simples, de uma cor só. Simple Vibes só existe no claro e Serene
  // só no escuro (cada um é um tema de um modo, sem o Switch Claro).
  "simple-vibes": {
    label: "Simple Vibes",
    theme: make(simpleGray, "light", { bg1: "", bg2: "", bg3: "", bg4: "" }, {
      neutrals: simpleLight,
      activeBg: simpleGray[70],
      solidBg: simpleLight.colorNeutralBackground1,
    }),
  },
  serene: {
    label: "Serene",
    theme: make(sereneWhite, "dark", { bg1: "", bg2: "", bg3: "", bg4: "" }, {
      neutrals: sereneDark,
      // item ativo do rail: o cinza claro do modo XBOX (#67696C), ícone
      // branco
      activeBg: "#62666C",
      solidBg: sereneDark.colorNeutralBackground1,
      brandText: sereneWhite[100],
      lift: sereneLift,
    }),
  },
  "alto-contraste": { label: "Alto contraste", theme: makeHighContrast() },
};

export const DEFAULT_THEME_ID: ThemeId = "reemu";

/** Um card em Configurações › Aparência: o tema e, quando existe, o par
 *  claro — o card mostra um botão Escuro/Claro em vez de dois cards. Os
 *  `ThemeId` continuam os mesmos (é o que fica salvo). */
export interface ThemeFamily {
  /** Chave do nome em `appearance.theme.names` (i18n). */
  nameKey:
    | "reemu"
    | "xboxGreen"
    | "xboxClassic"
    | "psBlue"
    | "psClassic"
    | "alva"
    | "snes"
    | "dream"
    | "simpleVibes"
    | "serene"
    | "highContrast";
  /** O tema da família (escuro, ou o único quando `lightOnly`). */
  dark: ThemeId;
  light?: ThemeId;
  /** Tema só claro, sem versão escura (o `dark` acima é ele). */
  lightOnly?: true;
}

export const THEME_FAMILIES: ThemeFamily[] = [
  { nameKey: "reemu", dark: "reemu" },
  { nameKey: "xboxGreen", dark: "xbox-green", light: "claro" },
  { nameKey: "xboxClassic", dark: "xbox-classico" },
  { nameKey: "psBlue", dark: "ps-blue", light: "ps-blue-claro" },
  { nameKey: "psClassic", dark: "ps1", light: "ps1-claro" },
  { nameKey: "alva", dark: "alva", light: "alva-claro" },
  { nameKey: "snes", dark: "snes", light: "snes-claro" },
  { nameKey: "dream", dark: "dream", light: "dream-claro" },
  { nameKey: "simpleVibes", dark: "simple-vibes", lightOnly: true },
  { nameKey: "serene", dark: "serene" },
  { nameKey: "highContrast", dark: "alto-contraste" },
];

/** Família e modo de um tema salvo. */
export function familyOf(id: ThemeId): { family: ThemeFamily; mode: ThemeMode } {
  for (const family of THEME_FAMILIES) {
    if (family.dark === id) return { family, mode: family.lightOnly ? "light" : "dark" };
    if (family.light === id) return { family, mode: "light" };
  }
  throw new Error(`tema sem família: ${id}`);
}

export function isThemeId(v: unknown): v is ThemeId {
  return typeof v === "string" && v in THEMES;
}

// --------------------------------------------------- tema "Personalizado" ----
//
// Igual à personalização de fundo do Xbox Series S/X: a pessoa escolhe
// claro/escuro e desliza UM controle de matiz — o resto da rampa (os outros
// 15 tons) é completado pelo sistema, não escolhido tom a tom. `RAMP_L`/
// `RAMP_S` abaixo são a mesma curva de luminosidade/saturação usada pra
// gerar as rampas "PS1" e "Xbox Clássico" à mão (extraída da rampa "Âmbar",
// a primeira calibrada manualmente) — só o matiz muda por tema; saturação e
// luminosidade vêm sempre dessa curva fixa, senão um matiz muito escuro ou
// pouco vívido geraria uma rampa sem contraste (por isso o controle exposto
// ao usuário é só o de matiz, não um seletor de cor livre).

const RAMP_STOPS = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160] as const;
const RAMP_L = [3.9, 8.0, 13.1, 18.0, 24.1, 30.0, 37.1, 43.9, 52.0, 57.8, 64.1, 71.0, 78.0, 84.9, 91.0, 96.1];
const RAMP_S = [50.0, 61.0, 73.1, 84.8, 91.9, 92.2, 91.5, 92.0, 91.8, 91.6, 92.3, 91.9, 85.7, 74.0, 60.9, 50.0];

function hslToHex(h: number, s: number, l: number): string {
  const sn = s / 100;
  const ln = l / 100;
  const c = (1 - Math.abs(2 * ln - 1)) * sn;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = ln - c / 2;
  const [r, g, b] =
    h < 60
      ? [c, x, 0]
      : h < 120
        ? [x, c, 0]
        : h < 180
          ? [0, c, x]
          : h < 240
            ? [0, x, c]
            : h < 300
              ? [x, 0, c]
              : [c, 0, x];
  const toHex = (v: number) =>
    Math.round((v + m) * 255)
      .toString(16)
      .padStart(2, "0")
      .toUpperCase();
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

/** Gera uma rampa de marca completa a partir de só um matiz (0-360°), pro
 *  tema "Personalizado" — ver comentário da seção acima. */
export function generateBrandRamp(hue: number): BrandVariants {
  const h = ((hue % 360) + 360) % 360;
  const out = {} as BrandVariants;
  RAMP_STOPS.forEach((stop, i) => {
    out[stop] = hslToHex(h, RAMP_S[i], RAMP_L[i]);
  });
  return out;
}

export type ThemeMode = "dark" | "light";

export type ThemeSelection =
  | { kind: "preset"; id: ThemeId }
  | { kind: "custom"; hue: number; mode: ThemeMode };

export const DEFAULT_CUSTOM_HUE = 205; // mesmo tom do "Azul PlayStation" — ponto de partida neutro.

/** Resolve uma seleção de tema (preset OU personalizado) pro `Theme` que o
 *  `FluentProvider` consome. Personalizado é gerado na hora — não fica em
 *  `THEMES`, que é só o catálogo de presets curados. */
/**
 * Paleta de fundo do "Personalizado": 4 matizes em volta do escolhido, no
 * mesmo arranjo dos presets — o próprio matiz no alto à esquerda, um vizinho
 * mais claro embaixo à direita, um vizinho escuro embaixo à esquerda e um
 * quase-complementar abafado no alto à direita (o "canto quente").
 */
export function customBgPalette(hue: number): BgPalette {
  const h = (d: number) => (((hue + d) % 360) + 360) % 360;
  return {
    bg1: hslToHex(h(0), 60, 30),
    bg2: hslToHex(h(35), 62, 38),
    bg3: hslToHex(h(-45), 55, 22),
    bg4: hslToHex(h(160), 42, 34),
  };
}

export function resolveTheme(selection: ThemeSelection): ReEmuTheme {
  if (selection.kind === "preset") return THEMES[selection.id].theme;
  return make(
    generateBrandRamp(selection.hue),
    selection.mode,
    customBgPalette(selection.hue),
  );
}
