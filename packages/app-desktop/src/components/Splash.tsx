import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'

const LOGO_SRC = '/reemu-logo.webp'
export const SPLASH_EXIT_MS = 240

const reveal = {
  from: { opacity: 0, transform: 'translate3d(0, 12px, 0)' },
  to: { opacity: 1, transform: 'translate3d(0, 0, 0)' },
}
const light = {
  '0%': { opacity: 0.04 },
  '48%': { opacity: 0.35 },
  '100%': { opacity: 0.23 },
}
const sweep = {
  from: { transform: 'translate3d(-100%, 0, 0)' },
  to: { transform: 'translate3d(250%, 0, 0)' },
}
const reducedMotion = '@media (prefers-reduced-motion: reduce)'

const useStyles = makeStyles({
  root: {
    position: 'fixed',
    inset: 0,
    zIndex: 9999,
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: tokens.spacingVerticalL,
    overflow: 'hidden',
    backgroundColor: '#050506',
    transitionProperty: 'opacity',
    transitionDuration: `${SPLASH_EXIT_MS}ms`,
    transitionTimingFunction: tokens.curveEasyEase,
    [reducedMotion]: { transitionDuration: '0ms' },
  },
  leaving: {
    opacity: 0,
    pointerEvents: 'none',
  },
  ambient: {
    position: 'absolute',
    inset: 0,
    backgroundImage: 'radial-gradient(ellipse at 50% 44%, var(--reemuBg3, #1b3f5c), transparent 73%)',
    opacity: 0.3,
    pointerEvents: 'none',
  },
  stage: {
    position: 'relative',
    width: 'clamp(96px, 46vmin, 350px)',
    aspectRatio: '1',
  },
  halo: {
    position: 'absolute',
    inset: '-38%',
    backgroundImage:
      'radial-gradient(ellipse closest-side, var(--reemuBrandSolid, #40dc78), var(--reemuBg1, #3c96ff) 40%, transparent 95%)',
    opacity: 0.23,
    animationName: light,
    animationDuration: '640ms',
    animationTimingFunction: tokens.curveEasyEase,
    animationFillMode: 'both',
    [reducedMotion]: { animationName: 'none' },
  },
  mark: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    objectFit: 'contain',
  },
  hiddenMark: { visibility: 'hidden' },
  appear: {
    animationName: reveal,
    animationDuration: tokens.durationUltraSlow,
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: 'both',
    [reducedMotion]: { animationName: 'none' },
  },
  wordmark: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontFamily: 'var(--reemuFontDisplay)',
    fontSize: 'clamp(36px, 8vmin, 72px)',
    fontWeight: tokens.fontWeightBold,
    letterSpacing: '0.03em',
    color: '#f4f6fb',
  },
  wordmarkAccent: { color: 'var(--reemuBrandSolid, #40dc78)' },
  status: {
    position: 'relative',
    display: 'grid',
    justifyItems: 'center',
    gap: tokens.spacingVerticalM,
    fontSize: tokens.fontSizeBase300,
    letterSpacing: '0.12em',
    textTransform: 'uppercase',
    color: '#c9cdd8',
  },
  track: {
    width: '112px',
    height: '2px',
    overflow: 'hidden',
    backgroundColor: '#ffffff24',
    borderRadius: tokens.borderRadiusCircular,
  },
  indicator: {
    display: 'block',
    width: '40%',
    height: '100%',
    transform: 'translate3d(75%, 0, 0)',
    backgroundColor: 'var(--reemuBrandSolid, #40dc78)',
    borderRadius: tokens.borderRadiusCircular,
    [reducedMotion]: { animationName: 'none' },
  },
  moving: {
    animationName: sweep,
    animationDuration: '1150ms',
    animationTimingFunction: tokens.curveEasyEase,
    animationIterationCount: 'infinite',
    animationFillMode: 'both',
  },
  paused: { animationPlayState: 'paused' },
  scanlines: {
    position: 'absolute',
    inset: 0,
    pointerEvents: 'none',
    backgroundImage: 'repeating-linear-gradient(to bottom, #00000015 0 1px, transparent 1px 4px)',
  },
})

export function Splash({ leaving, onExited }: { leaving: boolean; onExited: () => void }) {
  const { t } = useTranslation()
  const s = useStyles()
  const [logoState, setLogoState] = useState<'loading' | 'ready' | 'missing'>('loading')

  return (
    <div
      data-boot-splash=""
      className={mergeClasses(s.root, leaving && s.leaving)}
      onTransitionEnd={(event) => {
        if (leaving && event.target === event.currentTarget && event.propertyName === 'opacity') onExited()
      }}
    >
      <div className={s.ambient} aria-hidden />
      <div className={s.stage}>
        <div className={s.halo} aria-hidden />
        {logoState === 'missing' ? (
          <div className={mergeClasses(s.mark, s.wordmark, s.appear)}>
            Re<span className={s.wordmarkAccent}>Emu</span>
          </div>
        ) : (
          <img
            className={mergeClasses(s.mark, logoState === 'ready' ? s.appear : s.hiddenMark)}
            src={LOGO_SRC}
            width={512}
            height={512}
            alt="ReEmu"
            onLoad={() => setLogoState('ready')}
            onError={() => setLogoState('missing')}
          />
        )}
      </div>
      <div className={s.status} role="status">
        {t('shell.splashLoading')}
        <div className={s.track} aria-hidden>
          <span className={mergeClasses(s.indicator, s.moving, leaving && s.paused)} />
        </div>
      </div>
      <div className={s.scanlines} aria-hidden />
    </div>
  )
}
