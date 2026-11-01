import { Button, makeStyles, tokens } from '@fluentui/react-components'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { GameCard } from '../../components/GameCard'
import { SectionHeader } from '../../components/SectionHeader'
import { Shelf } from '../../components/Shelf'
import { formatPlayTime } from '../../lib/playTime'
import { platformLabel } from '../../lib/platform'
import type { RomEntry } from '../../lib/tauri'
import { profileStats } from './profileData'
import { relativeAge } from './relativeAge'

const useStyles = makeStyles({
  section: { marginTop: '26px' },
  stats: {
    display: 'grid',
    gridTemplateColumns: 'repeat(4, minmax(0, 1fr))',
    gap: '10px',
    '@media (max-width: 860px)': { gridTemplateColumns: 'repeat(2, minmax(0, 1fr))' },
  },
  stat: {
    backgroundColor: tokens.colorNeutralBackground2,
    borderRadius: tokens.borderRadiusMedium,
    padding: '14px 16px',
  },
  statValue: { fontSize: '26px', fontFamily: 'var(--reemuFontDisplay)', lineHeight: 1.2 },
  statLabel: { color: tokens.colorNeutralForeground3, fontSize: '12.5px', marginTop: '2px' },
  caption: {
    marginTop: '8px',
    color: tokens.colorNeutralForeground3,
    fontSize: '12px',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  },
  invite: { color: tokens.colorNeutralForeground2 },
  card: {
    backgroundColor: tokens.colorNeutralBackground2,
    borderRadius: tokens.borderRadiusMedium,
    padding: '16px 18px',
    minWidth: 0,
  },
  bars: { display: 'flex', flexDirection: 'column', rowGap: '10px' },
  bar: {
    display: 'grid',
    gridTemplateColumns: '36px minmax(0, 1fr) 100px',
    columnGap: '10px',
    alignItems: 'center',
    width: '100%',
    textAlign: 'left',
    background: 'none',
    border: 'none',
    padding: 0,
    color: 'inherit',
    cursor: 'pointer',
  },
  cover: {
    width: '36px',
    height: '48px',
    objectFit: 'cover',
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  barText: { display: 'block', minWidth: 0 },
  barName: {
    display: 'block',
    fontSize: '13px',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
  },
  track: {
    display: 'block',
    height: '6px',
    backgroundColor: tokens.colorNeutralBackground5,
    borderRadius: tokens.borderRadiusSmall,
    marginTop: '5px',
  },
  fill: {
    display: 'block',
    height: '100%',
    backgroundColor: 'var(--reemuBrandAccent)',
    borderRadius: tokens.borderRadiusSmall,
  },
  barTime: {
    color: tokens.colorNeutralForeground2,
    fontSize: '12.5px',
    fontVariantNumeric: 'tabular-nums',
    textAlign: 'right',
    whiteSpace: 'nowrap',
  },
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
    <section className={s.section} data-testid="profile-numbers">
      <SectionHeader title={t('profilePage.reemuTitle')} subtitle={t('profilePage.reemuSub')} />
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

/** "Jogados recentemente": a prateleira da Home, com os jogos já ordenados. */
export function RecentRow({ roms }: { roms: RomEntry[] }) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  const [now] = useState(() => Date.now())
  if (roms.length === 0) return null
  return (
    <section className={s.section} data-testid="profile-recent">
      <SectionHeader title={t('profilePage.recent')} />
      <Shelf>
        {roms.map((r) => (
          <div key={r.id}>
            <GameCard
              title={r.title}
              badge={platformLabel(r.systemId)}
              boxart={r.boxart}
              favorite={r.isFavorite}
              onClick={() => navigate(`/rom/${r.id}`)}
            />
            <div className={s.caption} data-caption>
              {[
                r.playTimeSecs > 0 ? formatPlayTime(r.playTimeSecs) : null,
                r.lastPlayedAt != null ? relativeAge(r.lastPlayedAt, now, i18n.language) : null,
              ]
                .filter(Boolean)
                .join(' · ')}
            </div>
          </div>
        ))}
      </Shelf>
    </section>
  )
}

/** "Mais jogados no ReEmu": barras relativas ao jogo mais jogado. Cada linha é
 *  um botão só com `span` dentro; o nome acessível é o título e o tempo. */
export function MostPlayedCard({ roms }: { roms: RomEntry[] }) {
  const { t } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  if (roms.length === 0) return null
  const top = roms[0].playTimeSecs
  return (
    <div className={s.card} data-testid="profile-most-played">
      <SectionHeader as="h3" title={t('profilePage.mostPlayed')} />
      <div className={s.bars}>
        {roms.map((r) => (
          <Button
            key={r.id}
            as="button"
            appearance="transparent"
            className={s.bar}
            onClick={() => navigate(`/rom/${r.id}`)}
          >
            {r.boxart ? <img className={s.cover} src={r.boxart} alt="" loading="lazy" /> : <span className={s.cover} />}
            <span className={s.barText}>
              <span className={s.barName}>{r.title}</span>
              <span className={s.track} aria-hidden>
                <span className={s.fill} style={{ width: `${Math.max(2, Math.round((r.playTimeSecs / top) * 100))}%` }} />
              </span>
            </span>
            <span className={s.barTime}>{formatPlayTime(r.playTimeSecs)}</span>
          </Button>
        ))}
      </div>
    </div>
  )
}
