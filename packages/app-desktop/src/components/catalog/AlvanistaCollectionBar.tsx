import {
  Button,
  makeStyles,
  mergeClasses,
  Select,
  tokens,
} from '@fluentui/react-components'
import {
  BookmarkFilled,
  BookmarkRegular,
  CheckmarkCircleFilled,
  CheckmarkCircleRegular,
  ClockArrowDownloadRegular,
  LibraryRegular,
} from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useAlvanistaConnect } from '../../hooks/useAlvanistaConnect'
import {
  collectionErrorCode,
  useCollectionItem,
  useUpdateCollectionItem,
} from '../../hooks/useCollection'
import { ALVANISTA_CACHED_KEY } from '../../hooks/useEffectiveProfile'
import {
  alvanistaCachedStatus,
  type CollectionJourneyStatus,
} from '../../lib/tauri'
import { sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'

const JOURNEY_ORDER: CollectionJourneyStatus[] = [
  'no_status',
  'unplayed',
  'want_to_play',
  'backlog',
  'playing',
  'played',
  'finished',
  'completed',
  'abandoned',
]

const useStyles = makeStyles({
  row: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: '8px',
    color: tokens.colorNeutralForeground2,
  },
  icon: {
    fontSize: '20px',
    flexShrink: 0,
  },
  select: {
    minWidth: '156px',
  },
  activeChip: {
    color: tokens.colorBrandForeground1,
  },
})

export function AlvanistaCollectionBar({
  gameUuid,
  systemId,
  playTimeSeconds,
  className,
}: {
  gameUuid: string | null | undefined
  systemId: string
  playTimeSeconds: number
  className?: string
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const push = useToastStore((st) => st.push)

  const cached = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
  })

  const connected = Boolean(cached.data?.connected && !cached.data?.ended)
  const hasScope = Boolean(cached.data?.scopes?.includes('collection'))

  const { connect, cancel, connecting } = useAlvanistaConnect({
    scopes: ['collection'],
  })

  const itemQuery = useCollectionItem(gameUuid, connected && hasScope)
  const errCode = itemQuery.isError ? collectionErrorCode(itemQuery.error) : null
  const needsConsent = !hasScope || errCode === 'needs_consent'

  const update = useUpdateCollectionItem(gameUuid ?? '', systemId)

  if (!connected || !gameUuid) return null

  if (needsConsent) {
    return (
      <div className={mergeClasses(s.row, className)} data-testid="alva-collection-consent">
        <LibraryRegular className={s.icon} aria-hidden />
        <span>{t('collection.connectPrompt')}</span>
        {connecting ? (
          <>
            <span>{t('collection.connecting')}</span>
            <Button size="small" appearance="subtle" onClick={cancel}>
              {t('collection.cancelConnect')}
            </Button>
          </>
        ) : (
          <Button size="small" appearance="secondary" onClick={connect}>
            {t('collection.connectButton')}
          </Button>
        )}
      </div>
    )
  }

  if (errCode === 'offline') {
    return (
      <div className={mergeClasses(s.row, className)}>
        <LibraryRegular className={s.icon} aria-hidden />
        <span>{t('collection.offline')}</span>
      </div>
    )
  }

  const item = itemQuery.data
  if (!item) return null

  const localMinutes = Math.floor(playTimeSeconds / 60)
  const canSyncTime = localMinutes > 0 && localMinutes > item.playtimeMinutes
  const currentStatus: CollectionJourneyStatus = JOURNEY_ORDER.includes(
    item.status as CollectionJourneyStatus,
  )
    ? (item.status as CollectionJourneyStatus)
    : 'no_status'

  const handleToggleCollection = () => {
    update.mutate(
      { inCollection: !item.inCollection },
      {
        onError: () => push(sysToast(t('collection.updateFailed'), 'Error')),
      },
    )
  }

  const handleToggleWishlist = () => {
    update.mutate(
      { inWishlist: !item.inWishlist },
      {
        onError: () => push(sysToast(t('collection.updateFailed'), 'Error')),
      },
    )
  }

  const handleChangeStatus = (nextStatus: CollectionJourneyStatus) => {
    if (nextStatus === currentStatus) return
    update.mutate(
      { status: nextStatus },
      {
        onError: () => push(sysToast(t('collection.updateFailed'), 'Error')),
      },
    )
  }

  const handleSyncTime = () => {
    update.mutate(
      { playtimeMinutes: localMinutes },
      {
        onSuccess: () => push(sysToast(t('collection.timeSynced'), 'Success')),
        onError: () => push(sysToast(t('collection.updateFailed'), 'Error')),
      },
    )
  }

  return (
    <div className={mergeClasses(s.row, className)} data-testid="alva-collection-bar">
      <Button
        size="small"
        appearance="secondary"
        aria-pressed={item.inCollection}
        disabled={update.isPending}
        icon={
          item.inCollection ? (
            <CheckmarkCircleFilled className={s.activeChip} />
          ) : (
            <CheckmarkCircleRegular />
          )
        }
        onClick={handleToggleCollection}
      >
        {item.inCollection ? t('collection.inCollection') : t('collection.addCollection')}
      </Button>

      <Button
        size="small"
        appearance="secondary"
        aria-pressed={item.inWishlist}
        disabled={update.isPending}
        icon={
          item.inWishlist ? (
            <BookmarkFilled className={s.activeChip} />
          ) : (
            <BookmarkRegular />
          )
        }
        onClick={handleToggleWishlist}
      >
        {item.inWishlist ? t('collection.wishlist') : t('collection.addWishlist')}
      </Button>

      <Select
        size="small"
        className={s.select}
        aria-label={t('collection.journeyLabel')}
        value={currentStatus}
        disabled={update.isPending}
        onChange={(_, data) =>
          handleChangeStatus(data.value as CollectionJourneyStatus)
        }
      >
        {JOURNEY_ORDER.map((st) => (
          <option key={st} value={st}>
            {t(`collection.status.${st}`)}
          </option>
        ))}
      </Select>

      {canSyncTime && (
        <Button
          size="small"
          appearance="subtle"
          disabled={update.isPending}
          icon={<ClockArrowDownloadRegular />}
          onClick={handleSyncTime}
        >
          {update.isPending
            ? t('collection.syncingTime')
            : t('collection.syncTime', { minutes: localMinutes })}
        </Button>
      )}
    </div>
  )
}
