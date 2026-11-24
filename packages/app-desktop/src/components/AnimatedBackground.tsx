import { makeStyles, mergeClasses } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { wallpaperPresetId, wallpaperUrl } from '../lib/tauri'
import { CAN_ANIMATE_WALLPAPER } from '../lib/wallpapers'
import { useWallpaperStore } from '../stores/useWallpaperStore'
import { WallpaperVideo } from './WallpaperVideo'

/**
 * Quatro gradientes nas cores do tema, com três camadas em movimento no
 * Windows. O modo estático exibe o quadro inicial, assim como no Linux ou
 * com movimento reduzido. Cada camada tem um único gradiente para manter
 * compatibilidade com o WebKitGTK mesmo sem composição acelerada.
 */
const glow = (v: string, fallback: string) =>
  `radial-gradient(ellipse closest-side, var(${v}, ${fallback}), transparent 100%)`

// O primeiro e o último quadro têm deslocamento zero. O navegador interpola
// a translação entre os pontos e os gradientes permanecem estáticos.
const flow = (horizontal: number, vertical: number, phase: number) => {
  const steps = 24
  const origin = 2 * Math.PI * phase
  return Object.fromEntries(Array.from({ length: steps + 1 }, (_, step) => {
    const progress = step / steps
    const angle = 2 * Math.PI * progress + origin
    const x = (horizontal * (Math.cos(angle) - Math.cos(origin))).toFixed(3)
    const y = (vertical * (Math.sin(angle) - Math.sin(origin))).toFixed(3)
    return [`${progress * 100}%`, { transform: `translate3d(${x}vw, ${y}vh, 0)` }]
  }))
}

const useStyles = makeStyles({
  root: {
    position: 'fixed',
    inset: 0,
    zIndex: -1,
    overflowX: 'hidden',
    overflowY: 'hidden',
    pointerEvents: 'none',
    // cor da página (ver `reemuPage` em styles/themes.ts)
    backgroundColor: 'var(--reemuPage)',
    contain: 'strict',
  },
  wallpaper: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    objectFit: 'cover',
    objectPosition: 'center',
  },
  glow: {
    position: 'absolute',
    width: '82vw',
    height: '100vh',
  },
  g1: { top: '-24%', left: '-21%', opacity: 0.91, backgroundImage: glow('--reemuBg1', '#1E7F74') },
  g2: { bottom: '-29%', right: '-19%', opacity: 0.89, backgroundImage: glow('--reemuBg2', '#2E9E4F') },
  g3: { bottom: '-54%', left: '-16%', opacity: 0.72, backgroundImage: glow('--reemuBg3', '#1B3F5C') },
  g4: { top: '-45%', right: '-14%', opacity: 0.8, backgroundImage: glow('--reemuBg4', '#8A3A4A') },
  flow: {
    animationDuration: '12s',
    animationTimingFunction: 'linear',
    animationIterationCount: 'infinite',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  d1: { animationName: flow(11, 16, 0.5) },
  d2: { animationName: flow(10, 13, 0), animationDirection: 'reverse' },
  d4: { animationName: flow(8, 13, 0.75) },
  paused: { animationPlayState: 'paused' },
  glowVeil: {
    position: 'absolute',
    inset: 0,
    backgroundImage: 'linear-gradient(var(--reemuGlowVeil, rgba(9, 9, 12, 0.1)), var(--reemuGlowVeil, rgba(9, 9, 12, 0.1)))',
  },
  wallpaperVeil: {
    position: 'absolute',
    inset: 0,
    backgroundImage:
      'var(--reemuVeil, radial-gradient(ellipse at center, rgba(9, 9, 12, 0) 0%, rgba(9, 9, 12, 0.82) 100%))',
  },
})

/**
 * `showWallpaper`: se o usuário escolheu um papel de parede, mostra SÓ ele
 * (sem as cores do tema). Quem decide é a tela: o `AppShell` liga só na
 * tela inicial ou em todas menos a página do jogo (`useWallpaperStore`);
 * O Onboarding fica com as cores do tema.
 */
/** `true` com a janela escondida/minimizada ou sem foco. */
function useWindowIdle(): boolean {
  const [idle, setIdle] = useState(() => document.hidden || !document.hasFocus())
  useEffect(() => {
    if (!CAN_ANIMATE_WALLPAPER) return
    const update = () => setIdle(document.hidden || !document.hasFocus())
    update()
    document.addEventListener('visibilitychange', update)
    window.addEventListener('focus', update)
    window.addEventListener('blur', update)
    return () => {
      document.removeEventListener('visibilitychange', update)
      window.removeEventListener('focus', update)
      window.removeEventListener('blur', update)
    }
  }, [])
  return idle
}

export function AnimatedBackground({ showWallpaper = false }: { showWallpaper?: boolean }) {
  const s = useStyles()
  const idle = useWindowIdle()
  const motion = useWallpaperStore((state) => state.themeBackgroundMotion)
  const animated = CAN_ANIMATE_WALLPAPER && motion === 'animated'
  const glowClass = (position: string, path?: string) =>
    mergeClasses(s.glow, position, animated && path && s.flow, animated && path, animated && path && idle && s.paused)
  // `staleTime: Infinity`: raramente muda — `SettingsAppearance` invalida a
  // query na mão quando o usuário troca/remove o papel de parede.
  const wallpaper = useQuery({
    queryKey: ['wallpaper'],
    queryFn: wallpaperUrl,
    staleTime: Infinity,
    enabled: showWallpaper,
  })
  const preset = useQuery({
    queryKey: ['wallpaper-preset'],
    queryFn: wallpaperPresetId,
    staleTime: Infinity,
    enabled: showWallpaper,
  })
  if (showWallpaper && preset.data) {
    return (
      <div className={s.root} data-wallpaper-visible aria-hidden>
        <WallpaperVideo key={preset.data} id={preset.data} idle={idle} className={s.wallpaper} />
        <div className={s.wallpaperVeil} />
      </div>
    )
  }
  if (showWallpaper && wallpaper.data) {
    return (
      <div className={s.root} data-wallpaper-visible aria-hidden>
        <img src={wallpaper.data} alt="" className={s.wallpaper} />
        <div className={s.wallpaperVeil} />
      </div>
    )
  }
  return (
    <div className={s.root} aria-hidden>
      <div className={glowClass(s.g3)} />
      <div className={glowClass(s.g4, s.d4)} />
      <div className={glowClass(s.g1, s.d1)} />
      <div className={glowClass(s.g2, s.d2)} />
      <div className={s.glowVeil} />
    </div>
  )
}
