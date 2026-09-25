import {
  Badge,
  Button,
  Caption1,
  Select,
  Spinner,
  Subtitle2,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  clearControllerMapping,
  clearDevicePort,
  describeRawInput,
  listControllerMappings,
  listDevicePorts,
  listGamepads,
  RETROPAD_BUTTONS,
  setDevicePort,
  type ControllerMapping,
} from '../lib/tauri'
import { EmptyState } from './EmptyState'
import { useBindingCaptureStore } from '../stores/useBindingCaptureStore'
import { useToastStore } from '../stores/useToastStore'
import { errorToast } from '../lib/toast'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  device: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXS,
    padding: tokens.spacingVerticalS,
    borderRadius: tokens.borderRadiusMedium,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  deviceHead: { display: 'flex', alignItems: 'center', justifyContent: 'space-between' },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(150px, 1fr))',
    gap: tokens.spacingHorizontalXS,
  },
  cell: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: tokens.spacingHorizontalXS,
    fontSize: tokens.fontSizeBase200,
  },
  bound: { color: tokens.colorNeutralForeground3 },
})

const toast = (message: string, variant: 'Success' | 'Error') => ({
  id: crypto.randomUUID(),
  message,
  variant,
  durationMs: variant === 'Error' ? 4000 : 2500,
  source: 'System' as const,
})

/** Combina os gamepads conectados agora com os mapas já salvos no DB. */
function useDevices() {
  const gamepads = useQuery({ queryKey: ['gamepads'], queryFn: listGamepads, retry: false })
  const mappings = useQuery({
    queryKey: ['controller-mappings'],
    queryFn: listControllerMappings,
    retry: false,
  })
  const byGuid = new Map<string, { name: string; mapping?: ControllerMapping; connected: boolean }>()
  for (const g of gamepads.data ?? []) byGuid.set(g.guid, { name: g.name, connected: true })
  for (const m of mappings.data ?? []) {
    const e = byGuid.get(m.guid)
    if (e) e.mapping = m
    else byGuid.set(m.guid, { name: m.displayName, mapping: m, connected: false })
  }
  return { devices: [...byGuid.entries()], loading: gamepads.isLoading || mappings.isLoading }
}

export function ControllerMappings() {
  const { t } = useTranslation()
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const beginCapture = useBindingCaptureStore((s) => s.begin)
  const { devices, loading } = useDevices()

  const ports = useQuery({ queryKey: ['device-ports'], queryFn: listDevicePorts, retry: false })
  const portFor = (guid: string) => ports.data?.find((p) => p.guid === guid)?.port ?? null

  const clearMap = useMutation({
    mutationFn: (guid: string) => clearControllerMapping(guid),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['controller-mappings'] })
      push(toast(t('controllers.mapRemoved'), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'deleteControllerMapping')),
  })

  const assignPort = useMutation({
    mutationFn: ({ guid, port }: { guid: string; port: number | null }) =>
      port === null ? clearDevicePort(guid) : setDevicePort(guid, port),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['device-ports'] }),
    onError: (e) => push(errorToast(e, 'setControllerPort')),
  })

  if (loading) return <Spinner label={t('controllers.searching')} />
  if (devices.length === 0)
    return (
      <EmptyState title={t('controllers.noneTitle')}>
        {t('controllers.noneHint')}
      </EmptyState>
    )

  return (
    <div className={styles.root}>
      <Caption1>
        {t('controllers.intro')}
      </Caption1>
      {devices.map(([guid, dev]) => {
        const bound = new Map(dev.mapping?.entries.map((e) => [e.button, e]) ?? [])
        return (
          <div key={guid} className={styles.device}>
            <div className={styles.deviceHead}>
              <Subtitle2>{dev.name}</Subtitle2>
              <span style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <Badge appearance="tint" color={dev.connected ? 'success' : 'informative'}>
                  {dev.connected ? t('controllers.connected') : t('controllers.saved')}
                </Badge>
                <Select
                  size="small"
                  value={portFor(guid) === null ? '' : String(portFor(guid))}
                  disabled={assignPort.isPending}
                  onChange={(_, d) =>
                    assignPort.mutate({ guid, port: d.value === '' ? null : Number(d.value) })
                  }
                >
                  <option value="">{t('controllers.portAuto')}</option>
                  {[0, 1, 2, 3].map((p) => (
                    <option key={p} value={String(p)}>
                      {t('controllers.port', { n: p + 1 })}
                    </option>
                  ))}
                </Select>
                {dev.mapping && (
                  <Button
                    size="small"
                    appearance="subtle"
                    disabled={clearMap.isPending}
                    onClick={() => clearMap.mutate(guid)}
                  >
                    {t('controllers.clearMap')}
                  </Button>
                )}
              </span>
            </div>
            <div className={styles.grid}>
              {RETROPAD_BUTTONS.map((btn) => {
                const entry = bound.get(btn)
                return (
                  <span key={btn} className={styles.cell}>
                    <span>
                      <Text as="strong" weight="semibold">{btn}</Text>
                      {entry && (
                        <span className={styles.bound}>
                          {' '}
                          {entry.trigger.map(describeRawInput).join(' + ')}
                        </span>
                      )}
                    </span>
                    <Button
                      size="small"
                      appearance="transparent"
                      onClick={() =>
                        beginCapture({
                          target: 'controller_mapping',
                          targetKey: `${guid}::${dev.name}::${btn}`,
                          label: `${dev.name} → ${btn}`,
                        })
                      }
                    >
                      {entry ? '↻' : '+'}
                    </Button>
                  </span>
                )
              })}
            </div>
          </div>
        )
      })}
    </div>
  )
}
