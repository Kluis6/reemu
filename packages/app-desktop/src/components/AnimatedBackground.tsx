import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
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
 * Tudo ESTÁTICO e sem `filter`/blur: o WebKitGTK sem compositing repinta
 * caro (ver `frontend-perf-webkitgtk` nas memórias) — o drift animado que
 * existia antes foi removido por isso. Cada brilho é um elemento com UM
 * `radial-gradient` só: o WebKitGTK quebra com radial-gradient multicamada
 * num elemento `position: fixed`.
 */
const glow = (v: string, fallback: string) =>
  `radial-gradient(circle closest-side, var(${v}, ${fallback}) 0%, transparent 100%)`

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
export function AnimatedBackground({ showWallpaper = false }: { showWallpaper?: boolean }) {
  const s = useStyles()
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
      <div className={mergeClasses(s.glow, s.g3)} />
      <div className={mergeClasses(s.glow, s.g4)} />
      <div className={mergeClasses(s.glow, s.g1)} />
      <div className={mergeClasses(s.glow, s.g2)} />
      <div className={s.glowVeil} />
    </div>
  )
}
