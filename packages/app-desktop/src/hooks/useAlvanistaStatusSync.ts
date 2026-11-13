import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import type { AlvanistaStatus } from '../lib/tauri'
import { sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'
import { ALVANISTA_CACHED_KEY } from './useEffectiveProfile'

/** Chave do status completo (confirmado no Alvanista), dividida pelas telas. */
export const ALVANISTA_STATUS_KEY = ['alvanista-status'] as const

export const DISCONNECTED: AlvanistaStatus = {
  connected: false,
  username: null,
  avatar_url: null,
  hero_url: null,
  bio: null,
  profile_url: null,
  offline: false,
  ended: false,
  card: null,
  fetched_at: null,
  connected_at: null,
  device_label: null,
  save_sync: null,
  scopes: [],
}

function sameIdentity(a: AlvanistaStatus, b: AlvanistaStatus): boolean {
  return (
    a.connected === b.connected &&
    a.username === b.username &&
    a.avatar_url === b.avatar_url &&
    a.bio === b.bio &&
    a.hero_url === b.hero_url &&
    JSON.stringify(a.card) === JSON.stringify(b.card)
  )
}

/**
 * Mantém o perfil mostrado em dia com o status completo: se ele traz outra
 * identidade que a guardada, o estado guardado é relido (o rail, o hub, a
 * saudação e o perfil seguem `useEffectiveProfile`); se a conexão acabou, avisa
 * uma vez e zera o status completo. `status` é o que a tela mostra.
 */
export function useAlvanistaStatusSync(
  status: AlvanistaStatus,
  full: AlvanistaStatus | undefined,
  kept: AlvanistaStatus | undefined,
) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const endedShown = useRef(false)

  useEffect(() => {
    if (!full || !kept) return
    if (!sameIdentity(full, kept)) void qc.invalidateQueries({ queryKey: ALVANISTA_CACHED_KEY })
  }, [full, kept, qc])

  useEffect(() => {
    if (!status.ended || endedShown.current) return
    endedShown.current = true
    push(sysToast(t('alvanista.ended'), 'Warning'))
    qc.setQueryData(ALVANISTA_STATUS_KEY, DISCONNECTED)
    // O topo lê o estado guardado: zera junto (como ao desconectar), depois confirma.
    void (async () => {
      await qc.cancelQueries({ queryKey: ALVANISTA_CACHED_KEY })
      qc.setQueryData(ALVANISTA_CACHED_KEY, DISCONNECTED)
      void qc.invalidateQueries({ queryKey: ALVANISTA_CACHED_KEY })
    })()
  }, [status.ended, push, qc, t])
}
