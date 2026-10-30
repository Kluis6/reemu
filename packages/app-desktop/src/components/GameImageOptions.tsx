import { Caption1, Field, Select, Switch, makeStyles, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  getAspect,
  getCoreOptions,
  setAspect,
  setCoreOption,
  type AspectScope,
  type AspectValue,
} from '../lib/tauri'
import { errorToast } from '../lib/toast'
import { widescreenActive, widescreenOption } from '../lib/widescreen'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalS },
  row: {
    display: 'grid',
    gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
    columnGap: tokens.spacingHorizontalM,
    rowGap: tokens.spacingVerticalS,
  },
  hint: { color: tokens.colorNeutralForeground3 },
})

/**
 * Formato da imagem do jogo, na aba Emulador. Proporção: decisão do ReEmu, em
 * cascata jogo → plataforma → todos. Widescreen 16:9: opção do próprio core,
 * que desenha mais cena (só aparece se ele tiver; ver `lib/widescreen.ts`).
 */
export function GameImageOptions({ coreId, romId }: { coreId: string; romId: string }) {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const [scope, setScope] = useState<AspectScope>('rom')

  const aspectKey = ['aspect', romId]
  const aspect = useQuery({ queryKey: aspectKey, queryFn: () => getAspect(romId), retry: false })
  const atScope =
    (scope === 'rom'
      ? aspect.data?.atRom
      : scope === 'system'
        ? aspect.data?.atSystem
        : aspect.data?.atDefault) ?? ''
  const change = useMutation({
    mutationFn: (value: string) => setAspect(scope, romId, (value || null) as AspectValue | null),
    onSuccess: () => qc.invalidateQueries({ queryKey: aspectKey }),
    onError: (e) => push(errorToast(e, 'changeAspect')),
  })

  // mesma chave de `CoreOptions`: as duas telas enxergam a mesma lista
  const optionsKey = ['core-options', coreId, romId] as const
  const options = useQuery({
    queryKey: optionsKey,
    queryFn: () => getCoreOptions(coreId, romId),
    enabled: !!coreId,
    retry: false,
  })
  const wide = widescreenOption(coreId, options.data)
  const toggle = useMutation({
    mutationFn: async (on: boolean) => {
      if (!wide) return
      // Desligar limpa o valor do jogo; se o core inteiro estiver ligado,
      // limpar não basta e o jogo grava o padrão.
      const off = wide.option.coreValue === wide.on ? wide.option.defaultValue : ''
      await setCoreOption(coreId, wide.option.key, on ? wide.on : off, romId)
      // ligar tira o que se somaria a ela (ver `lib/widescreen.ts`)
      if (on)
        for (const c of wide.conflicts)
          await setCoreOption(coreId, c.key, c.defaultValue, romId)
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: optionsKey }),
    onError: (e) => push(errorToast(e, 'applyCoreOption')),
  })

  return (
    <div className={s.root}>
      <div className={s.row}>
        <Field label={t('game.image.aspect')}>
          <Select
            value={atScope}
            disabled={change.isPending}
            onChange={(_, d) => change.mutate(d.value)}
          >
            <option value="">{t('game.image.inherit')}</option>
            <option value="core">{t('game.image.core')}</option>
            <option value="4:3">4:3</option>
            <option value="16:9">16:9</option>
            <option value="fill">{t('game.image.fill')}</option>
          </Select>
        </Field>
        <Field label={t('game.image.applyTo')}>
          <Select value={scope} onChange={(_, d) => setScope(d.value as AspectScope)}>
            <option value="rom">{t('game.image.scopeRom')}</option>
            <option value="system">{t('game.image.scopeSystem')}</option>
            <option value="default">{t('game.image.scopeDefault')}</option>
          </Select>
        </Field>
      </div>
      <Caption1 className={s.hint}>{t('game.image.aspectHint')}</Caption1>
      {wide && (
        <>
          <Switch
            label={t('game.image.widescreen')}
            checked={widescreenActive(wide)}
            disabled={toggle.isPending}
            onChange={(_, d) => toggle.mutate(d.checked)}
          />
          <Caption1 className={s.hint}>
            {t(wide.conflicts.length > 0 ? 'game.image.widescreenPatchHint' : 'game.image.widescreenHint')}
          </Caption1>
        </>
      )}
    </div>
  )
}
