import { create } from "zustand";

/**
 * Onde o papel de parede aparece: só na tela inicial (`home`, o padrão) ou
 * em todas as telas (`all`). Em qualquer um, a página do jogo fica sem ele
 * (lá o fundo é a arte do jogo) — quem aplica é o `AppShell`. A imagem em
 * si fica no Rust (`set_wallpaper`); esta escolha é uma conveniência de
 * interface, guardada no `localStorage` (com try/catch: sem storage, fica o
 * padrão).
 */
export type WallpaperScope = "home" | "all";
export type ThemeBackgroundMotion = "animated" | "static";

const SCOPE_KEY = "reemu.wallpaperScope";
const MOTION_KEY = "reemu.themeBackgroundMotion";

function readPreference(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writePreference(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Sem armazenamento, a preferência vale nesta sessão.
  }
}

interface WallpaperState {
  scope: WallpaperScope;
  themeBackgroundMotion: ThemeBackgroundMotion;
  setScope: (scope: WallpaperScope) => void;
  setThemeBackgroundMotion: (themeBackgroundMotion: ThemeBackgroundMotion) => void;
}

export const useWallpaperStore = create<WallpaperState>((set) => ({
  scope: readPreference(SCOPE_KEY) === "all" ? "all" : "home",
  themeBackgroundMotion: readPreference(MOTION_KEY) === "static" ? "static" : "animated",
  setScope: (scope) => {
    writePreference(SCOPE_KEY, scope);
    set({ scope });
  },
  setThemeBackgroundMotion: (themeBackgroundMotion) => {
    writePreference(MOTION_KEY, themeBackgroundMotion);
    set({ themeBackgroundMotion });
  },
}));
