import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { wallpaperUrl } from '../lib/tauri'

/**
 * Fundo das telas, em um de dois modos (nunca os dois juntos):
 *
 * - **Papel de parede** (só onde quem usa pede — hoje a tela inicial): a
 *   imagem escolhida em Configurações › Aparência + o `--reemuVeil` (véu
 *   neutro, escuro no tema escuro / claro no claro) pra manter o texto
 *   legível. SEM as cores do tema por cima.
 * - **Cores do tema** (todo o resto, e a tela inicial sem papel de parede):
 *   4 brilhos difusos, um por canto — `--reemuBg1..4`, ver `BgPalette` em
 *   `styles/themes.ts` — + o `--reemuGlowVeil`, leve e uniforme.
 *
 * Sem `filter`/blur. **Deriva lenta só no Windows**: o WebView2 anima
 * `transform` direto no compositor da GPU, sem layout nem repintura
 * (web.dev, "Stick to compositor-only properties": só `transform` e
 * `opacity` têm essa garantia). As cores NÃO mudam — os brilhos só
 * deslizam e respiram um pouco. No Linux fica ESTÁTICO: o WebKitGTK sem
 * compositing (NVIDIA proprietário, ver src-tauri/src/main.rs) repintaria a
 * tela inteira a cada quadro. A animação para com "reduzir movimento" do
 * sistema e fica pausada (`animation-play-state`, retoma de onde parou —
 * MDN) com a janela sem foco ou minimizada. Na tela de jogo o fundo nem
 * existe: `/play` fica fora do `AppShell`. Cada brilho é um elemento com UM
 * `radial-gradient` só: o WebKitGTK quebra com radial-gradient multicamada
 * num elemento `position: fixed`.
 */
const glow = (v: string, fallback: string) =>
  `radial-gradient(circle closest-side, var(${v}, ${fallback}) 0%, transparent 100%)`

/** WebView2 (Windows) compõe na GPU; WebKitGTK (Linux) pode estar sem
 *  compositing. */
const CAN_DRIFT = typeof navigator !== 'undefined' && /Windows/.test(navigator.userAgent)

// Deslocamento pequeno (poucos vmax) e ciclo longo: movimento de fundo,
// que não chama atenção nem compete com a interface.
const drift = (x: number, y: number, s: number) => ({
  from: { transform: 'translate3d(0, 0, 0) scale(1)' },
  to: { transform: `translate3d(${x}vmax, ${y}vmax, 0) scale(${s})` },
})

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
    width: '90vmax',
    height: '90vmax',
    opacity: 0.95,
  },
  // alto/esquerda — o brilho principal, puxado pro centro-alto como na
  // referência (o ciano atrás da prateleira "Jump back in").
  g1: { top: '-42vmax', left: '-18vmax', backgroundImage: glow('--reemuBg1', '#1E7F74') },
  // baixo/direita
  g2: { bottom: '-46vmax', right: '-30vmax', backgroundImage: glow('--reemuBg2', '#2E9E4F') },
  // baixo/esquerda — o mais discreto (azul fundo)
  g3: {
    bottom: '-50vmax',
    left: '-40vmax',
    opacity: 0.8,
    backgroundImage: glow('--reemuBg3', '#1B3F5C'),
  },
  // alto/direita — o "canto quente"
  g4: {
    top: '-50vmax',
    right: '-38vmax',
    opacity: 0.85,
    backgroundImage: glow('--reemuBg4', '#8A3A4A'),
  },
  // `alternate` = vai e volta sem salto; durações diferentes para os
  // brilhos nunca andarem em sincronia.
  drift: {
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
    animationDirection: 'alternate',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  d1: { animationName: drift(6, 4, 1.06), animationDuration: '38s' },
  d2: { animationName: drift(-5, -4, 1.08), animationDuration: '46s' },
  d3: { animationName: drift(4, -5, 1.05), animationDuration: '54s' },
  d4: { animationName: drift(-4, 5, 1.07), animationDuration: '42s' },
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
 * tela inicial; Splash e Onboarding ficam sempre com as cores do tema (o
 * boot é um momento de marca fixo, como o power-on de um console).
 */
/** `true` com a janela escondida/minimizada ou sem foco. */
function useWindowIdle(): boolean {
  const [idle, setIdle] = useState(false)
  useEffect(() => {
    if (!CAN_DRIFT) return
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
  const glowClass = (pos: string, d: string) =>
    CAN_DRIFT ? mergeClasses(s.glow, pos, s.drift, d, idle && s.paused) : mergeClasses(s.glow, pos)
  // `staleTime: Infinity`: raramente muda — `SettingsAppearance` invalida a
  // query na mão quando o usuário troca/remove o papel de parede.
  const wallpaper = useQuery({
    queryKey: ['wallpaper'],
    queryFn: wallpaperUrl,
    staleTime: Infinity,
    enabled: showWallpaper,
  })
  if (showWallpaper && wallpaper.data) {
    return (
      <div className={s.root} aria-hidden>
        <img src={wallpaper.data} alt="" className={s.wallpaper} />
        <div className={s.wallpaperVeil} />
      </div>
    )
  }
  return (
    <div className={s.root} aria-hidden>
      <div className={glowClass(s.g3, s.d3)} />
      <div className={glowClass(s.g4, s.d4)} />
      <div className={glowClass(s.g1, s.d1)} />
      <div className={glowClass(s.g2, s.d2)} />
      <div className={s.glowVeil} />
    </div>
  )
}
