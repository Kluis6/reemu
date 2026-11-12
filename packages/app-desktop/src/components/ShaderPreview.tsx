import {
  Caption1,
  Spinner,
  Tab,
  TabList,
  ToggleButton,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { EyeOffRegular, EyeRegular } from '@fluentui/react-icons'
import { useQuery } from '@tanstack/react-query'
import { useEffect, useState, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { renderShaderPreview, shaderPreviewSource, type PreviewScene } from '../lib/tauri'

/** Tamanho em que o shader é renderizado (3× a cena de 320×240, 4:3). */
const OUT_W = 960
const OUT_H = 720

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
  },
  // `fill`: a prévia na altura inteira da coluna, a imagem cobrindo o quadro
  rootFill: { height: '100%', minHeight: 0 },
  frameFill: { aspectRatio: 'auto', flexGrow: 1, minHeight: 0 },
  imgCover: { objectFit: 'cover' },
  // Barra acima da imagem, da mesma largura dela: cenas à esquerda, ligar/
  // desligar o shader à direita. 24 até a imagem (8 do `gap` + 16).
  bar: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    flexWrap: 'wrap',
    columnGap: tokens.spacingHorizontalM,
    rowGap: tokens.spacingVerticalS,
    minHeight: '32px',
    marginBottom: '16px',
  },
  tabs: {
    columnGap: '8px',
    // raio padrão do ReEmu (4) no lugar da pílula do `filled-circular`
    '& .fui-Tab': { borderRadius: tokens.borderRadiusMedium },
  },
  frame: {
    position: 'relative',
    // a coluna inteira; a altura sai do 4:3
    width: '100%',
    aspectRatio: '4 / 3',
    overflow: 'hidden',
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: '#000',
  },
  // Ampliado sem suavizar, como a tela do jogo mostra um quadro que o shader
  // não escalou (o "plain" sai no tamanho nativo).
  img: {
    position: 'absolute',
    inset: 0,
    width: '100%',
    height: '100%',
    imageRendering: 'pixelated',
    userSelect: 'none',
  },
  busy: {
    position: 'absolute',
    inset: 0,
    display: 'grid',
    placeItems: 'center',
    backgroundColor: 'rgba(0, 0, 0, 0.35)',
  },
  // sobre o fundo preto do quadro, em qualquer tema
  error: { color: '#ffffff', padding: tokens.spacingHorizontalL, textAlign: 'center' },
})

/** `ArrayBuffer` de PNG → URL de blob, revogada quando troca. Sem buffer
 *  (a prévia falhou) volta a `null`: a URL anterior já foi revogada e, se
 *  ficasse, a tela mostrava o ícone de imagem quebrada. */
function useBlobUrl(buf: ArrayBuffer | undefined) {
  // derivada do buffer, sem estado (um `setState` no efeito re-renderizava
  // duas vezes a cada prévia); a URL anterior é revogada quando troca ou sai
  const url = useMemo(
    () => (buf ? URL.createObjectURL(new Blob([buf], { type: 'image/png' })) : null),
    [buf],
  )
  useEffect(() => () => {
    if (url) URL.revokeObjectURL(url)
  }, [url])
  return url
}

/**
 * Prévia do shader ativo numa cena de exemplo gerada pelo app (2D em pixel
 * art ou 3D no jeito de PS1/N64). O botão sobre a imagem liga e desliga o
 * shader já carregado. Renderizada na GPU pelo mesmo código do jogo
 * (`render_shader_preview`), com os mesmos parâmetros.
 */
export function ShaderPreview({ reloadKey, fill = false }: { reloadKey: string; fill?: boolean }) {
  const { t } = useTranslation()
  const s = useStyles()
  const [scene, setScene] = useState<PreviewScene>('2d')
  const [on, setOn] = useState(true)

  const source = useQuery({
    queryKey: ['shader-preview-source', scene],
    queryFn: () => shaderPreviewSource(scene),
    staleTime: Infinity,
    retry: false,
  })
  const preview = useQuery({
    queryKey: ['shader-preview', scene, reloadKey],
    queryFn: () => renderShaderPreview(OUT_W, OUT_H, scene),
    retry: false,
    // mantém a imagem anterior na tela enquanto a nova é gerada
    placeholderData: (prev) => prev,
  })
  const before = useBlobUrl(source.data)
  const after = useBlobUrl(preview.data)
  const shown = on ? after : before

  return (
    <div className={mergeClasses(s.root, fill && s.rootFill)}>
      {/* Aparência de botão: `filled-circular` do próprio TabList do Fluent
          (pílulas; a escolhida preenchida na cor do tema). */}
      <div className={s.bar}>
        <TabList
          className={s.tabs}
          appearance="filled-circular"
          selectedValue={scene}
          onTabSelect={(_, d) => setScene(d.value as PreviewScene)}
        >
          <Tab value="2d">{t('video.preview.scene2d')}</Tab>
          <Tab value="3d">{t('video.preview.scene3d')}</Tab>
        </TabList>
        {/* liga/desliga o shader já carregado: troca entre as duas imagens,
            sem renderizar de novo */}
        <ToggleButton
          checked={on}
          appearance={on ? 'primary' : 'secondary'}
          icon={on ? <EyeRegular /> : <EyeOffRegular />}
          aria-label={t('video.preview.toggle')}
          onClick={() => setOn((v) => !v)}
        >
          {on ? t('video.preview.on') : t('video.preview.off')}
        </ToggleButton>
      </div>
      <div className={mergeClasses(s.frame, fill && s.frameFill)}>
        {shown && (
          <img
            className={mergeClasses(s.img, fill && s.imgCover)}
            src={shown}
            alt={on ? t('video.preview.shader') : t('video.preview.original')}
            draggable={false}
          />
        )}
        {on && preview.isFetching && (
          <div className={s.busy}>
            <Spinner size="small" />
          </div>
        )}
        {/* a prévia falhou (preset que o WebGPU recusa): o aviso no lugar
            da imagem */}
        {on && preview.isError && !preview.isFetching && (
          <div className={s.busy}>
            <Caption1 className={s.error}>{t('video.preview.error')}</Caption1>
          </div>
        )}
      </div>
    </div>
  )
}
