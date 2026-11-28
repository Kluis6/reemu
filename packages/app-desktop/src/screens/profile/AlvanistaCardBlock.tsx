import { Button, MessageBar, MessageBarBody, Spinner, makeStyles, tokens } from '@fluentui/react-components'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import symbol from '../../assets/alvanista-symbol.svg'
import { AlvanistaButton } from '../../components/AlvanistaButton'
import { useAlvanistaConnect } from '../../hooks/useAlvanistaConnect'
import type { AlvanistaCard } from '../../lib/tauri'
import { pickText } from './profileData'

const useStyles = makeStyles({
  card: {
    backgroundColor: tokens.colorNeutralBackground2,
    borderRadius: tokens.borderRadiusMedium,
    padding: '16px 18px',
    minWidth: 0,
  },
  promo: { display: 'flex', alignItems: 'center', columnGap: '16px' },
  promoSymbol: { width: '40px', height: '40px', flexShrink: 0 },
  promoText: { margin: '0 0 10px', color: tokens.colorNeutralForeground2, lineHeight: 1.45 },
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
