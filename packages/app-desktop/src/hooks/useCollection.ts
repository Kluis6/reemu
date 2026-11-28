import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  alvanistaCachedStatus,
  alvanistaCollectionStatus,
  alvanistaCollectionUpdate,
  type CollectionItem,
  type CollectionPatch,
} from '../lib/tauri'
import { ALVANISTA_CACHED_KEY } from './useEffectiveProfile'

export const collectionItemKey = (gameUuid: string) =>
  ['alvanista-collection', gameUuid] as const

export function collectionErrorCode(err: unknown): string {
  if (typeof err === 'string') return err
  if (err instanceof Error) return err.message
  return 'failed'
}

export function useCollectionItem(gameUuid: string | null | undefined, enabled: boolean) {
  return useQuery({
    queryKey: collectionItemKey(gameUuid ?? ''),
    queryFn: async (): Promise<CollectionItem | null> => {
      if (!gameUuid) return null
      const items = await alvanistaCollectionStatus([gameUuid])
      return items[0] ?? null
    },
    enabled: Boolean(gameUuid) && enabled,
    retry: false,
  })
}

export function useAlvanistaCollectionItem(gameUuid: string | null | undefined) {
  const account = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
  })
  const connected = Boolean(account.data?.connected && !account.data?.ended)
  const hasScope = Boolean(account.data?.scopes?.includes('collection'))
  const itemQuery = useCollectionItem(gameUuid, connected && hasScope)
  const errorCode = itemQuery.isError ? collectionErrorCode(itemQuery.error) : null
  const needsAccess = !account.isPending && (!connected || !hasScope || errorCode === 'needs_consent')
  return { accountPending: account.isPending, connected, hasScope, itemQuery, errorCode, needsAccess }
}

export function useUpdateCollectionItem(gameUuid: string, systemId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (patch: Omit<CollectionPatch, 'system'>) =>
      alvanistaCollectionUpdate(gameUuid, { ...patch, system: systemId }),
    onSuccess: (next) => {
      qc.setQueryData(collectionItemKey(gameUuid), next)
      void qc.invalidateQueries({ queryKey: ['alvanista-collection', 'profile-library'] })
    },
  })
}
