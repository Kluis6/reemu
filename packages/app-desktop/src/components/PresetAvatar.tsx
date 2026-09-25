import type { CSSProperties } from 'react'
import { presetDataUri, type PresetId } from '../lib/avatars'
import { useTranslation } from 'react-i18next'

/**
 * Avatar predefinido (preenche a caixa). Mesmo SVG do `<Avatar>` do Fluent
 * (`presetDataUri`) — uma fonte só pros dois (`lib/avatars.ts`).
 */
export function PresetAvatar({
  id,
  size = 64,
  style,
}: {
  id: PresetId
  size?: number
  style?: CSSProperties
}) {
  const { t } = useTranslation()
  return (
    <img
      src={presetDataUri(id)}
      width={size}
      height={size}
      alt={t('profileForm.avatarLabel', { name: t(`profileForm.presets.${id}`) })}
      draggable={false}
      style={{ display: 'block', ...style }}
    />
  )
}
