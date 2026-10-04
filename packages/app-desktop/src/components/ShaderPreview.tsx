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
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { renderShaderPreview, shaderPreviewSource, type PreviewScene } from '../lib/tauri'
import { useTabStyles } from '../styles/xbox'

/** Tamanho em que o shader é renderizado (3× a cena de 320×240, 4:3). */
const OUT_W = 960
const OUT_H = 720

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalS,
    maxWidth: '480px',
  },
  // abas no estilo do app; 24 até a imagem (8 do `gap` + 16)
  tabs: { alignSelf: 'flex-start', marginBottom: '16px' },
  frame: {
    position: 'relative',
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
  // Botão sobre a imagem, no canto de baixo: liga/desliga o shader já
  // carregado (troca entre as duas imagens, sem renderizar de novo).
  toggle: {
    position: 'absolute',
    right: tokens.spacingHorizontalS,
    bottom: tokens.spacingVerticalS,
  },
  toggleOff: {
    backgroundColor: 'rgba(0, 0, 0, 0.6) !important',
    color: '#fff !important',
  },
  busy: {
    position: 'absolute',
    inset: 0,
    display: 'grid',
    placeItems: 'center',
    backgroundColor: 'rgba(0, 0, 0, 0.35)',
  },
})

/** `ArrayBuffer` de PNG → URL de blob, revogada quando troca. */
function useBlobUrl(buf: ArrayBuffer | undefined) {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    if (!buf) return
    const u = URL.createObjectURL(new Blob([buf], { type: 'image/png' }))
    setUrl(u)
    return () => URL.revokeObjectURL(u)
  }, [buf])
  return url
}

/**
 * Prévia do shader ativo numa cena de exemplo gerada pelo app (2D em pixel
 * art ou 3D no jeito de PS1/N64). O botão sobre a imagem liga e desliga o
 * shader já carregado. Renderizada na GPU pelo mesmo código do jogo
 * (`render_shader_preview`), com os mesmos parâmetros.
 */
export function ShaderPreview({ reloadKey }: { reloadKey: string }) {
  const { t } = useTranslation()
  const s = useStyles()
  const tb = useTabStyles()
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
    <div className={s.root}>
      <TabList
        className={mergeClasses(tb.tabs, s.tabs)}
        selectedValue={scene}
        onTabSelect={(_, d) => setScene(d.value as PreviewScene)}
      >
        <Tab value="2d">{t('video.preview.scene2d')}</Tab>
        <Tab value="3d">{t('video.preview.scene3d')}</Tab>
      </TabList>
      <div className={s.frame}>
        {shown && (
          <img
            className={s.img}
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
        <ToggleButton
          className={mergeClasses(s.toggle, !on && s.toggleOff)}
          checked={on}
          appearance={on ? 'primary' : 'secondary'}
          icon={on ? <EyeRegular /> : <EyeOffRegular />}
          aria-label={t('video.preview.toggle')}
          onClick={() => setOn((v) => !v)}
        >
          {on ? t('video.preview.on') : t('video.preview.off')}
        </ToggleButton>
      </div>
      <Caption1>
        {preview.isError ? t('video.preview.error') : t('video.preview.hint')}
      </Caption1>
    </div>
  )
}
