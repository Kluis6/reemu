import { Caption1, makeStyles, tokens } from '@fluentui/react-components'
import { useTranslation } from 'react-i18next'
import { useStateNotes } from '../../hooks/useSync'
import type { SaveState } from '../../lib/tauri'

const useStyles = makeStyles({
  note: { color: tokens.colorNeutralForeground3, overflowWrap: 'anywhere' },
})

/** A frase de cada motivo que a vaga diz. */
const NOTE = {
  unversioned: 'sync.state.localUnversioned',
  too_large: 'sync.state.localTooLarge',
  rejected: 'sync.state.localRejected',
} as const

/**
 * Por que um save state desta máquina não vai para a nuvem, na vaga dele (D116,
 * D122): feito antes de a versão do core ser registrada (o state não tem a
 * versão), grande demais ou recusado pela nuvem (o que a última passada contou).
 * Só com o sync e os states ligados; sem motivo, nada aparece.
 */
export function LocalStateNote({ romId, state }: { romId: string; state: SaveState }) {
  const { t } = useTranslation()
  const s = useStyles()
  const notes = useStateNotes(romId)
  if (!notes.on) return null
  // Versão em branco vale por ausente, como no envio.
  const reason = state.coreVersion?.trim() ? notes.reasonOf(state.id) : 'unversioned'
  if (reason !== 'unversioned' && reason !== 'too_large' && reason !== 'rejected') return null
  return (
    <Caption1 block className={s.note} data-state-note={reason}>
      {t(NOTE[reason])}
    </Caption1>
  )
}
