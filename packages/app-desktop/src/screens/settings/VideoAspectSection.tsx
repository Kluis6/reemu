import {
  Badge,
  Body1,
  Button,
  Caption1,
  CounterBadge,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { ArrowResetRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { LoadingState } from '../../components/EmptyState'
import { BlockGroup, SelectBlock } from '../../components/SettingBlock'
import { knownPlatforms, platformLabel } from '../../lib/platform'
import { getAspectSettings, listRoms, setAspect, type AspectValue } from '../../lib/tauri'
import { errorToast, sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
    maxWidth: '780px',
  },
  hint: { color: tokens.colorNeutralForeground3 },
  overridesSection: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
  },
  overridesHead: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalS,
  },
  overrideList: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXS,
  },
  overrideRow: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    columnGap: tokens.spacingHorizontalM,
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: tokens.colorNeutralBackground2,
  },
  overrideInfo: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalS,
    minWidth: 0,
  },
  platformBtn: {
    justifyContent: 'flex-start',
    paddingLeft: 0,
    paddingRight: 0,
    minWidth: 0,
  },
})

export function VideoAspectSection() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const [selectedPlatform, setSelectedPlatform] = useState('')

  const aspectSettings = useQuery({
    queryKey: ['aspect-settings'],
    queryFn: getAspectSettings,
    retry: false,
  })
  const roms = useQuery({
    queryKey: ['roms'],
    queryFn: listRoms,
    retry: false,
  })

  const systems = aspectSettings.data?.systems ?? {}
  const platforms = useMemo(
    () => knownPlatforms().filter(([id]) => id !== 'disc' || id in systems),
    [systems],
  )
  const overriddenEntries = useMemo(
    () =>
      Object.entries(systems).sort(([a], [b]) =>
        platformLabel(a).localeCompare(platformLabel(b)),
      ),
    [systems],
  )

  const defaultPlatform = useMemo(() => {
    if (overriddenEntries.length > 0) return overriddenEntries[0][0]
    const fromLibrary = roms.data?.[0]?.systemId
    if (fromLibrary && platforms.some(([id]) => id === fromLibrary)) return fromLibrary
    return platforms[0]?.[0] ?? 'snes'
  }, [overriddenEntries, roms.data, platforms])

  const activePlatform = selectedPlatform || defaultPlatform

  const aspectOptions = useMemo(
    () => [
      { value: 'core', label: t('game.image.core') },
      { value: '4:3', label: '4:3' },
      { value: '16:9', label: '16:9' },
      { value: 'fill', label: t('game.image.fill') },
    ],
    [t],
  )

  const aspectLabel = (val: string) =>
    aspectOptions.find((o) => o.value === val)?.label ?? val

  const invalidate = () => {
    void qc.invalidateQueries({ queryKey: ['aspect-settings'] })
    void qc.invalidateQueries({ queryKey: ['aspect'] })
  }

  const changeDefault = useMutation({
    mutationFn: (value: AspectValue) => setAspect('default', '', value),
    onSuccess: (_, value) => {
      invalidate()
      push(sysToast(t('video.aspectDefaultSaved', { value: aspectLabel(value) }), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'changeAspect')),
  })

  const changePlatform = useMutation({
    mutationFn: ({ systemId, value }: { systemId: string; value: string }) =>
      setAspect('system', '', (value || null) as AspectValue | null, systemId),
    onSuccess: (_, { systemId, value }) => {
      invalidate()
      const platform = platformLabel(systemId)
      push(
        sysToast(
          value
            ? t('video.aspectPlatformSaved', { platform, value: aspectLabel(value) })
            : t('video.aspectPlatformInherited', { platform }),
          'Success',
        ),
      )
    },
    onError: (e) => push(errorToast(e, 'changeAspect')),
  })

  if (aspectSettings.isLoading) return <LoadingState />
  if (aspectSettings.isError || !aspectSettings.data) {
    return <Body1>{t('video.unavailable')}</Body1>
  }

  const atDefault = aspectSettings.data.atDefault ?? 'core'
  const atPlatform = systems[activePlatform] ?? ''

  return (
    <div className={s.root}>
      <Caption1 className={s.hint}>{t('video.aspectHint')}</Caption1>

      <BlockGroup title={t('video.aspectGlobalTitle')}>
        <SelectBlock
          label={t('video.aspectDefault')}
          value={atDefault}
          disabled={changeDefault.isPending}
          options={aspectOptions}
          onChange={(v) => changeDefault.mutate(v as AspectValue)}
        />
      </BlockGroup>

      <BlockGroup title={t('video.aspectPlatformTitle')}>
        <SelectBlock
          label={t('common.platform')}
          value={activePlatform}
          options={platforms.map(([id, label]) => ({
            value: id,
            label: systems[id] ? `${label} (${aspectLabel(systems[id])})` : label,
          }))}
          onChange={setSelectedPlatform}
        />
        <SelectBlock
          label={t('game.image.aspect')}
          value={atPlatform}
          disabled={changePlatform.isPending}
          options={[
            { value: '', label: t('video.inheritDefault') },
            ...aspectOptions,
          ]}
          onChange={(value) => changePlatform.mutate({ systemId: activePlatform, value })}
        />
      </BlockGroup>

      <section className={s.overridesSection} aria-label={t('video.aspectOverridesTitle')}>
        <div className={s.overridesHead}>
          <Body1 as="h3" style={{ margin: 0 }}>
            {t('video.aspectOverridesTitle')}
          </Body1>
          {overriddenEntries.length > 0 && (
            <CounterBadge
              count={overriddenEntries.length}
              size="small"
              color="brand"
            />
          )}
        </div>
        {overriddenEntries.length === 0 ? (
          <Caption1 className={s.hint}>{t('video.aspectNoOverrides')}</Caption1>
        ) : (
          <div className={s.overrideList} role="list">
            {overriddenEntries.map(([sys, val]) => (
              <div key={sys} className={s.overrideRow} role="listitem">
                <div className={s.overrideInfo}>
                  <Button
                    appearance="transparent"
                    className={s.platformBtn}
                    onClick={() => setSelectedPlatform(sys)}
                  >
                    <Text weight="semibold">{platformLabel(sys)}</Text>
                  </Button>
                  <Badge appearance="tint" color="brand" shape="rounded">
                    {aspectLabel(val)}
                  </Badge>
                </div>
                <Button
                  appearance="subtle"
                  icon={<ArrowResetRegular />}
                  disabled={changePlatform.isPending}
                  aria-label={t('video.resetPlatformAspect', { platform: platformLabel(sys) })}
                  onClick={() => changePlatform.mutate({ systemId: sys, value: '' })}
                >
                  {t('video.resetToDefault')}
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  )
}
