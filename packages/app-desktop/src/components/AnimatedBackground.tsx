import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'

/**
 * Fundo das telas — 2 manchas de cor ESTÁTICAS (sem animação nenhuma). As
 * cores vêm de `--reemuBg1/2` (o `FluentProvider` emite a partir do tema).
 *
 * Era animado (drift lento via `translate`), mas mesmo sem `filter`/blur —
 * já otimizado pra regra do WebKitGTK sem compositing (ver
 * `frontend-perf-webkitgtk` nas memórias) — 2 áreas de 70vmax repintando em
 * loop infinito o tempo todo ainda pesava. Removido por pedido direto: fica
 * só a cor, parado, sem custo de repintura contínua.
 */
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
  },
  b1: {
    top: '-26vmax',
    left: '-22vmax',
    backgroundImage:
      'radial-gradient(circle, var(--reemuBg1, #3b82f6) 0%, var(--reemuBg1, #3b82f6) 32%, transparent 68%)',
  },
  b2: {
    bottom: '-30vmax',
    right: '-22vmax',
    backgroundImage:
      'radial-gradient(circle, var(--reemuBg2, #8b5cf6) 0%, var(--reemuBg2, #8b5cf6) 32%, transparent 68%)',
  },
  // `--reemuVeil` inverte por tema (escuro abafa pro preto, claro abafa pro
  // branco) — sem isto o fundo ficaria escuro mesmo num tema claro, já que o
  // véu cobre as manchas por cima de tudo.
  veil: {
    position: 'absolute',
    inset: 0,
    backgroundImage:
      'var(--reemuVeil, linear-gradient(180deg, rgba(9, 9, 12, 0.55) 0%, rgba(9, 9, 12, 0.82) 100%))',
  },
})

export function AnimatedBackground() {
  const s = useStyles()
  return (
    <div className={s.root} aria-hidden>
      <div className={mergeClasses(s.blob, s.b1)} />
      <div className={mergeClasses(s.blob, s.b2)} />
      <div className={s.veil} />
    </div>
  )
}
