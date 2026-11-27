import {
  Badge,
  Button,
  Caption1,
  Text,
  Title2,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { SettingsRegular, TrophyRegular } from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { EmptyState, LoadingState } from '../components/EmptyState'
import { RA_STATUS_KEY } from '../hooks/useRetroAchievements'
import { platformLabel } from '../lib/platform'
import { listRoms, raStatus } from '../lib/tauri'
import { PAGE_PAD_B, PAGE_PAD_L, PAGE_PAD_R, PAGE_TOP } from '../styles/metrics'

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

/** Conquistas do jogador. Acessada pelo drawer de perfil (`/conquistas`). */
export function Achievements() {
  const { t } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()

  const status = useQuery({
    queryKey: RA_STATUS_KEY,
    queryFn: raStatus,
    retry: false,
  })
  const roms = useQuery({
    queryKey: ['roms'],
    queryFn: listRoms,
    enabled: Boolean(status.data?.connected),
    retry: false,
  })

  if (status.isLoading) return <LoadingState />

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
    <div className={s.page}>
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
    </div>
  )
}
