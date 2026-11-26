import {
  Badge,
  Button,
  Caption1,
  Switch,
  Text,
  Title2,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowSyncRegular,
  OpenRegular,
  PeopleRegular,
} from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { EmptyState, LoadingState } from '../components/EmptyState'
import { useAlvanistaConnect } from '../hooks/useAlvanistaConnect'
import { ALVANISTA_CACHED_KEY } from '../hooks/useEffectiveProfile'
import {
  socialErrorCode,
  useAlvanistaFriends,
  useNetworkShowOnline,
} from '../hooks/useSocial'
import { alvanistaCachedStatus, alvanistaOpenUser } from '../lib/tauri'
import { PAGE_PAD_B, PAGE_PAD_L, PAGE_PAD_R, PAGE_TOP } from '../styles/metrics'

const useStyles = makeStyles({
  page: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXL,
    padding: `${PAGE_TOP}px ${PAGE_PAD_R}px ${PAGE_PAD_B}px ${PAGE_PAD_L}px`,
    boxSizing: 'border-box',
  },
  header: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalL,
    padding: tokens.spacingHorizontalXL,
    borderRadius: tokens.borderRadiusLarge,
    backgroundColor: 'var(--reemuControlFill)',
  },
  headerMeta: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXXS,
  },
  headerActions: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalM,
  },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(min(300px, 100%), 1fr))',
    gap: tokens.spacingHorizontalM,
  },
  card: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalM,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuControlFill)',
  },
  person: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
    minWidth: 0,
  },
  avatarWrap: {
    position: 'relative',
    width: '48px',
    height: '48px',
    flexShrink: 0,
  },
  avatar: {
    width: '48px',
    height: '48px',
    borderRadius: tokens.borderRadiusCircular,
    objectFit: 'cover',
    backgroundColor: tokens.colorNeutralBackground4,
  },
  avatarInitial: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '48px',
    height: '48px',
    borderRadius: tokens.borderRadiusCircular,
    backgroundColor: tokens.colorNeutralBackground4,
    color: 'var(--reemuBrandAccent)',
    fontWeight: tokens.fontWeightSemibold,
    fontSize: '18px',
  },
  onlineDot: {
    position: 'absolute',
    right: 0,
    bottom: 0,
    width: '12px',
    height: '12px',
    borderRadius: tokens.borderRadiusCircular,
    backgroundColor: tokens.colorPaletteGreenForeground1,
    boxShadow: `0 0 0 2px ${tokens.colorNeutralBackground2}`,
  },
  info: {
    display: 'flex',
    flexDirection: 'column',
    minWidth: 0,
    gap: '2px',
  },
  nameLine: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalXS,
    minWidth: 0,
  },
  dim: {
    color: tokens.colorNeutralForeground3,
  },
})

/** Tela Minha rede (`/rede`): Gamers conectados no Alvanista, somente leitura (D166, D169). */
export function Network() {
  const { t } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  const [showOnline, setShowOnline] = useNetworkShowOnline()

  const alva = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
  })

  const connected = Boolean(alva.data?.connected && !alva.data?.ended)
  const hasScope = Boolean(alva.data?.scopes?.includes('friends:read'))

  const { connect, cancel, connecting } = useAlvanistaConnect({
    scopes: ['friends:read'],
  })

  const friends = useAlvanistaFriends(connected && hasScope)
  const errCode = friends.isError ? socialErrorCode(friends.error) : null
  const needsConsent = !hasScope || errCode === 'needs_consent'

  if (alva.isLoading) return <LoadingState />

  if (!connected) {
    return (
      <EmptyState
        size="large"
        art={<PeopleRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
        artAccent
        title={t('nav.network')}
        action={
          <Button appearance="primary" onClick={() => navigate('/settings/perfil')}>
            {t('network.openProfileSettings')}
          </Button>
        }
      >
        {t('network.notConnectedHint')}
      </EmptyState>
    )
  }

  if (needsConsent) {
    return (
      <EmptyState
        size="large"
        art={<PeopleRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
        artAccent
        title={t('nav.network')}
        action={
          connecting ? (
            <Button appearance="secondary" onClick={cancel}>
              {t('network.cancelConnect')}
            </Button>
          ) : (
            <Button appearance="primary" onClick={connect}>
              {t('network.connectButton')}
            </Button>
          )
        }
      >
        {connecting ? t('network.connecting') : t('network.consentHint')}
      </EmptyState>
    )
  }

  if (friends.isLoading) return <LoadingState />

  if (errCode === 'offline') {
    return (
      <EmptyState
        size="large"
        art={<PeopleRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
        artAccent
        title={t('nav.network')}
        action={
          <Button appearance="secondary" onClick={() => void friends.refetch()}>
            {t('network.retry')}
          </Button>
        }
      >
        {t('network.offlineHint')}
      </EmptyState>
    )
  }

  const list = friends.data ?? []
  const onlineCount = list.filter((f) => f.online).length

  if (list.length === 0) {
    return (
      <EmptyState
        size="large"
        art={<PeopleRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
        artAccent
        title={t('nav.network')}
      >
        {t('network.emptyHint')}
      </EmptyState>
    )
  }

  const ordered = showOnline
    ? [...list].sort((a, b) => Number(b.online) - Number(a.online))
    : list

  return (
    <div className={s.page} data-testid="network-screen">
      <div className={s.header}>
        <div className={s.headerMeta}>
          <Title2>{t('nav.network')}</Title2>
          <Caption1 className={s.dim}>
            {showOnline
              ? t('network.countWithOnline', {
                  total: list.length,
                  online: onlineCount,
                })
              : t('network.countTotal', { total: list.length })}
          </Caption1>
        </div>
        <div className={s.headerActions}>
          <Switch
            checked={showOnline}
            onChange={(_, d) => setShowOnline(d.checked)}
            label={t('network.showOnlineToggle')}
          />
          <Button
            appearance="secondary"
            icon={<ArrowSyncRegular />}
            disabled={friends.isFetching}
            onClick={() => void friends.refetch()}
          >
            {t('network.refresh')}
          </Button>
        </div>
      </div>

      <div className={s.grid}>
        {ordered.map((friend) => {
          const initial = (friend.username.trim()[0] ?? '?').toUpperCase()
          const visibleOnline = showOnline && friend.online
          const playing = visibleOnline ? friend.playingNow : null
          const activityLine = playing?.title
            ? playing.platform
              ? `${playing.title} · ${playing.platform}`
              : playing.title
            : playing?.customMessage ?? null

          return (
            <div key={friend.id} className={s.card} data-testid={`gamer-card-${friend.username}`}>
              <div className={s.person}>
                <div className={s.avatarWrap}>
                  {friend.avatarUrl ? (
                    <img className={s.avatar} src={friend.avatarUrl} alt="" />
                  ) : (
                    <span className={s.avatarInitial} aria-hidden>
                      {initial}
                    </span>
                  )}
                  {visibleOnline && <span className={s.onlineDot} aria-hidden />}
                </div>
                <div className={s.info}>
                  <div className={s.nameLine}>
                    <Text weight="semibold" truncate wrap={false}>
                      {friend.username}
                    </Text>
                    {visibleOnline && (
                      <Badge size="small" appearance="tint" color="success">
                        {t('network.onlineBadge')}
                      </Badge>
                    )}
                  </div>
                  {activityLine && (
                    <Caption1 className={s.dim} truncate wrap={false}>
                      {t('network.playingNow', { game: activityLine })}
                    </Caption1>
                  )}
                </div>
              </div>
              <Button
                size="small"
                appearance="subtle"
                icon={<OpenRegular />}
                aria-label={t('network.openProfileAria', { user: friend.username })}
                onClick={() => void alvanistaOpenUser(friend.username).catch(() => {})}
              >
                {t('network.openProfile')}
              </Button>
            </div>
          )
        })}
      </div>
    </div>
  )
}
