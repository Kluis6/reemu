import { create } from 'zustand'

/** Campo de texto que o teclado na tela edita (fora a busca). */
export type KeyboardField = HTMLInputElement | HTMLTextAreaElement

/** Busca global da biblioteca (Y no controle, `/` no teclado). */
interface SearchState {
  /** `true` quando o campo está aberto/em foco. */
  open: boolean
  query: string
  /** Teclado na tela aberto (`OnScreenKeyboard`): pela busca (Y) ou num
   *  campo de texto confirmado com o A do controle. */
  keyboard: boolean
  /** Campo que o teclado edita; `null` = a busca (`query`). */
  keyboardTarget: KeyboardField | null
  /** Quem recebe os pulsos do controle enquanto o teclado está aberto
   *  (`useMenuNav` chama antes da navegação normal; `true` = consumido). */
  keyboardNav: ((nav: string) => boolean) | null
  setOpen: (open: boolean) => void
  setQuery: (query: string) => void
  /** Abre (busca) ou fecha o teclado; fechar esquece o campo. */
  setKeyboard: (keyboard: boolean) => void
  /** Abre o teclado pra digitar em `field`. */
  openKeyboardFor: (field: KeyboardField) => void
  setKeyboardNav: (fn: ((nav: string) => boolean) | null) => void
  reset: () => void
}

export const useSearchStore = create<SearchState>((set) => ({
  open: false,
  query: '',
  keyboard: false,
  keyboardTarget: null,
  keyboardNav: null,
  setOpen: (open) => set({ open }),
  setQuery: (query) => set({ query }),
  setKeyboard: (keyboard) => set(keyboard ? { keyboard } : { keyboard, keyboardTarget: null }),
  openKeyboardFor: (keyboardTarget) => set({ keyboard: true, keyboardTarget }),
  setKeyboardNav: (keyboardNav) => set({ keyboardNav }),
  reset: () => set({ open: false, query: '', keyboard: false, keyboardTarget: null }),
}))
