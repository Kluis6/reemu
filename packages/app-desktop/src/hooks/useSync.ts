import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import i18n from '../i18n'
import { sysToast } from '../lib/toast'
import {
  alvanistaCancel,
  onSyncEvent,
  syncConflicts,
  syncMachines,
  syncNow,
  syncResolve,
  syncRestore,
  syncSaveStatus,
  syncStateNotes,
  syncStatesFor,
  syncSetEnabled,
  syncSetLabel,
  syncSetPaused,
  syncSetSavesEnabled,
  syncSetStatesEnabled,
  syncStatus,
  syncUploadOldStates,
  syncVersions,
  type SyncStatus,
} from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'

export const SYNC_STATUS_KEY = ['sync-status'] as const
export const SYNC_MACHINES_KEY = ['sync-machines'] as const
export const SYNC_CONFLICTS_KEY = ['sync-conflicts'] as const
/** Prefixo das linhas de save de todas as páginas de jogo. */
export const SYNC_SAVE_LINE_KEY = ['sync-save-line'] as const
export const SYNC_VERSIONS_KEY = ['sync-versions'] as const
/** Os states das outras máquinas, por jogo e core. */
export const SYNC_REMOTE_STATES_KEY = ['sync-remote-states'] as const
/** Por que cada state local fica só nesta máquina, por jogo. */
export const SYNC_STATE_NOTES_KEY = ['sync-state-notes'] as const

/** Os códigos de `sync_download_state` que a tela explica (cada um com uma
 *  chave `sync.state.err.<código>`); `other_core`, `other_version` e
 *  `unversioned` valem `unavailable`. O resto vira `failed`. */
export type StateDownloadError = 'unavailable' | 'offline' | 'loading' | 'backup_failed' | 'hash_mismatch' | 'not_found' | 'failed'
export function stateDownloadError(err: unknown): StateDownloadError {
  const code = typeof err === 'string' ? err : err instanceof Error ? err.message : ''
  if (['other_core', 'other_version', 'unversioned'].includes(code)) return 'unavailable'
  const known: StateDownloadError[] = ['offline', 'loading', 'backup_failed', 'hash_mismatch', 'not_found']
  return known.find((k) => k === code) ?? 'failed'
}

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
  'too_large',
  'state_too_large',
  'rejected',
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

  const [busy, setBusy] = useState<'enable' | 'label' | 'pause' | 'now' | 'saves' | 'states' | 'oldStates' | null>(null)
  const [error, setError] = useState<SyncErrorCode | null>(null)
  const busyRef = useRef(false)
  const busyKind = useRef<typeof busy>(null)
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
      busyKind.current = kind
      const mine = ++attempt.current
      enablingRef.current = kind === 'enable'
      mutating++
      setError(null)
      setBusy(kind)
      try {
        const next = await call()
        if (mine !== attempt.current) {
          // Outra ação passou à frente (os states desligados no meio): este
          // resultado, mais velho, não vale, mas o estado tem de ser relido.
          void qc.invalidateQueries({ queryKey: SYNC_STATUS_KEY })
          return
        }
        await qc.cancelQueries({ queryKey: SYNC_STATUS_KEY })
        qc.setQueryData(SYNC_STATUS_KEY, next)
        // A lista de aparelhos muda ao ligar, desligar e renomear.
        if (kind !== 'pause' && kind !== 'now' && kind !== 'saves' && kind !== 'states' && kind !== 'oldStates') {
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
        busyKind.current = null
        setBusy(null)
      }
    },
    [qc],
  )

  /** Enquanto "Sincronizar agora" ou "Enviar os states antigos também" esperam,
   *  o interruptor dos states segue valendo (desligar para a passada dos states
   *  no meio). Ele passa à frente da ação em curso: o resultado dela, mais velho,
   *  não sobrescreve o dele. */
  const statesBeside = useCallback(
    async (on: boolean) => {
      const mine = ++attempt.current
      mutating++
      try {
        const next = await syncSetStatesEnabled(on)
        if (mine !== attempt.current) return
        await qc.cancelQueries({ queryKey: SYNC_STATUS_KEY })
        qc.setQueryData(SYNC_STATUS_KEY, next)
      } catch (err) {
        if (mine === attempt.current) setError(syncErrorCode(err))
      } finally {
        mutating--
      }
    },
    [qc],
  )

  const setEnabled = useCallback((on: boolean) => run('enable', () => syncSetEnabled(on)), [run])
  const setLabel = useCallback((label: string) => run('label', () => syncSetLabel(label)), [run])
  const setPaused = useCallback((p: boolean) => run('pause', () => syncSetPaused(p)), [run])
  const now = useCallback(() => run('now', () => syncNow()), [run])
  const setSaves = useCallback((on: boolean) => run('saves', () => syncSetSavesEnabled(on)), [run])
  const setStates = useCallback(
    (on: boolean) =>
      busyKind.current === 'now' || busyKind.current === 'oldStates'
        ? statesBeside(on)
        : run('states', () => syncSetStatesEnabled(on)),
    [run, statesBeside],
  )
  /** "Enviar de novo": os states que um "apagar tudo" no site deixou só aqui. */
  const uploadOldStates = useCallback(() => run('oldStates', () => syncUploadOldStates()), [run])
  /** Cancela a espera do consentimento no navegador (o cancelar do Alvanista). */
  const cancel = useCallback(() => {
    void alvanistaCancel().catch(() => {})
  }, [])

  return { status, machines, busy, error, setEnabled, setLabel, setPaused, setSaves, setStates, uploadOldStates, now, cancel }
}

/** A linha de save de um jogo. Só disco e SQLite: sem sync ligado, revogado ou
 *  com o interruptor dos saves desligado o comando devolve `none` (a linha não
 *  aparece); sem conta ele falha e a página segue sem ela. */
export function useSaveStatus(romId: string, enabled = true) {
  return useQuery({
    queryKey: [...SYNC_SAVE_LINE_KEY, romId],
    queryFn: () => syncSaveStatus(romId),
    enabled,
    retry: false,
    staleTime: 0,
    refetchOnMount: 'always',
  })
}

/** Os conflitos abertos, para o diálogo e a contagem das configurações. */
export function useConflicts(enabled = true) {
  return useQuery({ queryKey: SYNC_CONFLICTS_KEY, queryFn: syncConflicts, retry: false, enabled })
}

/** Depois de resolver ou restaurar, a linha, os conflitos e o estado mudaram. */
export function useInvalidateSave() {
  const qc = useQueryClient()
  return useCallback(async () => {
    await Promise.all([
      qc.invalidateQueries({ queryKey: SYNC_SAVE_LINE_KEY }),
      qc.invalidateQueries({ queryKey: SYNC_CONFLICTS_KEY }),
      qc.invalidateQueries({ queryKey: SYNC_VERSIONS_KEY }),
      qc.invalidateQueries({ queryKey: SYNC_STATUS_KEY }),
    ])
  }, [qc])
}

/** Fecha um conflito (`local` ou `remote`) e relê o que a página mostra. */
export function useResolve() {
  const invalidate = useInvalidateSave()
  return useMutation({
    mutationFn: ({ key, keep }: { key: string; keep: 'local' | 'remote' }) => syncResolve(key, keep),
    onSuccess: invalidate,
  })
}

/** As versões de um save (o histórico). */
export function useVersions(key: string | null) {
  return useQuery({
    queryKey: [...SYNC_VERSIONS_KEY, key],
    queryFn: () => syncVersions(key as string),
    enabled: key !== null,
    retry: false,
  })
}

/** Volta uma versão do histórico e relê o que a página mostra. */
export function useRestore() {
  const invalidate = useInvalidateSave()
  return useMutation({
    mutationFn: ({ key, versionId }: { key: string; versionId: string }) => syncRestore(key, versionId),
    onSuccess: invalidate,
  })
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
      // Uma passada de states terminou: a nota de cada vaga pode ter mudado.
      if (e.type === 'status') void qc.invalidateQueries({ queryKey: SYNC_STATE_NOTES_KEY })
      // Uma passada mexeu nos saves: a linha da página do jogo e os conflitos
      // podem ter mudado.
      if (e.type === 'pass' || e.type === 'conflict') {
        void qc.invalidateQueries({ queryKey: SYNC_SAVE_LINE_KEY })
        void qc.invalidateQueries({ queryKey: SYNC_CONFLICTS_KEY })
      }
      if (e.type === 'pass' && e.uploaded > 0) {
        useToastStore.getState().push(sysToast(i18n.t('sync.toast.uploaded', { count: e.uploaded }), 'Success'))
      }
      // S13: um save apagado no site saiu desta máquina (para o resguardo).
      if (e.type === 'pass' && e.backedUp > 0) {
        useToastStore.getState().push(sysToast(i18n.t('sync.toast.removed', { count: e.backedUp })))
      }
      // D117: a nuvem não respondeu a tempo antes de abrir o jogo.
      if (e.type === 'prelaunch' && e.outcome === 'skipped' && (e.reason === 'timeout' || e.reason === 'offline')) {
        useToastStore.getState().push(sysToast(i18n.t('sync.toast.cloudUnchecked'), 'Warning'))
      }
      // O que a checagem trouxe ou achou aparece na página do jogo.
      if (e.type === 'prelaunch' && (e.outcome === 'downloaded' || e.outcome === 'conflict')) {
        void qc.invalidateQueries({ queryKey: SYNC_SAVE_LINE_KEY })
        void qc.invalidateQueries({ queryKey: SYNC_CONFLICTS_KEY })
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

/**
 * Os save states das outras máquinas para um jogo (D116). Só consulta com o
 * sync ligado e o interruptor dos states aceso, e nunca segura a página: sem
 * dados, a lista simplesmente não aparece. `coreId` nulo: o core aberto (menu
 * de pausa).
 */
export function useRemoteStates(romId: string, coreId: string | null, enabled = true) {
  const status = useQuery({ queryKey: SYNC_STATUS_KEY, queryFn: syncStatus, retry: false, staleTime: 60_000 })
  const st = status.data
  // Pausada aqui ou no site (D128): nada vai à rede.
  const on =
    enabled && st?.enabled === true && !st.revoked && !st.pausedLocal && !st.pausedSite && st.statesEnabled && romId !== ''
  const q = useQuery({
    queryKey: [...SYNC_REMOTE_STATES_KEY, romId, coreId],
    queryFn: () => syncStatesFor(romId, coreId),
    enabled: on,
    retry: false,
    staleTime: 60_000,
  })
  // Desligado depois de lido: o que ficou no cache não aparece.
  return { ...q, data: on ? q.data : undefined }
}

/**
 * Por que cada state local de um jogo fica só nesta máquina, pelo id do state
 * (a vaga diz: D116, D122). Só com o sync ligado e os states acesos; sem rede.
 * `on` diz se a nota vale (com os states desligados nada fica "só aqui").
 */
export function useStateNotes(romId: string) {
  const status = useQuery({ queryKey: SYNC_STATUS_KEY, queryFn: syncStatus, retry: false, staleTime: 60_000 })
  const st = status.data
  const on = st?.enabled === true && !st.revoked && st.statesEnabled && romId !== ''
  const q = useQuery({
    queryKey: [...SYNC_STATE_NOTES_KEY, romId],
    queryFn: () => syncStateNotes(romId),
    enabled: on,
    retry: false,
  })
  const notes = on ? q.data : undefined
  return { on, reasonOf: (stateId: string) => notes?.find((n) => n.stateId === stateId)?.reason ?? null }
}
