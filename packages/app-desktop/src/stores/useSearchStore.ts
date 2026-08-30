import { create } from 'zustand'

/** Busca global da biblioteca (Y no controle, `/` no teclado). */
interface SearchState {
  /** `true` quando o campo está aberto/em foco. */
  open: boolean
  query: string
  setOpen: (open: boolean) => void
  setQuery: (query: string) => void
  reset: () => void
}

export const useSearchStore = create<SearchState>((set) => ({
  open: false,
  query: '',
  setOpen: (open) => set({ open }),
  setQuery: (query) => set({ query }),
  reset: () => set({ open: false, query: '' }),
}))
