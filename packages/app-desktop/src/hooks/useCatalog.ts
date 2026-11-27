import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import {
  catalogRunNow,
  catalogSetEnabled,
  catalogStatus,
  onCatalogEvent,
  type CatalogStatus,
} from '../lib/tauri'

export const CATALOG_STATUS_KEY = ['catalog-status'] as const

/** Estado do catálogo do Alvanista (conexão, chave ligada e contagens). */
export function useCatalogStatus() {
  return useQuery({
    queryKey: CATALOG_STATUS_KEY,
    queryFn: catalogStatus,
    retry: false,
  })
}

/** Liga ou desliga o catálogo em Configurações › Metadados. */
export function useSetCatalogEnabled() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (enabled: boolean) => catalogSetEnabled(enabled),
    onSuccess: (next: CatalogStatus) => {
      qc.setQueryData(CATALOG_STATUS_KEY, next)
    },
  })
}

/** Roda uma passada imediata do catálogo do Alvanista sob demanda. */
export function useRunCatalogNow() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () => catalogRunNow(),
    onSuccess: (next: CatalogStatus) => {
      qc.setQueryData(CATALOG_STATUS_KEY, next)
      void qc.invalidateQueries({ queryKey: ['pending-matches'] })
      void qc.invalidateQueries({ queryKey: ['roms'] })
      void qc.invalidateQueries({ queryKey: ['rom-metadata'] })
    },
  })
}

/** Escuta `catalog` uma única vez no `AppShell` e atualiza grade, página do jogo e contagens. */
export function useCatalogEvents() {
  const qc = useQueryClient()
  useEffect(() => {
    let disposed = false
    let dispose: (() => void) | undefined
    void onCatalogEvent((ev) => {
      void qc.invalidateQueries({ queryKey: CATALOG_STATUS_KEY })
      if (ev.type === 'pass') {
        void qc.invalidateQueries({ queryKey: ['roms'] })
        void qc.invalidateQueries({ queryKey: ['rom-metadata'] })
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

