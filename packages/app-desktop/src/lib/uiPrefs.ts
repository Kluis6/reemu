// Preferências de retorno da interface, guardadas neste aparelho.
const SOUNDS_KEY = 'reemu.uiSounds'
const MOTION_KEY = 'reemu.reduceMotion'

function read(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function write(key: string, on: boolean) {
  try {
    localStorage.setItem(key, on ? '1' : '0')
  } catch {
    // sem armazenamento, a escolha vale até fechar o app
  }
}

/** Sons de navegação: ligados por padrão, como no console. */
export const getUiSounds = () => read(SOUNDS_KEY) !== '0'
export const setUiSounds = (on: boolean) => write(SOUNDS_KEY, on)

export const getReduceMotion = () => read(MOTION_KEY) === '1'

/** Marca o `<html>`; o `index.css` tira animações e transições. Quem pede isso no
 *  sistema já é atendido pelas regras `prefers-reduced-motion` de cada componente. */
export function applyReduceMotion(on = getReduceMotion()) {
  document.documentElement.toggleAttribute('data-reduce-motion', on)
}

export function setReduceMotion(on: boolean) {
  write(MOTION_KEY, on)
  applyReduceMotion(on)
}

export type UiSound = 'move' | 'confirm' | 'back'

// Tons curtos e baixos, gerados na hora: sem arquivo de áudio para distribuir.
const TONES: Record<UiSound, { freq: number; ms: number }> = {
  move: { freq: 660, ms: 25 },
  confirm: { freq: 880, ms: 45 },
  back: { freq: 440, ms: 45 },
}

let audio: AudioContext | null = null

export function playUiSound(sound: UiSound) {
  if (!getUiSounds() || typeof AudioContext === 'undefined') return
  audio ??= new AudioContext()
  if (audio.state === 'suspended') void audio.resume()
  const { freq, ms } = TONES[sound]
  const now = audio.currentTime
  const end = now + ms / 1000
  const osc = audio.createOscillator()
  const gain = audio.createGain()
  osc.frequency.value = freq
  gain.gain.setValueAtTime(0.03, now)
  gain.gain.exponentialRampToValueAtTime(0.001, end)
  osc.connect(gain).connect(audio.destination)
  osc.start(now)
  osc.stop(end)
}
