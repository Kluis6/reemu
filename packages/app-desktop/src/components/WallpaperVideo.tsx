import { useEffect, useRef, useState } from 'react'
import { CAN_ANIMATE_WALLPAPER, wallpaperPosterUrl, wallpaperVideoUrl, type WallpaperId } from '../lib/wallpapers'

/** "Reduzir movimento" do app (`data-reduce-motion` no `<html>`,
 *  `lib/uiPrefs.ts`) ou do sistema; acompanha a troca sem recarregar. */
function useReducedMotion(): boolean {
  const read = () =>
    document.documentElement.hasAttribute('data-reduce-motion') ||
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  const [reduced, setReduced] = useState(read)
  useEffect(() => {
    const update = () => setReduced(read())
    const mo = new MutationObserver(update)
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-reduce-motion'] })
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)')
    mq.addEventListener('change', update)
    return () => {
      mo.disconnect()
      mq.removeEventListener('change', update)
    }
  }, [])
  return reduced
}

/** O vídeo é decorativo e usa o pôster caso o sistema ou o usuário desative o movimento. */
export function WallpaperVideo({ id, idle = false, className }: {
  id: WallpaperId
  idle?: boolean
  className: string
}) {
  const ref = useRef<HTMLVideoElement>(null)
  const reduced = useReducedMotion()
  const [failed, setFailed] = useState(false)
  const plays = CAN_ANIMATE_WALLPAPER && !reduced && !failed
  useEffect(() => {
    const video = ref.current
    if (!video) return
    if (idle) {
      video.pause()
      return
    }
    let current = true
    void video.play().catch(() => {
      if (current) setFailed(true)
    })
    return () => { current = false }
  }, [idle, plays, id])

  const poster = wallpaperPosterUrl(id)
  if (!plays) return <img src={poster} alt="" className={className} draggable={false} />
  return (
    <video
      ref={ref}
      className={className}
      src={wallpaperVideoUrl(id)}
      poster={poster}
      muted
      loop
      playsInline
      disablePictureInPicture
      preload="none"
      aria-hidden
      onError={() => setFailed(true)}
    />
  )
}
