import type { CSSProperties } from 'react'
import { AVATAR_NAMES, presetDataUri, type PresetId } from '../lib/avatars'

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
  return (
    <img
      src={presetDataUri(id)}
      width={size}
      height={size}
      alt={`Avatar ${AVATAR_NAMES[id]}`}
      draggable={false}
      style={{ display: 'block', ...style }}
    />
  )
}
