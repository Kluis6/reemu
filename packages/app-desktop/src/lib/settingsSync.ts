// As configurações entre máquinas (D114) na tela: o texto do resultado de um
// grupo no "Enviar as desta máquina".

import type { TFunction } from 'i18next'
import { syncErrorCode } from '../hooks/useSync'
import type { SettingsGroupSent } from './bindings/SettingsGroupSent'

export type SettingsGroupName = SettingsGroupSent['group']

/** O texto do erro de um grupo no envio. Uma recusa da nuvem (`too_large`,
 *  `rejected`) é daquele grupo e diz qual, nunca "save"; o que parou o envio
 *  todo (o plano, a cota, a rede) é o texto de `sync.err`. */
export function settingsGroupErrorText(t: TFunction, group: SettingsGroupName, code: string): string {
  if (code === 'too_large' || code === 'rejected') {
    return t(`sync.settingsUpload.${code}`, { group: t(`sync.settingsGroup.${group}`) })
  }
  return t(`sync.err.${syncErrorCode(code)}`)
}
