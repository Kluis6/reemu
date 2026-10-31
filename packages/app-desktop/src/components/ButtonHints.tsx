import { mergeClasses } from '@fluentui/react-components'
import type { ReactNode } from 'react'
import { ArrowBidirectionalLeftRightRegular, LineHorizontal3Regular } from '@fluentui/react-icons'
import { TbXboxAFilled, TbXboxBFilled, TbXboxXFilled, TbXboxYFilled } from 'react-icons/tb'
import { useGamepadStore } from '../stores/useGamepadStore'
import { useHintStyles } from '../styles/xbox'

export type Glyph = 'A' | 'B' | 'X' | 'Y' | 'MENU' | 'LB/RB' | 'LT/RT' | 'DPAD'

const FACE: Partial<Record<Glyph, ReactNode>> = {
  A: <TbXboxAFilled />,
  B: <TbXboxBFilled />,
  X: <TbXboxXFilled />,
  Y: <TbXboxYFilled />,
}

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
  const color: Partial<Record<Glyph, string>> = { A: s.a, B: s.b, X: s.x, Y: s.y }
  return (
    <div className={mergeClasses('reemu-hints', s.hints, centered && s.hintsCentered)}>
      {hints.map((h) => (
        <span key={h.glyph} className={s.hint}>
          {h.glyph === 'MENU' ? (
            // ícone de menu do Fluent: o caractere ☰ (trigrama) saía grosso e
            // fora do centro do círculo
            <span className={mergeClasses(s.glyph, s.menu)}>
              <LineHorizontal3Regular />
            </span>
          ) : h.glyph === 'DPAD' ? (
            <span className={mergeClasses(s.glyph, s.menu)}>
              <ArrowBidirectionalLeftRightRegular />
            </span>
          ) : h.glyph === 'LB/RB' || h.glyph === 'LT/RT' ? (
            <span className={mergeClasses(s.glyph, s.shoulder)}>{h.glyph}</span>
          ) : (
            <span className={mergeClasses(s.glyph, s.face, color[h.glyph])}>{FACE[h.glyph]}</span>
          )}
          {h.label}
        </span>
      ))}
    </div>
  )
}
