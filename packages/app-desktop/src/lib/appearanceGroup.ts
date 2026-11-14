// Aparência como documento portátil: o grupo `appearance` das configurações
// copiadas entre máquinas (D114). Tema, escala da interface, margem da TV,
// sons, menos movimento e idioma, pelos getters e setters de cada um. O papel
// de parede não vai: é um arquivo desta máquina.

import {
  getLanguagePreference,
  LANGUAGES,
  setLanguagePreference,
  type LanguagePreference,
} from '../i18n'
import { syncFromRust, useThemeStore } from '../stores/useThemeStore'
import { getThemeSelection, setThemeSelection } from './tauri'
import { getReduceMotion, getUiSounds, setReduceMotion, setUiSounds } from './uiPrefs'
import { getOverscan, getUiScale, OVERSCANS, setOverscan, setUiScale, UI_SCALES } from './uiScale'

export const APPEARANCE_DOC_VERSION = 1
/** Documento de uma versão que esta máquina não conhece: nada muda. */
export const APPEARANCE_ERR_VERSION = 'settings_version'
/** Documento sem o formato do grupo: nada muda. */
export const APPEARANCE_ERR_INVALID = 'settings_invalid'

export interface AppearanceDoc {
  v: typeof APPEARANCE_DOC_VERSION
  /** O JSON do tema como o Rust guarda (`get_theme_selection`). */
  themeSelection: string
  uiScale: number
  overscan: number
  uiSounds: boolean
  reduceMotion: boolean
  language: LanguagePreference
}

/** O tema salvo no Rust; sem nada lá (nunca escolheu) ou sem Rust, o que a
 *  tela mostra agora. */
async function currentTheme(): Promise<string> {
  try {
    const saved = await getThemeSelection()
    if (saved) return saved
  } catch {
    // sem Rust: vale o que a store tem
  }
  const { selection, customDraft } = useThemeStore.getState()
  return JSON.stringify({ selection, customDraft })
}

export async function exportAppearance(): Promise<AppearanceDoc> {
  return {
    v: APPEARANCE_DOC_VERSION,
    themeSelection: await currentTheme(),
    uiScale: getUiScale(),
    overscan: getOverscan(),
    uiSounds: getUiSounds(),
    reduceMotion: getReduceMotion(),
    language: getLanguagePreference(),
  }
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

function isThemeJson(raw: string): boolean {
  try {
    const parsed: unknown = JSON.parse(raw)
    return isObject(parsed) && isObject(parsed.selection)
  } catch {
    return false
  }
}

/**
 * Aplica o documento com os setters de cada preferência. Versão desconhecida
 * ou formato errado: lança e nada muda. Um valor que esta versão não oferece
 * (uma escala, uma margem ou um idioma novos) deixa o desta máquina; um tema
 * que ela não conhece cai no padrão, como na store.
 */
export async function applyAppearance(doc: unknown): Promise<void> {
  if (!isObject(doc)) throw new Error(APPEARANCE_ERR_INVALID)
  if (doc.v !== APPEARANCE_DOC_VERSION) throw new Error(APPEARANCE_ERR_VERSION)
  const { themeSelection, uiScale, overscan, uiSounds, reduceMotion, language } = doc
  if (
    typeof themeSelection !== 'string' ||
    !isThemeJson(themeSelection) ||
    typeof uiScale !== 'number' ||
    typeof overscan !== 'number' ||
    typeof uiSounds !== 'boolean' ||
    typeof reduceMotion !== 'boolean' ||
    typeof language !== 'string'
  ) {
    throw new Error(APPEARANCE_ERR_INVALID)
  }

  // O tema primeiro: é o único que pode falhar (a gravação no Rust), e
  // falhando nada mudou. A store relê o que ficou salvo.
  await setThemeSelection(themeSelection)
  await syncFromRust()
  if (OVERSCANS.some((o) => o.value === overscan)) setOverscan(overscan)
  setUiSounds(uiSounds)
  setReduceMotion(reduceMotion)
  if (language === 'auto' || (LANGUAGES as readonly string[]).includes(language)) {
    await setLanguagePreference(language as LanguagePreference)
  }
  // Por último: guarda a escolha antes de refazer o zoom, que pode falhar.
  if (UI_SCALES.some((s) => s.value === uiScale)) await setUiScale(uiScale)
}
