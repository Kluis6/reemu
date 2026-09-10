import { makeStyles, tokens } from '@fluentui/react-components'
import { AnimatedBackground } from './AnimatedBackground'

const rise = {
  from: { opacity: 0, transform: 'translateY(10px) scale(0.98)' },
  to: { opacity: 1, transform: 'translateY(0) scale(1)' },
}
const pulse = {
  '0%, 100%': { opacity: 0.35 },
  '50%': { opacity: 0.9 },
}

const useStyles = makeStyles({
  root: {
    position: 'fixed',
    inset: 0,
    zIndex: 9999,
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: tokens.spacingVerticalXL,
  },
  wordmark: {
    position: 'relative',
    zIndex: 1,
    fontSize: 'clamp(48px, 9vw, 128px)',
    fontWeight: tokens.fontWeightSemibold,
    letterSpacing: '0.04em',
    color: tokens.colorNeutralForeground1,
    animationName: rise,
    animationDuration: '600ms',
    animationTimingFunction: 'ease-out',
    animationFillMode: 'both',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  tag: {
    position: 'relative',
    zIndex: 1,
    fontSize: tokens.fontSizeBase300,
    color: tokens.colorNeutralForeground3,
    animationName: pulse,
    animationDuration: '1800ms',
    animationIterationCount: 'infinite',
    animationTimingFunction: 'ease-in-out',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none', opacity: 0.6 },
  },
  leaving: {
    opacity: 0,
    transition: 'opacity 320ms ease',
    pointerEvents: 'none',
  },
})

/** Tela de abertura estilo console. `leaving` dispara o fade-out. */
export function Splash({ leaving = false }: { leaving?: boolean }) {
  const s = useStyles()
  return (
    <div className={leaving ? `${s.root} ${s.leaving}` : s.root}>
      <AnimatedBackground />
      <div className={s.wordmark}>ReEmu</div>
      <div className={s.tag}>carregando…</div>
    </div>
  )
}
