/**
 * 5 avatares embutidos — personagens originais com tema de jogo (robô de 8
 * bits, controle, slime, joystick de fliperama e nave), desenhados pra
 * funcionar recortados em círculo e legíveis até 32px (rail). O perfil guarda
 * `"preset:1".."preset:5"` ou `"file"` (imagem do usuário, servida por
 * `profileAvatarUrl()`). Os ids não mudam — quem já escolheu um preset só vê
 * o desenho novo.
 *
 * Fonte única: o SVG completo sai de `presetSvg()`; o componente
 * `PresetAvatar` e o `<Avatar>` do Fluent (via `presetDataUri`) usam o mesmo.
 */

export const PRESET_IDS = ['1', '2', '3', '4', '5'] as const
export type PresetId = (typeof PRESET_IDS)[number]

/** `"preset:3"` → `"3"`; qualquer outra coisa → `null`. */
export function presetOf(avatar: string): PresetId | null {
  const n = avatar.startsWith('preset:') ? avatar.slice(7) : ''
  return (PRESET_IDS as readonly string[]).includes(n) ? (n as PresetId) : null
}

/** Nome de cada preset — rótulo acessível e dica na grade de escolha. */
export const AVATAR_NAMES: Record<PresetId, string> = {
  '1': 'Robô',
  '2': 'Controle',
  '3': 'Slime',
  '4': 'Fliperama',
  '5': 'Nave',
}

/** Gradiente de fundo (canto superior esquerdo → inferior direito). */
export const AVATAR_GRADIENTS: Record<PresetId, [string, string]> = {
  '1': ['#1d4ed8', '#22d3ee'],
  '2': ['#6d28d9', '#ec4899'],
  '3': ['#047857', '#84cc16'],
  '4': ['#ea580c', '#be123c'],
  '5': ['#312e81', '#9333ea'],
}

/** Desenho de cada preset (viewBox 64×64, sem o fundo). Figura dentro do
 *  círculo central (~raio 26) pra não ser cortada no recorte redondo. */
const ART: Record<PresetId, string> = {
  // Robô de 8 bits: antena, cabeça com visor e olhos, corpo com luz no peito.
  '1':
    '<path d="M32 9v8" stroke="#e0f2fe" stroke-width="3" stroke-linecap="round"/>' +
    '<circle cx="32" cy="8" r="3.5" fill="#fde047"/>' +
    '<rect x="11" y="25" width="5" height="10" rx="2.5" fill="#bae6fd"/>' +
    '<rect x="48" y="25" width="5" height="10" rx="2.5" fill="#bae6fd"/>' +
    '<rect x="15" y="16" width="34" height="27" rx="8" fill="#f8fafc"/>' +
    '<rect x="19" y="21" width="26" height="14" rx="6" fill="#0f172a"/>' +
    '<rect x="23" y="24.5" width="6" height="7" rx="2" fill="#22d3ee"/>' +
    '<rect x="35" y="24.5" width="6" height="7" rx="2" fill="#22d3ee"/>' +
    '<rect x="24" y="36.5" width="16" height="2.5" rx="1.25" fill="#94a3b8"/>' +
    '<rect x="26" y="43" width="12" height="4" fill="#cbd5e1"/>' +
    '<rect x="13" y="47" width="38" height="20" rx="9" fill="#e2e8f0"/>' +
    '<circle cx="32" cy="55" r="3.5" fill="#f43f5e"/>',
  // Controle: corpo claro, direcional escuro e os 4 botões coloridos.
  '2':
    '<g transform="translate(32 36) scale(0.86) translate(-32 -36)">' +
    '<path d="M18 22h28c6 0 10.5 4 11.5 10l2 12c1 6.5-6.5 10-11 5.5L43 44H21l-5.5 5.5C11 54 3.5 50.5 4.5 44l2-12C7.5 26 12 22 18 22z" fill="#fdf4ff"/>' +
    '<rect x="15" y="31.5" width="12" height="4.5" rx="1.5" fill="#3b0764"/>' +
    '<rect x="18.75" y="27.75" width="4.5" height="12" rx="1.5" fill="#3b0764"/>' +
    '<circle cx="45" cy="29" r="2.8" fill="#22c55e"/>' +
    '<circle cx="50" cy="34" r="2.8" fill="#ef4444"/>' +
    '<circle cx="40" cy="34" r="2.8" fill="#3b82f6"/>' +
    '<circle cx="45" cy="39" r="2.8" fill="#eab308"/>' +
    '<rect x="29" y="31" width="6" height="2.5" rx="1.25" fill="#d8b4fe"/>' +
    '</g>',
  // Slime: gota sorridente com brilho e bochechas.
  '3':
    '<path d="M10 50c0-17 10-33 22-33s22 16 22 33c0 5-4 7-8 6-3 2-7 2-10 0-3 2-7 2-10 0-4 1-8 0-8-2-4 1-8-1-8-4z" fill="#d9f99d"/>' +
    '<path d="M10 50c0-17 10-33 22-33s22 16 22 33" fill="none" stroke="#ecfccb" stroke-width="2" opacity="0.7"/>' +
    '<ellipse cx="22" cy="30" rx="3.5" ry="6" fill="#fff" opacity="0.85" transform="rotate(25 22 30)"/>' +
    '<ellipse cx="25.5" cy="40" rx="3.2" ry="4.2" fill="#14532d"/>' +
    '<ellipse cx="38.5" cy="40" rx="3.2" ry="4.2" fill="#14532d"/>' +
    '<circle cx="26.5" cy="38.5" r="1.1" fill="#fff"/>' +
    '<circle cx="39.5" cy="38.5" r="1.1" fill="#fff"/>' +
    '<circle cx="19.5" cy="46" r="2.6" fill="#fb7185" opacity="0.6"/>' +
    '<circle cx="44.5" cy="46" r="2.6" fill="#fb7185" opacity="0.6"/>' +
    '<path d="M28.5 46.5q3.5 3.5 7 0" fill="none" stroke="#14532d" stroke-width="2" stroke-linecap="round"/>',
  // Joystick de fliperama: bola no topo, haste e base com dois botões.
  '4':
    '<rect x="29.5" y="24" width="5" height="22" rx="2" fill="#e5e7eb"/>' +
    '<circle cx="32" cy="20" r="11" fill="#fde047"/>' +
    '<circle cx="28" cy="16" r="3.5" fill="#fff" opacity="0.8"/>' +
    '<path d="M11 50c0-4 3-7 7-7h28c4 0 7 3 7 7v6H11z" fill="#1f2937"/>' +
    '<rect x="11" y="54" width="42" height="10" rx="3" fill="#111827"/>' +
    '<ellipse cx="32" cy="46.5" rx="7" ry="2.5" fill="#374151"/>' +
    '<circle cx="19" cy="50" r="3" fill="#22d3ee"/>' +
    '<circle cx="45" cy="50" r="3" fill="#a3e635"/>',
  // Nave: foguete com janela, aletas e chama, num céu estrelado.
  '5':
    '<g fill="#fff">' +
    '<circle cx="14" cy="16" r="1.3"/><circle cx="50" cy="13" r="1"/>' +
    '<circle cx="51" cy="31" r="1.5"/><circle cx="12" cy="35" r="1"/>' +
    '<circle cx="18" cy="52" r="1.2" opacity="0.7"/><circle cx="47" cy="50" r="1" opacity="0.7"/>' +
    '</g>' +
    '<path d="M32 8c6 5 9 13 9 22v14H23V30c0-9 3-17 9-22z" fill="#f8fafc"/>' +
    '<path d="M23 34l-8 10v5l8-3z" fill="#f43f5e"/>' +
    '<path d="M41 34l8 10v5l-8-3z" fill="#f43f5e"/>' +
    '<circle cx="32" cy="26" r="5" fill="#38bdf8" stroke="#1e1b4b" stroke-width="2"/>' +
    '<circle cx="30.5" cy="24.5" r="1.5" fill="#e0f2fe"/>' +
    '<rect x="26" y="44" width="12" height="3" rx="1" fill="#94a3b8"/>' +
    '<path d="M27 47h10l-5 11z" fill="#fbbf24"/>' +
    '<path d="M29.5 47h5l-2.5 6z" fill="#f97316"/>',
}

/** SVG completo do preset (fundo em gradiente + desenho). */
export function presetSvg(id: PresetId): string {
  const [a, b] = AVATAR_GRADIENTS[id]
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">` +
    `<stop offset="0" stop-color="${a}"/><stop offset="1" stop-color="${b}"/>` +
    `</linearGradient></defs>` +
    `<rect width="64" height="64" fill="url(#g)"/>${ART[id]}</svg>`
  )
}

/** SVG do avatar como `data:` URI — pro slot `image` do `<Avatar>` do Fluent. */
export function presetDataUri(id: PresetId): string {
  return `data:image/svg+xml,${encodeURIComponent(presetSvg(id))}`
}
