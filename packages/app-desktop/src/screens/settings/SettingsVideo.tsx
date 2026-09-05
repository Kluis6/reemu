import { Body1, Button, Caption1, Spinner, tokens } from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
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

export function SettingsVideo() {
  const qc = useQueryClient()
  const push = useToastStore((s) => s.push)

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
      const base = name.split(/[/\\]/).pop() ?? name
      push(sysToast(`Shader padrão: ${LABELS[name]?.title ?? base}`, 'Success'))
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

  if (isLoading) return <Spinner label="Carregando…" />
  if (isError || !data) return <Body1>Informação de shader indisponível.</Body1>

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 14, maxWidth: 460 }}>
      <Caption1>
        {data.gpu
          ? 'Shader padrão da biblioteca (roda na GPU offscreen). Cada jogo pode ter um shader próprio na tela de detalhe.'
          : 'Sem GPU disponível — o frame vai cru pra tela; a troca não tem efeito.'}
      </Caption1>
      <div style={{ display: 'grid', gap: 8 }}>
        {data.available.map((name) => {
          const on = name === data.active
          return (
            <button
              key={name}
              disabled={pick.isPending || !data.gpu}
              onClick={() => pick.mutate(name)}
              style={{
                display: 'flex',
                flexDirection: 'column',
                gap: 2,
                padding: '12px 14px',
                borderRadius: 12,
                border: `1px solid ${on ? tokens.colorBrandStroke1 : tokens.colorNeutralStroke1}`,
                background: on ? tokens.colorBrandBackground2 : tokens.colorNeutralBackground2,
                textAlign: 'left',
                cursor: pick.isPending || !data.gpu ? 'default' : 'pointer',
                color: 'inherit',
              }}
            >
              <strong>{LABELS[name]?.title ?? name}</strong>
              <Caption1>{LABELS[name]?.desc ?? ''}</Caption1>
            </button>
          )
        })}
      </div>

      {data.gpu && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <Caption1>
            Preset externo — arquivo <code>.slangp</code> (RetroArch). Chains
            multi-passe simples rodam; CRT-Royale / Mega Bezel completos ainda
            não.
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
              Ativo: <strong>{data.active}</strong>
            </Caption1>
          )}
          <ShaderParams scope="default" reloadKey={data.active} />
        </div>
      )}

      {data.gpu && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          <Caption1>
            Molduras / bezels — pasta no formato Bezel Project / RetroBat
            (<code>default.png</code>, <code>&lt;sistema&gt;/</code>,{' '}
            <code>games/&lt;sistema&gt;/&lt;rom&gt;.png</code>). O jogo é
            posicionado pelo <code>.cfg</code> irmão quando existe.
          </Caption1>
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
    </div>
  )
}
