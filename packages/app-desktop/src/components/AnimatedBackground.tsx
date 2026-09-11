import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { wallpaperUrl } from '../lib/tauri'

/**
 * Fundo das telas — papel de parede opcional (embaixo de tudo) + 2 manchas
 * de cor ESTÁTICAS (sem animação nenhuma) por cima. As cores vêm de
 * `--reemuBg1/2` (o `FluentProvider` emite a partir do tema).
 *
 * Era animado (drift lento via `translate`), mas mesmo sem `filter`/blur —
 * já otimizado pra regra do WebKitGTK sem compositing (ver
 * `frontend-perf-webkitgtk` nas memórias) — 2 áreas de 70vmax repintando em
 * loop infinito o tempo todo ainda pesava. Removido por pedido direto: fica
 * só a cor, parado, sem custo de repintura contínua.
 *
 * O papel de parede (`<img>`, Configurações › Aparência) é ESTÁTICO também —
 * o navegador decodifica uma vez e reusa o bitmap nas pinturas seguintes,
 * então não reintroduz o custo do blur animado. Sem `filter` nele.
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
  wallpaper: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    objectFit: 'cover',
    objectPosition: 'center',
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
  // véu cobre as manchas por cima de tudo. É radial (centro mais
  // transparente, bordas/cantos mais fortes) — deixa o papel de parede
  // aparecer mais no meio da tela sem lavar as manchas de cor nos cantos.
  veil: {
    position: 'absolute',
    inset: 0,
    backgroundImage:
      'var(--reemuVeil, radial-gradient(ellipse at center, rgba(9, 9, 12, 0) 0%, rgba(9, 9, 12, 0.82) 100%))',
  },
})

/** `showWallpaper = false` no Splash: o boot é um momento de marca fixo,
 *  igual ao power-on de um console de verdade — não deve variar com uma
 *  foto escolhida pelo usuário (só as cores do tema, que já eram
 *  compartilhadas ali antes do papel de parede existir). */
export function AnimatedBackground({ showWallpaper = true }: { showWallpaper?: boolean }) {
  const s = useStyles()
  // `staleTime: Infinity`: raramente muda: `SettingsAppearance` invalida a
  // query na mão quando o usuário troca/remove o papel de parede.
  const wallpaper = useQuery({
    queryKey: ['wallpaper'],
    queryFn: wallpaperUrl,
    staleTime: Infinity,
    enabled: showWallpaper,
  })
  return (
    <div className={s.root} aria-hidden>
      {showWallpaper && wallpaper.data && (
        <img src={wallpaper.data} alt="" className={s.wallpaper} />
      )}
      <div className={mergeClasses(s.blob, s.b1)} />
      <div className={mergeClasses(s.blob, s.b2)} />
      <div className={s.veil} />
    </div>
  )
}
