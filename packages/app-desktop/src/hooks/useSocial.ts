import { useQuery } from '@tanstack/react-query'
import { useCallback, useEffect, useState } from 'react'
import { alvanistaAchievements, alvanistaFriends } from '../lib/tauri'

export const ALVANISTA_FRIENDS_KEY = ['alvanista-friends'] as const
export const ALVANISTA_ACHIEVEMENTS_KEY = ['alvanista-achievements'] as const

const SHOW_ONLINE_STORAGE_KEY = 'reemu.network.showOnline'
const FRIENDS_ONLINE_POLL_MS = 60_000

function isDocumentForeground(): boolean {
  if (typeof document === 'undefined') return true
  const visible = document.visibilityState === 'visible'
  const focused = typeof document.hasFocus === 'function' ? document.hasFocus() : true
  return visible && focused
}

function useDocumentForeground(active: boolean): boolean {
  const [foreground, setForeground] = useState<boolean>(() => isDocumentForeground())

  useEffect(() => {
    if (!active || typeof window === 'undefined' || typeof document === 'undefined') {
      return
    }
    const update = () => setForeground(isDocumentForeground())
    update()
    document.addEventListener('visibilitychange', update)
    window.addEventListener('focus', update)
    window.addEventListener('blur', update)
    return () => {
      document.removeEventListener('visibilitychange', update)
      window.removeEventListener('focus', update)
      window.removeEventListener('blur', update)
    }
  }, [active])

  return foreground
}

export function socialErrorCode(err: unknown): string {
  if (typeof err === 'string') return err
  if (err instanceof Error) return err.message
  return 'failed'
}

export function useAlvanistaFriends(
  enabled: boolean,
  username?: string | null,
  connectedAt?: number | null,
  pollOnline = false,
) {
  const foreground = useDocumentForeground(enabled && pollOnline)
  return useQuery({
    queryKey: [...ALVANISTA_FRIENDS_KEY, username ?? null, connectedAt ?? null],
    queryFn: alvanistaFriends,
    enabled,
    retry: false,
    staleTime: 60_000,
    refetchInterval:
      enabled && pollOnline && foreground
        ? () => (isDocumentForeground() ? FRIENDS_ONLINE_POLL_MS : false)
        : false,
    refetchIntervalInBackground: false,
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
