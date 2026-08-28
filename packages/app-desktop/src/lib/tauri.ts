// Wrappers finos sobre os comandos/eventos do backend Rust.
// Tudo tolera rodar fora do Tauri (ex: `vite` puro no navegador) — nesse
// caso os comandos rejeitam e os listeners viram no-op.

import type { InputFocus } from '../stores/useFocusStore'

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) throw new Error(`fora do Tauri: ${cmd}`)
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(cmd, args)
}

export async function onFocusChanged(cb: (focus: InputFocus) => void): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<{ focus: InputFocus }>('focus-changed', (e) => cb(e.payload.focus))
}

export const toggleFocus = () => invoke<InputFocus>('toggle_focus')
export const inputKey = (code: string, pressed: boolean) =>
  invoke<void>('input_key', { code, pressed })
export const currentFocus = () => invoke<InputFocus>('current_focus')
export const sessionState = () => invoke<'Idle' | 'Running' | 'Paused'>('session_state')

export interface LoadedGame {
  baseWidth: number
  baseHeight: number
  fps: number
  sampleRate: number
}
export const loadGame = (coreId: string, romPath: string) =>
  invoke<LoadedGame>('load_game', { coreId, romPath })

export interface AudioConfig {
  outputDeviceId: string | null
  outputDeviceName: string | null
  rateControlEnabled: boolean
  rateControlDelta: number
  sampleRatePreference: number | null
}
export const getAudioConfig = () => invoke<AudioConfig>('get_audio_config')
export const updateAudioConfig = (config: AudioConfig) =>
  invoke<void>('update_audio_config', { config })

export interface InstalledCore {
  coreId: string
  version: string
  renderBackend: string | null
}
export const listInstalledCores = () => invoke<InstalledCore[]>('list_installed_cores')

export interface RomEntry {
  id: string
  title: string
  systemId: string
  filePath: string
}
export const listRoms = () => invoke<RomEntry[]>('list_roms')

export interface ScanReport {
  found: number
  added: number
  skippedKnown: number
  skippedUnrecognized: number
  errors: number
}
export const scanLibrary = (path: string) => invoke<ScanReport>('scan_library', { path })

export interface SaveState {
  id: string
  slot: number | null
  createdAt: number
  filePath: string
}
export const saveState = (romId: string, slot: number | null) =>
  invoke<SaveState>('save_state', { romId, slot })
export const listSaveStates = (romId: string) =>
  invoke<SaveState[]>('list_save_states', { romId })
export const loadSaveState = (stateId: string) =>
  invoke<void>('load_save_state', { stateId })
export const deleteSaveState = (stateId: string) =>
  invoke<void>('delete_save_state', { stateId })
