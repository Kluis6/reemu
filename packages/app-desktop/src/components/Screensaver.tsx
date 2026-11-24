import { makeStyles } from '@fluentui/react-components'
import { useLayoutEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { useScreensaverStore } from '../stores/useScreensaverStore'
import { WallpaperVideo } from './WallpaperVideo'

const useStyles = makeStyles({
  screen: {
    position: 'fixed',
    inset: 0,
    width: '100%',
    height: '100%',
    maxWidth: 'none',
    maxHeight: 'none',
    boxSizing: 'border-box',
    margin: 0,
    border: 'none',
    padding: 0,
    overflowX: 'hidden',
    overflowY: 'hidden',
    backgroundColor: '#000',
    cursor: 'none',
  },
  media: {
    display: 'block',
    width: '100%',
    height: '100%',
    objectFit: 'cover',
    pointerEvents: 'none',
  },
})

/** O diálogo ocupa a janela toda e deixa os controles atrás dele inertes. */
export function Screensaver() {
  const s = useStyles()
  const { t } = useTranslation()
  const dialog = useRef<HTMLDialogElement>(null)
  const wallpaper = useScreensaverStore((state) => state.wallpaper)
  const dismiss = useScreensaverStore((state) => state.dismiss)

  useLayoutEffect(() => {
    const surface = dialog.current
    surface?.showModal()
    return () => surface?.close()
  }, [])

  return (
    <dialog
      ref={dialog}
      className={s.screen}
      aria-label={t('appearance.screensaver.title')}
      aria-description={t('appearance.screensaver.dismiss')}
      onPointerMove={(event) => {
        if (event.pointerType === 'mouse' && !event.buttons && (event.movementX || event.movementY)) dismiss()
      }}
      onClick={dismiss}
      onContextMenu={(event) => {
        event.preventDefault()
        dismiss()
      }}
      onCancel={(event) => {
        event.preventDefault()
        dismiss()
      }}
    >
      <WallpaperVideo key={wallpaper} id={wallpaper} className={s.media} />
    </dialog>
  )
}
