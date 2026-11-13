import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import {
  alvanistaCancel,
  onSyncEvent,
  syncMachines,
  syncNow,
  syncSetEnabled,
  syncSetLabel,
  syncSetPaused,
  syncStatus,
  type SyncStatus,
} from '../lib/tauri'

export const SYNC_STATUS_KEY = ['sync-status'] as const
export const SYNC_MACHINES_KEY = ['sync-machines'] as const

/** Os códigos que os comandos `sync_*` devolvem ao falhar (`SyncError`), cada um
 *  com uma chave `sync.err.<código>`. */
export const SYNC_ERRORS = [
  'no_account',
  'no_tier',
  'needs_consent',
  'revoked',
  'paused',
  'quota',
  'offline',
  'keyring',
  'failed',
  'invalid_label',
  'not_enabled',
  'cancelled',
  'denied',
  'timeout',
] as const
export type SyncErrorCode = (typeof SYNC_ERRORS)[number]

/** O código de um erro de comando; o que não for conhecido vira `failed`. */
export function syncErrorCode(err: unknown): SyncErrorCode {
  const code = typeof err === 'string' ? err : err instanceof Error ? err.message : ''
  return (SYNC_ERRORS as readonly string[]).includes(code) ? (code as SyncErrorCode) : 'failed'
}

// Cada mutação devolve o estado novo e o backend também emite `status`: com uma
// mutação em curso o evento é ignorado, para não reler (rede) o que o resultado
// já trouxe.
let mutating = 0

/**
 * O estado da página Sincronização e as ações dela. As ações guardam o
 * `SyncStatus` que devolvem como o novo estado, sem reler.
 */
export function useSync() {
  const qc = useQueryClient()
  const status = useQuery({ queryKey: SYNC_STATUS_KEY, queryFn: syncStatus, retry: false })
  const active = status.data?.enabled === true && !status.data.revoked
  const machines = useQuery({
    queryKey: SYNC_MACHINES_KEY,
    queryFn: syncMachines,
    enabled: active,
    retry: false,
  })

  const [busy, setBusy] = useState<'enable' | 'label' | 'pause' | 'now' | null>(null)
  const [error, setError] = useState<SyncErrorCode | null>(null)
  const busyRef = useRef(false)
  const enablingRef = useRef(false)
  // Só o resultado da tentativa mais recente conta (cancelar e ligar de novo).
  const attempt = useRef(0)

  // Sair da tela com o navegador aberto cancela a espera.
  useEffect(
    () => () => {
      if (enablingRef.current) {
        attempt.current++
        void alvanistaCancel().catch(() => {})
      }
    },
    [],
  )

  const run = useCallback(
    async (kind: NonNullable<typeof busy>, call: () => Promise<SyncStatus>) => {
      if (busyRef.current) return
      busyRef.current = true
      const mine = ++attempt.current
      enablingRef.current = kind === 'enable'
      mutating++
      setError(null)
      setBusy(kind)
      try {
        const next = await call()
        if (mine !== attempt.current) return
        await qc.cancelQueries({ queryKey: SYNC_STATUS_KEY })
        qc.setQueryData(SYNC_STATUS_KEY, next)
        // A lista de aparelhos muda ao ligar, desligar e renomear.
        if (kind !== 'pause' && kind !== 'now') {
          void qc.invalidateQueries({ queryKey: SYNC_MACHINES_KEY })
        }
      } catch (err) {
        if (mine !== attempt.current) return
        const code = syncErrorCode(err)
        // Cancelar a espera do navegador não é erro.
        if (code !== 'cancelled') setError(code)
        // Um aparelho revogado ou uma conta que mudou: o estado do backend manda.
        if (code === 'revoked' || code === 'no_account' || code === 'no_tier') {
          void qc.invalidateQueries({ queryKey: SYNC_STATUS_KEY })
        }
      } finally {
        mutating--
        enablingRef.current = false
        busyRef.current = false
        setBusy(null)
      }
    },
    [qc],
  )

  const setEnabled = useCallback((on: boolean) => run('enable', () => syncSetEnabled(on)), [run])
  const setLabel = useCallback((label: string) => run('label', () => syncSetLabel(label)), [run])
  const setPaused = useCallback((p: boolean) => run('pause', () => syncSetPaused(p)), [run])
  const now = useCallback(() => run('now', () => syncNow()), [run])
  /** Cancela a espera do consentimento no navegador (o cancelar do Alvanista). */
  const cancel = useCallback(() => {
    void alvanistaCancel().catch(() => {})
  }, [])

  return { status, machines, busy, error, setEnabled, setLabel, setPaused, now, cancel }
}

/**
 * Relê o estado quando o backend avisa de uma mudança que a tela não causou
 * (revogado no site, uma passada que falhou). Montado uma vez, no `AppShell`.
 */
export function useSyncEvents() {
  const qc = useQueryClient()
  useEffect(() => {
    let disposed = false
    let dispose: (() => void) | undefined
    void onSyncEvent((e) => {
      if (e.type === 'status' && mutating === 0) {
        void qc.invalidateQueries({ queryKey: SYNC_STATUS_KEY })
        void qc.invalidateQueries({ queryKey: SYNC_MACHINES_KEY })
      }
    }).then((d) => {
      if (disposed) d()
      else dispose = d
    })
    return () => {
      disposed = true
      dispose?.()
    }
  }, [qc])
}
