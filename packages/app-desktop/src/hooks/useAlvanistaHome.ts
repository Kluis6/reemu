import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import {
  alvanistaHomeHighlights,
  alvanistaSetHomeHighlightsEnabled,
  onAlvanistaHome,
  type AlvanistaHomeView,
} from '../lib/tauri'

export const ALVANISTA_HOME_KEY = ['alvanista-home'] as const

/** Destaques do Alvanista para a tela inicial (preferência + até 2 novidades). */
export function useAlvanistaHome(lang: string) {
  return useQuery({
    queryKey: [...ALVANISTA_HOME_KEY, lang] as const,
    queryFn: () => alvanistaHomeHighlights(lang),
    retry: false,
    staleTime: 5 * 60 * 1000,
  })
}

/** Liga ou desliga os destaques do Alvanista em Configurações > Perfil. */
export function useSetAlvanistaHomeEnabled() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (enabled: boolean) => alvanistaSetHomeHighlightsEnabled(enabled),
    onSuccess: (_, enabled) => {
      qc.setQueriesData<AlvanistaHomeView>({ queryKey: ALVANISTA_HOME_KEY }, (prev) =>
        prev ? { ...prev, enabled, items: enabled ? prev.items : [] } : prev,
      )
      void qc.invalidateQueries({ queryKey: ALVANISTA_HOME_KEY })
    },
  })
}

/** Atualiza o cache da interface quando a abertura do app renova as novidades. */
export function useAlvanistaHomeEvents() {
  const qc = useQueryClient()
  useEffect(() => {
    let disposed = false
    let dispose: (() => void) | undefined
    void onAlvanistaHome(() => {
      void qc.invalidateQueries({ queryKey: ALVANISTA_HOME_KEY })
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
