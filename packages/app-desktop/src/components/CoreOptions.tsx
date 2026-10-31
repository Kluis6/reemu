import {
  Button,
  Caption1,
  MessageBar,
  MessageBarBody,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { ArrowResetRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { getCoreOptions, resetCoreOptions, setCoreOption } from '../lib/tauri'
import { errorToast } from '../lib/toast'
import { toggleValues } from '../lib/coreOptions'
import { useToastStore } from '../stores/useToastStore'
import { useTranslation } from 'react-i18next'
import { BlockGroup, SelectBlock, ToggleBlock } from './SettingBlock'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalS },
  head: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' },
  hint: { fontSize: tokens.fontSizeBase100, color: tokens.colorNeutralForeground3 },
})

/**
 * Opções de core **geradas do schema** que o próprio core declarou
 * (`retro_core_options*`). O schema só existe depois do core rodar uma vez.
 *
 * `romId`: edita só este jogo — os valores gravados ali vencem os valores por
 * core quando essa ROM está carregada. Sem ele, edita os valores do core; a
 * página do jogo não oferece isso (pedido do usuário, 2026-10-31), e o ajuste
 * por core vai para as Configurações.
 */
export function CoreOptions({ coreId, romId }: { coreId: string; romId?: string }) {
  const { t } = useTranslation()
  const styles = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const scope = romId ? 'rom' : 'core'

  const key = ['core-options', coreId, romId ?? null] as const
  const opts = useQuery({
    queryKey: key,
    queryFn: () => getCoreOptions(coreId, romId),
    enabled: !!coreId,
    retry: false,
  })

  const change = useMutation({
    mutationFn: (v: { key: string; value: string }) =>
      setCoreOption(coreId, v.key, v.value, romId),
    onSuccess: () => qc.invalidateQueries({ queryKey: key }),
    onError: (e) => push(errorToast(e, 'applyCoreOption')),
  })
  const reset = useMutation({
    mutationFn: () => resetCoreOptions(coreId, romId),
    onSuccess: () => qc.invalidateQueries({ queryKey: key }),
    onError: (e) => push(errorToast(e, 'resetCoreOptions')),
  })

  if (!coreId) return null
  if (opts.isLoading) return <Spinner size="tiny" label={t('coreOptions.loading')} />
  if (!opts.data || opts.data.length === 0)
    // O core só informa as opções ao carregar um jogo; o ReEmu as guarda nessa hora.
    return romId ? (
      <Caption1>{t('coreOptions.none')}</Caption1>
    ) : (
      <MessageBar intent="info">
        <MessageBarBody>{t('coreOptions.notRunYet')}</MessageBarBody>
      </MessageBar>
    )

  return (
    <div className={styles.root}>
      <div className={styles.head}>
        <Caption1 className={styles.hint}>
          {scope === 'rom'
            ? t('coreOptions.romHint')
            : t('coreOptions.coreHint')}
        </Caption1>
        <Button
          appearance="subtle"
          icon={<ArrowResetRegular />}
          disabled={reset.isPending}
          onClick={() => reset.mutate()}
        >
          {t('coreOptions.reset')}
        </Button>
      </div>
      <BlockGroup>
        {opts.data.map((o) => {
          // Só nas opções do core: no jogo também existe o "Herdar", um 3º estado.
          const toggle = romId ? null : toggleValues(o.choices)
          // "rom": mostra o override ("" = herda). "core": o valor por core,
          // ou o default do schema.
          const cur =
            scope === 'rom' ? (o.romValue ?? '') : (o.coreValue ?? o.defaultValue)
          if (toggle)
            return (
              <ToggleBlock
                key={o.key}
                label={o.displayName}
                checked={cur === toggle.on}
                disabled={change.isPending}
                onChange={(on) =>
                  change.mutate({ key: o.key, value: on ? toggle.on : toggle.off })
                }
              />
            )
          return (
            <SelectBlock
              key={o.key}
              label={o.displayName}
              value={cur}
              disabled={change.isPending}
              options={[
                ...(scope === 'rom'
                  ? [{ value: '', label: t('coreOptions.inherit', { value: o.value }) }]
                  : []),
                ...o.choices.map((c) => ({
                  value: c,
                  label: c === o.defaultValue ? t('coreOptions.default', { value: c }) : c,
                })),
              ]}
              onChange={(value) => change.mutate({ key: o.key, value })}
            />
          )
        })}
      </BlockGroup>
    </div>
  )
}
