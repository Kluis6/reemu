import { makeStyles, mergeClasses } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useMemo, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { LoadingState } from '../components/EmptyState'
import { ALVANISTA_STATUS_KEY, DISCONNECTED, useAlvanistaStatusSync } from '../hooks/useAlvanistaStatusSync'
import { ALVANISTA_CACHED_KEY, useEffectiveProfile, type EffectiveProfile } from '../hooks/useEffectiveProfile'
import {
  alvanistaCachedStatus,
  alvanistaStatus,
  getProfile,
  listRecentSaveStates,
  listRoms,
} from '../lib/tauri'
import { AlvanistaCardBlock, AlvanistaInvite } from './profile/AlvanistaCardBlock'
import { ConnectionCard } from './profile/ConnectionCard'
import { MomentsRow } from './profile/MomentsRow'
import { ProfileHero } from './profile/ProfileHero'
import { hasCardContent, mostPlayed, recentlyPlayed } from './profile/profileData'
import { MostPlayedCard, RecentRow, ReemuNumbers } from './profile/ReemuSections'

const useStyles = makeStyles({
  page: { width: '100%', paddingBottom: '40px' },
  cols: {
    display: 'grid',
    gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)',
    gap: '14px',
    marginTop: '26px',
    '@media (max-width: 860px)': { gridTemplateColumns: '1fr' },
  },
  // Um lado só: ocupa a linha inteira, sem deixar meia largura vazia.
  single: { gridTemplateColumns: 'minmax(0, 1fr)' },
})

/**
 * Meu perfil (`/perfil`), tela cheia na área de conteúdo da casca: o topo,
 * "No ReEmu", "Jogados recentemente", "Mais jogados no ReEmu" e "Seus
 * momentos", "No Alvanista" (ou o convite) e "Conexão com o Alvanista". Só lê a
 * biblioteca, os saves, o perfil local e o status do Alvanista (guardado e completo); sem sessão não toca no chaveiro nem na rede.
 */
export function ProfilePage() {
  const { t } = useTranslation()
  const s = useStyles()
  const effective = useEffectiveProfile()
  // Mesma chave do `useEffectiveProfile`: só para saber se a leitura falhou.
  const local = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })
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
        }
      : undefined)

  const all = useMemo(() => roms.data ?? [], [roms.data])
  const recent = useMemo(() => recentlyPlayed(all, 30), [all])
  const most = useMemo(() => mostPlayed(all, 5), [all])

  if (!profile || roms.isPending) return <LoadingState />
  // Pelo status, não pelo perfil: ao desconectar o status zera na hora.
  const connected = !!status?.connected
  const card = connected ? status?.card : null
  const fetchedAt = status?.fetched_at ?? null
  // A linha de duas colunas só existe com algo dentro; com um lado só, ele
  // ocupa a linha toda.
  const mostEl = most.length > 0 ? <MostPlayedCard roms={most} /> : null
  let alvaEl: ReactNode = null
  if (connected) {
    if (card && hasCardContent(card, fetchedAt)) alvaEl = <AlvanistaCardBlock card={card} fetchedAt={fetchedAt} />
  } else if (!cached.isError) {
    // Sem banco o Alvanista não tem onde guardar a sessão: sem convite.
    alvaEl = <AlvanistaInvite />
  }
  const sides = [mostEl, alvaEl].filter(Boolean).length

  return (
    <div className={s.page}>
      <ProfileHero
        profile={profile}
        status={status}
        onRefresh={() => void full.refetch()}
        refreshing={full.isFetching}
      />
      <ReemuNumbers roms={all} failed={roms.isError} />
      <RecentRow roms={recent} />
      {sides > 0 && (
        <div className={mergeClasses(s.cols, sides === 1 && s.single)} data-testid="profile-cols" data-sides={sides}>
          {mostEl}
          {alvaEl}
        </div>
      )}
      <MomentsRow states={saves.data ?? []} />
      {connected && status && <ConnectionCard status={status} />}
    </div>
  )
}
