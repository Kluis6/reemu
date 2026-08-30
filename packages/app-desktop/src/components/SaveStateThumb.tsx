import { makeStyles, tokens } from '@fluentui/react-components'
import { useEffect, useState } from 'react'
import { saveThumbnailUrl } from '../lib/tauri'

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
}: {
  stateId: string
  hasThumbnail: boolean
}) {
  const s = useStyles()
  const [url, setUrl] = useState<string | null>(null)

  useEffect(() => {
    if (!hasThumbnail) return
    let dead = false
    let made: string | null = null
    void saveThumbnailUrl(stateId).then((u) => {
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
  }, [stateId, hasThumbnail])

  return <img className={s.box} src={url ?? undefined} alt="" />
}
