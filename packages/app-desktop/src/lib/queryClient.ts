import { QueryClient } from '@tanstack/react-query'

// `refetchOnWindowFocus` desligado: o WebKitGTK dispara foco/visibilidade
// espúrios e a biblioteca re-renderizava sozinha (cartões "piscando"). Os
// dados só mudam em ações explícitas, que já invalidam a query.
export const queryClient = new QueryClient({
  defaultOptions: {
    // Tudo aqui é comando local do Tauri, que funciona sem rede. No modo padrão
    // (`online`), sem internet a operação ficava pausada para sempre, sem erro.
    mutations: { networkMode: 'always' },
    queries: { networkMode: 'always', refetchOnWindowFocus: false, staleTime: 5 * 60_000 },
  },
})
