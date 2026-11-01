import { makeStyles, tokens } from '@fluentui/react-components'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import { SaveStateThumb } from '../../components/SaveStateThumb'
import { SectionHeader } from '../../components/SectionHeader'
import { formatPlayTime } from '../../lib/playTime'
import type { RecentSaveState } from '../../lib/tauri'
import { CAPTION_SCRIM } from './profileData'

const useStyles = makeStyles({
  section: { marginTop: '26px' },
  // Folga na rolagem para ela não cortar o anel de foco (3 de vão + 3 de traço).
  row: { display: 'flex', columnGap: '10px', overflowX: 'auto', padding: '8px', margin: '-8px' },
  moment: {
    position: 'relative',
    flex: '0 0 240px',
    aspectRatio: '16 / 9',
    padding: 0,
    border: 'none',
    overflow: 'hidden',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground3,
    color: '#fff',
    fontFamily: 'inherit',
    cursor: 'pointer',
    textAlign: 'left',
  },
  thumb: { width: '100%', height: '100%' },
  cap: {
    position: 'absolute',
    insetInline: 0,
    bottom: 0,
    padding: '18px 10px 7px',
    fontSize: '12px',
    // Texto branco sobre qualquer captura, até as claras: o véu já começa escuro.
    backgroundImage: `linear-gradient(180deg, rgba(0, 0, 0, ${CAPTION_SCRIM.top}), rgba(0, 0, 0, ${CAPTION_SCRIM.bottom}))`,
  },
  capTitle: { display: 'block', fontSize: '12.5px' },
})

/** "Seus momentos": as capturas dos saves mais recentes; sem nenhum, some. */
export function MomentsRow({ states }: { states: RecentSaveState[] }) {
  const { t } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  if (states.length === 0) return null
  return (
    <section className={s.section} data-testid="profile-moments">
      <SectionHeader title={t('profilePage.moments')} subtitle={t('profilePage.momentsSub')} />
      <div className={s.row}>
        {states.map((st) => {
          const slot = st.slot == null ? t('game.auto') : t('game.slot', { n: st.slot })
          const played =
            st.playTimeAtSave != null && st.playTimeAtSave > 0
              ? t('game.playedFor', { time: formatPlayTime(st.playTimeAtSave) })
              : ''
          return (
            <button key={st.id} type="button" className={s.moment} onClick={() => navigate(`/rom/${st.romId}`)}>
              <SaveStateThumb stateId={st.id} hasThumbnail className={s.thumb} />
              <span className={s.cap}>
                <span className={s.capTitle}>{st.romTitle}</span>
                {slot}
                {played}
              </span>
            </button>
          )
        })}
      </div>
    </section>
  )
}
