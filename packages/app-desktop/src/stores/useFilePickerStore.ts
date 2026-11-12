import { create } from 'zustand'

export interface FilePickerRequest {
  title: string
  /** Escolhe a pasta atual, em vez de um arquivo. */
  directory: boolean
  /** Extensões aceitas, sem ponto. Vazio = todas. */
  extensions: string[]
}

interface FilePickerState {
  request: FilePickerRequest | null
  resolve: ((path: string | null) => void) | null
  /** Quem tinha o foco ao abrir: o foco volta pra ele ao fechar. */
  opener: HTMLElement | null
  /** Abre o navegador e devolve o caminho escolhido (`null` = cancelou). */
  ask: (request: FilePickerRequest) => Promise<string | null>
  finish: (path: string | null) => void
}

export const useFilePickerStore = create<FilePickerState>((set, get) => ({
  request: null,
  resolve: null,
  opener: null,
  ask: (request) =>
    new Promise((resolve) => {
      get().resolve?.(null)
      const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
      set({ request, resolve, opener })
    }),
  finish: (path) => {
    get().resolve?.(path)
    set({ request: null, resolve: null, opener: null })
  },
}))
