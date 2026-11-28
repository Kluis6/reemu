import { makeStyles, tokens } from '@fluentui/react-components'

export const useSettingsListStyles = makeStyles({
  tile: {
    width: '48px',
    height: '48px',
    flexShrink: 0,
    display: 'grid',
    placeItems: 'center',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuSettingsTileBg)',
    color: tokens.colorNeutralForeground1,
    fontSize: '28px',
  },
  titleLine: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    columnGap: tokens.spacingHorizontalS,
    rowGap: tokens.spacingVerticalXS,
  },
  titleName: {
    display: 'inline-flex',
    alignItems: 'center',
    columnGap: tokens.spacingHorizontalXS,
    minWidth: 0,
  },
  installedCheck: {
    color: 'var(--reemuBrandText)',
    fontSize: '20px',
    flexShrink: 0,
  },
})
