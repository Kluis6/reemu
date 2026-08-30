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
  /** Proporção de exibição do core (0 = usar baseWidth/baseHeight). */
  aspectRatio: number
}
export const loadGame = (coreId: string, romPath: string, romId?: string) =>
  invoke<LoadedGame>('load_game', { coreId, romPath, romId: romId ?? null })
export const unloadGame = () => invoke<void>('unload_game')

/** Frame do core: `ArrayBuffer` com `[w u32 LE][h u32 LE][rgba…]`, ou vazio. */
export const pollFrame = () => invoke<ArrayBuffer>('poll_frame')

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

export const quitApp = () => invoke<void>('quit_app')
export const isFullscreen = () => invoke<boolean>('is_fullscreen')
export const setFullscreen = (value: boolean) => invoke<void>('set_fullscreen', { value })

export interface ShaderInfo {
  active: string
  available: string[]
  /** false = sem adapter wgpu; trocar de preset não tem efeito. */
  gpu: boolean
}
export const getShaderInfo = () => invoke<ShaderInfo>('get_shader_info')
/** `scope`: undefined = só nesta sessão; 'default' = todos os jogos;
 *  'rom' (+`romId`) = só esse jogo (`name` vazio = limpar a atribuição). */
export const setShader = (
  name: string,
  scope?: 'default' | 'rom',
  romId?: string,
) => invoke<void>('set_shader', { name, scope: scope ?? null, romId: romId ?? null })

export interface RomShader {
  sourcePath: string | null
  fromRom: boolean
}
export const getRomShader = (romId: string) => invoke<RomShader>('get_rom_shader', { romId })

/** Parâmetro ajustável do shader ativo (`#pragma parameter`). Vazio pros
 *  builtins (plain/crt/lcd). */
export interface ShaderParam {
  name: string
  label: string
  value: number
  default: number
  min: number
  max: number
  step: number
}
export const getShaderParams = () => invoke<ShaderParam[]>('get_shader_params')
/** Ajusta um parâmetro agora; `scope` ('default'|'rom' + `romId`) persiste. */
export const setShaderParam = (
  name: string,
  value: number,
  scope?: 'default' | 'rom',
  romId?: string,
) =>
  invoke<void>('set_shader_param', {
    name,
    value,
    scope: scope ?? null,
    romId: romId ?? null,
  })
/** Volta os parâmetros pros defaults do preset (limpa os overrides do escopo). */
export const resetShaderParams = (scope?: 'default' | 'rom', romId?: string) =>
  invoke<void>('reset_shader_params', { scope: scope ?? null, romId: romId ?? null })

/** Importa uma pasta de bezels (formato Bezel Project/RetroBat). Devolve
 *  quantas atribuições foram gravadas. */
export const importDecorationPack = (path: string) =>
  invoke<number>('import_decoration_pack', { path })
export const clearDecorations = () => invoke<void>('clear_decorations')

export interface InstalledCore {
  coreId: string
  name: string
  version: string
  extensions: string[]
  renderBackend: string | null
}
export const listInstalledCores = () => invoke<InstalledCore[]>('list_installed_cores')

export interface CoreOption {
  key: string
  displayName: string
  choices: string[]
  defaultValue: string
  value: string
}
export const getCoreOptions = (coreId: string) =>
  invoke<CoreOption[]>('get_core_options', { coreId })
export const setCoreOption = (coreId: string, key: string, value: string) =>
  invoke<void>('set_core_option', { coreId, key, value })

export interface CatalogCore {
  coreId: string
  name: string
  systems: string
  license: string
  installed: boolean
}
export const listCoreCatalog = () => invoke<CatalogCore[]>('list_core_catalog')
export const downloadCore = (coreId: string) => invoke<void>('download_core', { coreId })
export const removeCore = (coreId: string) => invoke<void>('remove_core', { coreId })

export interface RomEntry {
  id: string
  title: string
  systemId: string
  filePath: string
  boxart: string | null
  /** Unix (s) do último load — pra "Continuar jogando". */
  lastPlayedAt: number | null
  /** Unix (s) de quando entrou na biblioteca — pra "Adicionados recentemente". */
  addedAt: number
}
export const listRoms = () => invoke<RomEntry[]>('list_roms')
export const removeRom = (romId: string) => invoke<void>('remove_rom', { romId })

// --- metadata / scraping (etapa 09) ---
export interface MetadataConfig {
  provider: string
  screenscraperUser: string | null
  screenscraperPassword: string | null
}
export const getMetadataConfig = () => invoke<MetadataConfig>('get_metadata_config')
export const setMetadataConfig = (config: MetadataConfig) =>
  invoke<void>('set_metadata_config', { config })

export interface GameMetadata {
  title: string
  description: string | null
  coverUrl: string | null
  releaseDate: string | null
  genre: string | null
  providerSource: string | null
}
export const getRomMetadata = (romId: string) =>
  invoke<GameMetadata | null>('get_rom_metadata', { romId })

export interface PendingMatch {
  romId: string
  fileStem: string
  title: string
  description: string | null
  coverUrl: string | null
  releaseDate: string | null
  genre: string | null
}
export const listPendingMatches = () => invoke<PendingMatch[]>('list_pending_matches')
export const resolvePendingMatch = (romId: string, accept: boolean) =>
  invoke<void>('resolve_pending_match', { romId, accept })

export interface ScrapeProgress {
  running: boolean
  done: number
  total: number
  auto: number
  pending: number
  failed: number
}
export const metadataScanProgress = () => invoke<ScrapeProgress>('metadata_scan_progress')
export const startMetadataScan = () => invoke<void>('start_metadata_scan')
export const cancelMetadataScan = () => invoke<void>('cancel_metadata_scan')

export interface RomSource {
  /** Pasta raiz da biblioteca (ex.: `.../RetroBat/roms`). */
  path: string
  count: number
}
export const listRomSources = () => invoke<RomSource[]>('list_rom_sources')
/** Remove todas as ROMs sob `path`. Devolve quantas saíram. */
export const removeRomSource = (path: string) => invoke<number>('remove_rom_source', { path })
/** Remove todas as ROMs de um sistema (snes, nes, …). Devolve quantas saíram. */
export const removeRomSystem = (systemId: string) =>
  invoke<number>('remove_rom_system', { systemId })
/** Esvazia a biblioteca inteira. Devolve quantas ROMs saíram. */
export const clearLibrary = () => invoke<number>('clear_library')

export interface ScanReport {
  found: number
  added: number
  skippedKnown: number
  skippedUnrecognized: number
  errors: number
}
export interface ScanProgress {
  current: number
  total: number
  file: string
}
export async function scanLibrary(
  path: string,
  onProgress?: (p: ScanProgress) => void,
): Promise<ScanReport> {
  if (!inTauri) throw new Error('fora do Tauri: scan_library')
  const { invoke: raw, Channel } = await import('@tauri-apps/api/core')
  const ch = new Channel<ScanProgress>()
  if (onProgress) ch.onmessage = onProgress
  return raw<ScanReport>('scan_library', { path, onProgress: ch })
}

/** Diálogo nativo de seleção de pasta. `null` se cancelado / fora do Tauri. */
export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  const sel = await open({ directory: true, multiple: false, title: 'Escolha a pasta das ROMs' })
  return typeof sel === 'string' ? sel : null
}

/** Diálogo nativo de seleção de arquivo `.slangp`. */
export async function pickSlangp(): Promise<string | null> {
  if (!inTauri) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  const sel = await open({
    multiple: false,
    title: 'Escolha um preset .slangp',
    filters: [{ name: 'slang preset', extensions: ['slangp'] }],
  })
  return typeof sel === 'string' ? sel : null
}

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

// --- input / captura de binding (etapa 05) -------------------------------

/** Espelha `domain::input::RawInputEvent` (serde externally tagged). */
export type RawInputEvent =
  | { Keyboard: { scancode: number } }
  | { GamepadButton: { device_guid: string; index: number } }
  | { GamepadAxis: { device_guid: string; index: number; value: number } }

/** Alvo da captura: hotkey de sistema ou mapeamento de controle. */
export type BindingTarget = 'system_hotkey' | 'controller_mapping'

/** `SystemAction::as_wire()` do backend. */
export type SystemActionKey = 'toggle_menu_overlay' | 'quick_save' | 'quick_load'

export async function onRawInputCaptured(
  cb: (ev: RawInputEvent) => void,
): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<RawInputEvent>('raw-input-captured', (e) => cb(e.payload))
}

export interface HotkeyActionEvent {
  action: SystemActionKey
  ok: boolean
  message: string
}

/** Ação de hotkey executada no backend (`quick_save`/`quick_load` — o toggle
 *  de menu o backend já trata sozinho). */
export async function onHotkeyAction(
  cb: (e: HotkeyActionEvent) => void,
): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<HotkeyActionEvent>('hotkey-action', (e) => cb(e.payload))
}

/** Pulso de navegação de menu vindo do gamepad (resolvido pelo gilrs no
 *  backend — a Gamepad API do WebKitGTK não funciona nesse setup). */
export type MenuNav =
  | 'up'
  | 'down'
  | 'left'
  | 'right'
  | 'confirm'
  | 'back'
  | 'search'
  | 'context'

export async function onMenuNav(cb: (nav: MenuNav) => void): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<MenuNav>('menu-nav', (e) => cb(e.payload))
}

export const startBindingCapture = () => invoke<void>('start_binding_capture')
export const cancelBindingCapture = () => invoke<void>('cancel_binding_capture')
export const saveBinding = (
  target: BindingTarget,
  targetKey: string,
  trigger: RawInputEvent[],
) => invoke<void>('save_binding', { target, targetKey, trigger })

export interface HotkeyBinding {
  action: SystemActionKey
  trigger: RawInputEvent[]
}
export const listSystemHotkeys = () => invoke<HotkeyBinding[]>('list_system_hotkeys')
export const clearSystemHotkey = (action: SystemActionKey) =>
  invoke<void>('clear_system_hotkey', { action })

export interface ControllerEntry {
  button: string
  trigger: RawInputEvent[]
}
export interface ControllerMapping {
  guid: string
  displayName: string
  source: string
  entries: ControllerEntry[]
}
export const listControllerMappings = () =>
  invoke<ControllerMapping[]>('list_controller_mappings')
export const clearControllerMapping = (guid: string) =>
  invoke<void>('clear_controller_mapping', { guid })

export interface Gamepad {
  guid: string
  name: string
}
export const listGamepads = () => invoke<Gamepad[]>('list_gamepads')

export interface DevicePort {
  guid: string
  port: number
}
export const listDevicePorts = () => invoke<DevicePort[]>('list_device_ports')
export const setDevicePort = (guid: string, port: number) =>
  invoke<void>('set_device_port', { guid, port })
export const clearDevicePort = (guid: string) =>
  invoke<void>('clear_device_port', { guid })

/** Ordem canônica dos botões RetroPad (bate com `domain::input::RetroPadButton`). */
export const RETROPAD_BUTTONS = [
  'Up', 'Down', 'Left', 'Right',
  'A', 'B', 'X', 'Y',
  'L1', 'R1', 'L2', 'R2', 'L3', 'R3',
  'Start', 'Select',
] as const

/** Nome físico do botão do gamepad (bate com `input_desktop::gilrs_button_index`). */
const GAMEPAD_BUTTON_NAMES: Record<number, string> = {
  0: 'A (baixo)', 1: 'B (direita)', 2: 'Y (cima)', 3: 'X (esquerda)',
  4: 'C', 5: 'Z',
  6: 'L1', 7: 'L2', 8: 'R1', 9: 'R2',
  10: 'Select', 11: 'Start', 12: 'Guia',
  13: 'L3', 14: 'R3',
  15: 'D-pad ↑', 16: 'D-pad ↓', 17: 'D-pad ←', 18: 'D-pad →',
}

/** Rótulo curto e legível de um evento bruto (pra chips da UI de captura). */
export function describeRawInput(ev: RawInputEvent): string {
  if ('Keyboard' in ev) return 'Tecla'
  if ('GamepadButton' in ev)
    return GAMEPAD_BUTTON_NAMES[ev.GamepadButton.index] ?? `Botão ${ev.GamepadButton.index}`
  return `Eixo ${ev.GamepadAxis.index}`
}
