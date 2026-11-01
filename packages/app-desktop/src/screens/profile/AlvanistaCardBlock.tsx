import { Button, MessageBar, MessageBarBody, Spinner, makeStyles, tokens } from '@fluentui/react-components'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import symbol from '../../assets/alvanista-symbol.svg'
import { AlvanistaButton } from '../../components/AlvanistaButton'
import { SectionHeader } from '../../components/SectionHeader'
import { useAlvanistaConnect } from '../../hooks/useAlvanistaConnect'
import type { AlvanistaCard } from '../../lib/tauri'
import {
  CARD_AGE_SHOWN_AFTER,
  cardComputedAt,
  cardLag,
  cardTiles,
  hasCardContent,
  networkLabel,
  pickText,
  playingNow,
} from './profileData'
import { relativeAge } from './relativeAge'

const useStyles = makeStyles({
  card: {
    backgroundColor: tokens.colorNeutralBackground2,
    borderRadius: tokens.borderRadiusMedium,
    padding: '16px 18px',
    minWidth: 0,
  },
  title: { display: 'flex', alignItems: 'center', columnGap: '8px' },
  symbol: { width: '20px', height: '20px', flexShrink: 0, marginBottom: '14px' },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
    gap: '14px 16px',
  },
  value: { fontSize: '22px', fontFamily: 'var(--reemuFontDisplay)', lineHeight: 1.2 },
  label: { color: tokens.colorNeutralForeground3, fontSize: '12.5px', marginTop: '2px' },
  nets: { display: 'flex', flexWrap: 'wrap', gap: '4px', marginTop: '5px' },
  net: {
    fontSize: '11px',
    padding: '1px 7px',
    borderRadius: tokens.borderRadiusCircular,
    backgroundColor: tokens.colorNeutralBackground3,
    color: tokens.colorNeutralForeground2,
  },
  badges: { display: 'flex', alignItems: 'center', flexWrap: 'wrap', gap: '8px', marginTop: '16px' },
  badge: { width: '40px', height: '40px', objectFit: 'contain', borderRadius: tokens.borderRadiusSmall },
  more: { color: tokens.colorNeutralForeground2, fontSize: '13px' },
  now: { display: 'flex', alignItems: 'center', columnGap: '12px', marginTop: '16px' },
  nowCover: {
    width: '36px',
    height: '48px',
    objectFit: 'cover',
    borderRadius: tokens.borderRadiusSmall,
    backgroundColor: tokens.colorNeutralBackground3,
  },
  nowTitle: { fontSize: '13.5px', overflowWrap: 'anywhere' },
  promo: { display: 'flex', alignItems: 'center', columnGap: '16px' },
  promoSymbol: { width: '40px', height: '40px', flexShrink: 0 },
  promoText: { margin: '0 0 10px', color: tokens.colorNeutralForeground2, lineHeight: 1.45 },
  // A idade do cartão quando ele é mais velho que a leitura (um `/me` sem cartão manteve o guardado).
  age: { color: tokens.colorNeutralForeground3, fontSize: '12px', marginTop: '14px' },
  waiting: { display: 'flex', alignItems: 'center', columnGap: '12px', flexWrap: 'wrap' },
  warn: { marginTop: '10px' },
})

/** Ícone de badge; se a imagem não carrega, some em vez de mostrar o ícone quebrado. */
export function BadgeIcon({ badge, className }: { badge: NonNullable<AlvanistaCard['badges']>['recent'][number]; className: string }) {
  const { i18n } = useTranslation()
  const [broken, setBroken] = useState(false)
  if (broken) return null
  const name = pickText(badge.names, i18n.language) ?? badge.name
  return (
    <img className={className} src={badge.icon_url} alt={name} title={name} loading="lazy" onError={() => setBroken(true)} />
  )
}

/**
 * "No Alvanista": o que o cartão trouxe; cada linha some quando o dado falta,
 * e sem nada para mostrar a seção some inteira. Quando o cartão é mais velho
 * que a leitura do status (um `/me` sem cartão manteve o guardado), diz a
 * própria idade, e "jogando agora" só aparece enquanto ele é recente.
 */
export function AlvanistaCardBlock({ card, fetchedAt }: { card: AlvanistaCard; fetchedAt: number | null }) {
  const { t, i18n } = useTranslation()
  const s = useStyles()
  const [nowMs] = useState(() => Date.now())
  if (!hasCardContent(card, fetchedAt)) return null
  const num = (n: number) => n.toLocaleString(i18n.language)
  const tiles = cardTiles(card).map((x) => {
    switch (x.key) {
      case 'collection':
        return {
          ...x,
          shown: num(x.value),
          label: x.platforms
            ? t('profilePage.alvaCollectionPlatforms', { count: x.platforms })
            : t('profilePage.alvaCollection'),
        }
      case 'trophies':
        return { ...x, shown: num(x.value), label: t('profilePage.alvaTrophies') }
      case 'hours':
        return { ...x, shown: t('profilePage.alvaHoursValue', { hours: num(x.value) }), label: t('profilePage.alvaHours') }
      case 'finished':
        return { ...x, shown: num(x.value), label: t('profilePage.alvaFinished') }
      case 'checkins':
        return { ...x, shown: num(x.value), label: t('profilePage.alvaCheckins') }
      case 'gamers':
        return { ...x, shown: num(x.value), label: t('profilePage.alvaGamers') }
    }
  })
  const recent = (card.badges?.recent ?? []).slice(0, 4)
  const total = card.badges?.count ?? 0
  const now = playingNow(card, fetchedAt)
  const lag = cardLag(card, fetchedAt)
  const computedAt = cardComputedAt(card)
  const age =
    lag != null && lag > CARD_AGE_SHOWN_AFTER && computedAt != null
      ? relativeAge(computedAt, nowMs, i18n.language)
      : null

  return (
    <div className={s.card} data-testid="profile-alvanista">
      <div className={s.title}>
        <img className={s.symbol} src={symbol} alt="" aria-hidden draggable={false} />
        <SectionHeader as="h3" title={t('profilePage.alvaTitle')} />
      </div>
      {tiles.length > 0 && (
        <div className={s.grid}>
          {tiles.map((x) => (
            <div key={x.key} data-tile={x.key}>
              <div className={s.value} data-value>
                {x.shown}
              </div>
              <div className={s.label}>{x.label}</div>
              {x.networks && x.networks.length > 0 && (
                <div className={s.nets}>
                  {x.networks.map((n) => (
                    <span key={n} className={s.net}>
                      {networkLabel(n)}
                    </span>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      {(recent.length > 0 || total > 0) && (
        <div className={s.badges} data-testid="profile-alvanista-badges">
          {recent.map((b, i) => (
            <BadgeIcon key={`${b.name}-${i}`} badge={b} className={s.badge} />
          ))}
          {total > 0 && <span className={s.more}>{t('profilePage.alvaBadges', { count: total })}</span>}
        </div>
      )}
      {now && (
        <div className={s.now} data-testid="profile-alvanista-now">
          {now.cover_url ? <img className={s.nowCover} src={now.cover_url} alt="" loading="lazy" /> : null}
          <div>
            <div className={s.label}>{t('profilePage.alvaPlaying')}</div>
            <div className={s.nowTitle}>{now.title}</div>
          </div>
        </div>
      )}
      {age && (
        <div className={s.age} data-testid="profile-alvanista-age">
          {t('profilePage.alvaCardAge', { age })}
        </div>
      )}
    </div>
  )
}

/** Sem conexão: o convite. O botão só inicia o fluxo; nada roda antes do clique. */
export function AlvanistaInvite() {
  const { t } = useTranslation()
  const s = useStyles()
  const { connect, cancel, connecting, outcome } = useAlvanistaConnect()
  return (
    <div className={s.card} data-testid="profile-alvanista-invite">
      <div className={s.promo}>
        <img className={s.promoSymbol} src={symbol} alt="" aria-hidden draggable={false} />
        <div>
          <p className={s.promoText}>{t('profilePage.alvaInvite')}</p>
          {connecting ? (
            <div className={s.waiting}>
              <Spinner size="tiny" />
              <span>{t('alvanista.waiting')}</span>
              <Button appearance="secondary" onClick={cancel}>
                {t('alvanista.cancel')}
              </Button>
            </div>
          ) : (
            <AlvanistaButton onClick={connect} />
          )}
        </div>
      </div>
      {outcome && (
        <MessageBar intent="warning" className={s.warn}>
          <MessageBarBody>{t(`alvanista.${outcome}`)}</MessageBarBody>
        </MessageBar>
      )}
    </div>
  )
}
