import { makeStyles, tokens } from '@fluentui/react-components'
import { useState } from 'react'
import { AnimatedBackground } from './AnimatedBackground'

/** Caminho da logo — coloque `reemu-logo.png` em `packages/app-desktop/public/`. */
const LOGO_SRC = '/reemu-logo.png'

// Liga estilo CRT: linha fina brilhante → estica na vertical → assenta.
const powerOn = {
  '0%': { opacity: 0, transform: 'scaleY(0.006) scaleX(1.25)', filter: 'brightness(6)' },
  '38%': { opacity: 1, transform: 'scaleY(0.02) scaleX(1.18)', filter: 'brightness(5)' },
  '62%': { transform: 'scaleY(1.06) scaleX(0.99)', filter: 'brightness(1.6)' },
  '100%': { opacity: 1, transform: 'scale(1)', filter: 'brightness(1)' },
}
const glow = {
  '0%, 100%': {
    filter:
      'drop-shadow(0 0 14px rgba(64,220,120,0.35)) drop-shadow(0 0 34px rgba(60,150,255,0.22))',
  },
  '50%': {
    filter:
      'drop-shadow(0 0 26px rgba(64,220,120,0.6)) drop-shadow(0 0 60px rgba(60,150,255,0.4))',
  },
}
const float = {
  '0%, 100%': { transform: 'translateY(0) rotate(-0.5deg)' },
  '50%': { transform: 'translateY(-8px) rotate(0.5deg)' },
}
const sweep = {
  '0%': { transform: 'translateX(-140%) skewX(-14deg)' },
  '100%': { transform: 'translateX(140%) skewX(-14deg)' },
}
const rise = {
  from: { opacity: 0, transform: 'translateY(10px)' },
  to: { opacity: 1, transform: 'translateY(0)' },
}
const pulse = {
  '0%, 100%': { opacity: 0.3 },
  '50%': { opacity: 0.85 },
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
    width: 'min(78vw, 900px)',
    maxWidth: '100%',
    aspectRatio: '16 / 9',
    animationName: powerOn,
    animationDuration: '820ms',
    animationTimingFunction: 'cubic-bezier(.2,.9,.2,1.1)',
    animationFillMode: 'both',
    '@media (prefers-reduced-motion: reduce)': {
      animationName: rise,
      animationDuration: '400ms',
    },
  },
  logo: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    objectFit: 'contain',
    animationName: [glow, float],
    animationDuration: '3.2s, 7s',
    animationTimingFunction: 'ease-in-out, ease-in-out',
    animationIterationCount: 'infinite, infinite',
    animationDelay: '400ms, 400ms',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  // banda de luz que varre a logo
  shimmer: {
    position: 'absolute',
    top: 0,
    bottom: 0,
    left: 0,
    width: '55%',
    pointerEvents: 'none',
    backgroundImage:
      'linear-gradient(100deg, transparent 0%, rgba(255,255,255,0.18) 45%, rgba(180,240,255,0.28) 50%, rgba(255,255,255,0.18) 55%, transparent 100%)',
    mixBlendMode: 'screen',
    animationName: sweep,
    animationDuration: '2.6s',
    animationDelay: '900ms',
    animationTimingFunction: 'cubic-bezier(.4,0,.2,1)',
    animationIterationCount: 'infinite',
    animationFillMode: 'both',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  // textura de scanline (CRT)
  scanlines: {
    position: 'absolute',
    inset: 0,
    pointerEvents: 'none',
    backgroundImage:
      'repeating-linear-gradient(rgba(0,0,0,0) 0 2px, rgba(0,0,0,0.16) 2px 4px)',
    mixBlendMode: 'multiply',
    opacity: 0.6,
  },
  // fallback enquanto reemu-logo.png não estiver em /public
  wordmark: {
    position: 'absolute',
    inset: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: 'clamp(44px, 8vw, 110px)',
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
    animationDuration: '1600ms',
    animationIterationCount: 'infinite',
    animationTimingFunction: 'ease-in-out',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none', opacity: 0.6 },
  },
  leaving: {
    opacity: 0,
    transform: 'scale(1.04)',
    transition: 'opacity 340ms ease, transform 340ms ease',
    pointerEvents: 'none',
  },
})

/** Tela de abertura estilo console — liga a logo com efeito CRT, glow que
 *  respira e uma varredura de luz. `leaving` dispara o fade-out. */
export function Splash({ leaving = false }: { leaving?: boolean }) {
  const s = useStyles()
  const [noImg, setNoImg] = useState(false)
  return (
    <div className={leaving ? `${s.root} ${s.leaving}` : s.root}>
      <AnimatedBackground />
      <div className={s.stage}>
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
        <div className={s.shimmer} aria-hidden />
        <div className={s.scanlines} aria-hidden />
      </div>
      <div className={s.tag}>carregando…</div>
    </div>
  )
}
