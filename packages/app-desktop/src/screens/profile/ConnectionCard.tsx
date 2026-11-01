import { Button, Spinner, makeStyles, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { ProfileAvatar } from '../../components/ProfileAvatar'
import { SectionHeader } from '../../components/SectionHeader'
import { useAlvanistaDisconnect } from '../../hooks/useAlvanistaConnect'
import { alvanistaOpenSite, getProfile, type AlvanistaStatus } from '../../lib/tauri'

const useStyles = makeStyles({
  section: { marginTop: '26px' },
  card: {
    display: 'grid',
    gridTemplateColumns: 'minmax(0, 1.3fr) minmax(0, 1fr)',
    gap: '18px',
    alignItems: 'start',
    backgroundColor: tokens.colorNeutralBackground2,
    borderRadius: tokens.borderRadiusMedium,
    padding: '16px 18px',
    '@media (max-width: 860px)': { gridTemplateColumns: '1fr' },
  },
  list: {
    margin: 0,
    display: 'grid',
    gridTemplateColumns: 'auto minmax(0, 1fr)',
    gap: '8px 16px',
    fontSize: '13px',
  },
  term: { color: tokens.colorNeutralForeground3 },
  def: { margin: 0, color: tokens.colorNeutralForeground2, overflowWrap: 'anywhere' },
  acts: { display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '12px' },
  back: {
    display: 'flex',
    alignItems: 'center',
    columnGap: '12px',
    backgroundColor: tokens.colorNeutralBackground3,
    borderRadius: tokens.borderRadiusMedium,
    padding: '12px',
  },
  backText: { margin: 0, fontSize: '12.5px', color: tokens.colorNeutralForeground2 },
})

/** `https://www.alvanista.com/u/Toy` → `alvanista.com/u/Toy`. */
function shortUrl(url: string): string {
  return url.replace(/^https?:\/\//, '').replace(/^www\./, '')
}

/**
 * "Conexão com o Alvanista" (só conectado): a conta, o que o ReEmu vê, este
 * aparelho, onde a sessão fica, o atalho dos apps conectados e o desconectar,
 * com o perfil local que volta.
 */
export function ConnectionCard({ status }: { status: AlvanistaStatus }) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const local = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })
  const { disconnect, disconnecting } = useAlvanistaDisconnect()
  const since =
    status.connected_at != null
      ? new Intl.DateTimeFormat(i18n.language, { day: 'numeric', month: 'short', year: 'numeric' }).format(
          new Date(status.connected_at * 1000),
        )
      : null
  const device = [status.device_label, since ? t('profilePage.connSince', { date: since }) : null]
    .filter(Boolean)
    .join(' · ')
  const account = [status.username, status.profile_url ? shortUrl(status.profile_url) : null]
    .filter(Boolean)
    .join(' · ')
  const localName = local.data?.name
  const localAvatar = local.data?.avatar ?? 'preset:1'

  return (
    <section className={s.section} data-testid="profile-connection">
      <SectionHeader title={t('profilePage.connTitle')} />
      <div className={s.card}>
        <div>
          <dl className={s.list}>
            {account && (
              <>
                <dt className={s.term}>{t('profilePage.connAccount')}</dt>
                <dd className={s.def} data-row="account">
                  {account}
                </dd>
              </>
            )}
            <dt className={s.term}>{t('profilePage.connSees')}</dt>
            <dd className={s.def}>{t('profilePage.connSeesValue')}</dd>
            {device && (
              <>
                <dt className={s.term}>{t('profilePage.connDevice')}</dt>
                <dd className={s.def} data-row="device">
                  {device}
                </dd>
              </>
            )}
            <dt className={s.term}>{t('profilePage.connSession')}</dt>
            <dd className={s.def} data-row="session">
              {t('profilePage.connSessionKeychain')}
            </dd>
          </dl>
          <div className={s.acts}>
            <Button appearance="transparent" onClick={() => void alvanistaOpenSite('apps').catch(() => {})}>
              {t('profilePage.connManage')}
            </Button>
          </div>
        </div>
        <div>
          {localName && (
            <div className={s.back} data-testid="profile-connection-back">
              <ProfileAvatar profile={{ name: localName, avatar: localAvatar }} size={44} />
              <p className={s.backText}>
                {t('profilePage.connBack')} <b>{localName}</b>
                {local.data?.bio ? '' : `, ${t('profilePage.connBackNoBio')}`}
              </p>
            </div>
          )}
          <div className={s.acts}>
            <Button
              appearance="outline"
              onClick={() => void disconnect()}
              disabledFocusable={disconnecting}
              icon={disconnecting ? <Spinner size="tiny" /> : undefined}
            >
              {t('alvanista.disconnect')}
            </Button>
          </div>
        </div>
      </div>
    </section>
  )
}
