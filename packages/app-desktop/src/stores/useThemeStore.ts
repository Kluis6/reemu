import { create } from "zustand";
import { DEFAULT_THEME_ID, isThemeId, type ThemeId } from "../styles/themes";

/**
 * Tema de cor escolhido (ver `styles/themes.ts`). Persistido em `localStorage`
 * e lido de forma síncrona na criação da store — assim o `FluentProvider` já
 * monta com o tema certo, sem flash. (Um persist no lado Rust entra junto com
 * a tela de Configurações › Aparência.)
 */
const KEY = "reemu.theme";

function initialThemeId(): ThemeId {
  try {
    const v = localStorage.getItem(KEY);
    if (isThemeId(v)) return v;
  } catch {
    /* modo privado / storage bloqueado */
  }
  return DEFAULT_THEME_ID;
}

interface ThemeState {
  themeId: ThemeId;
  setTheme: (id: ThemeId) => void;
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
}));
