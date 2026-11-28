import {
  Badge, Body1, Button, Caption1, Menu, MenuItem, MenuList, MenuPopover, MenuTrigger,
  Title1, makeStyles, tokens,
} from '@fluentui/react-components'
import { ArrowClockwiseRegular, ArrowLeftRegular, CloudRegular, MoreHorizontalRegular } from '@fluentui/react-icons'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router-dom'
import symbol from '../../assets/alvanista-symbol.svg'
import { ProfileAvatar } from '../../components/ProfileAvatar'
import type { EffectiveProfile } from '../../hooks/useEffectiveProfile'
import { navigateBack } from '../../lib/focusNav'
import { alvanistaOpenProfile, alvanistaOpenSite, type AlvanistaStatus } from '../../lib/tauri'
import { tierAccent } from '../../lib/tierRing'
import { PAGE_PAD_L, PAGE_TOP } from '../../styles/metrics'
import { pickText, readableInk } from './profileData'
import { relativeAge } from './relativeAge'

const useStyles = makeStyles({
  banner: {
    position: 'absolute',
    top: `calc(-1 * var(--reemuPageTop, ${PAGE_TOP}px))`,
    left: `-${PAGE_PAD_L}px`,
    right: `calc(-1 * var(--reemuScrollPadR, ${PAGE_PAD_L}px) - var(--reemuInsetX, 0px))`,
    height: 'clamp(470px, 78vh, 760px)',
    zIndex: -1,
    pointerEvents: 'none',
    backgroundImage: 'radial-gradient(ellipse at 30% 25%, var(--reemuBrandAccent), var(--reemuPage) 76%)',
    backgroundSize: 'cover',
    backgroundPosition: 'center',
    opacity: 0.34,
    maskImage: 'linear-gradient(#000 0%, #000 20%, transparent 100%)',
    '@media (forced-colors: active)': { display: 'none' },
  },
  hero: {
    display: 'flex',
    flexDirection: 'column',
    rowGap: '8px',
  },
  topBar: {
    display: 'flex',
    alignItems: 'center',
  },
  body: {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'flex-start',
    columnGap: 'clamp(18px, 3vw, 34px)',
    rowGap: '20px',
    paddingBlock: '12px 8px',
  },
  identity: {
    display: 'flex',
    gap: 'clamp(16px, 2.8vw, 30px)',
    alignItems: 'flex-start',
    flex: '1 1 360px',
    minWidth: 0,
    '@media (max-width: 530px)': { flexDirection: 'column' },
  },
  ident: { minWidth: 0, display: 'flex', flexDirection: 'column', rowGap: '10px' },
  name: {
    margin: 0,
    overflowWrap: 'anywhere',
    fontFamily: 'var(--reemuFontDisplay)',
    fontSize: 'clamp(30px, 3vw, 42px)',
    lineHeight: 1.15,
  },
  bio: { maxWidth: '58ch', overflowWrap: 'anywhere' },
  muted: { color: tokens.colorNeutralForeground2 },
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
    color: tokens.colorNeutralForeground2,
  },
  actions: {
    flex: '0 0 max(220px, 28%)',
    display: 'grid',
    gap: '8px',
    '@media (max-width: 760px)': { flexBasis: 'min(280px, 100%)' },
  },
  action: { width: '100%', minHeight: '36px', borderRadius: tokens.borderRadiusMedium },
  menu: { maxWidth: 'none' },
  sync: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    color: tokens.colorNeutralForeground2,
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
  const card = connected ? status?.card : null
  const identity = card?.identity
  const palette = card?.palette
  const isColor = (value: unknown): value is string =>
    typeof value === 'string' && /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(value)
  const bannerStyle = profile.bannerUrl
    ? {
        backgroundImage: `url(${JSON.stringify(profile.bannerUrl)})`,
        backgroundPosition: `${identity?.hero_position_x ?? 50}% ${identity?.hero_position_y ?? 50}%`,
      }
    : isColor(palette?.base) && isColor(palette?.mid)
      ? { backgroundImage: `linear-gradient(160deg, ${palette.base}, ${palette.mid})` }
      : undefined
  const fetchedAt = connected ? status?.fetched_at : null
  const tierName = pickText(identity?.tier?.name, i18n.language)
  const reino = pickText(card?.reino_title_text, i18n.language)
  // As cores do plano vêm do Alvanista: o anel do avatar e o chip.
  const tierColors = card?.identity?.tier?.colors
  const tint = tierAccent(tierColors)
  const memberYear = card?.member_since != null && card.member_since >= 1900 ? card.member_since : null
  const location = identity?.location || null

  return (
    <section className={s.hero} data-testid="profile-hero">
      <div className={s.banner} style={bannerStyle} data-testid="profile-banner" aria-hidden />
      <div className={s.topBar}>
        <Button
          appearance="secondary"
          size="small"
          icon={<ArrowLeftRegular />}
          onClick={() => navigateBack(navigate)}
        >
          {t('hints.back')}
        </Button>
      </div>
      <div className={s.body}>
        <div className={s.identity}>
          <ProfileAvatar profile={profile} size={120} tier={tierColors} pulse />
        <div className={s.ident}>
          <Title1 as="h1" className={s.name}>
            {profile.name}
          </Title1>
          {profile.bio ? (
            <Body1 className={s.bio}>{profile.bio}</Body1>
          ) : (
            !connected && <Body1 className={s.muted}>{t('profilePage.noBio')}</Body1>
          )}
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
                  // A cor do plano tinge o chip; a tinta é a de melhor contraste sobre ela.
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
      </div>
      <div className={s.actions}>
        {connected ? (
          <>
            <Button className={s.action} appearance="primary" onClick={() => void alvanistaOpenProfile().catch(() => {})}>
              {t('profilePage.viewOnAlvanista')}
            </Button>
            <Button className={s.action} appearance="secondary" onClick={() => void alvanistaOpenSite('edit').catch(() => {})}>
              {t('alvanista.editOnAlvanista')}
            </Button>
            <Menu positioning={{ position: 'below', align: 'end', matchTargetSize: 'width' }}>
              <MenuTrigger disableButtonEnhancement>
                <Button className={s.action} appearance="secondary" icon={<MoreHorizontalRegular />}>
                  {t('profilePage.moreActions')}
                </Button>
              </MenuTrigger>
            <MenuPopover className={s.menu}>
                <MenuList>
                  <MenuItem icon={<ArrowClockwiseRegular />} disabled={refreshing} onClick={onRefresh}>
                    {t('profilePage.refresh')}
                  </MenuItem>
                  {status?.save_sync === true && (
                    <MenuItem icon={<CloudRegular />} onClick={() => navigate('/settings/sincronizacao')}>
                      {t('profilePage.alvaCloudSave')}
                    </MenuItem>
                  )}
                </MenuList>
              </MenuPopover>
            </Menu>
            {fetchedAt != null && (
              <Caption1 className={s.sync}>
                {t('profilePage.updated', { age: relativeAge(fetchedAt, now, i18n.language) })}
              </Caption1>
            )}
          </>
        ) : (
          <Button className={s.action} appearance="primary" onClick={() => navigate('/settings/perfil')}>
            {t('profilePage.editProfile')}
          </Button>
        )}
        </div>
      </div>
    </section>
  )
}
