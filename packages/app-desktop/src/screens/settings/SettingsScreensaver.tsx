import { Button, makeStyles, tokens } from '@fluentui/react-components'
import { useTranslation } from 'react-i18next'
import { SelectBlock } from '../../components/SettingBlock'
import { isWallpaperId, WALLPAPER_IDS, wallpaperPosterUrl } from '../../lib/wallpapers'
import { SCREENSAVER_DELAYS, useScreensaverStore } from '../../stores/useScreensaverStore'
import { useModalButtonStyles } from '../../styles/buttons'

const useStyles = makeStyles({
  root: {
    display: 'grid',
    gridTemplateColumns: 'minmax(0, 1fr) clamp(280px, 34%, 360px)',
    alignItems: 'start',
    columnGap: tokens.spacingHorizontalXXL,
    rowGap: tokens.spacingVerticalL,
    '@media (max-width: 760px)': { gridTemplateColumns: 'minmax(0, 1fr)' },
  },
  preview: {
    width: '100%',
    aspectRatio: '16 / 9',
    borderRadius: tokens.borderRadiusLarge,
    objectFit: 'cover',
  },
  controls: {
    display: 'grid',
    gridAutoRows: '1fr',
    gap: tokens.spacingVerticalM,
    minWidth: 0,
  },
})

export function SettingsScreensaver() {
  const s = useStyles()
  const button = useModalButtonStyles()
  const { t } = useTranslation()
  const minutes = useScreensaverStore((state) => state.minutes)
  const wallpaper = useScreensaverStore((state) => state.wallpaper)
  const setMinutes = useScreensaverStore((state) => state.setMinutes)
  const setWallpaper = useScreensaverStore((state) => state.setWallpaper)
  const show = useScreensaverStore((state) => state.show)

  return (
    <div className={s.root}>
      <img className={s.preview} src={wallpaperPosterUrl(wallpaper)} alt="" draggable={false} />
      <div className={s.controls}>
        <SelectBlock
          label={t('appearance.screensaver.delay')}
          value={String(minutes)}
          options={SCREENSAVER_DELAYS.map((minutes) => ({
            value: String(minutes),
            label: minutes ? t('appearance.screensaver.minutes', { count: minutes }) : t('appearance.screensaver.off'),
          }))}
          onChange={(value) => {
            const delay = SCREENSAVER_DELAYS.find((minutes) => String(minutes) === value)
            if (delay !== undefined) setMinutes(delay)
          }}
        />
        <SelectBlock
          label={t('appearance.screensaver.wallpaper')}
          value={wallpaper}
          options={WALLPAPER_IDS.map((id) => ({ value: id, label: t(`appearance.wallpaper.presets.${id}`) }))}
          onChange={(value) => {
            if (isWallpaperId(value)) setWallpaper(value)
          }}
        />
        <Button className={button.block} appearance="secondary" onClick={show}>
          {t('appearance.screensaver.preview')}
        </Button>
      </div>
    </div>
  )
}
