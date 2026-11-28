import {
  Badge,
  Body1,
  Button,
  Caption1,
  MessageBar,
  MessageBarBody,
  ProgressBar,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkCircleFilled, LockClosedRegular, TrophyRegular } from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { raGameKey } from '../hooks/useRetroAchievements'
import { raGameAchievements, type RaAchievement, type RaLeaderboard } from '../lib/tauri'
import { EmptyState, LoadingState } from './EmptyState'

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalM,
  },
  summaryCard: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    padding: tokens.spacingHorizontalL,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuControlFill)',
  },
  summaryCompact: {
    padding: tokens.spacingHorizontalM,
  },
  summaryTop: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
  },
  gameBadge: {
    width: '48px',
    height: '48px',
    borderRadius: tokens.borderRadiusSmall,
    objectFit: 'cover',
    flexShrink: 0,
  },
  summaryMeta: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
    minWidth: 0,
    flexGrow: 1,
  },
  titleRow: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  richPresenceRow: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
    marginTop: '2px',
  },
  liveStrip: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXS,
  },
  liveItem: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalS,
    padding: `${tokens.spacingVerticalXS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuControlFill)',
    flexWrap: 'wrap',
  },
  liveItemMain: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalS,
    minWidth: 0,
  },
  challengeThumb: {
    width: '28px',
    height: '28px',
    borderRadius: tokens.borderRadiusSmall,
    objectFit: 'cover',
    flexShrink: 0,
  },
  trackerValue: {
    fontVariantNumeric: 'tabular-nums',
    fontWeight: tokens.fontWeightSemibold,
  },
  dim: {
    color: tokens.colorNeutralForeground3,
  },
  sections: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalXS,
  },
  filters: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalXS,
  },
  list: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
  },
  row: {
    display: 'flex',
    alignItems: 'flex-start',
    gap: tokens.spacingHorizontalM,
    padding: tokens.spacingHorizontalM,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'var(--reemuControlFill)',
  },
  rowLocked: {
    opacity: 0.86,
  },
  achBadge: {
    width: '48px',
    height: '48px',
    borderRadius: tokens.borderRadiusSmall,
    objectFit: 'cover',
    flexShrink: 0,
    backgroundColor: tokens.colorNeutralBackground4,
  },
  achBadgeCompact: {
    width: '40px',
    height: '40px',
  },
  achPlaceholder: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    color: tokens.colorNeutralForeground3,
    fontSize: '20px',
  },
  achBody: {
    display: 'flex',
    flexDirection: 'column',
    gap: '4px',
    minWidth: 0,
    flexGrow: 1,
  },
  achHeader: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalS,
    flexWrap: 'wrap',
  },
  pills: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalXS,
  },
  measuredRow: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalS,
    marginTop: '2px',
  },
})

type FilterMode = 'all' | 'locked' | 'unlocked'
type SectionMode = 'achievements' | 'leaderboards'

function kindBadgeMeta(kind: RaAchievement['kind']): {
  key: 'achievements.badgeMissable' | 'achievements.badgeProgression' | 'achievements.badgeWin'
  color: 'informative' | 'warning' | 'success'
} | null {
  if (kind === 'missable') return { key: 'achievements.badgeMissable', color: 'warning' }
  if (kind === 'progression') return { key: 'achievements.badgeProgression', color: 'informative' }
  if (kind === 'win_condition') return { key: 'achievements.badgeWin', color: 'success' }
  return null
}

function leaderboardStateMeta(state: RaLeaderboard['state']): {
  key:
    | 'achievements.leaderboardTracking'
    | 'achievements.leaderboardActive'
    | 'achievements.badgeUnsupported'
    | 'achievements.leaderboardInactive'
  color: 'warning' | 'informative' | 'danger' | 'subtle'
} {
  if (state === 'tracking') {
    return { key: 'achievements.leaderboardTracking', color: 'warning' }
  }
  if (state === 'active') {
    return { key: 'achievements.leaderboardActive', color: 'informative' }
  }
  if (state === 'disabled') {
    return { key: 'achievements.badgeUnsupported', color: 'danger' }
  }
  return { key: 'achievements.leaderboardInactive', color: 'subtle' }
}

function leaderboardFormatKey(
  format: RaLeaderboard['format'],
):
  | 'achievements.leaderboardFormatTime'
  | 'achievements.leaderboardFormatScore'
  | 'achievements.leaderboardFormatValue' {
  if (format === 'time') return 'achievements.leaderboardFormatTime'
  if (format === 'score') return 'achievements.leaderboardFormatScore'
  return 'achievements.leaderboardFormatValue'
}

export function GameAchievementsPanel({
  romId,
  compact = false,
  onNavigateSettings,
  onSelectRom,
}: {
  romId: string
  compact?: boolean
  onNavigateSettings?: () => void
  onSelectRom?: (nextRomId: string) => void
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const [filter, setFilter] = useState<FilterMode>('all')
  const [section, setSection] = useState<SectionMode>('achievements')

  const query = useQuery({
    queryKey: raGameKey(romId),
    queryFn: () => raGameAchievements(romId),
    enabled: Boolean(romId),
    staleTime: 15_000,
    retry: false,
  })

  const items = useMemo(() => query.data?.achievements ?? [], [query.data?.achievements])
  const leaderboards = useMemo(() => query.data?.leaderboards ?? [], [query.data?.leaderboards])
  const activeChallenges = useMemo(
    () => query.data?.activeChallenges ?? [],
    [query.data?.activeChallenges],
  )
  const activeTrackers = useMemo(
    () => query.data?.activeTrackers ?? [],
    [query.data?.activeTrackers],
  )
  const unlockedCount = useMemo(() => items.filter((a) => a.unlocked).length, [items])
  const lockedCount = items.length - unlockedCount
  const filtered = useMemo(() => {
    if (filter === 'unlocked') return items.filter((a) => a.unlocked)
    if (filter === 'locked') return items.filter((a) => !a.unlocked)
    return items
  }, [filter, items])

  if (query.isLoading) return <LoadingState />
  if (query.isError) {
    return (
      <EmptyState
        art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)', fontSize: '40px' }} />}
        artAccent
        title={t('game.tabs.achievements')}
        action={
          <Button appearance="secondary" onClick={() => void query.refetch()}>
            {t('common.retry')}
          </Button>
        }
      >
        {query.error instanceof Error ? query.error.message : String(query.error)}
      </EmptyState>
    )
  }

  const view = query.data
  if (!view || view.status === 'not_connected') {
    return (
      <EmptyState
        art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)', fontSize: '40px' }} />}
        artAccent
        title={t('achievements.notConnectedTitle')}
        action={
          onNavigateSettings ? (
            <Button appearance="primary" onClick={onNavigateSettings}>
              {t('achievements.openSettings')}
            </Button>
          ) : undefined
        }
      >
        {compact ? t('achievements.notConnectedPauseHint') : t('achievements.notConnectedHint')}
      </EmptyState>
    )
  }

  if (view.status === 'unsupported_system') {
    return (
      <EmptyState
        art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)', fontSize: '40px' }} />}
        artAccent
        title={t('game.tabs.achievements')}
      >
        {t('achievements.unsupportedSystem')}
      </EmptyState>
    )
  }

  if (view.status === 'unidentified' || view.status === 'incompatible_version') {
    const canSwitch = Boolean(view.compatibleRomId && onSelectRom && !compact)
    return (
      <EmptyState
        art={<TrophyRegular style={{ color: 'var(--reemuBrandAccent)', fontSize: '40px' }} />}
        artAccent
        title={
          view.status === 'incompatible_version'
            ? t('achievements.incompatibleVersionTitle')
            : t('game.tabs.achievements')
        }
        action={
          canSwitch && view.compatibleRomId ? (
            <Button appearance="primary" onClick={() => onSelectRom?.(view.compatibleRomId!)}>
              {t('achievements.switchCompatibleVersion')}
            </Button>
          ) : undefined
        }
      >
        {view.status === 'incompatible_version'
          ? t('achievements.incompatibleVersionHint')
          : t('achievements.unidentifiedGame')}
      </EmptyState>
    )
  }

  const ratio = view.totalAchievements > 0 ? view.unlockedAchievements / view.totalAchievements : 0
  const effectiveSection: SectionMode =
    leaderboards.length === 0 ? 'achievements' : section

  return (
    <div className={s.root}>
      {view.hardcoreDowngraded && (
        <MessageBar intent="info">
          <MessageBarBody>{t('achievements.hardcorePendingApproval')}</MessageBarBody>
        </MessageBar>
      )}
      <div className={mergeClasses(s.summaryCard, compact && s.summaryCompact)}>
        <div className={s.summaryTop}>
          {view.badgeUrl && <img className={s.gameBadge} src={view.badgeUrl} alt="" loading="lazy" />}
          <div className={s.summaryMeta}>
            <div className={s.titleRow}>
              <Text weight="semibold">{view.title ?? t('game.tabs.achievements')}</Text>
              <Badge appearance="tint" color={view.hardcore ? 'warning' : 'brand'}>
                {view.hardcore ? t('achievements.badgeHardcore') : t('achievements.badgeSoftcore')}
              </Badge>
            </div>
            <Caption1 className={s.dim}>
              {t('achievements.progressSummary', {
                unlocked: view.unlockedAchievements,
                total: view.totalAchievements,
                unlockedPoints: view.unlockedPoints,
                totalPoints: view.totalPoints,
              })}
            </Caption1>
            {view.playingLive && view.richPresence && (
              <div className={s.richPresenceRow} data-testid="ra-rich-presence">
                <Badge appearance="tint" color="success">
                  {t('achievements.richPresenceLabel')}
                </Badge>
                <Caption1>{view.richPresence}</Caption1>
              </div>
            )}
          </div>
        </div>
        {view.totalAchievements > 0 && (
          <ProgressBar
            thickness="large"
            value={Math.min(1, Math.max(0, ratio))}
            color={ratio >= 1 ? 'success' : 'brand'}
          />
        )}
      </div>

      {view.playingLive && (activeChallenges.length > 0 || activeTrackers.length > 0) && (
        <div className={s.liveStrip} data-testid="ra-live-indicators">
          {activeChallenges.map((ch) => (
            <div key={`ch-${ch.id}`} className={s.liveItem}>
              <div className={s.liveItemMain}>
                {ch.badgeUrl && (
                  <img className={s.challengeThumb} src={ch.badgeUrl} alt="" loading="lazy" />
                )}
                <Badge appearance="tint" color="warning">
                  {t('achievements.activeChallengeBadge')}
                </Badge>
                <Text weight="semibold">{ch.title}</Text>
              </div>
              <Badge appearance="tint" color="subtle">
                {t('achievements.pointsShort', { points: ch.points })}
              </Badge>
            </div>
          ))}
          {activeTrackers.map((tr) => (
            <div key={`tr-${tr.id}`} className={s.liveItem}>
              <div className={s.liveItemMain}>
                <Badge appearance="tint" color="brand">
                  {t('achievements.activeTrackerBadge')}
                </Badge>
              </div>
              <Text className={s.trackerValue}>{tr.display}</Text>
            </div>
          ))}
        </div>
      )}

      {leaderboards.length > 0 && (
        <div className={s.sections}>
          <Button
            size="small"
            appearance={effectiveSection === 'achievements' ? 'primary' : 'subtle'}
            onClick={() => setSection('achievements')}
          >
            {t('achievements.sectionAchievements', { count: items.length })}
          </Button>
          <Button
            size="small"
            appearance={effectiveSection === 'leaderboards' ? 'primary' : 'subtle'}
            onClick={() => setSection('leaderboards')}
          >
            {t('achievements.sectionLeaderboards', { count: leaderboards.length })}
          </Button>
        </div>
      )}

      {effectiveSection === 'leaderboards' ? (
        leaderboards.length === 0 ? (
          <Caption1 className={s.dim}>{t('achievements.emptyLeaderboards')}</Caption1>
        ) : (
          <div className={s.list} data-testid="ra-leaderboards-list">
            {leaderboards.map((lb) => {
              const stateBadge = leaderboardStateMeta(lb.state)
              return (
                <div key={lb.id} className={s.row}>
                  <div className={s.achBody}>
                    <div className={s.achHeader}>
                      <Body1>
                        <Text weight="semibold">{lb.title}</Text>
                      </Body1>
                      <div className={s.pills}>
                        {lb.trackerValue && (
                          <Badge
                            appearance={lb.state === 'tracking' ? 'filled' : 'tint'}
                            color="brand"
                          >
                            {lb.trackerValue}
                          </Badge>
                        )}
                        <Badge appearance="tint" color={stateBadge.color}>
                          {t(stateBadge.key)}
                        </Badge>
                        <Badge appearance="outline" color="subtle">
                          {t(leaderboardFormatKey(lb.format))}
                        </Badge>
                        {lb.lowerIsBetter && (
                          <Badge appearance="outline" color="subtle">
                            {t('achievements.leaderboardLowerBetter')}
                          </Badge>
                        )}
                      </div>
                    </div>
                    <Caption1 className={s.dim}>{lb.description}</Caption1>
                  </div>
                </div>
              )
            })}
          </div>
        )
      ) : items.length === 0 ? (
        <Caption1 className={s.dim}>{t('achievements.emptySet')}</Caption1>
      ) : (
        <>
          <div className={s.filters}>
            {(
              [
                { id: 'all', label: t('achievements.filterAll', { count: items.length }) },
                { id: 'locked', label: t('achievements.filterLocked', { count: lockedCount }) },
                { id: 'unlocked', label: t('achievements.filterUnlocked', { count: unlockedCount }) },
              ] as const
            ).map((f) => (
              <Button
                key={f.id}
                size="small"
                appearance={filter === f.id ? 'primary' : 'subtle'}
                onClick={() => setFilter(f.id)}
              >
                {f.label}
              </Button>
            ))}
          </div>

          <div className={s.list}>
            {filtered.map((ach) => {
              const badgeSrc = ach.unlocked ? ach.badgeUrl : (ach.badgeLockedUrl ?? ach.badgeUrl)
              const kindBadge = kindBadgeMeta(ach.kind)
              return (
                <div key={ach.id} className={mergeClasses(s.row, !ach.unlocked && s.rowLocked)}>
                  {badgeSrc ? (
                    <img
                      className={mergeClasses(s.achBadge, compact && s.achBadgeCompact)}
                      src={badgeSrc}
                      alt=""
                      loading="lazy"
                    />
                  ) : (
                    <span
                      className={mergeClasses(
                        s.achBadge,
                        compact && s.achBadgeCompact,
                        s.achPlaceholder,
                      )}
                      aria-hidden
                    >
                      {ach.unlocked ? <CheckmarkCircleFilled /> : <LockClosedRegular />}
                    </span>
                  )}

                  <div className={s.achBody}>
                    <div className={s.achHeader}>
                      <Body1>
                        <Text weight="semibold">{ach.title}</Text>
                      </Body1>
                      <div className={s.pills}>
                        {!ach.unlocked && ach.measuredProgress && (
                          <Badge appearance="tint" color="brand">
                            {ach.measuredProgress}
                          </Badge>
                        )}
                        <Badge appearance="tint" color={ach.unlocked ? 'success' : 'subtle'}>
                          {t('achievements.pointsShort', { points: ach.points })}
                        </Badge>
                        {ach.unlockedHardcore && (
                          <Badge appearance="tint" color="warning">
                            {t('achievements.badgeHardcore')}
                          </Badge>
                        )}
                        {kindBadge && (
                          <Badge appearance="tint" color={kindBadge.color}>
                            {t(kindBadge.key)}
                          </Badge>
                        )}
                        {ach.unofficial && (
                          <Badge appearance="outline" color="subtle">
                            {t('achievements.badgeUnofficial')}
                          </Badge>
                        )}
                        {ach.unsupported && (
                          <Badge appearance="tint" color="danger">
                            {t('achievements.badgeUnsupported')}
                          </Badge>
                        )}
                      </div>
                    </div>
                    <Caption1 className={s.dim}>{ach.description}</Caption1>
                    {!ach.unlocked && ach.measuredProgress && (
                      <div className={s.measuredRow}>
                        <ProgressBar
                          style={{ flexGrow: 1 }}
                          value={Math.min(1, Math.max(0, ach.measuredPercent / 100))}
                        />
                        <Caption1 className={s.dim}>{ach.measuredProgress}</Caption1>
                      </div>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        </>
      )}
    </div>
  )
}
