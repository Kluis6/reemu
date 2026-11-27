import {
  Badge,
  Button,
  Caption1,
  Tab,
  TabList,
  Text,
  Title2,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import {
  ArrowSyncRegular,
  SettingsRegular,
  TrophyRegular,
} from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { EmptyState, LoadingState } from '../components/EmptyState'
import { useAlvanistaConnect } from '../hooks/useAlvanistaConnect'
import { ALVANISTA_CACHED_KEY } from '../hooks/useEffectiveProfile'
import { RA_STATUS_KEY } from '../hooks/useRetroAchievements'
import { socialErrorCode, useAlvanistaAchievements } from '../hooks/useSocial'
import { platformLabel } from '../lib/platform'
import { alvanistaCachedStatus, listRoms, raStatus } from '../lib/tauri'
import { pickText } from './profile/profileData'
import { PAGE_PAD_B, PAGE_PAD_L, PAGE_PAD_R, PAGE_TOP } from '../styles/metrics'
import { useTabStyles } from '../styles/xbox'

const useStyles = makeStyles({
  page: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXL,
    padding: `${PAGE_TOP}px ${PAGE_PAD_R}px ${PAGE_PAD_B}px ${PAGE_PAD_L}px`,
    boxSizing: 'border-box',
  },
  hero: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalL,
    padding: tokens.spacingHorizontalXL,
    borderRadius: tokens.borderRadiusLarge,
    backgroundColor: 'var(--reemuControlFill)',
  },
  identity: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalL,
    minWidth: 0,
  },
  avatar: {
    width: '72px',
    height: '72px',
    borderRadius: tokens.borderRadiusCircular,
    objectFit: 'cover',
    flexShrink: 0,
    backgroundColor: tokens.colorNeutralBackground4,
  },
  avatarFallback: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '72px',
    height: '72px',
    borderRadius: tokens.borderRadiusCircular,
    backgroundColor: tokens.colorNeutralBackground4,
    color: 'var(--reemuBrandAccent)',
    fontSize: '32px',
    flexShrink: 0,
  },
  meta: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXS,
    minWidth: 0,
  },
  badges: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  section: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalM,
  },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(min(280px, 100%), 1fr))',
    gap: tokens.spacingHorizontalM,
  },
  gameCard: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalM,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuControlFill)',
    textAlign: 'left',
  },
  achRow: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuControlFill)',
  },
  achIcon: {
    width: '48px',
    height: '48px',
    borderRadius: tokens.borderRadiusSmall,
    objectFit: 'cover',
    flexShrink: 0,
    backgroundColor: tokens.colorNeutralBackground4,
  },
  achFallback: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    width: '48px',
    height: '48px',
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground4,
    color: 'var(--reemuBrandAccent)',
    flexShrink: 0,
  },
  gameInfo: {
    display: 'flex',
    flexDirection: 'column',
    minWidth: 0,
    gap: '2px',
  },
  dim: {
    color: tokens.colorNeutralForeground3,
  },
})

function sourceLabel(src: string, gasLabel: string): string {
  switch (src.toLowerCase()) {
    case 'steam':
      return 'Steam'
    case 'psn':
    case 'playstation':
      return 'PlayStation'
    case 'xbox':
      return 'Xbox'
    case 'retroachievements':
      return 'RetroAchievements'
    case 'gas':
      return gasLabel
    default:
      return src
  }
}

/** Conquistas do jogador (`/conquistas`): RetroAchievements local + visão multiplataforma do Alvanista (D170). */
export function Achievements() {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const tb = useTabStyles()
  const navigate = useNavigate()
  const [tab, setTab] = useState<'ra' | 'alvanista'>('ra')

  const status = useQuery({
    queryKey: RA_STATUS_KEY,
    queryFn: raStatus,
    retry: false,
  })
  const alva = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
  })
  const alvaConnected = Boolean(alva.data?.connected && !alva.data?.ended)
  const hasAlvaScope = Boolean(alva.data?.scopes?.includes('achievements:read'))

  const { connect, cancel, connecting } = useAlvanistaConnect({
    scopes: ['achievements:read'],
  })

  const alvaAch = useAlvanistaAchievements(
    alvaConnected && hasAlvaScope && tab === 'alvanista',
  )
  const alvaErr = alvaAch.isError ? socialErrorCode(alvaAch.error) : null
  const alvaNeedsConsent = !hasAlvaScope || alvaErr === 'needs_consent'

  const roms = useQuery({
    queryKey: ['roms'],
    queryFn: listRoms,
    enabled: Boolean(status.data?.connected) && tab === 'ra',
    retry: false,
  })

  if (status.isLoading || alva.isLoading) return <LoadingState />

  const renderRetroAchievements = () => {
    if (!status.data?.connected) {
      return (
        <EmptyState
          size="large"
          art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
          artAccent
          title={t('nav.achievements')}
          action={
            <Button appearance="primary" onClick={() => navigate('/settings/conquistas')}>
              {t('achievements.openSettings')}
            </Button>
          }
        >
          {t('achievements.profileEmptyHint')}
        </EmptyState>
      )
    }

    const st = status.data
    const games = (roms.data ?? []).slice(0, 24)

    return (
      <>
        <div className={s.hero}>
          <div className={s.identity}>
            {st.avatarUrl ? (
              <img className={s.avatar} src={st.avatarUrl} alt="" />
            ) : (
              <span className={s.avatarFallback} aria-hidden>
                <TrophyRegular />
              </span>
            )}
            <div className={s.meta}>
              <Title2>{st.displayName ?? st.username}</Title2>
              <div className={s.badges}>
                <Badge appearance="tint" color="warning">
                  {t('achievements.scoreHardcore', { points: st.score })}
                </Badge>
                <Badge appearance="tint" color="informative">
                  {t('achievements.scoreSoftcore', { points: st.scoreSoftcore })}
                </Badge>
                <Badge
                  appearance="outline"
                  color={!st.enabled ? 'subtle' : st.hardcore ? 'warning' : 'brand'}
                >
                  {!st.enabled
                    ? t('achievements.badgePaused')
                    : st.hardcore
                      ? t('achievements.badgeHardcore')
                      : t('achievements.badgeSoftcore')}
                </Badge>
              </div>
            </div>
          </div>
          <Button
            appearance="secondary"
            icon={<SettingsRegular />}
            onClick={() => navigate('/settings/conquistas')}
          >
            {t('settings.tabs.achievements')}
          </Button>
        </div>

        {games.length > 0 && (
          <section className={s.section}>
            <Text weight="semibold" size={400}>
              {t('achievements.compatibleGames')}
            </Text>
            <div className={s.grid}>
              {games.map((rom) => (
                <div key={rom.id} className={s.gameCard}>
                  <div className={s.gameInfo}>
                    <Text weight="semibold" truncate wrap={false}>
                      {rom.title}
                    </Text>
                    <Caption1 className={s.dim}>{platformLabel(rom.systemId)}</Caption1>
                  </div>
                  <Button
                    size="small"
                    appearance="subtle"
                    onClick={() =>
                      navigate(`/rom/${rom.id}`, { state: { openPanel: 'achievements' } })
                    }
                  >
                    {t('achievements.openGameAchievements')}
                  </Button>
                </div>
              ))}
            </div>
          </section>
        )}
      </>
    )
  }

  const renderAlvanistaAchievements = () => {
    if (!alvaConnected) {
      return (
        <EmptyState
          size="large"
          art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
          artAccent
          title={t('achievements.alvanistaTab')}
          action={
            <Button appearance="primary" onClick={() => navigate('/settings/perfil')}>
              {t('network.openProfileSettings')}
            </Button>
          }
        >
          {t('achievements.alvanistaNotConnectedHint')}
        </EmptyState>
      )
    }

    if (alvaNeedsConsent) {
      return (
        <EmptyState
          size="large"
          art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
          artAccent
          title={t('achievements.alvanistaTab')}
          action={
            connecting ? (
              <Button appearance="secondary" onClick={cancel}>
                {t('network.cancelConnect')}
              </Button>
            ) : (
              <Button appearance="primary" onClick={connect}>
                {t('achievements.alvanistaConnectButton')}
              </Button>
            )
          }
        >
          {connecting ? t('network.connecting') : t('achievements.alvanistaConsentHint')}
        </EmptyState>
      )
    }

    if (alvaAch.isLoading) return <LoadingState />

    if (alvaErr === 'offline') {
      return (
        <EmptyState
          size="large"
          art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
          artAccent
          title={t('achievements.alvanistaTab')}
          action={
            <Button appearance="secondary" onClick={() => void alvaAch.refetch()}>
              {t('network.retry')}
            </Button>
          }
        >
          {t('achievements.alvanistaOfflineHint')}
        </EmptyState>
      )
    }

    const data = alvaAch.data
    if (!data) return null

    return (
      <>
        <div className={s.hero}>
          <div className={s.meta}>
            <Title2>{alva.data?.username ?? t('achievements.alvanistaTab')}</Title2>
            <div className={s.badges}>
              <Badge appearance="tint" color="brand">
                {t('achievements.alvanistaUnlockedSummary', {
                  unlocked: data.stats.unlocked,
                  total: data.stats.total,
                })}
              </Badge>
              {data.stats.totalPoints > 0 && (
                <Badge appearance="tint" color="warning">
                  {t('achievements.pointsShort', { points: data.stats.totalPoints })}
                </Badge>
              )}
              {data.stats.byPlatform.map((plat) => (
                <Badge key={plat.platform} appearance="outline" color="informative">
                  {`${sourceLabel(plat.platform, t('achievements.alvanistaBadgesLabel'))}: ${plat.unlocked}/${plat.total}`}
                </Badge>
              ))}
            </div>
          </div>
          <Button
            appearance="secondary"
            icon={<ArrowSyncRegular />}
            disabled={alvaAch.isFetching}
            onClick={() => void alvaAch.refetch()}
          >
            {t('network.refresh')}
          </Button>
        </div>

        {data.recent.length === 0 ? (
          <EmptyState
            size="medium"
            art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)' }} />}
            artAccent
            title={t('achievements.alvanistaRecentTitle')}
          >
            {t('achievements.alvanistaEmptyHint')}
          </EmptyState>
        ) : (
          <section className={s.section}>
            <Text weight="semibold" size={400}>
              {t('achievements.alvanistaRecentTitle')}
            </Text>
            <div className={s.grid}>
              {data.recent.map((item, idx) => {
                const localizedName =
                  pickText(item.names, i18n.language) ?? item.name
                const localizedDesc =
                  pickText(item.descriptions, i18n.language) ?? item.description
                const sub = [
                  sourceLabel(item.source, t('achievements.alvanistaBadgesLabel')),
                  item.gameOrApp,
                ]
                  .filter(Boolean)
                  .join(' · ')

                return (
                  <div key={`${item.source}-${item.name}-${idx}`} className={s.achRow}>
                    {item.iconUrl ? (
                      <img className={s.achIcon} src={item.iconUrl} alt="" />
                    ) : (
                      <span className={s.achFallback} aria-hidden>
                        <TrophyRegular />
                      </span>
                    )}
                    <div className={s.gameInfo}>
                      <Text weight="semibold" truncate wrap={false}>
                        {localizedName}
                      </Text>
                      <Caption1 className={s.dim} truncate wrap={false}>
                        {sub}
                      </Caption1>
                      {localizedDesc && (
                        <Caption1 className={s.dim} truncate wrap={false}>
                          {localizedDesc}
                        </Caption1>
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
          </section>
        )}
      </>
    )
  }

  return (
    <div className={s.page}>
      <TabList
        className={tb.tabs}
        selectedValue={tab}
        onTabSelect={(_, d) => setTab(d.value as 'ra' | 'alvanista')}
      >
        <Tab value="ra">
          RetroAchievements
        </Tab>
        <Tab value="alvanista">
          {t('achievements.alvanistaTab')}
        </Tab>
      </TabList>

      {tab === 'ra' ? renderRetroAchievements() : renderAlvanistaAchievements()}
    </div>
  )
}
