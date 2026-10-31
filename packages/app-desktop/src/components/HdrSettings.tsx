import {
  Body1,
  Button,
  Field,
  Slider,
  Switch,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { ArrowResetRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  getHdrStatus,
  getVideoConfig,
  updateVideoConfig,
  type HdrStatus,
  type VideoConfig,
} from '../lib/tauri'
import { errorToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalL },
  // Slider: rótulo e valor atual na mesma linha, a faixa embaixo.
  // Rótulo e valor alinhados com a largura da faixa.
  head: {
    display: 'flex',
    alignItems: 'baseline',
    justifyContent: 'space-between',
    columnGap: tokens.spacingHorizontalM,
    maxWidth: '480px',
  },
  value: { color: tokens.colorNeutralForeground2, fontVariantNumeric: 'tabular-nums' },
  slider: { maxWidth: '480px', width: '100%' },
  reset: { alignSelf: 'flex-start' },
})

/** Grava 300 ms depois do último movimento do slider (não a cada pixel). */
const DEBOUNCE_MS = 300

/**
 * Saída HDR da janela do jogo (Configurações › Sistema › Vídeo). Switches
 * aplicam na hora e sliders pra valores aproximados (brilho), com o valor
 * atual à vista — orientações de uso de Switch e Slider do Fluent 2.
 */
export function HdrSettings() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const cfg = useQuery({ queryKey: ['video-config'], queryFn: getVideoConfig, retry: false })
  const status = useQuery({
    queryKey: ['video-hdr-status'],
    queryFn: getHdrStatus,
    retry: false,
    // o usuário pode ligar o HDR no sistema com a tela aberta
    refetchInterval: 5000,
  })
  const save = useMutation({
    mutationFn: (patch: Partial<VideoConfig>) => {
      const base = cfg.data
      if (!base) return Promise.resolve()
      return updateVideoConfig({ ...base, ...patch })
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['video-config'] })
      qc.invalidateQueries({ queryKey: ['video-hdr-status'] })
    },
    onError: (e) => push(errorToast(e, 'saveVideoConfig')),
  })

  const c = cfg.data
  const st: HdrStatus | undefined = status.data
  const usable = !!st && st.surface && st.capable && st.systemHdr !== false

  const hint = !st
    ? t('video.hdr.hintOff')
    : !st.surface
      ? t('video.hdr.noSurface')
      : !st.capable
        ? t('video.hdr.unsupported')
        : st.systemHdr === false
          ? t('video.hdr.noSystem')
          : st.peakNits
            ? t('video.hdr.monitor', {
                peak: Math.round(st.peakNits),
                full: Math.round(st.fullFrameNits ?? st.peakNits),
              })
            : t('video.hdr.hintOff')

  return (
    <div className={s.root}>
      <Field label={t('video.hdr.label')} hint={hint}>
        <Switch
          checked={c?.hdr ?? false}
          disabled={!c || save.isPending || (!usable && !c.hdr)}
          onChange={(_, d) => save.mutate({ hdr: d.checked })}
        />
      </Field>
      {c?.hdr && st?.active && (
        <>
          <Field label={t('video.hdr.highlights')} hint={t('video.hdr.highlightsHint')}>
            <Switch
              checked={c.hdrHighlights}
              disabled={save.isPending}
              onChange={(_, d) => save.mutate({ hdrHighlights: d.checked })}
            />
          </Field>
          <NitsSlider
            label={t('video.hdr.paperWhite')}
            hint={t('video.hdr.paperWhiteHint')}
            stored={c.hdrPaperWhiteNits}
            used={st.paperWhiteUsed}
            min={80}
            max={480}
            onCommit={(n) => save.mutate({ hdrPaperWhiteNits: n })}
          />
          {c.hdrHighlights && (
            <NitsSlider
              label={t('video.hdr.peak')}
              hint={t('video.hdr.peakHint')}
              stored={c.hdrPeakNits}
              used={st.peakUsed}
              min={200}
              max={2000}
              onCommit={(n) => save.mutate({ hdrPeakNits: n })}
            />
          )}
        </>
      )}
    </div>
  )
}

/**
 * Slider em nits. `stored` 0 = automático (mostra o valor que o app está
 * usando, `used`); mexer grava um valor fixo, e "Usar automático" volta.
 */
function NitsSlider(props: {
  label: string
  hint: string
  stored: number
  used: number
  min: number
  max: number
  onCommit: (nits: number) => void
}) {
  const { label, hint, stored, used, min, max, onCommit } = props
  const { t } = useTranslation()
  const s = useStyles()
  const auto = stored === 0
  const shown = auto ? used : stored
  const [draft, setDraft] = useState<number | null>(null)
  // A função muda a cada render; o temporizador só reinicia com o valor.
  const commit = useRef(onCommit)
  useEffect(() => {
    commit.current = onCommit
  }, [onCommit])

  useEffect(() => {
    if (draft === null) return
    const id = window.setTimeout(() => {
      commit.current(draft)
      setDraft(null)
    }, DEBOUNCE_MS)
    return () => window.clearTimeout(id)
  }, [draft])

  const value = Math.round(draft ?? shown)
  const valueText = auto && draft === null
    ? `${t('video.hdr.nits', { n: value })} · ${t('video.hdr.auto')}`
    : t('video.hdr.nits', { n: value })

  return (
    <Field hint={hint}>
      <div className={s.head}>
        <Body1>{label}</Body1>
        <Body1 className={s.value}>{valueText}</Body1>
      </div>
      <Slider
        className={s.slider}
        aria-label={label}
        aria-valuetext={valueText}
        min={min}
        max={max}
        step={10}
        value={Math.min(max, Math.max(min, value))}
        // Slider padrão, sem passos visíveis ("brightness controls", Fluent
        // 2); o valor vai de 10 em 10 nits.
        onChange={(_, d) => setDraft(Math.round(d.value / 10) * 10)}
      />
      {!auto && (
        <Button
          className={s.reset}
          appearance="subtle"
          size="small"
          icon={<ArrowResetRegular />}
          onClick={() => onCommit(0)}
        >
          {t('video.hdr.useAuto')}
        </Button>
      )}
    </Field>
  )
}
