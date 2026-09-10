import { create } from "zustand";
import { DEFAULT_THEME_ID, isThemeId, type ThemeId } from "../styles/themes";

/**
 * Aparência: tema de cor (ver `styles/themes.ts`) + se o fundo animado
 * (`components/AnimatedBackground`) se mexe. Persistido em `localStorage` e
 * lido de forma síncrona na criação da store — assim o `FluentProvider` já
 * monta com o tema certo, sem flash. (Um persist no lado Rust entra junto com
 * a evolução de Configurações › Aparência.)
 */
const KEY = "reemu.theme";
const ANIM_KEY = "reemu.bgAnimated";

function initialThemeId(): ThemeId {
  try {
    const v = localStorage.getItem(KEY);
    if (isThemeId(v)) return v;
  } catch {
    /* modo privado / storage bloqueado */
  }
  return DEFAULT_THEME_ID;
}

function initialBgAnimated(): boolean {
  try {
    return localStorage.getItem(ANIM_KEY) !== "0";
  } catch {
    return true;
  }
}

interface ThemeState {
  themeId: ThemeId;
  setTheme: (id: ThemeId) => void;
  /** Fundo animado se mexe (respeitando `prefers-reduced-motion` no CSS). */
  bgAnimated: boolean;
  setBgAnimated: (on: boolean) => void;
}

export const useThemeStore = create<ThemeState>((set) => ({
  themeId: initialThemeId(),
  setTheme: (themeId) => {
    try {
      localStorage.setItem(KEY, themeId);
    } catch {
      /* ignora — o tema ainda vale nesta sessão */
    }
    set({ themeId });
  },
  bgAnimated: initialBgAnimated(),
  setBgAnimated: (bgAnimated) => {
    try {
      localStorage.setItem(ANIM_KEY, bgAnimated ? "1" : "0");
    } catch {
      /* ignora */
    }
    set({ bgAnimated });
  },
}));
