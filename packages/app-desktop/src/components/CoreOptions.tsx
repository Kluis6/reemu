import { Caption1, Field, Select, Spinner, makeStyles, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { getCoreOptions, setCoreOption } from '../lib/tauri'
import { sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS, maxWidth: '440px' },
})

/**
 * Opções de core **geradas do schema** que o próprio core declarou
 * (`retro_core_options*`). As opções libretro são sempre enumeradas → um
 * `<Select>` por opção. O schema só existe depois do core rodar uma vez.
 */
export function CoreOptions({ coreId }: { coreId: string }) {
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)

  const opts = useQuery({
    queryKey: ['core-options', coreId],
    queryFn: () => getCoreOptions(coreId),
    enabled: !!coreId,
    retry: false,
  })

  const change = useMutation({
    mutationFn: (v: { key: string; value: string }) => setCoreOption(coreId, v.key, v.value),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['core-options', coreId] }),
    onError: (e) => push(sysToast(`Falha ao aplicar opção: ${e}`, 'Error')),
  })

  if (!coreId) return null
  if (opts.isLoading) return <Spinner size="tiny" label="Carregando opções…" />
  if (!opts.data || opts.data.length === 0)
    return <Caption1>Sem opções — ou o core ainda não foi executado nenhuma vez.</Caption1>

  return (
    <div className={styles.root}>
      {opts.data.map((o) => (
        <Field key={o.key} label={o.displayName}>
          <Select
            value={o.value}
            disabled={change.isPending}
            onChange={(_, d) => change.mutate({ key: o.key, value: d.value })}
          >
            {o.choices.map((c) => (
              <option key={c} value={c}>
                {c === o.defaultValue ? `${c} (padrão)` : c}
              </option>
            ))}
          </Select>
        </Field>
      ))}
    </div>
  )
}
