import { Skeleton, SkeletonItem, makeStyles } from '@fluentui/react-components'
import { useBrowseStyles, shell } from '../styles/xbox'

const useStyles = makeStyles({
  item: {
    aspectRatio: '1 / 1',
    width: '100%',
    borderRadius: shell.radius,
  },
})

/**
 * Placeholder da grade de jogos (mesma grid/tamanho de card do `GameCard`
 * real, via `useBrowseStyles().grid`) — substitui o spinner genérico
 * enquanto a lista de uma plataforma/biblioteca carrega, sem o "salto" de
 * layout quando os cards de verdade chegam.
 */
export function CardGridSkeleton({ count = 12 }: { count?: number }) {
  const g = useBrowseStyles()
  const s = useStyles()
  return (
    <Skeleton className={g.grid} aria-label="Carregando jogos…">
      {Array.from({ length: count }).map((_, i) => (
        <SkeletonItem key={i} className={s.item} />
      ))}
    </Skeleton>
  )
}
