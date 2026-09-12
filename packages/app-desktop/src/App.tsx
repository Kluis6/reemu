import { FluentProvider } from '@fluentui/react-components'
import { QueryClientProvider } from '@tanstack/react-query'
import { useMemo } from 'react'
import { RouterProvider } from 'react-router-dom'
import { queryClient } from './lib/queryClient'
import { router } from './router'
import { useThemeStore } from './stores/useThemeStore'
import { resolveTheme } from './styles/themes'

/**
 * Raiz da UI: aplica o tema de cor escolhido (`useThemeStore`) no
 * `FluentProvider` — trocar de tema re-renderiza tudo com os novos tokens.
 * "Personalizado" (`kind: "custom"`) é gerado na hora a partir do matiz —
 * `resolveTheme` cuida dos dois casos (preset já pronto ou gerado).
 */
export function App() {
  const selection = useThemeStore((s) => s.selection)
  const theme = useMemo(() => resolveTheme(selection), [selection])
  return (
    <FluentProvider theme={theme} style={{ minHeight: '100%' }}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </FluentProvider>
  )
}
