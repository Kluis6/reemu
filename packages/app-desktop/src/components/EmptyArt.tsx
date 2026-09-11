import type { ReactNode } from 'react'

/**
 * Ilustrações de estado vazio — linha fina, tom de marca (via `currentColor`),
 * no espírito das artes isométricas do modo XBOX. Decorativas (`aria-hidden`).
 * Passe uma destas no prop `art` do `EmptyState`.
 */
const COMMON = {
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 2,
  strokeLinecap: 'round' as const,
  strokeLinejoin: 'round' as const,
}

function Frame({ children }: { children: ReactNode }) {
  return (
    <svg viewBox="0 0 160 120" width="160" height="120" aria-hidden role="img">
      {children}
    </svg>
  )
}

/** Controle — biblioteca vazia / boas-vindas. */
export function GamepadArt() {
  return (
    <Frame>
      <g {...COMMON}>
        <path d="M52 44 h56 c14 0 24 12 26 26 l4 22 c2 12 -8 20 -18 14 l-16 -10 h-52 l-16 10 c-10 6 -20 -2 -18 -14 l4 -22 c2 -14 12 -26 26 -26 Z" />
        <line x1="40" y1="70" x2="56" y2="70" />
        <line x1="48" y1="62" x2="48" y2="78" />
        <circle cx="106" cy="64" r="4" />
        <circle cx="120" cy="74" r="4" />
        <path d="M70 30 q10 -10 20 0" opacity="0.5" />
      </g>
    </Frame>
  )
}

/** Lupa — busca sem resultado. */
export function SearchArt() {
  return (
    <Frame>
      <g {...COMMON}>
        <circle cx="70" cy="54" r="30" />
        <line x1="92" y1="76" x2="118" y2="102" />
        <path d="M58 54 a12 12 0 0 1 12 -12" opacity="0.5" />
      </g>
    </Frame>
  )
}

/** Estrela — sem favoritos. */
export function StarArt() {
  return (
    <Frame>
      <g {...COMMON}>
        <path d="M80 26 l16 34 37 5 -27 26 7 37 -33 -18 -33 18 7 -37 -27 -26 37 -5 Z" />
      </g>
    </Frame>
  )
}
