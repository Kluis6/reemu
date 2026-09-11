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

const roxo: BrandVariants = {
  10: "#08050F",
  20: "#0F0821",
  30: "#17083B",
  40: "#1E0557",
  50: "#270576",
  60: "#310693",
  70: "#3D08B5",
  80: "#4809D7",
  90: "#5914F5",
  100: "#6E31F6",
  110: "#824FF8",
  120: "#9B71F9",
  130: "#B495F9",
  140: "#CDBBF6",
  150: "#E2DAF7",
  160: "#F3F0FA",
};

const ambar: BrandVariants = {
  10: "#0F0B05",
  20: "#211608",
  30: "#3A2509",
  40: "#553407",
  50: "#764705",
  60: "#935806",
  70: "#B56D08",
  80: "#D78109",
  90: "#F59714",
  100: "#F6A431",
  110: "#F8B14F",
  120: "#F9C071",
  130: "#F7CF97",
  140: "#F5DDBC",
  150: "#F6EADA",
  160: "#FAF6F0",
};

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

function make(ramp: BrandVariants, mode: "dark" | "light" = "dark"): ReEmuTheme {
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
    reemuBg1: ramp[70],
    reemuBg2: ramp[90],
    reemuBg3: ramp[50],
    // Radial (não mais vertical): centro bem mais transparente — deixa o
    // papel de parede aparecer no meio da tela — e as bordas/cantos (onde
    // ficam as manchas de cor do tema) mantêm a força de antes. Camada
    // ÚNICA de gradiente (WebKitGTK quebra com radial-gradient multicamada
    // num elemento `position: fixed` — ver comentário no `.app`).
    reemuVeil: light
      ? "radial-gradient(ellipse at center, rgba(255, 255, 255, 0.08) 0%, rgba(255, 255, 255, 0.9) 100%)"
      : "radial-gradient(ellipse at center, rgba(9, 9, 12, 0.04) 0%, rgba(9, 9, 12, 0.82) 100%)",
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
  | "roxo"
  | "ambar"
  | "ps-blue"
  | "claro"
  | "roxo-claro"
  | "ambar-claro"
  | "ps-blue-claro";

export const THEMES: Record<ThemeId, { label: string; theme: ReEmuTheme }> = {
  "xbox-green": { label: "Verde Xbox", theme: make(xboxGreen) },
  "ps-blue": { label: "Azul PlayStation", theme: make(psBlue) },
  roxo: { label: "Roxo", theme: make(roxo) },
  ambar: { label: "Âmbar", theme: make(ambar) },
  // Modo claro do dashboard Xbox (Series S/X e "modo XBOX" no PC): fundo
  // branco/cinza bem claro — só a luminosidade da casca inverte, a marca
  // não muda. Uma variante claro por rampa, mesmo par light/dark que o
  // verde já tinha.
  claro: { label: "Claro", theme: make(xboxGreen, "light") },
  "roxo-claro": { label: "Roxo Claro", theme: make(roxo, "light") },
  "ambar-claro": { label: "Âmbar Claro", theme: make(ambar, "light") },
  "ps-blue-claro": { label: "Azul Claro", theme: make(psBlue, "light") },
};

export const DEFAULT_THEME_ID: ThemeId = "xbox-green";

export function isThemeId(v: unknown): v is ThemeId {
  return typeof v === "string" && v in THEMES;
}
