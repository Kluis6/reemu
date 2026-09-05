import { FluentProvider } from '@fluentui/react-components'
import { QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from 'react-router-dom'
import { queryClient } from './lib/queryClient'
import { router } from './router'
import { useThemeStore } from './stores/useThemeStore'
import { THEMES } from './styles/themes'

/**
 * Raiz da UI: aplica o tema de cor escolhido (`useThemeStore`) no
 * `FluentProvider` — trocar de tema re-renderiza tudo com os novos tokens.
 */
export function App() {
  const themeId = useThemeStore((s) => s.themeId)
  return (
    <FluentProvider theme={THEMES[themeId].theme} style={{ minHeight: '100%' }}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </FluentProvider>
  )
}
