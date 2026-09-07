import {
  Button,
  Caption1,
  Field,
  Select,
  Spinner,
  Tab,
  TabList,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { ArrowResetRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { getCoreOptions, resetCoreOptions, setCoreOption } from '../lib/tauri'
import { sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS, maxWidth: '440px' },
  head: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' },
  hint: { fontSize: tokens.fontSizeBase100, color: tokens.colorNeutralForeground3 },
})

/**
 * Opções de core **geradas do schema** que o próprio core declarou
 * (`retro_core_options*`). O schema só existe depois do core rodar uma vez.
 *
 * `romId` (opcional): habilita a aba "Este jogo" — os valores gravados ali
 * vencem os valores por core só quando essa ROM está carregada.
 */
export function CoreOptions({ coreId, romId }: { coreId: string; romId?: string }) {
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const [scope, setScope] = useState<'core' | 'rom'>('core')
  const scopeRomId = scope === 'rom' ? romId : undefined

  const key = ['core-options', coreId, romId ?? null] as const
  const opts = useQuery({
    queryKey: key,
    queryFn: () => getCoreOptions(coreId, romId),
    enabled: !!coreId,
    retry: false,
  })

  const change = useMutation({
    mutationFn: (v: { key: string; value: string }) =>
      setCoreOption(coreId, v.key, v.value, scopeRomId),
    onSuccess: () => qc.invalidateQueries({ queryKey: key }),
    onError: (e) => push(sysToast(`Falha ao aplicar opção: ${e}`, 'Error')),
  })
  const reset = useMutation({
    mutationFn: () => resetCoreOptions(coreId, scopeRomId),
    onSuccess: () => qc.invalidateQueries({ queryKey: key }),
    onError: (e) => push(sysToast(`Falha: ${e}`, 'Error')),
  })

  if (!coreId) return null
  if (opts.isLoading) return <Spinner size="tiny" label="Carregando opções…" />
  if (!opts.data || opts.data.length === 0)
    return <Caption1>Sem opções — ou o core ainda não foi executado nenhuma vez.</Caption1>

  return (
    <div className={styles.root}>
      {romId && (
        <TabList
          size="small"
          selectedValue={scope}
          onTabSelect={(_, d) => setScope(d.value as 'core' | 'rom')}
        >
          <Tab value="core">Este core</Tab>
          <Tab value="rom">Este jogo</Tab>
        </TabList>
      )}
      <div className={styles.head}>
        <Caption1 className={styles.hint}>
          {scope === 'rom'
            ? 'Overrides deste jogo — vencem quando ele está carregado.'
            : 'Valem pra todos os jogos deste core.'}
        </Caption1>
        <Button
          size="small"
          appearance="subtle"
          icon={<ArrowResetRegular />}
          disabled={reset.isPending}
          onClick={() => reset.mutate()}
        >
          Resetar
        </Button>
      </div>
      {opts.data.map((o) => {
        // "rom": mostra o override ("" = herda). "core": o valor por core, ou
        // o default do schema.
        const cur =
          scope === 'rom' ? (o.romValue ?? '') : (o.coreValue ?? o.defaultValue)
        return (
          <Field
            key={o.key}
            label={o.displayName}
            hint={
              scope === 'rom' && o.romValue == null
                ? `herdando: ${o.value}`
                : undefined
            }
          >
            <Select
              value={cur}
              disabled={change.isPending}
              onChange={(_, d) => change.mutate({ key: o.key, value: d.value })}
            >
              {scope === 'rom' && <option value="">Herdar ({o.value})</option>}
              {o.choices.map((c) => (
                <option key={c} value={c}>
                  {c === o.defaultValue ? `${c} (padrão)` : c}
                </option>
              ))}
            </Select>
          </Field>
        )
      })}
    </div>
  )
}
