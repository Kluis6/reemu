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

const KEY = "reemu.wallpaperScope";

function load(): WallpaperScope {
  try {
    return localStorage.getItem(KEY) === "all" ? "all" : "home";
  } catch {
    return "home";
  }
}

interface WallpaperState {
  scope: WallpaperScope;
  setScope: (scope: WallpaperScope) => void;
}

export const useWallpaperStore = create<WallpaperState>((set) => ({
  scope: load(),
  setScope: (scope) => {
    try {
      localStorage.setItem(KEY, scope);
    } catch {
      /* modo privado / storage bloqueado: vale só nesta sessão */
    }
    set({ scope });
  },
}));
