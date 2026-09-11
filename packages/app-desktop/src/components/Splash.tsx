import { makeStyles, tokens } from '@fluentui/react-components'
import { useState } from 'react'
import { AnimatedBackground } from './AnimatedBackground'

/** Caminho da logo — coloque `reemu-logo.png` em `packages/app-desktop/public/`. */
const LOGO_SRC = '/reemu-logo.png'

// Entrada: fade + leve expansão vertical (toque de CRT), sem animar filter.
const powerOn = {
  from: { opacity: 0, transform: 'scaleY(0.55) scaleX(1.02)' },
  '60%': { opacity: 1 },
  to: { opacity: 1, transform: 'scaleY(1) scaleX(1)' },
}
// glow que respira via OPACITY de um brilho separado (composited, barato)
const breathe = {
  '0%, 100%': { opacity: 0.4 },
  '50%': { opacity: 0.75 },
}
const rise = {
  from: { opacity: 0, transform: 'translateY(8px)' },
  to: { opacity: 1, transform: 'translateY(0)' },
}
const pulse = {
  '0%, 100%': { opacity: 0.35 },
  '50%': { opacity: 0.7 },
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
    gap: tokens.spacingVerticalXXL,
    backgroundColor: '#050506',
  },
  stage: {
    position: 'relative',
    zIndex: 1,
    width: 'min(78vw, 880px)',
    maxWidth: '100%',
    aspectRatio: '16 / 9',
    animationName: powerOn,
    animationDuration: '460ms',
    animationTimingFunction: 'cubic-bezier(.16,.84,.3,1)',
    animationFillMode: 'both',
    '@media (prefers-reduced-motion: reduce)': {
      animationName: rise,
      animationDuration: '260ms',
    },
  },
  // brilho estático atrás da logo; só a opacity anima
  glow: {
    position: 'absolute',
    inset: '-8%',
    borderRadius: '50%',
    backgroundImage:
      'radial-gradient(closest-side, rgba(64,220,120,0.35), rgba(60,150,255,0.16) 55%, transparent 78%)',
    animationName: breathe,
    animationDuration: '4s',
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none', opacity: 0.5 },
  },
  logo: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    objectFit: 'contain',
  },
  wordmark: {
    position: 'absolute',
    inset: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: 'clamp(44px, 8vw, 108px)',
    fontWeight: tokens.fontWeightBold,
    letterSpacing: '0.03em',
    color: tokens.colorNeutralForeground1,
  },
  wmGreen: { color: tokens.colorBrandForeground1 },
  tag: {
    position: 'relative',
    zIndex: 1,
    fontSize: tokens.fontSizeBase300,
    letterSpacing: '0.14em',
    textTransform: 'uppercase',
    color: tokens.colorNeutralForeground3,
    animationName: pulse,
    animationDuration: '2s',
    animationIterationCount: 'infinite',
    animationTimingFunction: 'ease-in-out',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none', opacity: 0.6 },
  },
  leaving: {
    opacity: 0,
    transition: 'opacity 240ms ease',
    pointerEvents: 'none',
  },
})

/** Tela de abertura — liga a logo com uma expansão curta + brilho que respira.
 *  `leaving` dispara o fade-out. */
export function Splash({ leaving = false }: { leaving?: boolean }) {
  const s = useStyles()
  const [noImg, setNoImg] = useState(false)
  return (
    <div className={leaving ? `${s.root} ${s.leaving}` : s.root}>
      <AnimatedBackground />
      <div className={s.stage}>
        <div className={s.glow} aria-hidden />
        {noImg ? (
          <div className={`${s.logo} ${s.wordmark}`}>
            Re<span className={s.wmGreen}>Emu</span>
          </div>
        ) : (
          <img
            className={s.logo}
            src={LOGO_SRC}
            alt="ReEmu"
            onError={() => setNoImg(true)}
          />
        )}
      </div>
      <div className={s.tag}>carregando…</div>
    </div>
  )
}
