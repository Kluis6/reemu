/**
 * 5 avatares embutidos — SVG geométrico com gradiente, estilo "modo console".
 * Placeholders substituíveis. O perfil guarda `"preset:1".."preset:5"` ou
 * `"file"` (imagem do usuário, servida por `profileAvatarUrl()`).
 *
 * Dados + helpers puros aqui; o componente inline fica em
 * `components/PresetAvatar.tsx`.
 */

export const PRESET_IDS = ['1', '2', '3', '4', '5'] as const
export type PresetId = (typeof PRESET_IDS)[number]

/** `"preset:3"` → `"3"`; qualquer outra coisa → `null`. */
export function presetOf(avatar: string): PresetId | null {
  const n = avatar.startsWith('preset:') ? avatar.slice(7) : ''
  return (PRESET_IDS as readonly string[]).includes(n) ? (n as PresetId) : null
}

export const AVATAR_GRADIENTS: Record<PresetId, [string, string]> = {
  '1': ['#3b82f6', '#22d3ee'],
  '2': ['#8b5cf6', '#ec4899'],
  '3': ['#10b981', '#84cc16'],
  '4': ['#f59e0b', '#ef4444'],
  '5': ['#6366f1', '#a855f7'],
}

/** Formas SVG (sem o `<rect>` de fundo) — usadas inline e no data URI. */
export const AVATAR_SHAPES: Record<PresetId, string> = {
  '1': '<path d="M0 64 L64 0 L64 64 Z" fill="#000" opacity="0.18"/>',
  '2': '<circle cx="20" cy="44" r="30" fill="none" stroke="#fff" stroke-width="5" opacity="0.35"/><circle cx="20" cy="44" r="16" fill="none" stroke="#fff" stroke-width="5" opacity="0.5"/>',
  '3': '<circle cx="24" cy="26" r="18" fill="#fff" opacity="0.28"/><circle cx="42" cy="40" r="18" fill="#000" opacity="0.16"/>',
  '4': '<path d="M0 0 L64 22 L28 64 Z" fill="#fff" opacity="0.22"/><path d="M64 64 L14 52 L64 14 Z" fill="#000" opacity="0.16"/>',
  '5': '<g opacity="0.3" stroke="#fff" stroke-width="6"><path d="M32 32 L32 -8"/><path d="M32 32 L72 12"/><path d="M32 32 L72 52"/><path d="M32 32 L4 72"/></g><circle cx="32" cy="32" r="9" fill="#fff" opacity="0.55"/>',
}

/** SVG do avatar como `data:` URI — pro slot `image` do `<Avatar>` do Fluent. */
export function presetDataUri(id: PresetId): string {
  const [a, b] = AVATAR_GRADIENTS[id]
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
    `<stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/>` +
    `</linearGradient></defs>` +
    `<rect width="64" height="64" fill="url(#g)"/>${AVATAR_SHAPES[id]}</svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}
