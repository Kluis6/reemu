import {
  Badge,
  Body1,
  Button,
  Caption1,
  Select,
  Spinner,
  Subtitle2,
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
import { useBindingCaptureStore } from '../stores/useBindingCaptureStore'
import { useToastStore } from '../stores/useToastStore'

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
      push(toast('Mapa de controle removido (volta ao padrão).', 'Success'))
    },
    onError: (e) => push(toast(`Falha: ${e}`, 'Error')),
  })

  const assignPort = useMutation({
    mutationFn: ({ guid, port }: { guid: string; port: number | null }) =>
      port === null ? clearDevicePort(guid) : setDevicePort(guid, port),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['device-ports'] }),
    onError: (e) => push(toast(`Falha ao definir a porta: ${e}`, 'Error')),
  })

  if (loading) return <Spinner label="Procurando controles…" />
  if (devices.length === 0)
    return <Body1>Nenhum controle conectado nem mapa salvo.</Body1>

  return (
    <div className={styles.root}>
      <Caption1>
        Sem override, o botão do controle segue o padrão do SDL_GameControllerDB.
        Combinação é opção avançada — o normal é um botão por função.
      </Caption1>
      {devices.map(([guid, dev]) => {
        const bound = new Map(dev.mapping?.entries.map((e) => [e.button, e]) ?? [])
        return (
          <div key={guid} className={styles.device}>
            <div className={styles.deviceHead}>
              <Subtitle2>{dev.name}</Subtitle2>
              <span style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <Badge appearance="tint" color={dev.connected ? 'success' : 'informative'}>
                  {dev.connected ? 'conectado' : 'salvo'}
                </Badge>
                <Select
                  size="small"
                  value={portFor(guid) === null ? '' : String(portFor(guid))}
                  disabled={assignPort.isPending}
                  onChange={(_, d) =>
                    assignPort.mutate({ guid, port: d.value === '' ? null : Number(d.value) })
                  }
                >
                  <option value="">Porta: auto</option>
                  <option value="0">Porta 1</option>
                  <option value="1">Porta 2</option>
                  <option value="2">Porta 3</option>
                  <option value="3">Porta 4</option>
                </Select>
                {dev.mapping && (
                  <Button
                    size="small"
                    appearance="subtle"
                    disabled={clearMap.isPending}
                    onClick={() => clearMap.mutate(guid)}
                  >
                    Limpar mapa
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
                      <strong>{btn}</strong>
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
