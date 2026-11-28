import {
  Button,
  Caption1,
  makeStyles,
  mergeClasses,
  Spinner,
  tokens,
} from '@fluentui/react-components'
import {
  ClockArrowDownloadRegular,
  LibraryRegular,
} from '@fluentui/react-icons'
import { useTranslation } from 'react-i18next'
import { useAlvanistaConnect } from '../../hooks/useAlvanistaConnect'
import {
  useAlvanistaCollectionItem,
  useUpdateCollectionItem,
} from '../../hooks/useCollection'
import type { CollectionJourneyStatus, CollectionPatch } from '../../lib/tauri'
import { sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'
import { BlockGroup, SelectBlock, ToggleBlock } from '../SettingBlock'

const JOURNEY_ORDER: CollectionJourneyStatus[] = [
  'no_status',
  'unplayed',
  'want_to_play',
  'backlog',
  'playing',
  'played',
  'finished',
  'completed',
  'abandoned',
]

const useStyles = makeStyles({
  row: {
    display: 'flex',
    alignItems: 'center',
    flexWrap: 'wrap',
    gap: '8px',
    color: tokens.colorNeutralForeground2,
  },
  icon: {
    fontSize: '20px',
    flexShrink: 0,
  },
  consent: {
    display: 'grid',
    gridTemplateColumns: 'auto minmax(0, 1fr)',
    alignItems: 'start',
  },
  consentContent: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-start',
    gap: tokens.spacingVerticalS,
  },
  panel: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'stretch',
    gap: tokens.spacingVerticalM,
  },
  syncButton: { alignSelf: 'flex-start' },
  saveNote: { color: tokens.colorNeutralForeground3 },
})

export function AlvanistaCollectionConnectPrompt({ connected, className }: {
  connected: boolean
  className?: string
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const { connect, cancel, connecting } = useAlvanistaConnect({
    scopes: ['collection'],
  })

  return (
    <div className={mergeClasses(s.row, className, s.consent)} data-testid="alva-collection-consent">
      <LibraryRegular className={s.icon} aria-hidden />
      <div className={s.consentContent}>
        <span>{t('collection.connectPrompt')}</span>
        {connecting ? (
          <>
            <span>{t('collection.connecting')}</span>
            <Button size="small" appearance="subtle" onClick={cancel}>
              {t('collection.cancelConnect')}
            </Button>
          </>
        ) : (
          <Button size="small" appearance="secondary" onClick={connect}>
            {t(connected ? 'collection.connectButton' : 'alvanista.signIn')}
          </Button>
        )}
      </div>
    </div>
  )
}

export function AlvanistaCollectionBar({
  gameUuid,
  systemId,
  playTimeSeconds,
}: {
  gameUuid: string | null | undefined
  systemId: string
  playTimeSeconds: number
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const push = useToastStore((st) => st.push)
  const { accountPending, connected, itemQuery, errorCode, needsAccess } = useAlvanistaCollectionItem(gameUuid)
  const update = useUpdateCollectionItem(gameUuid ?? '', systemId)

  if (!gameUuid) return <Caption1>{t('collection.unlinked')}</Caption1>
  if (accountPending) return <Spinner size="small" label={t('common.loading')} />
  if (needsAccess) return <AlvanistaCollectionConnectPrompt connected={connected} />

  if (errorCode === 'offline') {
    return (
      <div className={s.row}>
        <LibraryRegular className={s.icon} aria-hidden />
        <span>{t('collection.offline')}</span>
      </div>
    )
  }

  const item = itemQuery.data
  if (itemQuery.isPending) return <Spinner size="small" label={t('common.loading')} />
  if (itemQuery.isError || !item) return <Caption1>{t('collection.unavailable')}</Caption1>

  const localMinutes = Math.floor(playTimeSeconds / 60)
  const canSyncTime = localMinutes > 0 && localMinutes > item.playtimeMinutes
  const currentStatus = JOURNEY_ORDER.find((status) => status === item.status) ?? 'no_status'

  const updateCollection = (patch: Omit<CollectionPatch, 'system'>) => {
    if (update.isPending) return
    update.mutate(patch, { onError: () => push(sysToast(t('collection.updateFailed'), 'Error')) })
  }

  const handleSyncTime = () => {
    update.mutate(
      { playtimeMinutes: localMinutes },
      {
        onSuccess: () => push(sysToast(t('collection.timeSynced'), 'Success')),
        onError: () => push(sysToast(t('collection.updateFailed'), 'Error')),
      },
    )
  }

  return (
    <div className={s.panel} data-testid="alva-collection-bar">
      <BlockGroup fill>
        <ToggleBlock
          label={t('collection.inCollection')}
          checked={item.inCollection}
          busy={update.isPending}
          onChange={(checked) => updateCollection({ inCollection: checked })}
        />
        <ToggleBlock
          label={t('collection.addWishlist')}
          checked={item.inWishlist}
          busy={update.isPending}
          onChange={(checked) => updateCollection({ inWishlist: checked })}
        />
        <SelectBlock
          label={t('collection.journeyLabel')}
          value={currentStatus}
          options={JOURNEY_ORDER.map((status) => ({ value: status, label: t(`collection.status.${status}`) }))}
          onChange={(value) => {
            const status = JOURNEY_ORDER.find((entry) => entry === value)
            if (status && status !== currentStatus) updateCollection({ status })
          }}
        />
      </BlockGroup>

      {canSyncTime && (
        <Button
          className={s.syncButton}
          appearance="secondary"
          disabledFocusable={update.isPending}
          icon={<ClockArrowDownloadRegular />}
          onClick={handleSyncTime}
        >
          {update.isPending
            ? t('collection.syncingTime')
            : t('collection.syncTime', { minutes: localMinutes })}
        </Button>
      )}
      <Caption1 className={s.saveNote}>{t('collection.autoSave')}</Caption1>
    </div>
  )
}
