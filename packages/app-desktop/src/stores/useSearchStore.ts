import { create } from 'zustand'

/** Busca global da biblioteca (Y no controle, `/` no teclado). */
interface SearchState {
  /** `true` quando o campo está aberto/em foco. */
  open: boolean
  query: string
  /** Teclado na tela aberto (busca pelo controle, `OnScreenKeyboard`). */
  keyboard: boolean
  /** Quem recebe os pulsos do controle enquanto o teclado está aberto
   *  (`useMenuNav` chama antes da navegação normal; `true` = consumido). */
  keyboardNav: ((nav: string) => boolean) | null
  setOpen: (open: boolean) => void
  setQuery: (query: string) => void
  setKeyboard: (keyboard: boolean) => void
  setKeyboardNav: (fn: ((nav: string) => boolean) | null) => void
  reset: () => void
}

export const useSearchStore = create<SearchState>((set) => ({
  open: false,
  query: '',
  keyboard: false,
  keyboardNav: null,
  setOpen: (open) => set({ open }),
  setQuery: (query) => set({ query }),
  setKeyboard: (keyboard) => set({ keyboard }),
  setKeyboardNav: (keyboardNav) => set({ keyboardNav }),
  reset: () => set({ open: false, query: '', keyboard: false }),
}))
