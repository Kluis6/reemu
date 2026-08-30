import { mergeClasses } from '@fluentui/react-components'
import { useHintStyles } from '../styles/xbox'

export type Glyph = 'A' | 'B' | 'X' | 'Y' | 'MENU'

export interface Hint {
  glyph: Glyph
  label: string
}

/** Barra de dicas de botão do controle, canto inferior direito (estilo Xbox). */
export function ButtonHints({ hints }: { hints: readonly Hint[] }) {
  const s = useHintStyles()
  if (hints.length === 0) return null
  const color: Record<Exclude<Glyph, 'MENU'>, string> = { A: s.a, B: s.b, X: s.x, Y: s.y }
  return (
    <div className={s.hints}>
      {hints.map((h) => (
        <span key={h.glyph} className={s.hint}>
          {h.glyph === 'MENU' ? (
            <span className={mergeClasses(s.glyph, s.menu)}>☰</span>
          ) : (
            <span className={mergeClasses(s.glyph, color[h.glyph])}>{h.glyph}</span>
          )}
          {h.label}
        </span>
      ))}
    </div>
  )
}
