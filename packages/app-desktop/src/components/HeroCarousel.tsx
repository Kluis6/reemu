import { Button, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { ChevronLeftRegular, ChevronRightRegular } from '@fluentui/react-icons'
import { useCallback, useEffect, useState } from 'react'
import { initials } from '../lib/initials'
import { platformLabel } from '../lib/platform'
import type { RomEntry } from '../lib/tauri'

const ADVANCE_MS = 7000

const useStyles = makeStyles({
  wrap: {
    position: 'relative',
    width: '100%',
    minWidth: 0,
    height: 'clamp(220px, 26vw, 460px)',
    borderRadius: tokens.borderRadiusXLarge,
    overflow: 'hidden',
    marginTop: '8px',
    backgroundImage: 'linear-gradient(135deg, #1a1a1f 0%, #101014 100%)',
  },
  slide: {
    position: 'absolute',
    inset: 0,
    border: 'none',
    padding: 0,
    width: '100%',
    textAlign: 'left',
    color: 'inherit',
    cursor: 'pointer',
    backgroundColor: 'transparent',
    opacity: 0,
    transitionProperty: 'opacity',
    transitionDuration: '360ms',
    transitionTimingFunction: tokens.curveEasyEase,
    pointerEvents: 'none',
  },
  slideOn: { opacity: 1, pointerEvents: 'auto' },
  img: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    objectFit: 'cover',
  },
  fallback: {
    position: 'absolute',
    inset: 0,
    display: 'grid',
    placeItems: 'center',
    fontSize: 'clamp(48px, 8vw, 120px)',
    fontWeight: tokens.fontWeightBold,
    color: 'rgba(255,255,255,0.12)',
  },
  scrim: {
    position: 'absolute',
    inset: 0,
    backgroundImage:
      'linear-gradient(90deg, rgba(0,0,0,0.82) 0%, rgba(0,0,0,0.2) 55%, transparent 78%), linear-gradient(0deg, rgba(0,0,0,0.6), transparent 42%)',
  },
  body: {
    position: 'absolute',
    left: 'clamp(20px, 3vw, 40px)',
    bottom: 'clamp(20px, 3vw, 40px)',
    maxWidth: '62%',
    zIndex: 1,
  },
  kicker: {
    fontSize: tokens.fontSizeBase200,
    fontWeight: tokens.fontWeightBold,
    letterSpacing: '0.08em',
    textTransform: 'uppercase',
    color: tokens.colorBrandForeground1,
  },
  title: {
    fontSize: 'clamp(20px, 2.4vw, 42px)',
    fontWeight: tokens.fontWeightBold,
    lineHeight: 1.12,
    margin: '4px 0 2px',
  },
  sub: {
    fontSize: 'clamp(12px, 0.9vw, 17px)',
    color: tokens.colorNeutralForeground2,
  },
  arrow: {
    position: 'absolute',
    top: '50%',
    transform: 'translateY(-50%)',
    zIndex: 2,
    borderRadius: tokens.borderRadiusCircular,
    backgroundColor: 'rgba(0,0,0,0.45)',
    color: "#ffffff",
    transitionProperty: 'background-color, transform',
    transitionDuration: '150ms',
    transitionTimingFunction: tokens.curveEasyEase,
    ':hover': {
      backgroundColor: 'rgba(0,0,0,0.72)',
      color: "#ffffff",
      transform: 'translateY(-50%) scale(1.06)',
    },
  },
  arrowL: { left: '10px' },
  arrowR: { right: '10px' },
  dots: {
    position: 'absolute',
    bottom: '12px',
    left: '50%',
    transform: 'translateX(-50%)',
    display: 'flex',
    gap: '6px',
    zIndex: 2,
  },
  dot: {
    width: '7px',
    height: '7px',
    borderRadius: tokens.borderRadiusCircular,
    border: 'none',
    padding: 0,
    cursor: 'pointer',
    backgroundColor: 'rgba(255,255,255,0.35)',
    transitionProperty: 'width, background-color',
    transitionDuration: '200ms',
    transitionTimingFunction: tokens.curveEasyEase,
  },
  dotOn: { width: '20px', backgroundColor: "#ffffff" },
})

/**
 * Carrossel de destaque da Início (modelo modo XBOX): 1 slide por vez,
 * crossfade, setas circulares + dots, avanço automático a cada 7s (pausa no
 * hover/foco e quando a aba está oculta). Cada slide abre o jogo.
 */
export function HeroCarousel({
  items,
  onOpen,
}: {
  items: readonly RomEntry[]
  onOpen: (id: string) => void
}) {
  const s = useStyles()
  const [rawIdx, setRawIdx] = useState(0)
  const [paused, setPaused] = useState(false)
  const n = items.length
  // a lista pode encolher entre renders — clampa aqui, sem efeito
  const idx = n > 0 ? ((rawIdx % n) + n) % n : 0
  const go = useCallback((next: number) => setRawIdx(next), [])

  useEffect(() => {
    if (n <= 1 || paused) return
    const t = setInterval(() => {
      if (!document.hidden) setRawIdx((i) => i + 1)
    }, ADVANCE_MS)
    return () => clearInterval(t)
  }, [n, paused])

  if (n === 0) return null

  return (
    <div
      className={s.wrap}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocusCapture={() => setPaused(true)}
      onBlurCapture={() => setPaused(false)}
      aria-roledescription="carrossel"
    >
      {items.map((r, i) => (
        <button
          key={r.id}
          type="button"
          className={mergeClasses(s.slide, i === idx && s.slideOn)}
          onClick={() => onOpen(r.id)}
          aria-hidden={i !== idx}
          tabIndex={i === idx ? 0 : -1}
        >
          {r.boxart ? (
            <img className={s.img} src={r.boxart} alt="" />
          ) : (
            <span className={s.fallback}>{initials(r.title)}</span>
          )}
          <span className={s.scrim} />
          <span className={s.body}>
            <span className={s.kicker}>
              {r.lastPlayedAt ? 'Continuar' : 'Destaque'}
            </span>
            <span className={s.title}>{r.title}</span>
            <span className={s.sub}>{platformLabel(r.systemId)}</span>
          </span>
        </button>
      ))}

      {n > 1 && (
        <>
          <Button
            shape="circular"
            appearance="subtle"
            className={mergeClasses(s.arrow, s.arrowL)}
            icon={<ChevronLeftRegular />}
            aria-label="Anterior"
            onClick={() => go(idx - 1)}
          />
          <Button
            shape="circular"
            appearance="subtle"
            className={mergeClasses(s.arrow, s.arrowR)}
            icon={<ChevronRightRegular />}
            aria-label="Próximo"
            onClick={() => go(idx + 1)}
          />
          <div className={s.dots}>
            {items.map((r, i) => (
              <button
                key={r.id}
                type="button"
                className={mergeClasses(s.dot, i === idx && s.dotOn)}
                aria-label={`Ir para o destaque ${i + 1}`}
                aria-current={i === idx}
                onClick={() => go(i)}
              />
            ))}
          </div>
        </>
      )}
    </div>
  )
}
