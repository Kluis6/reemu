import { makeStyles, mergeClasses } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { LoadingState } from '../components/EmptyState'
import { PAGE_PAD_B, PAGE_PAD_L, PAGE_TOP } from '../styles/metrics'
import { ALVANISTA_STATUS_KEY, DISCONNECTED, useAlvanistaStatusSync } from '../hooks/useAlvanistaStatusSync'
import { ALVANISTA_CACHED_KEY, useEffectiveProfile, type EffectiveProfile } from '../hooks/useEffectiveProfile'
import { RA_STATUS_KEY } from '../hooks/useRetroAchievements'
import {
  alvanistaCachedStatus,
  alvanistaStatus,
  getProfile,
  listRecentSaveStates,
  listRoms,
  raStatus,
} from '../lib/tauri'
import { AlvanistaInvite } from './profile/AlvanistaCardBlock'
import { AlvanistaGalleries } from './profile/AlvanistaGalleries'
import { MomentsRow } from './profile/MomentsRow'
import { ProfileHero } from './profile/ProfileHero'
import { mostPlayed, recentlyPlayed } from './profile/profileData'
import { MostPlayedCard, ProfileAchievements, RecentRow, ReemuNumbers } from './profile/ReemuSections'

const useStyles = makeStyles({
  page: {
    position: 'relative',
    isolation: 'isolate',
    width: '100%',
    minHeight: `calc(100vh - var(--reemuPageTop, ${PAGE_TOP}px) - var(--reemuScrollPadB, ${PAGE_PAD_B}px) - var(--reemuInsetY, 0px))`,
    '&::before': {
      content: '""',
      position: 'absolute',
      inset: `calc(-1 * var(--reemuPageTop, ${PAGE_TOP}px)) calc(-1 * var(--reemuScrollPadR, ${PAGE_PAD_L}px) - var(--reemuInsetX, 0px)) 0 -${PAGE_PAD_L}px`,
      zIndex: -2,
      pointerEvents: 'none',
      backgroundImage: 'linear-gradient(var(--reemuPage), var(--reemuPage) 78%, transparent)',
    },
  },
  cols: {
    display: 'grid',
    gridTemplateColumns: 'minmax(0, 1fr) minmax(220px, 28%)',
    columnGap: 'clamp(28px, 5vw, 82px)',
    rowGap: '32px',
    marginTop: 'clamp(28px, 5vw, 72px)',
    '@media (max-width: 840px)': { gridTemplateColumns: 'minmax(0, 1fr)' },
  },
  single: { gridTemplateColumns: 'minmax(0, 1fr)' },
  column: {
    minWidth: 0,
    display: 'flex',
    flexDirection: 'column',
    gap: '32px',
  },
})

/**
 * Meu perfil (`/perfil`): identidade, atividade local, status das conquistas,
 * jogos recentes e dados da conta Alvanista, quando conectada.
 */
export function ProfilePage() {
  const { t } = useTranslation()
  const s = useStyles()
  const effective = useEffectiveProfile()
  // Mesma chave do `useEffectiveProfile`: só para saber se a leitura falhou.
  const local = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })
  const achievements = useQuery({ queryKey: RA_STATUS_KEY, queryFn: raStatus, retry: false })
  // staleTime 0: leituras baratas do SQLite, relidas a cada vez que a página abre
  // (jogar e salvar não invalidam estas chaves).
  const roms = useQuery({ queryKey: ['roms'], queryFn: listRoms, retry: false, staleTime: 0 })
  const saves = useQuery({
    queryKey: ['recent-save-states', 8],
    queryFn: () => listRecentSaveStates(8),
    retry: false,
    staleTime: 0,
  })
  // Primeiro o guardado (instantâneo), depois o completo, como no SettingsProfile.
  const cached = useQuery({ queryKey: ALVANISTA_CACHED_KEY, queryFn: alvanistaCachedStatus, retry: false })
  const full = useQuery({ queryKey: ALVANISTA_STATUS_KEY, queryFn: alvanistaStatus, retry: false })
  const status = full.data ?? cached.data
  useAlvanistaStatusSync(status ?? DISCONNECTED, full.data, cached.data)
  // Sem perfil local (a leitura falhou, sem banco por exemplo) e sem a conta
  // do Alvanista: um topo mínimo, como o do drawer, em vez de girar para sempre.
  const profile: EffectiveProfile | undefined =
    effective ??
    (local.isError && !cached.isPending
      ? {
          name: t('profilePage.defaultName'),
          avatar: 'preset:1',
          avatarUrl: null,
          bio: null,
          bannerUrl: null,
          source: 'local',
          tierColors: null,
        }
      : undefined)

  const all = useMemo(() => roms.data ?? [], [roms.data])
  const recent = useMemo(() => recentlyPlayed(all), [all])
  const most = useMemo(() => mostPlayed(all, 5), [all])

  if (!profile || roms.isPending) return <LoadingState />
  // Pelo status, não pelo perfil: ao desconectar o status zera na hora.
  const connected = !!status?.connected && !status.ended
  const card = connected ? status?.card : null
  const scores = achievements.data?.connected ? achievements.data : undefined
  const showInvite = !connected && !cached.isError
  const hasSidebar = recent.length > 0 || connected || showInvite

  return (
    <div className={s.page}>
      <ProfileHero
        profile={profile}
        status={status}
        onRefresh={() => void full.refetch()}
        refreshing={full.isFetching}
      />
      <div className={mergeClasses(s.cols, !hasSidebar && s.single)}>
        <div className={s.column}>
          <ProfileAchievements trophies={card?.proofs?.trophies?.count} scores={scores} />
          {most.length > 0 && <MostPlayedCard roms={most} achievementsConnected={scores !== undefined} />}
          <ReemuNumbers roms={all} failed={roms.isError} />
        </div>
        {hasSidebar && (
          <div className={s.column} data-testid="profile-sidebar">
            <RecentRow roms={recent} />
            {connected && <AlvanistaGalleries key={status.username} status={status} roms={all} />}
            {showInvite && <AlvanistaInvite />}
          </div>
        )}
      </div>
      <MomentsRow states={saves.data ?? []} />
    </div>
  )
}
