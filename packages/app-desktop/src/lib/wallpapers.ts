/**
 * 6 papéis de parede embutidos em VP9 a 1600×900 e 30 fps. O loop do
 * Fliperama dura 30 s e os demais duram 20 s. Inclui o fundo de partículas
 * do ReEmu e um fundo para cada avatar
 * (`avatars.ts`), nas cores dele e sem o personagem. Gerados por
 * `scripts/wallpaper-video/render.py` e servidos pelo app em
 * `/wallpapers/<id>.webm`, com o 1º quadro em `/wallpapers/<id>.jpg` (pôster,
 * miniatura e o fundo parado onde o vídeo não toca — ver `AnimatedBackground`).
 *
 * O backend guarda só o id escolhido (`set_wallpaper_preset`).
 */

/** Ordem da tela: o do ReEmu primeiro, depois os dos avatares 1 a 5. */
export const WALLPAPER_IDS = ['6', '1', '2', '3', '4', '5'] as const
export type WallpaperId = (typeof WALLPAPER_IDS)[number]

export function isWallpaperId(v: unknown): v is WallpaperId {
  return typeof v === 'string' && (WALLPAPER_IDS as readonly string[]).includes(v)
}

export const wallpaperVideoUrl = (id: WallpaperId) => `/wallpapers/${id}.webm`
export const wallpaperPosterUrl = (id: WallpaperId) => `/wallpapers/${id}.jpg`

// O WebKitGTK pode rodar com a composição desabilitada no Linux.
export const CAN_ANIMATE_WALLPAPER = typeof navigator !== 'undefined' && /Windows/.test(navigator.userAgent)
