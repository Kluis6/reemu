import { useQuery } from '@tanstack/react-query'
import { useCallback, useState } from 'react'
import { alvanistaAchievements, alvanistaFriends } from '../lib/tauri'

export const ALVANISTA_FRIENDS_KEY = ['alvanista-friends'] as const
export const ALVANISTA_ACHIEVEMENTS_KEY = ['alvanista-achievements'] as const

const SHOW_ONLINE_STORAGE_KEY = 'reemu.network.showOnline'

export function socialErrorCode(err: unknown): string {
  if (typeof err === 'string') return err
  if (err instanceof Error) return err.message
  return 'failed'
}

export function useAlvanistaFriends(enabled: boolean) {
  return useQuery({
    queryKey: ALVANISTA_FRIENDS_KEY,
    queryFn: alvanistaFriends,
    enabled,
    retry: false,
    staleTime: 60_000,
  })
}

export function useAlvanistaAchievements(enabled: boolean) {
  return useQuery({
    queryKey: ALVANISTA_ACHIEVEMENTS_KEY,
    queryFn: alvanistaAchievements,
    enabled,
    retry: false,
    staleTime: 2 * 60_000,
  })
}

/** Escolha local do usuário para mostrar quem está online e o jogo atual (desligada por padrão, D169). */
export function useNetworkShowOnline(): [boolean, (next: boolean) => void] {
  const [showOnline, setShowOnlineState] = useState<boolean>(() => {
    try {
      return window.localStorage.getItem(SHOW_ONLINE_STORAGE_KEY) === '1'
    } catch {
      return false
    }
  })

  const setShowOnline = useCallback((next: boolean) => {
    setShowOnlineState(next)
    try {
      if (next) window.localStorage.setItem(SHOW_ONLINE_STORAGE_KEY, '1')
      else window.localStorage.removeItem(SHOW_ONLINE_STORAGE_KEY)
    } catch {
      // ignora ambiente sem localStorage
    }
  }, [])

  return [showOnline, setShowOnline]
}
