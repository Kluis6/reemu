import { useRef, type ReactNode } from 'react'
import { useShelfStyles } from '../styles/xbox'

/**
 * Prateleira horizontal estilo Xbox (scroll-snap + chevron de "próxima
 * página"). Usada nas faixas curadas da Início ("Continuar jogando",
 * "Adicionados recentemente"…).
 */
export function Shelf({ children }: { children: ReactNode }) {
  const s = useShelfStyles()
  const ref = useRef<HTMLDivElement>(null)
  return (
    <div className={s.wrap}>
      <div className={s.shelf} ref={ref}>
        {children}
      </div>
      <button
        className={s.next}
        aria-label="Rolar"
        tabIndex={-1}
        onClick={() =>
          ref.current?.scrollBy({ left: ref.current.clientWidth * 0.8, behavior: 'smooth' })
        }
      >
        ›
      </button>
    </div>
  )
}
