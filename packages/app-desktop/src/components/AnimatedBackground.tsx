import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useThemeStore } from '../stores/useThemeStore'

/**
 * Fundo animado das telas — 2 manchas de cor que derivam devagar. As cores
 * vêm de `--reemuBg1/2` (o `FluentProvider` emite a partir do tema).
 *
 * SEM `filter: blur()`: num software renderer (WebKitGTK sem compositing
 * acelerado, caso do NVIDIA — ver main.rs) borrar uma área de metade da
 * tela é uma convolução cara, refeita a cada frame da animação, o tempo
 * todo que o app fica aberto — era o item mais pesado do frontend. Um
 * `radial-gradient` de UMA camada (a regra do topo do arquivo é nada de
 * gradiente multicamada nesse WebKitGTK) já dá a borda suave sozinho, sem
 * blur nenhum — muito mais barato de rasterizar. Só `translate` no
 * keyframe (sem `scale`, que mudaria a caixa e forçaria recálculo).
 */
const drift1 = {
  '0%, 100%': { transform: 'translate3d(-4%, -3%, 0)' },
  '50%': { transform: 'translate3d(4%, 3%, 0)' },
}
const drift2 = {
  '0%, 100%': { transform: 'translate3d(3%, 4%, 0)' },
  '50%': { transform: 'translate3d(-3%, -3%, 0)' },
}

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
  blob: {
    position: 'absolute',
    width: '70vmax',
    height: '70vmax',
    opacity: 0.5,
    willChange: 'transform',
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  b1: {
    top: '-26vmax',
    left: '-22vmax',
    backgroundImage:
      'radial-gradient(circle, var(--reemuBg1, #3b82f6) 0%, var(--reemuBg1, #3b82f6) 32%, transparent 68%)',
    animationName: drift1,
    animationDuration: '40s',
  },
  b2: {
    bottom: '-30vmax',
    right: '-22vmax',
    backgroundImage:
      'radial-gradient(circle, var(--reemuBg2, #8b5cf6) 0%, var(--reemuBg2, #8b5cf6) 32%, transparent 68%)',
    animationName: drift2,
    animationDuration: '52s',
  },
  veil: {
    position: 'absolute',
    inset: 0,
    backgroundImage:
      'linear-gradient(180deg, rgba(9, 9, 12, 0.55) 0%, rgba(9, 9, 12, 0.82) 100%)',
  },
  still: { animationName: 'none' },
})

/** `forceStill` ignora a preferência (ex.: pra um preview). */
export function AnimatedBackground({ forceStill = false }: { forceStill?: boolean }) {
  const s = useStyles()
  const animated = useThemeStore((st) => st.bgAnimated)
  const still = forceStill || !animated ? s.still : undefined
  return (
    <div className={s.root} aria-hidden>
      <div className={mergeClasses(s.blob, s.b1, still)} />
      <div className={mergeClasses(s.blob, s.b2, still)} />
      <div className={s.veil} />
    </div>
  )
}
