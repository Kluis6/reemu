import {
  Body1,
  Button,
  Caption1,
  Radio,
  RadioGroup,
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
import { sysToast } from '../../lib/toast'
import {
  clearDecorations,
  getShaderInfo,
  importDecorationPack,
  pickFolder,
  pickSlangp,
  setShader,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'

const LABELS: Record<string, { title: string; desc: string }> = {
  plain: { title: 'Nenhum (nítido)', desc: 'Pixels do core sem filtro.' },
  crt: { title: 'CRT', desc: 'Scanlines, máscara de fósforo e vinheta.' },
  lcd: { title: 'LCD portátil', desc: 'Grade sutil de pixels, cara de handheld.' },
}

type VideoTab = 'shaders' | 'molduras'

export function SettingsVideo() {
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
      const curated = data?.curated.find((c) => c.id === name)?.label
      const base = name.split(/[/\\]/).pop() ?? name
      push(sysToast(`Shader padrão: ${LABELS[name]?.title ?? curated ?? base}`, 'Success'))
    },
    onError: (e) => push(sysToast(`Falha: ${e}`, 'Error')),
  })

  const deco = useMutation({
    mutationFn: (path: string) => importDecorationPack(path),
    onSuccess: (n) =>
      push(sysToast(`Bezels importados — ${n} atribuição(ões). Aplica no próximo jogo.`, 'Success')),
    onError: (e) => push(sysToast(`Falha ao importar: ${e}`, 'Error')),
  })
  const decoClear = useMutation({
    mutationFn: () => clearDecorations(),
    onSuccess: () => push(sysToast('Bezels removidos.', 'Success')),
    onError: (e) => push(sysToast(`Falha: ${e}`, 'Error')),
  })

  if (isLoading) return <LoadingState />
  if (isError || !data) return <Body1>Informação de shader indisponível.</Body1>

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
                <Text as="strong" weight="semibold">{LABELS[name]?.title ?? name}</Text>
                <Caption1>{LABELS[name]?.desc ?? ''}</Caption1>
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
                <Text as="strong" weight="semibold">{c.label}</Text>
                <Caption1>
                  {c.available
                    ? c.desc
                    : `${c.desc} — precisa do pacote de shaders (abaixo).`}
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
      <Caption1>
        {data.gpu
          ? 'Shader padrão da biblioteca (roda na GPU offscreen). Cada jogo pode ter um shader próprio na tela de detalhe.'
          : 'Sem GPU disponível — o frame vai cru pra tela; a troca não tem efeito.'}
      </Caption1>

      {!data.gpu && presetPicker}

      {data.gpu && (
        <>
          <TabList
            selectedValue={tab}
            onTabSelect={(_, d) => setTab(d.value as VideoTab)}
          >
            <Tab value="shaders">Shaders</Tab>
            <Tab value="molduras">Molduras</Tab>
          </TabList>

          {tab === 'shaders' && (
            // `auto-fit`/`minmax`: 2 colunas quando cabe, 1 coluna sozinha
            // quando a janela é estreita — responsivo sem media query.
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))',
                columnGap: 24,
                rowGap: 14,
                alignItems: 'start',
              }}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8, minWidth: 0 }}>
                {presetPicker}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6, minWidth: 0 }}>
                <Caption1>
                  Preset externo — arquivo <code>.slangp</code> (RetroArch).
                  ~93% dos presets do pacote rodam (Mega Bezel inclusive);
                  glow/bloom que dependem de mipmap ainda ficam mais duros
                  que no RetroArch.
                </Caption1>
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
                  Carregar .slangp avulso…
                </Button>
                {!data.available.includes(data.active) && (
                  <Caption1>
                    Ativo: <Text as="strong" weight="semibold">{data.active}</Text>
                  </Caption1>
                )}
                <ShaderParams scope="default" reloadKey={data.active} />
              </div>
            </div>
          )}

          {tab === 'molduras' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              <Caption1>
                Baixe direto do The Bezel Project por sistema, ou importe uma
                pasta no formato Bezel Project / RetroBat (
                <code>default.png</code>, <code>&lt;sistema&gt;/</code>,{' '}
                <code>games/&lt;sistema&gt;/&lt;rom&gt;.png</code>). O jogo é
                posicionado pelo <code>.cfg</code> irmão ou pela janela
                transparente da arte.
              </Caption1>
              <BezelLibrary />
              <div style={{ display: 'flex', gap: 8 }}>
                <Button
                  disabled={deco.isPending}
                  onClick={async () => {
                    const p = await pickFolder()
                    if (p) deco.mutate(p)
                  }}
                >
                  Importar pasta de bezels…
                </Button>
                <Button
                  appearance="subtle"
                  disabled={decoClear.isPending}
                  onClick={() => decoClear.mutate()}
                >
                  Remover bezels
                </Button>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  )
}
