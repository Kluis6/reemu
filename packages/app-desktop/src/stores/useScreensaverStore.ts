import { create } from 'zustand'
import { isWallpaperId, type WallpaperId } from '../lib/wallpapers'

export const SCREENSAVER_DELAYS = [0, 1, 3, 5, 10, 15] as const
export const SCREENSAVER_CONTROLLER_ACTIVITY = 'reemu-screensaver-controller-activity'
type Delay = typeof SCREENSAVER_DELAYS[number]
const KEY = 'reemu.screensaver'

interface ScreensaverPreferences {
  minutes: Delay
  wallpaper: WallpaperId
}

function load(): ScreensaverPreferences {
  const defaults: ScreensaverPreferences = { minutes: 0, wallpaper: '4' }
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? 'null')
    return {
      minutes: SCREENSAVER_DELAYS.find((value) => value === saved?.minutes) ?? defaults.minutes,
      wallpaper: isWallpaperId(saved?.wallpaper) ? saved.wallpaper : defaults.wallpaper,
    }
  } catch {
    return defaults
  }
}

interface ScreensaverState extends ScreensaverPreferences {
  active: boolean
  setMinutes: (minutes: Delay) => void
  setWallpaper: (wallpaper: WallpaperId) => void
  show: () => void
  dismiss: () => void
}

export const useScreensaverStore = create<ScreensaverState>((set, get) => {
  const save = (change: Partial<ScreensaverPreferences>) => {
    const { minutes, wallpaper } = get()
    const preferences = { minutes, wallpaper, ...change }
    try {
      localStorage.setItem(KEY, JSON.stringify(preferences))
    } catch {
      // Preferências ainda valem nesta sessão.
    }
    set(preferences)
  }

  return {
    ...load(),
    active: false,
    setMinutes: (minutes) => save({ minutes }),
    setWallpaper: (wallpaper) => save({ wallpaper }),
    show: () => set({ active: true }),
    dismiss: () => set({ active: false }),
  }
})
