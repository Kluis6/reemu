// Wrappers finos sobre os comandos/eventos do backend Rust.
// Tudo tolera rodar fora do Tauri (ex: `vite` puro no navegador) — nesse
// caso os comandos rejeitam e os listeners viram no-op.

import i18n from '../i18n'
import type { InputFocus } from '../stores/useFocusStore'
// Gerados do Rust (`cargo test`, ts-rs). Não editar à mão.
import type { CatalogCore } from './bindings/CatalogCore'
import type { AspectInfo } from './bindings/AspectInfo'
import type { AudioConfig } from './bindings/AudioConfig'
import type { BezelCatalogItem } from './bindings/BezelCatalogItem'
import type { BezelProgress } from './bindings/BezelProgress'
import type { BiosStatus } from './bindings/BiosStatus'
import type { Cheat } from './bindings/Cheat'
import type { ControllerDriverSettings } from './bindings/ControllerDriverSettings'
import type { CoreOption } from './bindings/CoreOption'
import type { CuratedPreset } from './bindings/CuratedPreset'
import type { DevicePort } from './bindings/DevicePort'
import type { GameMetadata } from './bindings/GameMetadata'
import type { Gamepad } from './bindings/Gamepad'
import type { HardwareInfo } from './bindings/HardwareInfo'
import type { HdrStatus } from './bindings/HdrStatus'
import type { KeyboardBinding } from './bindings/KeyboardBinding'
import type { MetadataConfig } from './bindings/MetadataConfig'
import type { PendingMatch } from './bindings/PendingMatch'
import type { Profile } from './bindings/Profile'
import type { RomEntry } from './bindings/RomEntry'
import type { RomShader } from './bindings/RomShader'
import type { RomSource } from './bindings/RomSource'
import type { SaveState } from './bindings/SaveState'
import type { ScanProgress } from './bindings/ScanProgress'
import type { ScanReport } from './bindings/ScanReport'
import type { ScrapeProgress } from './bindings/ScrapeProgress'
import type { ShaderInfo } from './bindings/ShaderInfo'
import type { ShaderPackProgress } from './bindings/ShaderPackProgress'
import type { ShaderPackStatus } from './bindings/ShaderPackStatus'
import type { ShaderParam } from './bindings/ShaderParam'
import type { SlangpEntry } from './bindings/SlangpEntry'
import type { SystemSettings } from './bindings/SystemSettings'
import type { UpdateInfo } from './bindings/UpdateInfo'
import type { VideoConfig } from './bindings/VideoConfig'
import type { InstalledCore } from './bindings/InstalledCore'
import type { LoadedGame } from './bindings/LoadedGame'

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

export type { LoadedGame }
export const loadGame = (coreId: string, romPath: string, romId?: string) =>
  invoke<LoadedGame>('load_game', { coreId, romPath, romId: romId ?? null })
export const unloadGame = () => invoke<void>('unload_game')

/** Quadro do jogo: cabeçalho de `FRAME_HEADER` bytes (`[w][h][decoGen]
 *  [cx cy hw hh f32][proporção f32][zoom da moldura f32]`, LE) + RGBA8.
 *  Vazio = sem quadro novo. */
export const pollFrame = () => invoke<ArrayBuffer>('poll_frame')

/** Modo canvas: tamanho, em pixels físicos, da área onde o jogo é desenhado —
 *  o "viewport" dos shaders (spec slang: `FinalViewportSize`). */
export const setVideoViewport = (width: number, height: number) =>
  invoke<void>('set_video_viewport', { width, height })
export const FRAME_HEADER = 36

/** Moldura ativa do modo canvas: `[decoGen][w][h]` (u32 LE) + RGBA8.
 *  Vazio = sem moldura. Buscada só quando `decoGen` do quadro muda. */
export const decorationImage = () => invoke<ArrayBuffer>('decoration_image')

/** `true` = vídeo numa surface nativa atrás da webview (`REEMU_NATIVE_VIDEO=1`);
 *  a PlayScreen fica transparente e não roda o loop do canvas. */
export const nativeVideoActive = () => invoke<boolean>('native_video_active')
/** Protótipo `REEMU_WIN_OVERLAY` (Windows): jogo atrás da webview transparente —
 *  o menu de pausa fica por cima do jogo, sem print de fundo. */
export const nativeVideoOverlay = () => invoke<boolean>('native_video_overlay')

// Configurações › Sistema (src-tauri/src/system.rs).
export type { SystemSettings }
export const getSystemSettings = () => invoke<SystemSettings>('get_system_settings')

/** Configurações › Sistema › Hardware (`commands/hardware.rs`). */
export type { HardwareInfo }
export const getHardwareInfo = () => invoke<HardwareInfo>('get_hardware_info')

/** Prévia de shader (Configurações › Vídeo): a cena de exemplo sem shader
 *  (PNG 320×240) e com o preset ativo, renderizada em `width`×`height`. */
export type PreviewScene = '2d' | '3d'
export const shaderPreviewSource = (scene: PreviewScene) =>
  invoke<ArrayBuffer>('shader_preview_source', { scene })
export const renderShaderPreview = (width: number, height: number, scene: PreviewScene) =>
  invoke<ArrayBuffer>('render_shader_preview', { width, height, scene })
export const setAutostart = (enabled: boolean) => invoke<void>('set_autostart', { enabled })
export const setStartFullscreen = (enabled: boolean) =>
  invoke<void>('set_start_fullscreen', { enabled })
export const setMinimizeToTray = (enabled: boolean) =>
  invoke<void>('set_minimize_to_tray', { enabled })

/** Driver de controle do SDL (Configurações › Controles, só no Windows —
 *  `input_desktop::pad_source::Driver`). */
export type ControllerDriver = 'auto' | 'xinput' | 'directinput' | 'wgi'
export const CONTROLLER_DRIVERS: ControllerDriver[] = ['auto', 'xinput', 'directinput', 'wgi']
export type { ControllerDriverSettings }
export const getControllerDriver = () =>
  invoke<ControllerDriverSettings>('get_controller_driver')
/** Troca na hora (reinicia só a leitura dos controles) e salva. */
export const setControllerDriver = (driver: ControllerDriver) =>
  invoke<void>('set_controller_driver', { driver })
/** Rótulos do menu do ícone da bandeja, no idioma da interface. */
export const setTrayLabels = (open: string, quit: string) =>
  invoke<void>('set_tray_labels', { open, quit })

export type { AudioConfig }
export const getAudioConfig = () => invoke<AudioConfig>('get_audio_config')
export const updateAudioConfig = (config: AudioConfig) =>
  invoke<void>('update_audio_config', { config })

export type { VideoConfig }
export const getVideoConfig = () => invoke<VideoConfig>('get_video_config')

/** Estado do HDR na janela do jogo (`video_hdr_status`). */
export type { HdrStatus }
export const getHdrStatus = () => invoke<HdrStatus>('video_hdr_status')
export const updateVideoConfig = (config: VideoConfig) =>
  invoke<void>('update_video_config', { config })

export const quitApp = () => invoke<void>('quit_app')

/** Desliga a máquina (`systemctl poweroff`) — menu de energia do rail. */
export const shutdownSystem = () => invoke<void>('shutdown_system')

/** Reinicia a máquina (`systemctl reboot`). */
export const restartSystem = () => invoke<void>('restart_system')
export const isFullscreen = () => invoke<boolean>('is_fullscreen')
export const setFullscreen = (value: boolean) => invoke<void>('set_fullscreen', { value })

/** Preset "de 1 clique" que aponta pro pacote `slang-shaders`. */
export type { CuratedPreset }
export type { ShaderInfo }
export const getShaderInfo = () => invoke<ShaderInfo>('get_shader_info')

export type ShaderScope = 'default' | 'system' | 'rom'
/** Problema conhecido de um preset na GPU (lista em `gpu/known_issues.txt`):
 *  `gpuHang` = trava a GPU (a imagem para); `renderError` = erro ao
 *  renderizar (o ReEmu volta pro padrão sozinho). */
export type ShaderIssue = 'gpuHang' | 'renderError'
/** `scope`: undefined = só nesta sessão; 'default' = todos os jogos;
 *  'system' (+`systemId` ou derivado do `romId`) = uma plataforma;
 *  'rom' (+`romId`) = só esse jogo (`name` vazio = limpar a atribuição). */
export const setShader = (
  name: string,
  scope?: ShaderScope,
  romId?: string,
  systemId?: string,
) =>
  // devolve o problema conhecido do preset escolhido, se houver
  invoke<ShaderIssue | null>('set_shader', {
    name,
    scope: scope ?? null,
    systemId: systemId ?? null,
    romId: romId ?? null,
  })

export type { RomShader }
export const getRomShader = (romId: string) => invoke<RomShader>('get_rom_shader', { romId })

/** Um `.slangp` achado varrendo a pasta de shaders do usuário. `category` é a
 *  subpasta de topo (`crt`, `handheld`…) ou `''` pros presets na raiz. */
export type { SlangpEntry }
export const listSlangpDir = (root: string) =>
  invoke<SlangpEntry[]>('list_slangp_dir', { root })

/** Parâmetro ajustável do shader ativo (`#pragma parameter`). Vazio pros
 *  builtins (plain/crt/lcd). */
export type { ShaderParam }
export const getShaderParams = () => invoke<ShaderParam[]>('get_shader_params')
/** Ajusta um parâmetro agora; `scope` (+`systemId`/`romId`) persiste. */
export const setShaderParam = (
  name: string,
  value: number,
  scope?: ShaderScope,
  romId?: string,
  systemId?: string,
) =>
  invoke<void>('set_shader_param', {
    name,
    value,
    scope: scope ?? null,
    systemId: systemId ?? null,
    romId: romId ?? null,
  })
/** Volta os parâmetros pros defaults do preset (limpa os overrides do escopo). */
export const resetShaderParams = (
  scope?: ShaderScope,
  romId?: string,
  systemId?: string,
) =>
  invoke<void>('reset_shader_params', {
    scope: scope ?? null,
    systemId: systemId ?? null,
    romId: romId ?? null,
  })

/** Importa uma pasta de bezels (formato Bezel Project/RetroBat). Devolve
 *  quantas atribuições foram gravadas. */
export const importDecorationPack = (path: string) =>
  invoke<number>('import_decoration_pack', { path })
export const clearDecorations = () => invoke<void>('clear_decorations')

/** Um sistema do catálogo de bezels do The Bezel Project. */
export type { BezelCatalogItem }
export const bezelCatalog = () =>
  invoke<BezelCatalogItem[]>('bezel_catalog')

export type { BezelProgress }
/** Baixa o pack de bezels de um sistema (The Bezel Project) e re-importa a
 *  árvore. Devolve o total de atribuições gravadas. */
export async function downloadBezelPack(
  systemId: string,
  onProgress?: (p: BezelProgress) => void,
): Promise<number> {
  if (!inTauri) throw new Error('fora do Tauri: download_bezel_pack')
  const { invoke: raw, Channel } = await import('@tauri-apps/api/core')
  const ch = new Channel<BezelProgress>()
  if (onProgress) ch.onmessage = onProgress
  return raw<number>('download_bezel_pack', { systemId, onProgress: ch })
}

export type { InstalledCore }
export const listInstalledCores = () => invoke<InstalledCore[]>('list_installed_cores')

/** Proporção da imagem: a do core, fixa, ou a janela inteira. */
export type AspectValue = 'core' | '4:3' | '16:9' | 'fill'
export type AspectScope = 'rom' | 'system' | 'default'
export type { AspectInfo }
export const getAspect = (romId: string) => invoke<AspectInfo>('get_aspect', { romId })
/** `aspect` null = volta a herdar. Com um jogo aberto, vale na hora. */
export const setAspect = (scope: AspectScope, romId: string, aspect: AspectValue | null) =>
  invoke<void>('set_aspect', { scope, romId, aspect })
/** A moldura saiu porque a imagem do jogo está larga demais pra ela. */
export async function onWideBezelOff(cb: () => void): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen('wide-bezel-off', () => cb())
}

export type { CoreOption }
export const getCoreOptions = (coreId: string, romId?: string) =>
  invoke<CoreOption[]>('get_core_options', { coreId, romId: romId ?? null })
/** `romId` ausente = valor por core; presente = override do jogo. `value`
 *  vazio = limpar (herdar). */
export const setCoreOption = (
  coreId: string,
  key: string,
  value: string,
  romId?: string,
) => invoke<void>('set_core_option', { coreId, key, value, romId: romId ?? null })
export const resetCoreOptions = (coreId: string, romId?: string) =>
  invoke<void>('reset_core_options', { coreId, romId: romId ?? null })

export type { CatalogCore }
export const listCoreCatalog = () => invoke<CatalogCore[]>('list_core_catalog')
export const downloadCore = (coreId: string) => invoke<void>('download_core', { coreId })
export const removeCore = (coreId: string) => invoke<void>('remove_core', { coreId })
/** Ids dos cores instalados com versão nova no buildbot. */
export const checkCoreUpdates = () => invoke<string[]>('check_core_updates')
export const restorePreviousCore = (coreId: string) =>
  invoke<void>('restore_previous_core', { coreId })

/** Arquivo de sistema (BIOS) — status contra `<dados>/system`. Nunca baixa
 *  nada (copyright da fabricante); só confere o que já está no disco. */
export type { BiosStatus }
export const listBiosStatus = () => invoke<BiosStatus[]>('list_bios_status')
export const importBiosFile = (systemId: string, filename: string, path: string) =>
  invoke<void>('import_bios_file', { systemId, filename, path })
export const removeBiosFile = (systemId: string, filename: string) =>
  invoke<void>('remove_bios_file', { systemId, filename })
/** Pasta `PPSSPP/` (assets do core de PSP) instalada na pasta de sistema? */
export const ppssppAssetsInstalled = () => invoke<boolean>('ppsspp_assets_installed')
/** Baixa os assets do PPSSPP (GPL, buildbot da libretro). Devolve nº de arquivos. */
export const downloadPpssppAssets = () => invoke<number>('download_ppsspp_assets')

/** Diálogo nativo de seleção de um arquivo de BIOS (sem filtro de extensão —
 *  vem em `.bin`/`.zip`/etc conforme o sistema). */
export async function pickBiosFile(): Promise<string | null> {
  if (!inTauri) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  const sel = await open({ multiple: false, title: i18n.t('dialogs.pickBios') })
  return typeof sel === 'string' ? sel : null
}

export type { RomEntry }
export const listRoms = () => invoke<RomEntry[]>('list_roms')
export const removeRom = (romId: string) => invoke<void>('remove_rom', { romId })
export const setRomFavorite = (romId: string, favorite: boolean) =>
  invoke<void>('set_rom_favorite', { romId, favorite })

/** Edição manual: `title` vazio volta pro nome do arquivo; `systemId` vazio não
 *  mexe na plataforma. */
export const setRomMetadata = (
  romId: string,
  title?: string,
  systemId?: string,
) =>
  invoke<void>('set_rom_metadata', {
    romId,
    title: title ?? null,
    systemId: systemId ?? null,
  })

// --- metadata / scraping (etapa 09) ---
export type { MetadataConfig }
export const getMetadataConfig = () => invoke<MetadataConfig>('get_metadata_config')
export const setMetadataConfig = (config: MetadataConfig) =>
  invoke<void>('set_metadata_config', { config })

export type { GameMetadata }
export const getRomMetadata = (romId: string) =>
  invoke<GameMetadata | null>('get_rom_metadata', { romId })

export type { PendingMatch }
export const listPendingMatches = () => invoke<PendingMatch[]>('list_pending_matches')
export const resolvePendingMatch = (romId: string, accept: boolean) =>
  invoke<void>('resolve_pending_match', { romId, accept })

export type { ScrapeProgress }
export const metadataScanProgress = () => invoke<ScrapeProgress>('metadata_scan_progress')
export const startMetadataScan = () => invoke<void>('start_metadata_scan')
/** "Buscar de novo" de um jogo: `auto` = aplicado, `pending` = foi pra revisão,
 *  `none` = nenhum provedor achou. Erro `ss_dev_login` = ScreenScraper recusou o app. */
export const rescrapeRom = (romId: string) =>
  invoke<'auto' | 'pending' | 'none'>('rescrape_rom', { romId })
export const cancelMetadataScan = () => invoke<void>('cancel_metadata_scan')

export type { RomSource }
export const listRomSources = () => invoke<RomSource[]>('list_rom_sources')
/** Remove todas as ROMs sob `path`. Devolve quantas saíram. */
export const removeRomSource = (path: string) => invoke<number>('remove_rom_source', { path })
/** Remove todas as ROMs de um sistema (snes, nes, …). Devolve quantas saíram. */
export const removeRomSystem = (systemId: string) =>
  invoke<number>('remove_rom_system', { systemId })
/** Esvazia a biblioteca inteira. Devolve quantas ROMs saíram. */
export const clearLibrary = () => invoke<number>('clear_library')

export type { ShaderPackStatus }
export const shaderPackStatus = () =>
  invoke<ShaderPackStatus>('shader_pack_status')
export type { ShaderPackProgress }
/** Baixa o pacote de slang shaders (buildbot da libretro; GitHub como
 * fallback) — os mesmos que o RetroArch usa. Devolve o caminho instalado. */
export async function downloadShaderPack(
  onProgress?: (p: ShaderPackProgress) => void,
): Promise<string> {
  if (!inTauri) throw new Error('fora do Tauri: download_shader_pack')
  const { invoke: raw, Channel } = await import('@tauri-apps/api/core')
  const ch = new Channel<ShaderPackProgress>()
  if (onProgress) ch.onmessage = onProgress
  return raw<string>('download_shader_pack', { onProgress: ch })
}
export const listSystemCores = () =>
  invoke<Record<string, string>>('list_system_cores')
/** `coreId` vazio limpa a preferência da plataforma. */
export const setSystemCore = (systemId: string, coreId: string) =>
  invoke<void>('set_system_core', { systemId, coreId })

export type { ScanReport }
export type { ScanProgress }
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

/** Varre de novo todas as pastas já adicionadas ("Atualizar biblioteca"). */
export async function rescanLibrary(
  onProgress?: (p: ScanProgress) => void,
): Promise<ScanReport> {
  if (!inTauri) throw new Error('fora do Tauri: rescan_library')
  const { invoke: raw, Channel } = await import('@tauri-apps/api/core')
  const ch = new Channel<ScanProgress>()
  if (onProgress) ch.onmessage = onProgress
  return raw<ScanReport>('rescan_library', { onProgress: ch })
}

/** Diálogo nativo de seleção de pasta. `null` se cancelado / fora do Tauri. */
export async function pickFolder(title = i18n.t('dialogs.pickRomFolder')): Promise<string | null> {
  if (!inTauri) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  const sel = await open({ directory: true, multiple: false, title })
  return typeof sel === 'string' ? sel : null
}

/** Diálogo nativo de seleção de arquivo `.slangp`. */
export async function pickSlangp(): Promise<string | null> {
  if (!inTauri) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  const sel = await open({
    multiple: false,
    title: i18n.t('dialogs.pickSlangp'),
    filters: [{ name: 'slang preset', extensions: ['slangp'] }],
  })
  return typeof sel === 'string' ? sel : null
}

export type { SaveState }
/** JSON da escolha de tema salva no Rust (`""` se nunca escolheu). */
export const getThemeSelection = () => invoke<string>('get_theme_selection')
export const setThemeSelection = (json: string) => invoke<void>('set_theme_selection', { json })
/** Tempo total de jogo da ROM, em segundos. */
export const getRomPlayTime = (romId: string) => invoke<number>('rom_play_time', { romId })
export const saveState = (romId: string, slot: number | null) =>
  invoke<SaveState>('save_state', { romId, slot })
export const listSaveStates = (romId: string) =>
  invoke<SaveState[]>('list_save_states', { romId })

/** Cheat por código de um jogo (`commands/cheats.rs`, doc 17). */
export type { Cheat }
export const listCheats = (romId: string) => invoke<Cheat[]>('list_cheats', { romId })
/** Entra ligado; vale na hora no jogo aberto. */
export const addCheat = (romId: string, description: string, code: string) =>
  invoke<Cheat>('add_cheat', { romId, description, code })
export const updateCheat = (id: number, description: string, code: string, enabled: boolean) =>
  invoke<Cheat>('update_cheat', { id, description, code, enabled })
export const deleteCheat = (id: number) => invoke<void>('delete_cheat', { id })
export const loadSaveState = (stateId: string) =>
  invoke<void>('load_save_state', { stateId })
export const deleteSaveState = (stateId: string) =>
  invoke<void>('delete_save_state', { stateId })

/** PNG do frame que estava na tela quando o menu de pausa abriu (vídeo nativo),
 *  como `blob:` URL. `null` se não há (modo canvas ou sem captura ainda). */
export async function pauseBackgroundUrl(): Promise<string | null> {
  if (!inTauri) return null
  const buf = await invoke<ArrayBuffer>('pause_background')
  if (buf.byteLength === 0) return null
  return URL.createObjectURL(new Blob([buf], { type: 'image/png' }))
}

/** PNG do thumbnail de um save state como `blob:` URL, ou `null` se não tem. */
export async function saveThumbnailUrl(stateId: string): Promise<string | null> {
  if (!inTauri) return null
  const buf = await invoke<ArrayBuffer>('read_save_thumbnail', { stateId })
  if (buf.byteLength === 0) return null
  return URL.createObjectURL(new Blob([buf], { type: 'image/png' }))
}

// --- perfil local (um por instalação) ----------------------------------

export type { Profile }
export const getProfile = () => invoke<Profile>('get_profile')

/** `name` obrigatório; `avatar` = 'preset:N' (1..5) ou 'file'. Marca o
 *  onboarding como concluído. */
export const setProfile = (name: string, bio: string | null, avatar: string) =>
  invoke<void>('set_profile', { name, bio: bio ?? null, avatar })

/** Copia a imagem escolhida pra `<dados>/profile/avatar.<ext>`. */
export const setProfileAvatarFile = (srcPath: string) =>
  invoke<void>('set_profile_avatar_file', { srcPath })

/** A imagem de avatar do usuário como `blob:` URL, ou `null` se ele usa um
 *  preset / ainda não escolheu. Passe um nonce pra furar cache após troca. */
export async function profileAvatarUrl(): Promise<string | null> {
  if (!inTauri) return null
  const buf = await invoke<ArrayBuffer>('read_profile_avatar')
  if (buf.byteLength === 0) return null
  return URL.createObjectURL(new Blob([buf]))
}

/** Diálogo nativo pra escolher uma imagem (avatar, papel de parede…). */
export async function pickImage(title = i18n.t('dialogs.pickImage')): Promise<string | null> {
  if (!inTauri) return null
  const { open } = await import('@tauri-apps/plugin-dialog')
  const sel = await open({
    multiple: false,
    title,
    filters: [{ name: i18n.t('dialogs.imageFilter'), extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }],
  })
  return typeof sel === 'string' ? sel : null
}

// --- papel de parede da tela inicial (opcional) --------------------------

/** Copia a imagem escolhida pra `<dados>/appearance/wallpaper.<ext>`. */
export const setWallpaperFile = (srcPath: string) =>
  invoke<void>('set_wallpaper_file', { srcPath })

/** Remove o papel de parede — a tela inicial volta a mostrar só a cor do tema. */
export const clearWallpaper = () => invoke<void>('clear_wallpaper')

/** O papel de parede escolhido como `blob:` URL, ou `null` se não há nenhum. */
export async function wallpaperUrl(): Promise<string | null> {
  if (!inTauri) return null
  const buf = await invoke<ArrayBuffer>('read_wallpaper')
  if (buf.byteLength === 0) return null
  return URL.createObjectURL(new Blob([buf]))
}

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
  | 'action'
  | 'tabPrev'
  | 'tabNext'

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

export type { Gamepad }
export const listGamepads = () => invoke<Gamepad[]>('list_gamepads')

/** Um alvo do teclado (botão do RetroPad ou direção de analógico) e a tecla
 *  dele (`KeyboardEvent.code`; vazio = sem tecla). */
export type { KeyboardBinding }
export const listKeyboardBindings = () => invoke<KeyboardBinding[]>('list_keyboard_bindings')
export const setKeyboardBinding = (target: string, code: string) =>
  invoke<void>('set_keyboard_binding', { target, code })
export const resetKeyboardBindings = () => invoke<void>('reset_keyboard_bindings')

/** Controle plugado agora (toast + ícone na topbar). */
export async function onGamepadConnected(cb: (g: Gamepad) => void): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<Gamepad>('gamepad-connected', (e) => cb(e.payload))
}

/** Controle desligado agora — `guid_hex`. Some do ícone, sem toast. */
export async function onGamepadDisconnected(cb: (guid: string) => void): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<string>('gamepad-disconnected', (e) => cb(e.payload))
}

export type { DevicePort }
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
// Função (não constante): os rótulos com texto seguem o idioma ativo.
const gamepadButtonNames = (): Record<number, string> => ({
  0: i18n.t('input.pad.south'), 1: i18n.t('input.pad.east'), 2: i18n.t('input.pad.north'), 3: i18n.t('input.pad.west'),
  4: 'C', 5: 'Z',
  6: 'L1', 7: 'L2', 8: 'R1', 9: 'R2',
  10: 'Select', 11: 'Start', 12: i18n.t('input.pad.guide'),
  13: 'L3', 14: 'R3',
  15: 'D-pad ↑', 16: 'D-pad ↓', 17: 'D-pad ←', 18: 'D-pad →',
})

/** Rótulo curto e legível de um evento bruto (pra chips da UI de captura). */
export function describeRawInput(ev: RawInputEvent): string {
  if ('Keyboard' in ev) return i18n.t('input.key')
  if ('GamepadButton' in ev)
    return (
      gamepadButtonNames()[ev.GamepadButton.index] ??
      i18n.t('input.button', { n: ev.GamepadButton.index })
    )
  return i18n.t('input.axis', { n: ev.GamepadAxis.index })
}

/** Manda uma linha pro log do Rust (terminal do `tauri dev` / log do app) —
 *  diagnóstico da webview. Nunca lança. */
export const jsLog = (level: 'info' | 'warn' | 'error', message: string) =>
  invoke<void>('js_log', { level, message }).catch(() => {})

// ── Atualização do app (`src-tauri/src/updates.rs`) ─────────────────────

export type { UpdateInfo }

/** `null` = já está na última versão (ou auto-update sem chave configurada). */
export const checkForUpdate = () => invoke<UpdateInfo | null>('update_check')

/** Baixa, instala e reinicia o app — só retorna se der erro. */
export const installUpdate = () => invoke<void>('update_install')

/** Shader que impediu o jogo de aparecer e saiu da atribuição
 *  (`shader_failure.rs`, evento `shader-failed`). */
export interface ShaderFailure {
  source: string
  name: string
  /** `build` = não montou; `knownGpuHang` = trava a GPU (nem aplicado);
   *  `gpu` = erro da GPU no meio do jogo */
  kind: 'build' | 'knownGpuHang' | 'gpu'
  /** de onde saiu; null = não estava atribuído */
  scope: 'rom' | 'system' | 'default' | null
  systemId: string | null
  error: string
}
export async function onShaderFailed(cb: (f: ShaderFailure) => void): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<ShaderFailure>('shader-failed', (e) => cb(e.payload))
}

export async function onUpdateProgress(
  cb: (p: { downloaded: number; total: number | null }) => void,
): Promise<() => void> {
  if (!inTauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<{ downloaded: number; total: number | null }>('update-progress', (e) =>
    cb(e.payload),
  )
}

/** Versão do app instalado (`tauri.conf.json` › `version`). */
export async function appVersion(): Promise<string> {
  if (!inTauri) throw new Error('fora do Tauri: appVersion')
  const { getVersion } = await import('@tauri-apps/api/app')
  return getVersion()
}
