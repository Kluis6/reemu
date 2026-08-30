import { create } from 'zustand'

/** Espelha o estado de tela cheia da janela (decidido no Rust). */
interface FullscreenState {
  on: boolean
  setOn: (on: boolean) => void
}

export const useFullscreenStore = create<FullscreenState>((set) => ({
  on: true, // o app abre em tela cheia (tauri.conf.json)
  setOn: (on) => set({ on }),
}))
