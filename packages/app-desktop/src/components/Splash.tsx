import { makeStyles, tokens } from '@fluentui/react-components'
import { useImageExists } from '../hooks/useImageExists'
import { AnimatedBackground } from './AnimatedBackground'

/** Caminho da logo — coloque `reemu-logo.png` em `packages/app-desktop/public/`. */
const LOGO_SRC = '/reemu-logo.png'

// Entrada: fade + leve expansão vertical (toque de CRT), sem animar filter.
const powerOn = {
  from: { opacity: 0, transform: 'scaleY(0.55) scaleX(1.02)' },
  '60%': { opacity: 1 },
  to: { opacity: 1, transform: 'scaleY(1) scaleX(1)' },
}
// glow que respira via OPACITY de um brilho separado (composited, barato).
// Faixa mais baixa que um "0.4-0.75" ingênuo: as cores do gradiente agora são
// sólidas (tokens do tema, não dá pra variar alpha por stop num var()), então
// quem faz o efeito "glow suave" (em vez de mancha sólida) é só a opacity do
// elemento inteiro.
const breathe = {
  '0%, 100%': { opacity: 0.16 },
  '50%': { opacity: 0.3 },
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
  // brilho estático atrás da logo; só a opacity anima. Cor do TEMA, não fixa
  // verde/azul — o boot é "momento de marca fixo" só quanto ao papel de
  // parede do usuário (ver memória frontend-xbox-design-reference); a cor
  // em si sempre foi pra vir do tema, e o wordmark (`.wmGreen` abaixo) já
  // faz isso. `--reemuBrandSolid`/`--reemuBg1` (tons da rampa do tema ativo,
  // ver `styles/themes.ts`) com fallback pro azul/verde antigo.
  glow: {
    position: 'absolute',
    inset: '-8%',
    borderRadius: '50%',
    backgroundImage:
      'radial-gradient(closest-side, var(--reemuBrandSolid, #40dc78), var(--reemuBg1, #3c96ff) 55%, transparent 78%)',
    animationName: breathe,
    animationDuration: '4s',
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none', opacity: 0.22 },
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
  // Scanlines de CRT — ESTÁTICO: só um `repeating-linear-gradient` (sem
  // filter, sem animação), pinta uma vez e fica. Por cima de tudo (a tela
  // toda "por trás do vidro"), não só do fundo.
  scanlines: {
    position: 'absolute',
    inset: 0,
    zIndex: 2,
    pointerEvents: 'none',
    backgroundImage:
      'repeating-linear-gradient(to bottom, rgba(0, 0, 0, 0.18) 0px, rgba(0, 0, 0, 0.18) 1px, transparent 1px, transparent 3px)',
  },
})

/** Tela de abertura — liga a logo com uma expansão curta + brilho que respira.
 *  `leaving` dispara o fade-out. */
export function Splash({ leaving = false }: { leaving?: boolean }) {
  const s = useStyles()
  // Pré-carrega fora do DOM — nunca monta um <img> quebrado (o ícone de
  // imagem ausente do navegador piscava na tela até o onError reagir).
  const imgOk = useImageExists(LOGO_SRC)
  return (
    <div className={leaving ? `${s.root} ${s.leaving}` : s.root}>
      <AnimatedBackground showWallpaper={false} />
      <div className={s.stage}>
        <div className={s.glow} aria-hidden />
        {imgOk ? (
          <img className={s.logo} src={LOGO_SRC} alt="ReEmu" />
        ) : (
          <div className={`${s.logo} ${s.wordmark}`}>
            Re<span className={s.wmGreen}>Emu</span>
          </div>
        )}
      </div>
      <div className={s.tag}>carregando…</div>
      <div className={s.scanlines} aria-hidden />
    </div>
  )
}
