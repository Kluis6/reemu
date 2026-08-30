import { useEffect, useState } from 'react'

/** Hora atual `HH:MM`, atualizada a cada 30s (relógio do topbar estilo Xbox). */
export function useClock(): string {
  const [now, setNow] = useState(() => new Date())
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 30_000)
    return () => window.clearInterval(id)
  }, [])
  return now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}
