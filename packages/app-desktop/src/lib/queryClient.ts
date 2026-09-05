import { QueryClient } from '@tanstack/react-query'

// `refetchOnWindowFocus` desligado: o WebKitGTK dispara foco/visibilidade
// espúrios e a biblioteca re-renderizava sozinha (cartões "piscando"). Os
// dados só mudam em ações explícitas, que já invalidam a query.
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { refetchOnWindowFocus: false, staleTime: 5 * 60_000 },
  },
})
