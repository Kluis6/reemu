import {
  Body1,
  Button,
  Caption1,
  Field,
  Radio,
  RadioGroup,
  Switch,
  Tab,
  TabList,
  Text,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { BezelLibrary } from '../../components/BezelLibrary'
import { LoadingState } from '../../components/EmptyState'
import { ShaderLibrary } from '../../components/ShaderLibrary'
import { ShaderParams } from '../../components/ShaderParams'
import { errorToast, sysToast } from '../../lib/toast'
import {
  clearDecorations,
  getShaderInfo,
  getVideoConfig,
  importDecorationPack,
  pickFolder,
  pickSlangp,
  setShader,
  updateVideoConfig,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { curatedText } from '../../lib/backendText'
import { useTranslation } from 'react-i18next'

// Presets embutidos com nome/descrição traduzidos (`video.presets.<id>`).
const BUILTIN = ['plain', 'crt', 'lcd'] as const
type Builtin = (typeof BUILTIN)[number]
const isBuiltin = (n: string): n is Builtin => (BUILTIN as readonly string[]).includes(n)

type VideoTab = 'shaders' | 'molduras'

export function SettingsVideo() {
  const { t } = useTranslation()
  const presetTitle = (n: string) => (isBuiltin(n) ? t(`video.presets.${n}.title`) : n)
  const presetDesc = (n: string) => (isBuiltin(n) ? t(`video.presets.${n}.desc`) : '')
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const [tab, setTab] = useState<VideoTab>('shaders')

  const { data, isLoading, isError } = useQuery({
    queryKey: ['shader-info'],
    queryFn: getShaderInfo,
    retry: false,
  })

  const pick = useMutation({
    // 'default' persiste: vale pra todos os jogos (jogos podem ter override próprio).
    mutationFn: (name: string) => setShader(name, 'default'),
    onSuccess: (_, name) => {
      qc.invalidateQueries({ queryKey: ['shader-info'] })
      const c = data?.curated.find((x) => x.id === name)
      const curated = c && curatedText(t, c.id, 'label', c.label)
      const base = name.split(/[/\\]/).pop() ?? name
      push(sysToast(t('video.defaultShader', { name: isBuiltin(name) ? presetTitle(name) : (curated ?? base) }), 'Success'))
    },
    onError: (e) => push(errorToast(e, 'changeDefaultShader')),
  })

  const videoCfg = useQuery({
    queryKey: ['video-config'],
    queryFn: getVideoConfig,
    retry: false,
  })
  const setIntegerScaling = useMutation({
    mutationFn: (integerScaling: boolean) => updateVideoConfig({ integerScaling }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['video-config'] }),
    onError: (e) => push(errorToast(e, 'saveVideoConfig')),
  })

  const deco = useMutation({
    mutationFn: (path: string) => importDecorationPack(path),
    onSuccess: (n) =>
      push(sysToast(t('video.bezelsImported', { count: n }), 'Success')),
    onError: (e) => push(errorToast(e, 'importBezels')),
  })
  const decoClear = useMutation({
    mutationFn: () => clearDecorations(),
    onSuccess: () => push(sysToast(t('video.bezelsRemoved'), 'Success')),
    onError: (e) => push(errorToast(e, 'clearBezels')),
  })

  if (isLoading) return <LoadingState />
  if (isError || !data) return <Body1>{t('video.unavailable')}</Body1>

  // Presets embutidos (plain/CRT/LCD) + curados (xBR/ScaleFX/…) — parte da
  // aba "Shaders", mas também mostrado (desabilitado) sem GPU, só pra
  // informar o que existiria.
  const presetPicker = (
    <RadioGroup
      value={
        data.available.includes(data.active) ||
        data.curated.some((c) => c.id === data.active)
          ? data.active
          : ''
      }
      onChange={(_, d) => pick.mutate(d.value)}
    >
      {data.available.map((name) => (
        <Radio
          key={name}
          value={name}
          disabled={pick.isPending || !data.gpu}
          label={{
            children: (
              <span style={{ display: 'flex', flexDirection: 'column' }}>
                <Text as="strong" weight="semibold">{presetTitle(name)}</Text>
                <Caption1>{presetDesc(name)}</Caption1>
              </span>
            ),
          }}
        />
      ))}
      {data.curated.map((c) => (
        <Radio
          key={c.id}
          value={c.id}
          disabled={pick.isPending || !data.gpu || !c.available}
          label={{
            children: (
              <span style={{ display: 'flex', flexDirection: 'column' }}>
                <Text as="strong" weight="semibold">{curatedText(t, c.id, 'label', c.label)}</Text>
                <Caption1>
                  {c.available
                    ? curatedText(t, c.id, 'desc', c.desc)
                    : t('video.needsPack', { desc: curatedText(t, c.id, 'desc', c.desc) })}
                </Caption1>
              </span>
            ),
          }}
        />
      ))}
    </RadioGroup>
  )

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 14,
        // A aba Shaders precisa de mais largura pras 2 colunas — o resto
        // (sem GPU / Molduras) fica no mesmo limite estreito de antes.
        maxWidth: data.gpu && tab === 'shaders' ? 860 : 460,
      }}
    >
      <Field
        label={t('video.integerScaling')}
        hint={t('video.integerScalingHint')}
      >
        <Switch
          checked={videoCfg.data?.integerScaling ?? false}
          disabled={videoCfg.isLoading || setIntegerScaling.isPending}
          onChange={(_, d) => setIntegerScaling.mutate(d.checked)}
        />
      </Field>

      <Caption1>
        {data.gpu
          ? t('video.gpuHint')
          : t('video.noGpu')}
      </Caption1>

      {!data.gpu && presetPicker}

      {data.gpu && (
        <>
          <TabList
            selectedValue={tab}
            onTabSelect={(_, d) => setTab(d.value as VideoTab)}
          >
            <Tab value="shaders">{t('video.tabShaders')}</Tab>
            <Tab value="molduras">{t('video.tabBezels')}</Tab>
          </TabList>

          {tab === 'shaders' && (
            // `auto-fit`/`minmax`: 2 colunas quando cabe, 1 coluna sozinha
            // quando a janela é estreita — responsivo sem media query.
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
                columnGap: 48,
                rowGap: 14,
                alignItems: 'start',
              }}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8, minWidth: 0 }}>
                {presetPicker}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6, minWidth: 0 }}>
                <Caption1>{t('video.externalPreset')}</Caption1>
                <ShaderLibrary
                  onPick={(p) => pick.mutate(p)}
                  activePath={data.active}
                  busy={pick.isPending}
                />
                <Button
                  appearance="subtle"
                  disabled={pick.isPending}
                  onClick={async () => {
                    const p = await pickSlangp()
                    if (p) pick.mutate(p)
                  }}
                >
                  {t('game.shader.loadFile')}
                </Button>
                {!data.available.includes(data.active) && (
                  <Caption1>
                    {t('video.active')} <Text as="strong" weight="semibold">{data.active}</Text>
                  </Caption1>
                )}
                <ShaderParams scope="default" reloadKey={data.active} />
              </div>
            </div>
          )}

          {tab === 'molduras' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <Caption1>{t('video.bezelsHint')}</Caption1>
              <BezelLibrary />
              <div style={{ display: 'flex', gap: 8 }}>
                <Button
                  disabled={deco.isPending}
                  onClick={async () => {
                    const p = await pickFolder(t('dialogs.pickBezelFolder'))
                    if (p) deco.mutate(p)
                  }}
                >
                  {t('video.importBezels')}
                </Button>
                <Button
                  appearance="subtle"
                  disabled={decoClear.isPending}
                  onClick={() => decoClear.mutate()}
                >
                  {t('video.removeBezels')}
                </Button>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  )
}
