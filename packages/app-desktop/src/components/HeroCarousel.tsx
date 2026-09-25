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
    // Teto subiu de 460 pra 960px — mesma proporção de tela, só sem travar
    // bem antes de 4K (~1769px de viewport).
    height: 'clamp(220px, 26vw, 960px)',
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
    fontSize: 'clamp(48px, 8vw, 220px)',
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
    left: 'clamp(20px, 3vw, 115px)',
    bottom: 'clamp(20px, 3vw, 115px)',
    maxWidth: '62%',
    zIndex: 1,
    // kicker / título / sistema são <span> (dentro de <button>) — em coluna,
    // senão saíam colados na mesma linha ("ContinuarJogo 2Super Nintendo").
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-start',
    textAlign: 'left',
  },
  // fluent2.microsoft.design/typography: "use sentence case, nunca all
  // caps" — tracking largo (0.08em) era calibrado pra maiúsculas, reduzido
  // junto (senão "Continuar"/"Destaque" minúsculo ficava esparramado).
  kicker: {
    fontSize: tokens.fontSizeBase200,
    fontWeight: tokens.fontWeightBold,
    letterSpacing: '0.02em',
    color: tokens.colorBrandForeground1,
  },
  title: {
    fontSize: 'clamp(20px, 2.4vw, 64px)',
    fontWeight: tokens.fontWeightBold,
    lineHeight: 1.12,
    margin: '4px 0 2px',
  },
  sub: {
    fontSize: 'clamp(12px, 0.9vw, 26px)',
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
    // Fluido: o `.wrap` (hero) cresce de 220 até 960px de altura
    // (`clamp(220px, 26vw, 960px)` acima) — as setas ficavam do tamanho
    // "medium" fixo do Fluent (32px) em qualquer altura de hero, minúsculas
    // num banner de 960px em 4K. `max-width`/`min-width`: o Fluent injeta um
    // `max-width` próprio em botão circular icon-only que vence o `width`
    // mesmo com `!important` (mesmo caso da topbar em `xbox.ts`).
    width: "clamp(32px, 2.4vw, 72px) !important",
    height: "clamp(32px, 2.4vw, 72px) !important",
    minWidth: "0 !important",
    maxWidth: "clamp(32px, 2.4vw, 72px) !important",
    fontSize: "clamp(15px, 1.1vw, 32px) !important",
    "& .fui-Button__icon": {
      fontSize: "1em",
      width: "1em",
      height: "1em",
    },
    ':hover': {
      backgroundColor: 'rgba(0,0,0,0.72)',
      color: "#ffffff",
      transform: 'translateY(-50%) scale(1.06)',
    },
  },
  arrowL: { left: 'clamp(10px, 0.8vw, 28px)' },
  arrowR: { right: 'clamp(10px, 0.8vw, 28px)' },
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
