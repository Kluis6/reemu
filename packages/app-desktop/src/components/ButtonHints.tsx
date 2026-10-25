import { mergeClasses } from '@fluentui/react-components'
import { LineHorizontal3Regular } from '@fluentui/react-icons'
import { useGamepadStore } from '../stores/useGamepadStore'
import { useHintStyles } from '../styles/xbox'

export type Glyph = 'A' | 'B' | 'X' | 'Y' | 'MENU'

export interface Hint {
  glyph: Glyph
  label: string
}

/** Barra de dicas de botão do controle, canto inferior direito (estilo Xbox)
 *  ou, com `centered`, no meio embaixo. Só aparece com um controle conectado
 *  (`useGamepadStore`, o mesmo do ícone da topbar): sem controle, os glifos
 *  A/B/X/Y não servem pra nada. */
export function ButtonHints({
  hints,
  centered = false,
}: {
  hints: readonly Hint[]
  centered?: boolean
}) {
  const s = useHintStyles()
  const connected = useGamepadStore((st) => st.devices.length > 0)
  if (!connected || hints.length === 0) return null
  const color: Record<Exclude<Glyph, 'MENU'>, string> = { A: s.a, B: s.b, X: s.x, Y: s.y }
  return (
    <div className={mergeClasses(s.hints, centered && s.hintsCentered)}>
      {hints.map((h) => (
        <span key={h.glyph} className={s.hint}>
          {h.glyph === 'MENU' ? (
            // ícone de menu do Fluent: o caractere ☰ (trigrama) saía grosso e
            // fora do centro do círculo
            <span className={mergeClasses(s.glyph, s.menu)}>
              <LineHorizontal3Regular />
            </span>
          ) : (
            <span className={mergeClasses(s.glyph, color[h.glyph])}>{h.glyph}</span>
          )}
          {h.label}
        </span>
      ))}
    </div>
  )
}
