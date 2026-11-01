import { Badge, Body1, Button, Caption1, Title1, makeStyles, tokens } from '@fluentui/react-components'
import { ArrowClockwiseRegular } from '@fluentui/react-icons'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import symbol from '../../assets/alvanista-symbol.svg'
import { ProfileAvatar } from '../../components/ProfileAvatar'
import type { EffectiveProfile } from '../../hooks/useEffectiveProfile'
import { alvanistaOpenProfile, alvanistaOpenSite, type AlvanistaStatus } from '../../lib/tauri'
import { isHexColor, pickText, readableInk } from './profileData'
import { relativeAge } from './relativeAge'

const BANNER_H = 210
const AVATAR = 128

const useStyles = makeStyles({
  hero: {
    borderRadius: tokens.borderRadiusMedium,
    overflow: 'hidden',
    backgroundColor: tokens.colorNeutralBackground2,
  },
  // Mesmo banner do `ProfileDrawer`: a capa; sem ela, as cores do tema.
  banner: {
    position: 'relative',
    height: `${BANNER_H}px`,
    backgroundImage: `linear-gradient(160deg, var(--reemuBrandAccent) 0%, ${tokens.colorBrandBackground} 100%)`,
    backgroundSize: 'cover',
    backgroundPosition: 'center',
  },
  shade: {
    position: 'absolute',
    inset: 0,
    backgroundImage: `linear-gradient(180deg, transparent 35%, ${tokens.colorNeutralBackground2} 100%)`,
  },
  body: {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'flex-end',
    columnGap: '22px',
    rowGap: '12px',
    padding: '0 24px 22px',
    marginTop: `-${AVATAR / 2}px`,
    position: 'relative',
  },
  avatar: {
    flexShrink: 0,
    borderRadius: tokens.borderRadiusCircular,
    boxShadow: `0 0 0 4px ${tokens.colorNeutralBackground2}`,
  },
  ident: { flex: 1, minWidth: '260px', display: 'flex', flexDirection: 'column', rowGap: '6px' },
  // Título da página, na fonte de títulos dos cabeçalhos de seção.
  name: { margin: 0, overflowWrap: 'anywhere', fontFamily: 'var(--reemuFontDisplay)' },
  bio: { color: tokens.colorNeutralForeground2, maxWidth: '640px', overflowWrap: 'anywhere' },
  muted: { color: tokens.colorNeutralForeground3 },
  badge: { alignSelf: 'flex-start' },
  chips: { display: 'flex', flexWrap: 'wrap', gap: '6px' },
  chip: {
    display: 'inline-flex',
    alignItems: 'center',
    columnGap: '6px',
    padding: '3px 10px',
    borderRadius: tokens.borderRadiusCircular,
    fontSize: '12px',
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground1,
  },
  chipSymbol: { width: '14px', height: '14px' },
  meta: {
    display: 'flex',
    flexWrap: 'wrap',
    columnGap: '14px',
    color: tokens.colorNeutralForeground3,
  },
  actions: { display: 'flex', flexDirection: 'column', rowGap: '8px', alignItems: 'stretch' },
  sync: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'flex-end',
    columnGap: '4px',
    color: tokens.colorNeutralForeground3,
  },
})

/**
 * O topo do perfil: capa, avatar, nome e bio do perfil efetivo. Conectado ao
 * Alvanista, as ações abrem o site e a linha "Atualizado" renova o espelho;
 * fora disso, "Editar perfil" leva ao perfil local. Os chips, o local e o
 * "membro desde" do Alvanista entram à parte.
 */
export function ProfileHero({
  profile,
  status,
  onRefresh,
  refreshing,
}: {
  profile: EffectiveProfile
  status: AlvanistaStatus | undefined
  onRefresh: () => void
  refreshing: boolean
}) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const navigate = useNavigate()
  const connected = profile.source === 'alvanista'
  // Relê a cada meio minuto para o "Atualizado há…" acompanhar o relógio.
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 30_000)
    return () => clearInterval(id)
  }, [])
  const identity = status?.card?.identity
  const bannerStyle = profile.bannerUrl
    ? {
        backgroundImage: `url(${JSON.stringify(profile.bannerUrl)})`,
        backgroundPosition: `${identity?.hero_position_x ?? 50}% ${identity?.hero_position_y ?? 50}%`,
      }
    : undefined
  const fetchedAt = connected ? status?.fetched_at : null
  const card = connected ? status?.card : null
  const tierName = pickText(identity?.tier?.name, i18n.language)
  const reino = pickText(card?.reino_title_text, i18n.language)
  // Só uma cor `#rrggbb` tinge o chip do plano; outra fica no estilo padrão.
  const mid = card?.palette?.mid
  const tint = isHexColor(mid) ? mid : null
  const memberYear = card?.member_since != null && card.member_since >= 1900 ? card.member_since : null
  const location = identity?.location || null

  return (
    <section className={s.hero} data-testid="profile-hero">
      <div className={s.banner} style={bannerStyle} data-testid="profile-banner" aria-hidden>
        <div className={s.shade} />
      </div>
      <div className={s.body}>
        <ProfileAvatar profile={profile} size={AVATAR} className={s.avatar} />
        <div className={s.ident}>
          <Title1 as="h1" className={s.name}>
            {profile.name}
          </Title1>
          {card && (
            <div className={s.chips} data-testid="profile-chips">
              {identity?.level != null && (
                <span className={s.chip} data-chip="level">
                  {t('profilePage.chipLevel', { level: identity.level })}
                </span>
              )}
              {tierName && (
                <span
                  className={s.chip}
                  data-chip="tier"
                  // A paleta só tinge este chip; a tinta é a de melhor contraste sobre o tom.
                  style={tint ? { backgroundColor: tint, color: readableInk(tint), borderColor: tint } : undefined}
                >
                  {tierName}
                </span>
              )}
              {reino && (
                <span className={s.chip} data-chip="reino">
                  {reino}
                </span>
              )}
              <span className={s.chip} data-chip="alvanista">
                <img className={s.chipSymbol} src={symbol} alt="" aria-hidden draggable={false} />
                {t('profilePage.chipAlvanista')}
              </span>
            </div>
          )}
          {/* Conectado sem bio no Alvanista: nenhuma, nunca a local. */}
          {profile.bio ? (
            <Body1 className={s.bio}>{profile.bio}</Body1>
          ) : (
            !connected && <Body1 className={s.muted}>{t('profilePage.noBio')}</Body1>
          )}
          {(location || memberYear != null) && connected && (
            <Caption1 className={s.meta} data-testid="profile-meta">
              {location && <span>{location}</span>}
              {memberYear != null && <span>{t('profilePage.memberSince', { year: memberYear })}</span>}
            </Caption1>
          )}
          {!connected && <Caption1 className={s.muted}>{t('profilePage.localMeta')}</Caption1>}
          {connected && status?.offline && (
            <Badge appearance="tint" color="warning" className={s.badge}>
              {t('alvanista.offlineBadge')}
            </Badge>
          )}
        </div>
        <div className={s.actions}>
          {connected ? (
            <>
              <Button appearance="primary" onClick={() => void alvanistaOpenProfile().catch(() => {})}>
                {t('profilePage.viewOnAlvanista')}
              </Button>
              <Button appearance="outline" onClick={() => void alvanistaOpenSite('edit').catch(() => {})}>
                {t('alvanista.editOnAlvanista')}
              </Button>
              <div className={s.sync}>
                {fetchedAt != null && (
                  <Caption1>
                    {t('profilePage.updated', { age: relativeAge(fetchedAt, now, i18n.language) })}
                  </Caption1>
                )}
                <Button
                  appearance="subtle"
                  size="small"
                  icon={<ArrowClockwiseRegular />}
                  aria-label={t('profilePage.refresh')}
                  disabled={refreshing}
                  onClick={onRefresh}
                />
              </div>
            </>
          ) : (
            <Button appearance="primary" onClick={() => navigate('/settings/perfil')}>
              {t('profilePage.editProfile')}
            </Button>
          )}
        </div>
      </div>
    </section>
  )
}
