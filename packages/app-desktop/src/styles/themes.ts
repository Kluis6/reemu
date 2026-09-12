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
  type BrandVariants,
  type Theme,
} from "@fluentui/react-components";

// ---------------------------------------------------------------- rampas ----

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
const ps1Accents = { bg1: "#00AC9F", bg2: "#2E6DB4", bg3: "#F3C300" };

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
export interface ReEmuTokens {
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
  /** Manchas do fundo (`components/AnimatedBackground`) — 3 tons da
   *  rampa de marca, então cada tema pinta o fundo com a própria cor. */
  reemuBg1: string;
  reemuBg2: string;
  reemuBg3: string;
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
}

export type ReEmuTheme = Theme & ReEmuTokens;


function readableOn(hex: string): string {
  const n = hex.replace("#", "");
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(n.slice(i, i + 2), 16));
  // brilho percebido (YIQ) — > 128 pede texto escuro.
  return (r * 299 + g * 587 + b * 114) / 1000 > 128 ? "#0b0b0d" : "#ffffff";
}

function make(
  ramp: BrandVariants,
  mode: "dark" | "light" = "dark",
  bgAccents?: { bg1: string; bg2: string; bg3: string },
): ReEmuTheme {
  const light = mode === "light";
  return {
    ...(light ? createLightTheme(ramp) : createDarkTheme(ramp)),
    ...(light ? consoleLight : consoleDark),
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
    // Tema normal: 3 tons DA MESMA rampa. `bgAccents` (só o "PlayStation
    // Clássico" usa por ora) troca isso por cores fixas de marca oficiais,
    // pra um degradê multicor em vez de tons derivados de um único matiz.
    reemuBg1: bgAccents?.bg1 ?? ramp[70],
    reemuBg2: bgAccents?.bg2 ?? ramp[90],
    reemuBg3: bgAccents?.bg3 ?? ramp[50],
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
  };
}

// -------------------------------------------------------------- registro ----

export type ThemeId =
  | "xbox-green"
  | "xbox-classico"
  | "ps-blue"
  | "ps1"
  | "claro"
  | "ps-blue-claro"
  | "ps1-claro";

// "Roxo"/"Âmbar" (e seus pares "-claro") foram removidos: eram só uma rampa
// de cor simples, sem identidade nenhuma — redundante agora que o
// "Personalizado" deixa escolher qualquer matiz (ver seção abaixo). Os temas
// que sobram são todos "de marca" (Xbox, PlayStation).
export const THEMES: Record<ThemeId, { label: string; theme: ReEmuTheme }> = {
  "xbox-green": { label: "Verde Xbox", theme: make(xboxGreen) },
  // Sem par "-claro" de propósito — o dashboard Blades nunca teve modo claro.
  "xbox-classico": { label: "Xbox Clássico", theme: make(xboxClassico) },
  "ps-blue": { label: "Azul PlayStation", theme: make(psBlue) },
  ps1: { label: "PlayStation Clássico", theme: make(ps1Red, "dark", ps1Accents) },
  // Modo claro do dashboard Xbox (Series S/X e "modo XBOX" no PC): fundo
  // branco/cinza bem claro — só a luminosidade da casca inverte, a marca
  // não muda. Uma variante claro por rampa, mesmo par light/dark que o
  // verde já tinha.
  claro: { label: "Claro", theme: make(xboxGreen, "light") },
  "ps-blue-claro": { label: "Azul Claro", theme: make(psBlue, "light") },
  "ps1-claro": {
    label: "PlayStation Clássico Claro",
    theme: make(ps1Red, "light", ps1Accents),
  },
};

export const DEFAULT_THEME_ID: ThemeId = "xbox-green";

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
export function resolveTheme(selection: ThemeSelection): ReEmuTheme {
  if (selection.kind === "preset") return THEMES[selection.id].theme;
  return make(generateBrandRamp(selection.hue), selection.mode);
}
