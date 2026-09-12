import { FluentProvider } from '@fluentui/react-components'
import { MotionBehaviourProvider } from '@fluentui/react-motion'
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
 *
 * `MotionBehaviourProvider value="skip"` desliga a animação embutida de todo
 * componente Fluent v9 (Dialog, Menu, Tooltip…) — ela anima a propriedade CSS
 * `scale` isolada via Web Animations API, e nesta janela (WebKitGTK sem
 * aceleração de compositing na NVIDIA, ver `frontend-perf-webkitgtk`) isso
 * fica visivelmente quebrado: o navegador não consegue interpolar suave, o
 * elemento "salta" de um estado pro outro em vez de animar. Mesma classe de
 * problema que já tinha nos removido a animação de fundo contínua.
 */
export function App() {
  const selection = useThemeStore((s) => s.selection)
  const theme = useMemo(() => resolveTheme(selection), [selection])
  return (
    <FluentProvider theme={theme} style={{ minHeight: '100%' }}>
      <MotionBehaviourProvider value="skip">
        <QueryClientProvider client={queryClient}>
          <RouterProvider router={router} />
        </QueryClientProvider>
      </MotionBehaviourProvider>
    </FluentProvider>
  )
}
