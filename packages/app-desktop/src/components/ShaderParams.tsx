import { Button, Caption1, Slider, makeStyles, tokens } from '@fluentui/react-components'
import { ArrowResetRegular } from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { getShaderParams, resetShaderParams, setShaderParam, type ShaderParam } from '../lib/tauri'
import { sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  row: { display: 'grid', gridTemplateColumns: '1fr', gap: tokens.spacingVerticalXXS },
  head: { display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' },
  val: { fontVariantNumeric: 'tabular-nums', color: tokens.colorNeutralForeground3 },
})

/**
 * Controles dos `#pragma parameter` do shader ativo. `scope` decide onde
 * persiste: `'default'` (Config › Vídeo) ou `'rom'` (+ `romId`, no RomDetail).
 * Não renderiza nada se o preset ativo não tem parâmetros (builtins).
 */
export function ShaderParams({
  scope,
  romId,
  reloadKey,
}: {
  scope: 'default' | 'rom'
  romId?: string
  /** muda quando o preset troca lá fora → refaz o fetch. */
  reloadKey?: string
}) {
  const s = useStyles()
  const push = useToastStore((st) => st.push)
  const q = useQuery({
    queryKey: ['shader-params', scope, romId ?? null, reloadKey ?? null],
    queryFn: getShaderParams,
    retry: false,
  })

  // só os valores que o usuário mexeu nesta sessão; o resto vem do fetch.
  const [dirty, setDirty] = useState<Record<string, number>>({})

  // persiste com debounce (o slider dispara muitos onChange)
  const timers = useRef<Record<string, ReturnType<typeof setTimeout>>>({})
  const commit = (name: string, value: number) => {
    setDirty((v) => ({ ...v, [name]: value }))
    clearTimeout(timers.current[name])
    timers.current[name] = setTimeout(() => {
      void setShaderParam(name, value, scope, romId).catch((e) =>
        push(sysToast(`Falha ao salvar parâmetro: ${e}`, 'Error')),
      )
    }, 200)
  }

  const reset = () => {
    void resetShaderParams(scope, romId)
      .then(() => {
        setDirty({})
        return q.refetch()
      })
      .catch((e) => push(sysToast(`Falha: ${e}`, 'Error')))
  }

  const params: ShaderParam[] = q.data ?? []
  if (params.length === 0) return null

  return (
    <div className={s.root}>
      <div className={s.head}>
        <Caption1>Parâmetros do shader</Caption1>
        <Button size="small" appearance="subtle" icon={<ArrowResetRegular />} onClick={reset}>
          Restaurar padrões
        </Button>
      </div>
      {params.map((p) => {
        const v = dirty[p.name] ?? p.value
        return (
          <div key={p.name} className={s.row}>
            <div className={s.head}>
              <Caption1>{p.label || p.name}</Caption1>
              <span className={s.val}>{Number.isInteger(v) ? v : v.toFixed(2)}</span>
            </div>
            <Slider
              min={p.min}
              max={p.max}
              step={p.step > 0 ? p.step : undefined}
              value={v}
              onChange={(_, d) => commit(p.name, d.value)}
            />
          </div>
        )
      })}
    </div>
  )
}
