import {
  Body1,
  Button,
  Caption1,
  Radio,
  RadioGroup,
  Text,
  makeStyles,
  Tab,
  TabList,
  mergeClasses,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { BezelLibrary } from '../../components/BezelLibrary'
import { LoadingState } from '../../components/EmptyState'
import { ShaderLibrary } from '../../components/ShaderLibrary'
import { ShaderParams } from '../../components/ShaderParams'
import { ShaderPreview } from '../../components/ShaderPreview'
import { errorToast, sysToast } from '../../lib/toast'
import {
  clearDecorations,
  getShaderInfo,
  importDecorationPack,
  pickFolder,
  pickSlangp,
  setShader,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { curatedText } from '../../lib/backendText'
import { useTranslation } from 'react-i18next'
import { SettingsBreadcrumb, SettingsLinkList } from '../../components/SettingsNav'
import { useTabStyles } from '../../styles/xbox'
import { FrameRegular, SparkleRegular } from '@fluentui/react-icons'

// Presets embutidos com nome/descrição traduzidos (`video.presets.<id>`).
const BUILTIN = ['plain', 'crt', 'lcd'] as const
type Builtin = (typeof BUILTIN)[number]
const isBuiltin = (n: string): n is Builtin => (BUILTIN as readonly string[]).includes(n)

const useStyles = makeStyles({
  crumb: { marginBottom: '10px' },
  // Shaders: prévia numa coluna, opções na outra (uma só em tela estreita).
  columns: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
    columnGap: '48px',
    rowGap: '24px',
    alignItems: 'start',
  },
  options: { display: 'flex', flexDirection: 'column', gap: '8px', minWidth: 0 },
  // abas no estilo do app; 24 até o conteúdo (8 do `gap` + 16)
  tabs: { alignSelf: 'flex-start', marginBottom: '16px' },
})

type ShaderSource = 'reemu' | 'pack'

type VideoSection = 'shaders' | 'molduras'

/**
 * Configurações › Vídeo. Como nas Configurações do Windows: sem `section`
 * é a página da categoria, com um card por subseção; `shaders` e
 * `molduras` são as subseções (rotas `video/shaders` e `video/molduras`),
 * com o caminho "Vídeo › …" no topo.
 */
export function SettingsVideo({ section }: { section?: VideoSection }) {
  const { t } = useTranslation()
  const presetTitle = (n: string) => (isBuiltin(n) ? t(`video.presets.${n}.title`) : n)
  const presetDesc = (n: string) => (isBuiltin(n) ? t(`video.presets.${n}.desc`) : '')
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const st = useStyles()
  const crumbGap = st.crumb
  const tb = useTabStyles()
  const [source, setSource] = useState<ShaderSource>('reemu')
  // muda a cada parâmetro gravado → refaz a prévia do shader
  const [paramsRev, setParamsRev] = useState(0)

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

  if (!section)
    return (
      <SettingsLinkList
        items={[
          {
            to: 'shaders',
            icon: <SparkleRegular />,
            title: t('video.tabShaders'),
            description: t('video.shadersDesc'),
          },
          {
            to: 'molduras',
            icon: <FrameRegular />,
            title: t('video.tabBezels'),
            description: t('video.bezelsDesc'),
          },
        ]}
      />
    )

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
        // Molduras: cards na largura útil inteira; Shaders: 2 colunas até
        // 860; sem GPU fica no limite estreito de antes.
        maxWidth: !data.gpu ? 460 : 'none',
      }}
    >
      {/* 24 até o conteúdo: 14 do `gap` + 10 */}
      <SettingsBreadcrumb
        className={crumbGap}
        parent={t('settings.tabs.video')}
        parentTo="/settings/video"
        current={section === 'shaders' ? t('video.tabShaders') : t('video.tabBezels')}
      />
      {section === 'shaders' && (
        <Caption1>{data.gpu ? t('video.gpuHint') : t('video.noGpu')}</Caption1>
      )}

      {!data.gpu && section === 'shaders' && presetPicker}

      {data.gpu && (
        <>
          {section === 'shaders' && (
            <div className={st.columns}>
              <ShaderPreview reloadKey={`${data.active}#${paramsRev}`} />
              <div className={st.options}>
                <TabList
                  className={mergeClasses(tb.tabs, st.tabs)}
                  selectedValue={source}
                  onTabSelect={(_, d) => setSource(d.value as ShaderSource)}
                >
                  <Tab value="reemu">{t('video.sourceReemu')}</Tab>
                  <Tab value="pack">{t('video.sourcePack')}</Tab>
                </TabList>
                {source === 'reemu' ? (
                  presetPicker
                ) : (
                  <>
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
                  </>
                )}
                {!data.available.includes(data.active) && (
                  <Caption1>
                    {t('video.active')} <Text as="strong" weight="semibold">{data.active}</Text>
                  </Caption1>
                )}
                <ShaderParams
                  scope="default"
                  reloadKey={data.active}
                  onChanged={() => setParamsRev((n) => n + 1)}
                />
              </div>
            </div>
          )}

          {section === 'molduras' && (
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
