import {
  Body1,
  Button,
  Caption1,
  Text,
  makeStyles,
  Tab,
  TabList,
  mergeClasses,
  tokens,
  shorthands,
  Subtitle2,
  Card,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
import { BezelLibrary } from '../../components/BezelLibrary'
import { LoadingState } from '../../components/EmptyState'
import { ShaderLibrary } from '../../components/ShaderLibrary'
import { ShaderParams } from '../../components/ShaderParams'
import { ShaderPreview } from '../../components/ShaderPreview'
import { errorToast, sysToast } from '../../lib/toast'
import {
  clearDecorations,
  getShaderInfo,
  getShaderParams,
  importDecorationPack,
  pickFolder,
  setShader,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'
import { curatedText } from '../../lib/backendText'
import { useTranslation } from 'react-i18next'
import { SettingsLinkList } from '../../components/SettingsNav'
import { useTabStyles } from '../../styles/xbox'
import { ArrowLeftRegular, FrameRegular, OptionsRegular, SparkleRegular } from '@fluentui/react-icons'
import { ShaderCard } from '../../components/ShaderCard'

// Presets embutidos com nome/descrição traduzidos (`video.presets.<id>`).
const BUILTIN = ['plain', 'crt', 'lcd'] as const
type Builtin = (typeof BUILTIN)[number]
const isBuiltin = (n: string): n is Builtin => (BUILTIN as readonly string[]).includes(n)

const useStyles = makeStyles({
  // Shaders: opções na 1ª coluna, prévia na 2ª (uma só em tela estreita).
  columns: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
    columnGap: '48px',
    rowGap: '24px',
    alignItems: 'start',
  },
  options: { display: 'flex', flexDirection: 'column', gap: '8px', minWidth: 0 },
  // Lista de shaders com rolagem própria; a folga (8) não deixa o anel de
  // foco ser cortado pela borda da área que rola.
  shaderList: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    maxHeight: '360px',
    overflowY: 'auto',
    padding: '8px',
    margin: '-8px',
    // mesmo scrollbar fino do resto do app (`.scroll` do shell)
    '::-webkit-scrollbar': { width: '6px' },
    '::-webkit-scrollbar-thumb': {
      backgroundColor: tokens.colorNeutralStroke2,
      borderRadius: tokens.borderRadiusCircular,
    },
  },
  activeRow: { display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' },
  swap: {
    display: 'flex',
    flexDirection: 'column',
    gap: '8px',
    animationDuration: '300ms',
    animationTimingFunction: tokens.curveDecelerateMax,
    // `backwards` (não `both`): o estado final é o natural — ver RouteTransition
    animationFillMode: 'backwards',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  inFwd: {
    animationName: {
      from: { opacity: 0, transform: 'translateX(24px)' },
      to: { opacity: 1, transform: 'translateX(0)' },
    },
  },
  inBack: {
    animationName: {
      from: { opacity: 0, transform: 'translateX(-24px)' },
      to: { opacity: 1, transform: 'translateX(0)' },
    },
  },
  // 24 até os parâmetros (8 do `gap` + 16)
  panelHead: { display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px' },
  panelTitle: { display: 'flex', flexDirection: 'column', minWidth: 0 },
  panelSub: { color: tokens.colorNeutralForeground3 },
  // Parâmetros num card; com muitos, rolam dentro dele (mesma altura da
  // lista de shaders) com o scrollbar fino do app.
  paramsCard: { padding: 0 },
  paramsScroll: {
    maxHeight: '360px',
    overflowY: 'auto',
    padding: tokens.spacingHorizontalM,
    '::-webkit-scrollbar': { width: '6px' },
    '::-webkit-scrollbar-thumb': {
      backgroundColor: tokens.colorNeutralStroke2,
      borderRadius: tokens.borderRadiusCircular,
    },
  },
  // `outline` com borda visível (ver `ShaderCard`)
  settingsBtn: {
    ...shorthands.borderColor(`${tokens.colorNeutralStrokeAccessible} !important`),
  },
  // abas no estilo do app; 24 até o conteúdo (8 do `gap` + 16)
  tabs: { alignSelf: 'flex-start', marginBottom: '16px' },
})

type ShaderSource = 'reemu' | 'pack'

type VideoSection = 'shaders' | 'molduras'

/**
 * Configurações › Vídeo. Como nas Configurações do Windows: sem `section`
 * é a página da categoria, com um card por subseção; `shaders` e
 * `molduras` são as subseções (rotas `video/shaders` e `video/molduras`),
 * com o caminho "Configurações › Vídeo › …" no topo (do `SettingsLayout`).
 */
export function SettingsVideo({ section }: { section?: VideoSection }) {
  const { t } = useTranslation()
  const presetTitle = (n: string) => (isBuiltin(n) ? t(`video.presets.${n}.title`) : n)
  const presetDesc = (n: string) => (isBuiltin(n) ? t(`video.presets.${n}.desc`) : '')
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const st = useStyles()
  const tb = useTabStyles()
  const [source, setSource] = useState<ShaderSource>('reemu')
  // muda a cada parâmetro gravado → refaz a prévia do shader
  const [paramsRev, setParamsRev] = useState(0)

  const { data, isLoading, isError } = useQuery({
    queryKey: ['shader-info'],
    queryFn: getShaderInfo,
    retry: false,
  })
  // Parâmetros do shader ativo — mesma chave do `ShaderParams` (scope
  // default), então a gaveta abre já com os dados.
  const params = useQuery({
    queryKey: ['shader-params', 'default', null, null, data?.active ?? null],
    queryFn: getShaderParams,
    enabled: !!data?.gpu,
    retry: false,
  })
  const hasParams = (params.data?.length ?? 0) > 0
  const [paramsOpen, setParamsOpen] = useState(false)
  // Direção do deslize ao trocar lista ↔ ajustes (nada no 1º render).
  const [swapDir, setSwapDir] = useState<'fwd' | 'back' | null>(null)
  const backRef = useRef<HTMLButtonElement>(null)
  const openParams = (open: boolean) => {
    setSwapDir(open ? 'fwd' : 'back')
    setParamsOpen(open)
  }
  // Ao abrir os ajustes, o foco vai pro "voltar" (controle/teclado seguem
  // dali); ao voltar, a lista recebe o foco de novo pelo próprio card.
  useEffect(() => {
    if (paramsOpen) backRef.current?.focus()
  }, [paramsOpen])

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
  const activeCurated = data.curated.find((c) => c.id === data.active)
  const activeName = isBuiltin(data.active)
    ? presetTitle(data.active)
    : activeCurated
      ? curatedText(t, activeCurated.id, 'label', activeCurated.label)
      : (data.active.split(/[/\\]/).pop() ?? data.active)
  // Abre os ajustes do shader ativo (só ele tem os parâmetros carregados).
  const settingsFor = (id: string) =>
    id === data.active && hasParams ? () => openParams(true) : undefined
  const presetPicker = (
    <div className={st.shaderList}>
      {data.available.map((name) => (
        <ShaderCard
          key={name}
          title={presetTitle(name)}
          description={presetDesc(name)}
          selected={data.active === name}
          disabled={pick.isPending || !data.gpu}
          onSelect={() => pick.mutate(name)}
          onSettings={settingsFor(name)}
        />
      ))}
      {data.curated.map((c) => (
        <ShaderCard
          key={c.id}
          title={curatedText(t, c.id, 'label', c.label)}
          description={
            c.available
              ? curatedText(t, c.id, 'desc', c.desc)
              : t('video.needsPack', { desc: curatedText(t, c.id, 'desc', c.desc) })
          }
          selected={data.active === c.id}
          disabled={pick.isPending || !data.gpu || !c.available}
          onSelect={() => pick.mutate(c.id)}
          onSettings={settingsFor(c.id)}
        />
      ))}
    </div>
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
      {section === 'shaders' && (
        <Caption1>{data.gpu ? t('video.gpuHint') : t('video.noGpu')}</Caption1>
      )}

      {!data.gpu && section === 'shaders' && presetPicker}

      {data.gpu && (
        <>
          {section === 'shaders' && (
            <div className={st.columns}>
              <div className={st.options}>
                {/* Lista ou ajustes do shader ativo, no mesmo lugar (como uma
                    subpágina: "voltar" em cima). Desliza pro lado ao trocar. */}
                <div
                  key={paramsOpen ? 'params' : 'list'}
                  className={mergeClasses(
                    st.swap,
                    swapDir === 'fwd' && st.inFwd,
                    swapDir === 'back' && st.inBack,
                  )}
                >
                  {paramsOpen && hasParams ? (
                    <>
                      <div className={st.panelHead}>
                        <Button
                          ref={backRef}
                          appearance="subtle"
                          icon={<ArrowLeftRegular />}
                          aria-label={t('video.backToShaders')}
                          onClick={() => openParams(false)}
                        />
                        <div className={st.panelTitle}>
                          <Subtitle2>{t('video.shaderSettings')}</Subtitle2>
                          <Caption1 className={st.panelSub}>{activeName}</Caption1>
                        </div>
                      </div>
                      <Card className={st.paramsCard} appearance="filled-alternative">
                        <div className={st.paramsScroll}>
                          <ShaderParams
                            scope="default"
                            reloadKey={data.active}
                            onChanged={() => setParamsRev((n) => n + 1)}
                          />
                        </div>
                      </Card>
                    </>
                  ) : (
                    <>
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
                          <ShaderLibrary
                            onPick={(p) => pick.mutate(p)}
                            activePath={data.active}
                            busy={pick.isPending}
                          />
                        </>
                      )}
                      {!data.available.includes(data.active) &&
                        !data.curated.some((c) => c.id === data.active) && (
                          <div className={st.activeRow}>
                            <Caption1>
                              {t('video.active')}{' '}
                              <Text as="strong" weight="semibold">
                                {data.active.split(/[/\\]/).pop()}
                              </Text>
                            </Caption1>
                            {hasParams && (
                              <Button
                                className={st.settingsBtn}
                                appearance="outline"
                                icon={<OptionsRegular />}
                                onClick={() => openParams(true)}
                              >
                                {t('video.shaderSettings')}
                              </Button>
                            )}
                          </div>
                        )}
                    </>
                  )}
                </div>
              </div>
              <ShaderPreview reloadKey={`${data.active}#${paramsRev}`} />
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
