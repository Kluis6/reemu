import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useThemeStore } from '../stores/useThemeStore'

/**
 * Fundo animado das telas — 2 manchas de cor desfocadas que derivam devagar.
 * As cores vêm de `--reemuBg1/2` (o `FluentProvider` emite a partir do tema).
 * Leve de propósito: só `translate` no keyframe (sem `scale`/`filter`
 * animados — re-rasterizavam o blur a cada frame no WebKitGTK). Decorativo.
 */
const drift1 = {
  '0%, 100%': { transform: 'translate3d(-4%, -3%, 0)' },
  '50%': { transform: 'translate3d(4%, 3%, 0)' },
}
const drift2 = {
  '0%, 100%': { transform: 'translate3d(3%, 4%, 0)' },
  '50%': { transform: 'translate3d(-3%, -3%, 0)' },
}

const useStyles = makeStyles({
  root: {
    position: 'fixed',
    inset: 0,
    zIndex: -1,
    overflowX: 'hidden',
    overflowY: 'hidden',
    pointerEvents: 'none',
    backgroundColor: tokens.colorNeutralBackground1,
    contain: 'strict',
  },
  blob: {
    position: 'absolute',
    width: '55vmax',
    height: '55vmax',
    borderRadius: '50%',
    filter: 'blur(55px)',
    opacity: 0.4,
    willChange: 'transform',
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  b1: {
    top: '-18vmax',
    left: '-14vmax',
    backgroundColor: 'var(--reemuBg1, #3b82f6)',
    animationName: drift1,
    animationDuration: '40s',
  },
  b2: {
    bottom: '-22vmax',
    right: '-14vmax',
    backgroundColor: 'var(--reemuBg2, #8b5cf6)',
    animationName: drift2,
    animationDuration: '52s',
  },
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
      <div className={s.veil} />
    </div>
  )
}
