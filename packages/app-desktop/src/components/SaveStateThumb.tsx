import { makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import { useEffect, useState } from 'react'
import { saveThumbnailUrl, syncStateThumbnailUrl } from '../lib/tauri'

const useStyles = makeStyles({
  box: {
    width: '96px',
    height: '72px',
    flexShrink: 0,
    borderRadius: tokens.borderRadiusSmall,
    objectFit: 'cover',
    backgroundColor: tokens.colorNeutralBackground3,
    imageRendering: 'pixelated',
  },
})

/** Miniatura de um save state (PNG servido por IPC → blob URL). */
export function SaveStateThumb({
  stateId,
  hasThumbnail,
  className,
  remote,
}: {
  stateId: string
  hasThumbnail: boolean
  /** Troca o tamanho padrão (96×72), como o perfil faz nos "momentos". */
  className?: string
  /** O state é de outra máquina (`stateId` é o `save_id` da nuvem). */
  remote?: boolean
}) {
  const s = useStyles()
  const [url, setUrl] = useState<string | null>(null)

  useEffect(() => {
    if (!hasThumbnail) return
    let dead = false
    let made: string | null = null
    void (remote ? syncStateThumbnailUrl : saveThumbnailUrl)(stateId).then((u) => {
      if (dead) {
        if (u) URL.revokeObjectURL(u)
      } else {
        made = u
        setUrl(u)
      }
    })
    return () => {
      dead = true
      if (made) URL.revokeObjectURL(made)
    }
  }, [stateId, hasThumbnail, remote])

  return <img className={mergeClasses(s.box, className)} src={url ?? undefined} alt="" />
}
