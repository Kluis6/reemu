import { useQueryClient } from '@tanstack/react-query'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import {
  onRaAchievementUnlocked,
  onRaGameLoaded,
  onRaLeaderboardEvent,
  onRaLiveUpdated,
  onRaStatusChanged,
} from '../lib/tauri'
import { sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

export const RA_STATUS_KEY = ['ra-status'] as const
export const raGameKey = (romId: string) => ['ra-game', romId] as const

/**
 * Escuta eventos globais do RetroAchievements vindos do Rust:
 * - login, logout, preferências ou sync de credenciais atualizam `['ra-status']`;
 * - jogo reconhecido no core avisa quantas conquistas estão ativas;
 * - conquista desbloqueada, placares e indicadores atualizam o cache e exibem avisos sem roubar foco do jogo.
 */
export function useRaEvents() {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const update = useToastStore((s) => s.update)

  useEffect(() => {
    let alive = true
    const offs: (() => void)[] = []

    const register = (p: Promise<() => void>) => {
      void p.then((off) => {
        if (alive) offs.push(off)
        else off()
      })
    }

    register(
      onRaStatusChanged((st) => {
        qc.setQueryData(RA_STATUS_KEY, st)
        void qc.invalidateQueries({ queryKey: ['ra-game'] })
      }),
    )

    register(
      onRaGameLoaded((ev) => {
        if (ev.romId) {
          void qc.invalidateQueries({ queryKey: raGameKey(ev.romId) })
        }
        if (ev.totalAchievements > 0) {
          const key = ev.hardcore
            ? 'achievements.toastLoadedHardcore'
            : 'achievements.toastLoaded'
          push(
            sysToast(
              t(key, {
                title: ev.title,
                unlocked: ev.unlockedAchievements,
                total: ev.totalAchievements,
              }),
              'Info',
            ),
          )
        }
      }),
    )

    register(
      onRaLiveUpdated((ev) => {
        if (ev.romId) {
          void qc.invalidateQueries({ queryKey: raGameKey(ev.romId) })
        }
      }),
    )

    register(
      onRaLeaderboardEvent((ev) => {
        if (ev.romId) {
          void qc.invalidateQueries({ queryKey: raGameKey(ev.romId) })
        }
        if (ev.kind === 'started') {
          const detail = ev.formattedScore
            ? `${ev.title} · ${ev.formattedScore}`
            : ev.title
          push(
            sysToast(
              t('achievements.toastLeaderboardStarted', { title: detail }),
              'Info',
            ),
          )
          return
        }
        if (ev.kind === 'failed') {
          push(
            sysToast(
              t('achievements.toastLeaderboardFailed', { title: ev.title }),
              'Warning',
            ),
          )
          return
        }
        if (ev.kind === 'submitted') {
          const score = ev.formattedScore ?? ''
          const toastId = `ra-lb-submit-${ev.id}`
          const title =
            ev.rank != null && ev.totalEntries != null
              ? t('achievements.toastLeaderboardSubmittedRank', {
                  score,
                  rank: ev.rank,
                  total: ev.totalEntries,
                })
              : !ev.hardcore
                ? t('achievements.toastLeaderboardSubmittedSoftcore', { score })
                : t('achievements.toastLeaderboardSubmitted', { score })
          const message = ev.bestScore
            ? t('achievements.toastLeaderboardBestScore', {
                title: ev.title,
                best: ev.bestScore,
              })
            : ev.title
          const variant = ev.hardcore ? 'Success' : 'Info'
          const exists = useToastStore.getState().queue.some((item) => item.id === toastId)
          if (exists) {
            update(toastId, { title, message, variant })
          } else {
            push({
              id: toastId,
              title,
              message,
              variant,
              durationMs: 5500,
              source: 'System',
            })
          }
        }
      }),
    )

    register(
      onRaAchievementUnlocked((ev) => {
        if (ev.romId) {
          void qc.invalidateQueries({ queryKey: raGameKey(ev.romId) })
        }
        void qc.invalidateQueries({ queryKey: RA_STATUS_KEY })
        if (ev.gameCompleted) {
          push({
            id: crypto.randomUUID(),
            title: t('achievements.toastMastered'),
            message: ev.title,
            variant: 'Success',
            durationMs: 6000,
            source: 'System',
          })
          return
        }
        const titleKey = ev.hardcore
          ? 'achievements.toastUnlockedHardcore'
          : 'achievements.toastUnlocked'
        push({
          id: crypto.randomUUID(),
          title: t(titleKey, { points: ev.points }),
          message: ev.description ? `${ev.title} · ${ev.description}` : ev.title,
          variant: 'Success',
          durationMs: 5500,
          source: 'System',
        })
      }),
    )

    return () => {
      alive = false
      for (const off of offs) off()
    }
  }, [push, qc, t, update])
}
