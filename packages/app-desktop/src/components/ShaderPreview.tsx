import { Caption1, Slider, Spinner, makeStyles, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { renderShaderPreview, shaderPreviewSource } from '../lib/tauri'

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
  divider: {
    position: 'absolute',
    top: 0,
    bottom: 0,
    width: '2px',
    marginLeft: '-1px',
    backgroundColor: '#fff',
    boxShadow: '0 0 4px rgba(0, 0, 0, 0.6)',
    pointerEvents: 'none',
  },
  tag: {
    position: 'absolute',
    top: tokens.spacingVerticalS,
    paddingLeft: tokens.spacingHorizontalS,
    paddingRight: tokens.spacingHorizontalS,
    borderRadius: tokens.borderRadiusMedium,
    backgroundColor: 'rgba(0, 0, 0, 0.6)',
    color: '#fff',
    pointerEvents: 'none',
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
 * Prévia do shader ativo numa cena de exemplo (pixel art gerada pelo app),
 * com comparador "antes / depois": o slider move a divisão entre a cena
 * original (à esquerda) e a com shader (à direita). Renderizada na GPU pelo
 * mesmo código do jogo (`render_shader_preview`), com os mesmos parâmetros.
 */
export function ShaderPreview({ reloadKey }: { reloadKey: string }) {
  const { t } = useTranslation()
  const s = useStyles()
  const [split, setSplit] = useState(50)

  const source = useQuery({
    queryKey: ['shader-preview-source'],
    queryFn: shaderPreviewSource,
    staleTime: Infinity,
    retry: false,
  })
  const preview = useQuery({
    queryKey: ['shader-preview', reloadKey],
    queryFn: () => renderShaderPreview(OUT_W, OUT_H),
    retry: false,
    // mantém a imagem anterior na tela enquanto a nova é gerada
    placeholderData: (prev) => prev,
  })
  const before = useBlobUrl(source.data)
  const after = useBlobUrl(preview.data)

  return (
    <div className={s.root}>
      <div className={s.frame}>
        {after && <img className={s.img} src={after} alt={t('video.preview.shader')} draggable={false} />}
        {before && (
          <img
            className={s.img}
            src={before}
            alt={t('video.preview.original')}
            draggable={false}
            style={{ clipPath: `inset(0 ${100 - split}% 0 0)` }}
          />
        )}
        <div className={s.divider} style={{ left: `${split}%` }} />
        <Caption1 className={s.tag} style={{ left: tokens.spacingHorizontalS }}>
          {t('video.preview.original')}
        </Caption1>
        <Caption1 className={s.tag} style={{ right: tokens.spacingHorizontalS }}>
          {t('video.preview.shader')}
        </Caption1>
        {preview.isFetching && (
          <div className={s.busy}>
            <Spinner size="small" />
          </div>
        )}
      </div>
      <Slider
        min={0}
        max={100}
        value={split}
        onChange={(_, d) => setSplit(d.value)}
        aria-label={t('video.preview.compare')}
      />
      <Caption1>
        {preview.isError ? t('video.preview.error') : t('video.preview.hint')}
      </Caption1>
    </div>
  )
}
