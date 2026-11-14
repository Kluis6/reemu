// As configurações entre máquinas (D114) na tela: o texto do resultado de um
// grupo no "Enviar as desta máquina".

import type { TFunction } from 'i18next'
import { syncErrorCode } from '../hooks/useSync'
import type { AppearanceDoc } from './appearanceGroup'
import type { SettingsGroupName } from './tauri'

export type { SettingsDbGroup, SettingsGroupName } from './tauri'

/** Os seis grupos, na ordem do diálogo. */
export const SETTINGS_GROUPS: readonly SettingsGroupName[] = [
  'core-options',
  'shaders-decorations',
  'hotkeys-keyboard',
  'audio',
  'appearance',
  'video',
]

export const SETTINGS_SENT_KEY = ['sync-settings-sent'] as const
export const SETTINGS_UNDO_KEY = ['sync-settings-undo-info'] as const

/** A aparência de antes da última cópia, para o "Desfazer" (o Rust guarda só o
 *  banco). Cada acesso ao localStorage pode falhar: nunca derruba a tela. */
export const APPEARANCE_UNDO_KEY = 'reemu.sync.appearanceUndo'

/** O que fica guardado: a aparência e a cópia a que ela pertence (o
 *  `copiedAt` do marcador do Rust). */
interface SavedAppearance {
  copiedAt: number
  doc: AppearanceDoc
}

/** Guarda a aparência de antes da cópia de `copiedAt` (`CopyReport.copiedAt`). */
export function saveAppearanceUndo(doc: AppearanceDoc, copiedAt: number): void {
  try {
    const saved: SavedAppearance = { copiedAt, doc }
    localStorage.setItem(APPEARANCE_UNDO_KEY, JSON.stringify(saved))
  } catch {
    // sem localStorage: a aparência não volta, o resto volta
  }
}

/** A aparência de antes da cópia de `copiedAt` (`SettingsUndoInfo.copiedAt`),
 *  ou `null`. A de outra cópia não volta: o app pode ter fechado entre a cópia
 *  no Rust e a gravação daqui, e o que ficou é de uma cópia mais velha. */
export function readAppearanceUndo(copiedAt: number): unknown {
  try {
    const raw = localStorage.getItem(APPEARANCE_UNDO_KEY)
    if (raw === null) return null
    const saved = JSON.parse(raw) as unknown
    if (typeof saved !== 'object' || saved === null || !('copiedAt' in saved) || !('doc' in saved)) return null
    return saved.copiedAt === copiedAt ? saved.doc : null
  } catch {
    return null
  }
}

/** Uma cópia sem aparência apaga a de antes: o "Desfazer" dela não pode trazer
 *  de volta uma aparência mais velha. */
export function clearAppearanceUndo(): void {
  try {
    localStorage.removeItem(APPEARANCE_UNDO_KEY)
  } catch {
    // nada a apagar
  }
}

/** O texto do erro de um grupo no envio. Uma recusa da nuvem (`too_large`,
 *  `rejected`) é daquele grupo e diz qual, nunca "save"; o que parou o envio
 *  todo (o plano, a cota, a rede) é o texto de `sync.err`. */
export function settingsGroupErrorText(t: TFunction, group: SettingsGroupName, code: string): string {
  if (code === 'too_large' || code === 'rejected') {
    return t(`sync.settingsUpload.${code}`, { group: t(`sync.settingsGroup.${group}`) })
  }
  return t(`sync.err.${syncErrorCode(code)}`)
}
