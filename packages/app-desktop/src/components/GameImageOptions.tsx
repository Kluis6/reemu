import { Field, Select, Switch, makeStyles, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import {
  getAspect,
  getCoreOptions,
  setAspect,
  setCoreOption,
  type AspectValue,
} from '../lib/tauri'
import { errorToast } from '../lib/toast'
import { widescreenActive, widescreenOption } from '../lib/widescreen'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  row: {
    display: 'grid',
    gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
    alignItems: 'start',
    columnGap: tokens.spacingHorizontalM,
    rowGap: tokens.spacingVerticalS,
  },
})

/**
 * Formato da imagem do jogo, na aba Emulador. Proporção: decisão do ReEmu, em
 * cascata jogo → plataforma → todos; aqui se edita só o jogo. Widescreen 16:9: opção do próprio core,
 * que desenha mais cena (só aparece se ele tiver; ver `lib/widescreen.ts`).
 */
export function GameImageOptions({
  coreId,
  romId,
  leading,
}: {
  coreId: string
  romId: string
  /** Primeira coluna da linha: o seletor de core. */
  leading?: ReactNode
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)

  const aspectKey = ['aspect', romId]
  const aspect = useQuery({ queryKey: aspectKey, queryFn: () => getAspect(romId), retry: false })
  const atRom = aspect.data?.atRom ?? ''
  const change = useMutation({
    mutationFn: (value: string) => setAspect('rom', romId, (value || null) as AspectValue | null),
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
    <div className={s.row}>
      {leading}
      <Field label={t('game.image.aspect')} hint={t('game.image.aspectHint')}>
        <Select
          value={atRom}
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
      {wide && (
        <Field
          label={t('game.image.widescreen')}
          hint={t(wide.conflicts.length > 0 ? 'game.image.widescreenPatchHint' : 'game.image.widescreenHint')}
        >
          <Switch
            checked={widescreenActive(wide)}
            disabled={toggle.isPending}
            onChange={(_, d) => toggle.mutate(d.checked)}
          />
        </Field>
      )}
    </div>
  )
}
