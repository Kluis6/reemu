import type { CSSProperties } from 'react'
import { AVATAR_GRADIENTS, type PresetId } from '../lib/avatars'

/**
 * Avatar predefinido como SVG inline (preenche a caixa). Pro slot `image` do
 * `<Avatar>` do Fluent use `presetDataUri(id)`; este aqui é pra render direto
 * (ex.: a grade de escolha no formulário de perfil).
 */
function Shape({ id, fill }: { id: PresetId; fill: string }) {
  switch (id) {
    case '1':
      return (
        <>
          <rect width="64" height="64" fill={fill} />
          <path d="M0 64 L64 0 L64 64 Z" fill="#000" opacity="0.18" />
        </>
      )
    case '2':
      return (
        <>
          <rect width="64" height="64" fill={fill} />
          <circle cx="20" cy="44" r="30" fill="none" stroke="#fff" strokeWidth="5" opacity="0.35" />
          <circle cx="20" cy="44" r="16" fill="none" stroke="#fff" strokeWidth="5" opacity="0.5" />
        </>
      )
    case '3':
      return (
        <>
          <rect width="64" height="64" fill={fill} />
          <circle cx="24" cy="26" r="18" fill="#fff" opacity="0.28" />
          <circle cx="42" cy="40" r="18" fill="#000" opacity="0.16" />
        </>
      )
    case '4':
      return (
        <>
          <rect width="64" height="64" fill={fill} />
          <path d="M0 0 L64 22 L28 64 Z" fill="#fff" opacity="0.22" />
          <path d="M64 64 L14 52 L64 14 Z" fill="#000" opacity="0.16" />
        </>
      )
    case '5':
      return (
        <>
          <rect width="64" height="64" fill={fill} />
          <g opacity="0.3" stroke="#fff" strokeWidth="6">
            <path d="M32 32 L32 -8" />
            <path d="M32 32 L72 12" />
            <path d="M32 32 L72 52" />
            <path d="M32 32 L4 72" />
          </g>
          <circle cx="32" cy="32" r="9" fill="#fff" opacity="0.55" />
        </>
      )
  }
}

export function PresetAvatar({
  id,
  size = 64,
  style,
}: {
  id: PresetId
  size?: number
  style?: CSSProperties
}) {
  const [a, b] = AVATAR_GRADIENTS[id]
  const gid = `reemu-av-${id}`
  return (
    <svg
      viewBox="0 0 64 64"
      width={size}
      height={size}
      style={{ display: 'block', ...style }}
      role="img"
      aria-label={`Avatar ${id}`}
    >
      <defs>
        <linearGradient id={gid} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor={a} />
          <stop offset="1" stopColor={b} />
        </linearGradient>
      </defs>
      <Shape id={id} fill={`url(#${gid})`} />
    </svg>
  )
}
