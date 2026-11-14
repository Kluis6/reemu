/**
 * 6 papéis de parede embutidos, em vídeo (loop de 12 s, VP9, 1920×1080 a 24
 * quadros): o do tema principal do ReEmu (Neon Circuit) e um para cada avatar
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
