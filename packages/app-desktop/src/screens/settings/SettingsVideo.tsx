import {
  Body1,
  Button,
  Caption1,
  makeStyles,
  mergeClasses,
  tokens,
  Subtitle2,
  Card,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useRef, useState } from 'react'
import { BezelLibrary } from '../../components/BezelLibrary'
import { LoadingState } from '../../components/EmptyState'
import { SelectBlock } from '../../components/SettingBlock'
import { ShaderParams } from '../../components/ShaderParams'
import { ShaderPreview } from '../../components/ShaderPreview'
import * as M from '../../styles/metrics'
import { usePanelSlideStyles } from '../../lib/motion'
import { errorToast, sysToast, shaderIssueToast } from '../../lib/toast'
import {
  clearDecorations,
  getShaderAssignments,
  getShaderInfo,
  getShaderParams,
  importDecorationPack,
  listRoms,
  pickFolder,
  setShader,
  type ShaderScope,
} from '../../lib/tauri'
import { knownPlatforms, platformLabel } from '../../lib/platform'
import { useToastStore } from '../../stores/useToastStore'
import { curatedText } from '../../lib/backendText'
import { useTranslation } from 'react-i18next'
import { SettingsHubCards } from './SettingsHome'
import { VideoAspectSection } from './VideoAspectSection'
import {
  ArrowLeftRegular,
  ArrowResetRegular,
  FrameRegular,
  ScaleFitRegular,
  SparkleRegular,
} from '@fluentui/react-icons'
import { ShaderPicker, ShaderPresetList } from '../../components/ShaderPicker'

// Presets embutidos com nome/descrição traduzidos (`video.presets.<id>`).
const BUILTIN = ['plain', 'crt', 'lcd'] as const
type Builtin = (typeof BUILTIN)[number]
const isBuiltin = (n: string): n is Builtin => (BUILTIN as readonly string[]).includes(n)

// Importar pack de molduras de uma pasta e remover os bezels: escondidos por
// ora (pedido do usuário); a importação ficou no backlog (TASKS.md ›
// Funcionalidades).
const SHOW_BEZEL_PACK_TOOLS = false

const useStyles = makeStyles({
  // Página de Shaders na altura útil do card (o SettingsLayout dá a altura).
  // A área das colunas é um container de tamanho, pra coluna da prévia
  // calcular a largura pela altura que sobra.
  // O card reserva embaixo o espaço da barra de dicas do controle; a página
  // de Shaders usa essa área (a margem negativa a devolve), e lista e prévia
  // descem até a margem de baixo do card, a mesma das laterais.
  fill: {
    height: `calc(100% + ${M.PAGE_PAD_B - M.SAFE_Y}px)`,
    marginBottom: `-${M.PAGE_PAD_B - M.SAFE_Y}px`,
  },
  columnsBox: { flexGrow: 1, minHeight: '200px', containerType: 'size' },
  // Opções na 1ª coluna, prévia na 2ª, na altura inteira do card: a lista
  // rola por dentro e a prévia cobre o espaço que sobra (pedido do usuário).
  columns: {
    display: 'grid',
    height: '100%',
    gridTemplateColumns: 'minmax(320px, 5fr) minmax(0, 6fr)',
    gridTemplateRows: 'minmax(0, 1fr)',
    columnGap: '48px',
  },
  // A altura da linha é a da prévia: as opções não contam (`contain: size`)
  // e esticam até ela, com a lista rolando dentro — terminam junto com a
  // imagem mesmo quando ela é limitada pela largura (tela 4:3).
  options: {
    display: 'flex',
    flexDirection: 'column',
    gap: '8px',
    minWidth: 0,
    contain: 'size',
  },
  swap: {
    display: 'flex',
    flexDirection: 'column',
    gap: '8px',
    flexGrow: 1,
    minHeight: 0,
  },
  // 24 até os parâmetros (8 do `gap` + 16)
  panelHead: { display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px' },
  panelTitle: { display: 'flex', flexDirection: 'column', minWidth: 0 },
  panelSub: { color: tokens.colorNeutralForeground3 },
  // Parâmetros num card que desce até o fim da prévia; com muitos, rolam
  // dentro dele com o scrollbar fino do app.
  // sem sombra (pedido do usuário): o card se destaca só pelo fundo
  paramsCard: { padding: 0, flexGrow: 1, minHeight: 0, boxShadow: 'none' },
  paramsScroll: {
    flexGrow: 1,
    minHeight: 0,
    overflowY: 'auto',
    padding: tokens.spacingHorizontalM,
  },
  scopeBar: {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    gap: tokens.spacingHorizontalM,
  },
  scopeSelect: {
    flex: '1 1 220px',
    maxWidth: '340px',
  },
})

type VideoSection = 'proporcao' | 'shaders' | 'molduras'

/**
 * Configurações › Aparência › Decorações. Sem `section`, exibe os cards
 * que levam às páginas de Proporção da imagem, Shaders e Molduras.
 */
export function SettingsVideo({ section }: { section?: VideoSection }) {
  const { t } = useTranslation()
  const presetTitle = (n: string) => (isBuiltin(n) ? t(`video.presets.${n}.title`) : n)
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)
  const st = useStyles()
  const slide = usePanelSlideStyles()
  // muda a cada parâmetro gravado → refaz a prévia do shader
  const [paramsRev, setParamsRev] = useState(0)
  const [shaderScope, setShaderScope] = useState<'default' | 'system'>('default')
  const [shaderSystemId, setShaderSystemId] = useState('')

  const { data, isLoading, isError } = useQuery({
    queryKey: ['shader-info'],
    queryFn: getShaderInfo,
    retry: false,
  })
  // Parâmetros do shader ativo — mesma chave do `ShaderParams` (scope
  // default), então a gaveta abre já com os dados.
  const params = useQuery({
    queryKey: ['shader-params', 'default', null, null, data?.active ?? null],
    queryFn: () => getShaderParams('default'),
    enabled: section !== 'proporcao' && shaderScope === 'default' && !!data?.gpu,
    retry: false,
  })
  const assignments = useQuery({
    queryKey: ['shader-assignments'],
    queryFn: getShaderAssignments,
    enabled: section === 'shaders',
    retry: false,
  })
  const roms = useQuery({
    queryKey: ['roms'],
    queryFn: listRoms,
    enabled: section === 'shaders' && shaderScope === 'system',
    retry: false,
  })
  const systemParams = useQuery({
    queryKey: [
      'shader-params',
      'system',
      null,
      shaderSystemId || 'snes',
      assignments.data?.atDefault ?? data?.active ?? null,
      assignments.data?.systems[shaderSystemId || 'snes'] ?? '',
    ],
    queryFn: () => getShaderParams('system' as ShaderScope, undefined, shaderSystemId || 'snes'),
    enabled: section === 'shaders' && shaderScope === 'system' && !!data?.gpu,
    retry: false,
  })
  const activeParams = shaderScope === 'system' ? systemParams : params
  const systemShaders = assignments.data?.systems ?? {}
  const platforms = useMemo(
    () => knownPlatforms().filter(([id]) => id !== 'disc' || id in systemShaders),
    [systemShaders],
  )
  const activeSystemId = useMemo(() => {
    if (shaderSystemId) return shaderSystemId
    const overridden = Object.keys(systemShaders).sort((a, b) =>
      platformLabel(a).localeCompare(platformLabel(b)),
    )
    if (overridden.length > 0) return overridden[0]
    const fromLibrary = roms.data?.[0]?.systemId
    if (fromLibrary && platforms.some(([id]) => id === fromLibrary)) return fromLibrary
    return platforms[0]?.[0] ?? 'snes'
  }, [shaderSystemId, systemShaders, roms.data, platforms])

  useEffect(() => {
    if (shaderScope === 'system' && !shaderSystemId && activeSystemId) {
      setShaderSystemId(activeSystemId)
    }
  }, [shaderScope, shaderSystemId, activeSystemId])

  const defaultShader = assignments.data
    ? (assignments.data.atDefault ?? 'plain')
    : (data?.active ?? 'plain')
  const selectedShader =
    shaderScope === 'default' ? defaultShader : (systemShaders[activeSystemId] ?? '')
  const resolvedShader =
    shaderScope === 'default' ? defaultShader : (selectedShader || defaultShader)
  const hasParams =
    (shaderScope === 'default' || selectedShader !== '') && (activeParams.data?.length ?? 0) > 0
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

  const shaderLabel = (name: string) => {
    const c = data?.curated.find((x) => x.id === name)
    const curated = c && curatedText(t, c.id, 'label', c.label)
    const base = name.split(/[/\\]/).pop() ?? name
    return isBuiltin(name) ? presetTitle(name) : (curated ?? base)
  }

  const pick = useMutation({
    mutationFn: (name: string) =>
      setShader(
        name,
        shaderScope,
        undefined,
        shaderScope === 'system' ? activeSystemId : undefined,
      ),
    onSuccess: (issue, name) => {
      void qc.invalidateQueries({ queryKey: ['shader-assignments'] })
      void qc.invalidateQueries({ queryKey: ['shader-info'] })
      void qc.invalidateQueries({ queryKey: ['shader-params'] })
      if (shaderScope === 'system') {
        const platform = platformLabel(activeSystemId)
        if (!name) {
          push(sysToast(t('video.platformShaderInherited', { platform }), 'Success'))
          return
        }
        const label = shaderLabel(name)
        push(sysToast(t('video.platformShaderSet', { platform, name: label }), 'Success'))
        if (issue) push(shaderIssueToast(t(`video.shaderIssue.${issue}`, { name: label })))
      } else {
        const label = shaderLabel(name)
        push(sysToast(t('video.defaultShader', { name: label }), 'Success'))
        // preset com problema conhecido na GPU (gpu/known_issues.txt)
        if (issue) push(shaderIssueToast(t(`video.shaderIssue.${issue}`, { name: label })))
      }
    },
    onError: (e) =>
      push(
        errorToast(
          e,
          shaderScope === 'system' ? 'changePlatformShader' : 'changeDefaultShader',
        ),
      ),
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
      <SettingsHubCards
        label={t('settings.tabs.video')}
        items={[
          {
            to: 'proporcao',
            icon: <ScaleFitRegular />,
            title: t('video.tabAspect'),
            description: t('video.aspectDesc'),
          },
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

  if (section === 'proporcao') return <VideoAspectSection />

  if (isLoading) return <LoadingState />
  if (isError || !data) return <Body1>{t('video.unavailable')}</Body1>

  // Presets embutidos (plain/CRT/LCD) + curados (xBR/ScaleFX/…) — parte da
  // aba "Shaders", mas também mostrado (desabilitado) sem GPU, só pra
  // informar o que existiria.
  const activeCurated = data.curated.find((c) => c.id === data.active)
  const activeName = shaderLabel(resolvedShader)
  void activeCurated
  const inheritHint = shaderScope === 'system' ? t('video.shaderInheritDefault') : undefined
  // Abre os ajustes do shader escolhido neste escopo (só quando não está herdando).
  const settingsFor = (id: string) =>
    id !== '' && id === selectedShader && hasParams ? () => openParams(true) : undefined
  const presetPicker = (
    <ShaderPresetList
      info={data}
      selected={selectedShader}
      busy={pick.isPending}
      onPick={(n) => pick.mutate(n)}
      inherit={inheritHint}
      settingsFor={settingsFor}
    />
  )
  const scopeControls = section === 'shaders' && (
    <div className={st.scopeBar}>
      <SelectBlock
        compact
        className={st.scopeSelect}
        label={t('video.shaderScopeLabel')}
        value={shaderScope}
        options={[
          { value: 'default', label: t('video.scopeDefault') },
          { value: 'system', label: t('video.scopePlatform') },
        ]}
        onChange={(v) => {
          setShaderScope(v as 'default' | 'system')
          setParamsOpen(false)
        }}
      />
      {shaderScope === 'system' && (
        <>
          <SelectBlock
            compact
            className={st.scopeSelect}
            label={t('common.platform')}
            value={activeSystemId}
            options={platforms.map(([id, label]) => ({
              value: id,
              label: systemShaders[id]
                ? `${label} (${shaderLabel(systemShaders[id])})`
                : label,
            }))}
            onChange={(v) => {
              setShaderSystemId(v)
              setParamsOpen(false)
            }}
          />
          {selectedShader !== '' && (
            <Button
              appearance="subtle"
              icon={<ArrowResetRegular />}
              disabled={pick.isPending}
              onClick={() => {
                setParamsOpen(false)
                pick.mutate('')
              }}
            >
              {t('video.resetToDefault')}
            </Button>
          )}
        </>
      )}
    </div>
  )

  return (
    <div
      className={section === 'shaders' ? st.fill : undefined}
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
      {scopeControls}

      {!data.gpu && section === 'shaders' && presetPicker}

      {data.gpu && (
        <>
          {section === 'shaders' && (
            <div className={st.columnsBox}>
              <div className={st.columns}>
                <div className={st.options}>
                  {/* Lista ou ajustes do shader ativo, no mesmo lugar (como uma
                      subpágina: "voltar" em cima). Desliza pro lado ao trocar. */}
                  <div
                    key={paramsOpen ? 'params' : 'list'}
                    className={mergeClasses(
                      st.swap,
                      slide.transition,
                      swapDir === 'fwd' && slide.forward,
                      swapDir === 'back' && slide.backward,
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
                              scope={shaderScope}
                              systemId={shaderScope === 'system' ? activeSystemId : undefined}
                              reloadKey={`${shaderScope}:${activeSystemId}:${selectedShader}`}
                              onChanged={() => setParamsRev((n) => n + 1)}
                            />
                          </div>
                        </Card>
                      </>
                    ) : (
                      <ShaderPicker
                        info={data}
                        selected={selectedShader}
                        busy={pick.isPending}
                        onPick={(n) => pick.mutate(n)}
                        inherit={inheritHint}
                        settingsFor={settingsFor}
                        fill
                      />
                    )}
                  </div>
                </div>
                <ShaderPreview
                  reloadKey={`${shaderScope}:${shaderScope === 'system' ? activeSystemId : ''}:${resolvedShader}:${activeParams.dataUpdatedAt}#${paramsRev}`}
                  fill
                />
              </div>
            </div>
          )}

          {section === 'molduras' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <Caption1>{t('video.bezelsHint')}</Caption1>
              <BezelLibrary />
              {SHOW_BEZEL_PACK_TOOLS && (
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
              )}
            </div>
          )}
        </>
      )}
    </div>
  )
}
