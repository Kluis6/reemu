import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useThemeStore } from '../stores/useThemeStore'

/**
 * Fundo animado das telas — manchas de cor grandes e desfocadas que derivam
 * devagar. As cores vêm de CSS custom properties (`--reemuBg1..3`) que o
 * `FluentProvider` emite a partir do tema (`styles/themes.ts`); sem tema, cai
 * num azul/roxo neutro. Respeita
 * `prefers-reduced-motion`. Puramente decorativo (`aria-hidden`, sem input).
 */
const drift1 = {
  '0%': { transform: 'translate3d(-8%, -6%, 0) scale(1)' },
  '50%': { transform: 'translate3d(6%, 4%, 0) scale(1.15)' },
  '100%': { transform: 'translate3d(-8%, -6%, 0) scale(1)' },
}
const drift2 = {
  '0%': { transform: 'translate3d(6%, 8%, 0) scale(1.1)' },
  '50%': { transform: 'translate3d(-6%, -4%, 0) scale(0.95)' },
  '100%': { transform: 'translate3d(6%, 8%, 0) scale(1.1)' },
}
const drift3 = {
  '0%': { transform: 'translate3d(0, 4%, 0) scale(1)' },
  '50%': { transform: 'translate3d(4%, -6%, 0) scale(1.2)' },
  '100%': { transform: 'translate3d(0, 4%, 0) scale(1)' },
}

const useStyles = makeStyles({
  root: {
    position: 'fixed',
    inset: 0,
    // atrás do conteúdo em fluxo, à frente do fundo do container
    zIndex: -1,
    overflow: 'hidden',
    pointerEvents: 'none',
    backgroundColor: tokens.colorNeutralBackground1,
  },
  blob: {
    position: 'absolute',
    width: '60vmax',
    height: '60vmax',
    borderRadius: '50%',
    filter: 'blur(80px)',
    opacity: 0.55,
    willChange: 'transform',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  b1: {
    top: '-20vmax',
    left: '-15vmax',
    backgroundColor: 'var(--reemuBg1, #3b82f6)',
    animationName: drift1,
    animationDuration: '26s',
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
  },
  b2: {
    bottom: '-25vmax',
    right: '-15vmax',
    backgroundColor: 'var(--reemuBg2, #8b5cf6)',
    animationName: drift2,
    animationDuration: '34s',
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
  },
  b3: {
    top: '20%',
    left: '35%',
    backgroundColor: 'var(--reemuBg3, #06b6d4)',
    opacity: 0.35,
    animationName: drift3,
    animationDuration: '42s',
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
  },
  // véu escuro pra dar contraste no conteúdo por cima (a shell é dark-first)
  veil: {
    position: 'absolute',
    inset: 0,
    backgroundImage:
      'linear-gradient(180deg, rgba(9, 9, 12, 0.55) 0%, rgba(9, 9, 12, 0.82) 100%)',
  },
  still: { animationName: 'none' },
})

/** `forceStill` ignora a preferência (ex.: pra um preview). */
export function AnimatedBackground({ forceStill = false }: { forceStill?: boolean }) {
  const s = useStyles()
  const animated = useThemeStore((st) => st.bgAnimated)
  const still = forceStill || !animated ? s.still : undefined
  return (
    <div className={s.root} aria-hidden>
      <div className={mergeClasses(s.blob, s.b1, still)} />
      <div className={mergeClasses(s.blob, s.b2, still)} />
      <div className={mergeClasses(s.blob, s.b3, still)} />
      <div className={s.veil} />
    </div>
  )
}
