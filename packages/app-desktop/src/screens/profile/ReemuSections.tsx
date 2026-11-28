import { Button, makeStyles, tokens } from '@fluentui/react-components'
import { ClockRegular, GamesRegular, TrophyRegular } from '@fluentui/react-icons'
import { useQueries } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { GameCard } from '../../components/GameCard'
import { SectionHeader } from '../../components/SectionHeader'
import { raGameKey } from '../../hooks/useRetroAchievements'
import { formatPlayTime } from '../../lib/playTime'
import { platformLabel } from '../../lib/platform'
import { raGameAchievements, type RaStatus, type RomEntry } from '../../lib/tauri'
import { profileStats } from './profileData'
import { ProfileGallery } from './ProfileGallery'

const GAME_ACHIEVEMENTS_STALE_MS = 2 * 60 * 1000
const rowHighlight = {
  backgroundColor: `color-mix(in srgb, ${tokens.colorNeutralForeground1} 16%, transparent) !important`,
  color: tokens.colorNeutralForeground1,
  outlineColor: `${tokens.colorNeutralForeground1} !important`,
}

const useStyles = makeStyles({
  stats: {
    display: 'grid',
    gridTemplateColumns: 'repeat(4, minmax(0, 1fr))',
    gap: '10px',
    '@media (max-width: 860px)': { gridTemplateColumns: 'repeat(2, minmax(0, 1fr))' },
  },
  stat: {
    minWidth: 0,
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
    paddingTop: '10px',
  },
  statValue: { fontSize: '26px', fontFamily: 'var(--reemuFontDisplay)', lineHeight: 1.2 },
  statLabel: { color: tokens.colorNeutralForeground3, fontSize: '12.5px', marginTop: '2px' },
  invite: { color: tokens.colorNeutralForeground2 },
  achievementsTitle: { fontSize: '16px', lineHeight: '22px', fontWeight: tokens.fontWeightRegular },
  sectionAction: { whiteSpace: 'nowrap' },
  summary: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: '8px 16px',
    color: tokens.colorNeutralForeground2,
  },
  statistic: { display: 'inline-flex', alignItems: 'center', gap: '6px' },
  bars: { display: 'flex', flexDirection: 'column', rowGap: '8px' },
  bar: {
    '--colorBrandStroke1': tokens.colorNeutralForeground1,
    display: 'grid',
    gridTemplateColumns: 'clamp(60px, 8vw, 104px) minmax(0, 1fr)',
    columnGap: 'clamp(10px, 1.3vw, 20px)',
    alignItems: 'stretch',
    width: 'calc(100% + 20px)',
    marginInline: '-10px',
    textAlign: 'left',
    backgroundColor: 'transparent',
    border: 'none',
    borderRadius: tokens.borderRadiusXLarge,
    padding: '10px',
    color: 'inherit',
    cursor: 'pointer',
    '&.fui-Button:hover': rowHighlight,
    '&.fui-Button:focus': rowHighlight,
    '&.fui-Button::before': { boxShadow: 'none !important' },
  },
  cover: {
    position: 'relative',
    display: 'grid',
    placeItems: 'center',
    width: '100%',
    aspectRatio: '1',
    overflow: 'hidden',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground3,
    fontSize: '24px',
  },
  coverImage: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    objectFit: 'cover',
  },
  barText: {
    display: 'flex',
    flexDirection: 'column',
    justifyContent: 'flex-end',
    rowGap: '6px',
    minWidth: 0,
  },
  barName: {
    display: 'block',
    fontSize: '16px',
    fontWeight: tokens.fontWeightSemibold,
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  },
  track: {
    display: 'block',
    height: '3px',
    backgroundColor: `color-mix(in srgb, ${tokens.colorNeutralForeground1} 26%, transparent)`,
    borderRadius: tokens.borderRadiusSmall,
    marginTop: '4px',
  },
  fill: {
    display: 'block',
    height: '100%',
    backgroundColor: tokens.colorNeutralForeground1,
    borderRadius: tokens.borderRadiusSmall,
  },
  barStats: {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    gap: '4px 18px',
    color: tokens.colorNeutralForeground2,
    fontSize: '14px',
    fontVariantNumeric: 'tabular-nums',
  },
  barStat: {
    display: 'inline-flex',
    alignItems: 'center',
    gap: '6px',
    whiteSpace: 'nowrap',
  },
  percent: { marginInlineStart: 'auto' },
})

/**
 * "No ReEmu": os quatro números; biblioteca vazia, só o convite a jogar. Uma
 * leitura que falhou não é uma biblioteca vazia: nem números nem convite, só
 * uma linha neutra.
 */
export function ReemuNumbers({ roms, failed = false }: { roms: RomEntry[]; failed?: boolean }) {
  const { t } = useTranslation()
  const s = useStyles()
  const stats = profileStats(roms)
  const time = stats.playTimeSecs > 0 ? formatPlayTime(stats.playTimeSecs) : t('profilePage.statTimeZero')
  const items: { key: string; value: string; label: string }[] = [
    { key: 'time', value: time, label: t('profilePage.statTime') },
    { key: 'games', value: String(stats.games), label: t('profilePage.statGames') },
    { key: 'systems', value: String(stats.systems), label: t('profilePage.statSystems') },
    { key: 'favorites', value: String(stats.favorites), label: t('profilePage.statFavorites') },
  ]
  return (
    <section data-testid="profile-numbers">
      <SectionHeader title={t('profilePage.reemuTitle')} onCard />
      {failed ? (
        <div className={s.invite} data-testid="profile-library-error">
          {t('profilePage.libraryError')}
        </div>
      ) : roms.length === 0 ? (
        <div className={s.invite}>{t('profilePage.invite')}</div>
      ) : (
        <div className={s.stats}>
          {items.map((i) => (
            <div key={i.key} className={s.stat}>
              <div className={s.statValue} data-stat={i.key}>
                {i.value}
              </div>
              <div className={s.statLabel}>{i.label}</div>
            </div>
          ))}
        </div>
      )}
    </section>
  )
}

export function ProfileAchievements({
  trophies,
  scores,
}: {
  trophies?: number
  scores?: Pick<RaStatus, 'score' | 'scoreSoftcore'>
}) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  return (
    <section>
      <SectionHeader
        title={t('settings.tabs.achievements')}
        titleClassName={s.achievementsTitle}
        onCard
        right={
          <Button appearance="secondary" className={s.sectionAction} onClick={() => navigate('/conquistas')}>
            {t('profilePage.viewAchievements')}
          </Button>
        }
      />
      {(scores || trophies != null) && (
        <div className={s.summary}>
          {scores && (
            <>
              <span className={s.statistic}>
                <TrophyRegular aria-hidden />
                {t('achievements.scoreHardcore', { points: scores.score.toLocaleString(i18n.language) })}
              </span>
              <span>{t('achievements.scoreSoftcore', { points: scores.scoreSoftcore.toLocaleString(i18n.language) })}</span>
            </>
          )}
          {trophies != null && (
            <span className={s.statistic}>
              <TrophyRegular aria-hidden />
              {t('profilePage.alvaTrophyTotal', { count: trophies })}
            </span>
          )}
        </div>
      )}
    </section>
  )
}

export function RecentRow({ roms }: { roms: RomEntry[] }) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  if (roms.length === 0) return null
  return (
    <ProfileGallery
      title={t('profilePage.recent')}
      testId="profile-recent"
      items={roms.map((r) => (
        <GameCard
          key={r.id}
          title={r.title}
          badge={platformLabel(r.systemId)}
          boxart={r.boxart}
          favorite={r.isFavorite}
          onClick={() => navigate(`/rom/${r.id}`)}
        />
      ))}
    />
  )
}

export function MostPlayedCard({ roms, achievementsConnected }: {
  roms: RomEntry[]
  achievementsConnected: boolean
}) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  // Reaproveita a consulta da tela do jogo e a invalidação após desbloqueios.
  const achievements = useQueries({
    queries: achievementsConnected ? roms.map(({ id }) => ({
      queryKey: raGameKey(id),
      queryFn: () => raGameAchievements(id),
      staleTime: GAME_ACHIEVEMENTS_STALE_MS,
      refetchOnWindowFocus: false,
      retry: false,
    })) : [],
  })
  const locale = i18n.resolvedLanguage ?? i18n.language
  const formatNumber = new Intl.NumberFormat(locale)
  const formatPercent = new Intl.NumberFormat(locale, { style: 'percent' })
  return (
    <section data-testid="profile-most-played">
      <SectionHeader title={t('profilePage.mostPlayed')} onCard />
      <div className={s.bars}>
        {roms.map((r, index) => {
          const game = achievements[index]?.data
          const progress = game?.status === 'loaded' && game.totalAchievements > 0 ? game : null
          const fraction = progress ? progress.unlockedAchievements / progress.totalAchievements : 0
          return (
            <Button
              key={r.id}
              as="button"
              appearance="transparent"
              className={s.bar}
              onClick={() => navigate(`/rom/${r.id}`)}
            >
              <span className={s.cover} aria-hidden>
                <GamesRegular />
                {r.boxart && (
                  <img key={r.boxart} className={s.coverImage} src={r.boxart} alt="" loading="lazy" onError={(event) => {
                    event.currentTarget.hidden = true
                  }} />
                )}
              </span>
              <span className={s.barText}>
                <span className={s.barName}>{r.title}</span>
                <span className={s.barStats}>
                  <span className={s.barStat}><ClockRegular aria-hidden />{formatPlayTime(r.playTimeSecs)}</span>
                  {progress && (
                    <>
                      <span className={s.barStat} role="img" aria-label={t('profilePage.gameAchievementCount', {
                        unlocked: formatNumber.format(progress.unlockedAchievements),
                        total: formatNumber.format(progress.totalAchievements),
                      })}>
                        <TrophyRegular aria-hidden />
                        {formatNumber.format(progress.unlockedAchievements)}/{formatNumber.format(progress.totalAchievements)}
                      </span>
                      <span className={s.percent} aria-hidden>{formatPercent.format(fraction)}</span>
                    </>
                  )}
                </span>
                {progress && (
                  <span className={s.track} aria-hidden>
                    <span className={s.fill} style={{ width: `${fraction * 100}%` }} />
                  </span>
                )}
              </span>
            </Button>
          )
        })}
      </div>
    </section>
  )
}
