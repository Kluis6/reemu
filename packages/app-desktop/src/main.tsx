import { FluentProvider, webDarkTheme } from '@fluentui/react-components'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import App from './App.tsx'
import './index.css'

// Dados do SQLite passam por IPC + cache de query (ver docs/ai-context/07).
const queryClient = new QueryClient()

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <FluentProvider theme={webDarkTheme} style={{ background: 'transparent', minHeight: '100%' }}>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </FluentProvider>
  </StrictMode>,
)
