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
import { errorToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  head: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' },
  hint: { fontSize: tokens.fontSizeBase100, color: tokens.colorNeutralForeground3 },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
    columnGap: tokens.spacingHorizontalM,
    rowGap: tokens.spacingVerticalS,
  },
})

/**
 * Opções de core **geradas do schema** que o próprio core declarou
 * (`retro_core_options*`). O schema só existe depois do core rodar uma vez.
 *
 * `romId` (opcional): habilita a aba "Este jogo" — os valores gravados ali
 * vencem os valores por core só quando essa ROM está carregada.
 */
export function CoreOptions({ coreId, romId }: { coreId: string; romId?: string }) {
  const { t } = useTranslation()
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
    onError: (e) => push(errorToast(e, 'applyCoreOption')),
  })
  const reset = useMutation({
    mutationFn: () => resetCoreOptions(coreId, scopeRomId),
    onSuccess: () => qc.invalidateQueries({ queryKey: key }),
    onError: (e) => push(errorToast(e, 'resetCoreOptions')),
  })

  if (!coreId) return null
  if (opts.isLoading) return <Spinner size="tiny" label={t('coreOptions.loading')} />
  if (!opts.data || opts.data.length === 0)
    return <Caption1>{t('coreOptions.none')}</Caption1>

  return (
    <div className={styles.root}>
      {romId && (
        <TabList
          size="small"
          selectedValue={scope}
          onTabSelect={(_, d) => setScope(d.value as 'core' | 'rom')}
        >
          <Tab value="core">{t('coreOptions.thisCore')}</Tab>
          <Tab value="rom">{t('coreOptions.thisGame')}</Tab>
        </TabList>
      )}
      <div className={styles.head}>
        <Caption1 className={styles.hint}>
          {scope === 'rom'
            ? t('coreOptions.romHint')
            : t('coreOptions.coreHint')}
        </Caption1>
        <Button
          size="small"
          appearance="subtle"
          icon={<ArrowResetRegular />}
          disabled={reset.isPending}
          onClick={() => reset.mutate()}
        >
          {t('coreOptions.reset')}
        </Button>
      </div>
      <div className={styles.grid}>
        {opts.data.map((o) => {
          // "rom": mostra o override ("" = herda). "core": o valor por core,
          // ou o default do schema.
          const cur =
            scope === 'rom' ? (o.romValue ?? '') : (o.coreValue ?? o.defaultValue)
          return (
            <Field
              key={o.key}
              label={o.displayName}
              hint={
                scope === 'rom' && o.romValue == null
                  ? t('coreOptions.inheriting', { value: o.value })
                  : undefined
              }
            >
              <Select
                value={cur}
                disabled={change.isPending}
                onChange={(_, d) => change.mutate({ key: o.key, value: d.value })}
              >
                {scope === 'rom' && <option value="">{t('coreOptions.inherit', { value: o.value })}</option>}
                {o.choices.map((c) => (
                  <option key={c} value={c}>
                    {c === o.defaultValue ? t('coreOptions.default', { value: c }) : c}
                  </option>
                ))}
              </Select>
            </Field>
          )
        })}
      </div>
    </div>
  )
}
