/**
 * Temas de cor do ReEmu (Fluent 2 / Griffel).
 *
 * Cada tema = uma rampa de marca (`BrandVariants`, 16 tons) passada por
 * `createDarkTheme`, mais:
 *  - `consoleDark`: neutros mais escuros que o `webDarkTheme` — a rampa neutra
 *    do Fluent é clara demais pra uma tela cheia tipo TV (era o motivo do
 *    antigo objeto `brand` em `xbox.ts`).
 *  - tokens **custom** `reemu*`: o `FluentProvider` emite toda chave do objeto
 *    de tema como CSS var (`--reemuAppBg`, …), então o Griffel referencia com
 *    `var(--reemuAppBg)`.
 *
 * Regenerar rampas: Fluent Theme Designer
 * (https://react.fluentui.dev/?path=/docs/theme-theme-designer--docs) ou o
 * script HSL em docs/design/fluent2.md. Semente ~tom 80.
 */
import {
  createDarkTheme,
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

/** Tokens custom do ReEmu, emitidos como `--reemu*` pelo `FluentProvider`. */
export interface ReEmuTokens {
  /** Fundo da casca (`.app`) — gradiente com leve lavagem da cor de marca. */
  reemuAppBg: string;
  /** Tinta de marca translúcida (brilho de canto, chip ativo). */
  reemuAccentSoft: string;
  /** Preenchimento sólido de marca (círculo do rail, glifos de dica). */
  reemuBrandSolid: string;
  /** Cor de texto legível sobre `reemuBrandSolid`. */
  reemuOnBrand: string;
}

export type ReEmuTheme = Theme & ReEmuTokens;

function rgba(hex: string, alpha: number): string {
  const n = hex.replace("#", "");
  const r = parseInt(n.slice(0, 2), 16);
  const g = parseInt(n.slice(2, 4), 16);
  const b = parseInt(n.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function readableOn(hex: string): string {
  const n = hex.replace("#", "");
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(n.slice(i, i + 2), 16));
  // brilho percebido (YIQ) — > 128 pede texto escuro.
  return (r * 299 + g * 587 + b * 114) / 1000 > 128 ? "#0b0b0d" : "#ffffff";
}

function make(ramp: BrandVariants): ReEmuTheme {
  return {
    ...createDarkTheme(ramp),
    ...consoleDark,
    reemuAppBg: `linear-gradient(180deg, ${rgba(ramp[30], 0.35)} 0%, #0b0b0d 45%)`,
    reemuAccentSoft: rgba(ramp[70], 0.16),
    reemuBrandSolid: ramp[80],
    reemuOnBrand: readableOn(ramp[80]),
  };
}

// -------------------------------------------------------------- registro ----

export type ThemeId = "xbox-green" | "roxo" | "ambar";

export const THEMES: Record<ThemeId, { label: string; theme: ReEmuTheme }> = {
  "xbox-green": { label: "Verde Xbox", theme: make(xboxGreen) },
  roxo: { label: "Roxo", theme: make(roxo) },
  ambar: { label: "Âmbar", theme: make(ambar) },
};

export const DEFAULT_THEME_ID: ThemeId = "xbox-green";

export function isThemeId(v: unknown): v is ThemeId {
  return typeof v === "string" && v in THEMES;
}
