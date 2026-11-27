import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  alvanistaCollectionStatus,
  alvanistaCollectionUpdate,
  type CollectionItem,
  type CollectionPatch,
} from '../lib/tauri'

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

export function useUpdateCollectionItem(gameUuid: string, systemId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (patch: Omit<CollectionPatch, 'system'>) =>
      alvanistaCollectionUpdate(gameUuid, { ...patch, system: systemId }),
    onSuccess: (next) => {
      qc.setQueryData(collectionItemKey(gameUuid), next)
    },
  })
}
