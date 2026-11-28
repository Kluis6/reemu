import { Button, makeStyles, tokens } from '@fluentui/react-components'
import { useInfiniteQuery } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { GameCard } from '../../components/GameCard'
import { useAlvanistaConnect } from '../../hooks/useAlvanistaConnect'
import { collectionErrorCode } from '../../hooks/useCollection'
import { socialErrorCode, useAlvanistaFriends } from '../../hooks/useSocial'
import { platformLabel } from '../../lib/platform'
import {
  alvanistaCollectionStatus, alvanistaOpenGame, alvanistaOpenSite, alvanistaOpenUser,
  alvanistaProfileCheckins, type AlvanistaStatus, type RomEntry,
} from '../../lib/tauri'
import { BadgeIcon } from './AlvanistaCardBlock'
import { ProfileGallery } from './ProfileGallery'
import { pickText } from './profileData'

const COLLECTION_BATCH_SIZE = 200
const GAME_UUID = /^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i

const useStyles = makeStyles({
  badge: {
    display: 'grid',
    justifyItems: 'center',
    alignContent: 'start',
    gap: '6px',
    margin: 0,
    minWidth: 0,
    textAlign: 'center',
    fontSize: tokens.fontSizeBase200,
  },
  icon: { width: '82%', aspectRatio: '1', objectFit: 'contain' },
  galleryActions: { display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '8px' },
})

function GalleryConsent({ scope, label }: { scope: string; label: string }) {
  const { t } = useTranslation()
  const { connect, cancel, connecting, outcome } = useAlvanistaConnect({ scopes: [scope] })
  return (
    <>
      <Button appearance="primary" onClick={connecting ? cancel : connect}>
        {connecting ? t('common.cancel') : label}
      </Button>
      {connecting && <p role="status">{t('alvanista.waiting')}</p>}
      {outcome && <p role="status">{t(`alvanista.${outcome}`)}</p>}
    </>
  )
}

function ProfileGameGalleries({ status, roms }: { status: AlvanistaStatus; roms: RomEntry[] }) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  const [open, setOpen] = useState<'finished' | 'collection' | 'checkins' | null>(null)
  const hasScope = status.scopes?.includes('collection') === true
  const { linked, ids } = useMemo(() => {
    const linked = new Map<string, RomEntry>()
    const ordered = [...roms].sort((a, b) => (b.lastPlayedAt ?? 0) - (a.lastPlayedAt ?? 0))
    for (const rom of ordered) {
      const id = rom.gameUuid?.toLowerCase()
      if (id && GAME_UUID.test(id) && !linked.has(id)) linked.set(id, rom)
    }
    return { linked, ids: [...linked.keys()] }
  }, [roms])
  const collection = useInfiniteQuery({
    queryKey: ['alvanista-collection', 'profile-library', status.username, status.connected_at, ids],
    enabled: hasScope && ids.length > 0,
    initialPageParam: 0,
    queryFn: ({ pageParam }) => alvanistaCollectionStatus(ids.slice(pageParam, pageParam + COLLECTION_BATCH_SIZE)),
    getNextPageParam: (_, __, offset) => {
      const next = offset + COLLECTION_BATCH_SIZE
      return next < ids.length ? next : undefined
    },
    staleTime: 5 * 60 * 1000,
    refetchOnWindowFocus: false,
    retry: false,
  })
  const checkins = useInfiniteQuery({
    queryKey: ['alvanista-profile-checkins', status.username, status.connected_at],
    enabled: Boolean(status.username),
    initialPageParam: 1,
    queryFn: ({ pageParam }) => alvanistaProfileCheckins(pageParam),
    getNextPageParam: (page) => page.nextPage ?? undefined,
    staleTime: 2 * 60 * 1000,
    refetchOnWindowFocus: false,
    retry: false,
  })
  const { fetchNextPage: nextCollection, hasNextPage: hasMoreCollection, isFetching: fetchingCollection, isError: collectionFailed } = collection
  const { fetchNextPage: nextCheckins, hasNextPage: hasMoreCheckins, isFetching: fetchingCheckins, isError: checkinsFailed } = checkins
  useEffect(() => {
    if ((open === 'collection' || open === 'finished') && hasScope && hasMoreCollection && !fetchingCollection && !collectionFailed) {
      void nextCollection({ cancelRefetch: false })
    }
  }, [open, hasScope, hasMoreCollection, fetchingCollection, collectionFailed, nextCollection])
  useEffect(() => {
    if ((open === 'checkins' || open === 'finished') && hasMoreCheckins && !fetchingCheckins && !checkinsFailed) {
      void nextCheckins({ cancelRefetch: false })
    }
  }, [open, hasMoreCheckins, fetchingCheckins, checkinsFailed, nextCheckins])

  const checkinRows = [...new Map((checkins.data?.pages ?? []).flatMap((page) => page.items)
    .map((entry) => [entry.id, entry])).values()]
  const finished = new Map<string, { title: string; cover: string | null }>()
  const owned: RomEntry[] = []
  for (const entry of checkinRows) {
    const id = entry.gameId?.toLowerCase()
    if (id && (entry.status === 'finished' || entry.status === 'completed') && entry.title && !finished.has(id)) {
      const rom = linked.get(id)
      finished.set(id, { title: rom?.title ?? entry.title, cover: rom?.boxart ?? entry.coverUrl })
    }
  }
  for (const item of (hasScope ? collection.data?.pages ?? [] : []).flat()) {
    const id = item.gameId.toLowerCase()
    const rom = linked.get(id)
    if (!rom) continue
    if (item.inCollection) owned.push(rom)
    if ((item.status === 'finished' || item.status === 'completed') && !finished.has(id)) {
      finished.set(id, { title: rom.title, cover: rom.boxart })
    }
  }

  const openGame = (id: string | null) => {
    if (id && GAME_UUID.test(id)) void alvanistaOpenGame(id).catch(() => {})
    else void alvanistaOpenSite('checkins').catch(() => {})
  }
  const collectionState = !hasScope || (collectionFailed && collectionErrorCode(collection.error) === 'needs_consent') ? (
    <GalleryConsent scope="collection" label={t('collection.connectButton')} />
  ) : collectionFailed ? (
    <Button onClick={() => void (collection.isFetchNextPageError ? nextCollection() : collection.refetch())}>
      {t('common.retry')}
    </Button>
  ) : (hasMoreCollection || fetchingCollection) ? (
    <span role="status">{t('common.loading')}</span>
  ) : undefined
  const checkinsState = checkinsFailed ? (
    <Button onClick={() => void (checkins.isFetchNextPageError ? nextCheckins() : checkins.refetch())}>
      {t('common.retry')}
    </Button>
  ) : (hasMoreCheckins || fetchingCheckins) ? (
    <span role="status">{t('common.loading')}</span>
  ) : undefined
  const noEntries = <span>{t('profilePage.noAlvanistaEntries')}</span>

  return (
    <>
      <ProfileGallery
        title={t('profilePage.finishedTitle')}
        testId="profile-alvanista-finished"
        description={t('profilePage.finishedSources')}
        footer={<div className={s.galleryActions}>
          {collectionState}{checkinsState}
          {!collectionState && !checkinsState && !finished.size && noEntries}
        </div>}
        onOpenChange={(active) => setOpen(active ? 'finished' : null)}
        items={[...finished].map(([id, game]) => (
          <GameCard key={id} title={game.title} boxart={game.cover} onClick={() => {
            const rom = linked.get(id)
            if (rom) navigate(`/rom/${rom.id}`)
            else openGame(id)
          }} />
        ))}
      />
      <ProfileGallery
        title={t('profilePage.collectionTitle')}
        testId="profile-alvanista-collection"
        description={t('profilePage.collectionLibraryScope')}
        footer={
          <div className={s.galleryActions}>
            <Button onClick={() => void alvanistaOpenSite('collection').catch(() => {})}>
              {t('profilePage.openAlvanistaCollection')}
            </Button>
            {collectionState ?? (!owned.length && noEntries)}
          </div>
        }
        onOpenChange={(active) => setOpen(active ? 'collection' : null)}
        items={owned.map((rom) => (
          <GameCard
            key={rom.id}
            title={rom.title}
            badge={platformLabel(rom.systemId)}
            boxart={rom.boxart}
            onClick={() => navigate(`/rom/${rom.id}`)}
          />
        ))}
      />
      <ProfileGallery
        title={t('profilePage.checkinsTitle')}
        testId="profile-alvanista-checkins"
        description={t('profilePage.publicCheckinsHint')}
        footer={
          <div className={s.galleryActions}>
            <Button onClick={() => void alvanistaOpenSite('checkins').catch(() => {})}>
              {t('profilePage.openAlvanistaCheckins')}
            </Button>
            {checkinsState ?? (!checkinRows.length && noEntries)}
          </div>
        }
        onOpenChange={(active) => setOpen(active ? 'checkins' : null)}
        items={checkinRows.map((entry) => {
          const time = Date.parse(entry.createdAt ?? '')
          return (
            <GameCard
              key={entry.id}
              title={entry.title || t('profilePage.checkin')}
              badge={Number.isFinite(time) ? new Date(time).toLocaleDateString(i18n.language) : undefined}
              boxart={entry.coverUrl}
              onClick={() => openGame(entry.gameId)}
            />
          )
        })}
      />
    </>
  )
}

export function AlvanistaGalleries({ status, roms }: { status: AlvanistaStatus; roms: RomEntry[] }) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const friendsAllowed = status.scopes?.includes('friends:read') === true
  const friends = useAlvanistaFriends(friendsAllowed, status.username, status.connected_at)
  const friendRows = friendsAllowed ? friends.data ?? [] : []
  const badges = status.card?.badges
  return (
    <>
      {badges?.recent?.length ? (
        <ProfileGallery
          title={t('profilePage.badgesTitle')}
          testId="profile-alvanista-badges"
          description={badges.count > badges.recent.length
            ? t('profilePage.recentBadgesHint', { count: badges.count })
            : undefined}
          items={badges.recent.map((badge, index) => (
            <figure key={`${badge.name}-${index}`} className={s.badge}>
              <BadgeIcon badge={badge} className={s.icon} />
              <figcaption>{pickText(badge.names, i18n.language) ?? badge.name}</figcaption>
            </figure>
          ))}
        />
      ) : null}
      <ProfileGameGalleries status={status} roms={roms} />
      {(friendRows.length > 0 || (status.card?.ledger?.gamers ?? 0) > 0) && (
        <ProfileGallery
          title={t('profilePage.gamersTitle')}
          testId="profile-alvanista-gamers"
          footer={!friendsAllowed || (friends.isError && socialErrorCode(friends.error) === 'needs_consent') ? (
            <GalleryConsent scope="friends:read" label={t('network.connectButton')} />
          ) : friends.isError ? (
            <Button onClick={() => void friends.refetch()}>{t('common.retry')}</Button>
          ) : friends.isPending ? (
            <span role="status">{t('common.loading')}</span>
          ) : friendRows.length === 0 ? (
            <span>{t('network.emptyHint')}</span>
          ) : undefined}
          items={friendRows.map((friend) => (
            <GameCard
              key={friend.id}
              title={friend.username}
              boxart={friend.avatarUrl}
              onClick={() => void alvanistaOpenUser(friend.username).catch(() => {})}
            />
          ))}
        />
      )}
    </>
  )
}
