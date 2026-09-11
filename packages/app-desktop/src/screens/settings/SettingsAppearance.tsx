import {
  Button,
  Caption1,
  Text,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkFilled, ImageAddRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { clearWallpaper, pickImage, setWallpaperFile, wallpaperUrl } from '../../lib/tauri'
import { sysToast } from '../../lib/toast'
import { useToastStore } from '../../stores/useToastStore'
import { useThemeStore } from '../../stores/useThemeStore'
import { THEMES, type ThemeId } from '../../styles/themes'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL, maxWidth: '520px' },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))',
    gap: tokens.spacingHorizontalM,
  },
  // Cor de fundo/borda/texto vêm inline do PRÓPRIO tema sendo mostrado (`t`),
  // não do tema ativo — senão o card do tema "Claro" ficaria escuro sempre
  // que o usuário já estivesse num tema escuro (e vice-versa), e a prévia
  // não serviria pra nada.
  card: {
    position: 'relative',
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXS,
    padding: tokens.spacingHorizontalS,
    borderRadius: tokens.borderRadiusLarge,
    border: '2px solid transparent',
    cursor: 'pointer',
    textAlign: 'left',
  },
  swatch: {
    height: '46px',
    borderRadius: tokens.borderRadiusMedium,
    display: 'flex',
    overflow: 'hidden',
  },
  seg: { flexGrow: 1 },
  check: {
    position: 'absolute',
    top: tokens.spacingVerticalXS,
    right: tokens.spacingHorizontalXS,
  },
  wallRow: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalL,
  },
  wallPreview: {
    width: '96px',
    height: '64px',
    flexShrink: 0,
    borderRadius: tokens.borderRadiusLarge,
    border: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground3,
    backgroundSize: 'cover',
    backgroundPosition: 'center',
    display: 'grid',
    alignItems: 'center',
    justifyItems: 'center',
    color: tokens.colorNeutralForeground3,
    fontSize: '20px',
  },
  wallActions: { display: 'flex', gap: tokens.spacingHorizontalS },
})

const IDS = Object.keys(THEMES) as ThemeId[]

/** Configurações › Aparência — tema de cor + papel de parede da tela inicial. */
export function SettingsAppearance() {
  const s = useStyles()
  const { themeId, setTheme } = useThemeStore()
  const qc = useQueryClient()
  const push = useToastStore((t) => t.push)
  const wallpaper = useQuery({ queryKey: ['wallpaper'], queryFn: wallpaperUrl })

  const upload = useMutation({
    mutationFn: async () => {
      const path = await pickImage('Escolha um papel de parede')
      if (!path) return false
      await setWallpaperFile(path)
      return true
    },
    onSuccess: (ok) => {
      if (!ok) return
      qc.invalidateQueries({ queryKey: ['wallpaper'] })
    },
    onError: (e) => push(sysToast(`Falha ao carregar imagem: ${e}`, 'Error')),
  })
  const remove = useMutation({
    mutationFn: clearWallpaper,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['wallpaper'] }),
    onError: (e) => push(sysToast(`Falha ao remover: ${e}`, 'Error')),
  })

  return (
    <div className={s.root}>
      <div>
        <Text as="strong" weight="semibold">
          Tema de cor
        </Text>
        <Caption1 as="p" style={{ margin: '2px 0 0' }}>
          Muda a cor de destaque e do fundo. Mais temas vêm depois.
        </Caption1>
      </div>

      <div className={s.grid} role="radiogroup" aria-label="Tema de cor">
        {IDS.map((id) => {
          const t = THEMES[id].theme
          const on = id === themeId
          return (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={on}
              className={s.card}
              style={{
                backgroundColor: t.colorNeutralBackground2,
                borderColor: on ? t.colorBrandStroke1 : t.colorNeutralStroke2,
              }}
              onClick={() => setTheme(id)}
            >
              {on && (
                <CheckmarkFilled
                  className={s.check}
                  style={{ color: t.colorBrandForeground1 }}
                />
              )}
              <div className={s.swatch}>
                <span className={s.seg} style={{ background: t.reemuBg1 }} />
                <span className={s.seg} style={{ background: t.reemuBrandSolid }} />
                <span className={s.seg} style={{ background: t.reemuBg2 }} />
              </div>
              <Text
                weight={on ? 'semibold' : 'regular'}
                style={{ color: t.colorNeutralForeground1 }}
              >
                {THEMES[id].label}
              </Text>
            </button>
          )
        })}
      </div>

      <div>
        <Text as="strong" weight="semibold">
          Papel de parede
        </Text>
        <Caption1 as="p" style={{ margin: '2px 0 0' }}>
          Uma imagem sua atrás das cores do tema, na tela inicial. Opcional —
          as manchas de cor ficam por cima, então o tema continua dando o tom.
        </Caption1>
      </div>

      <div className={s.wallRow}>
        <div
          className={s.wallPreview}
          style={
            wallpaper.data ? { backgroundImage: `url(${wallpaper.data})` } : undefined
          }
        >
          {!wallpaper.data && <ImageAddRegular />}
        </div>
        <div className={s.wallActions}>
          <Button
            appearance="secondary"
            disabled={upload.isPending}
            onClick={() => upload.mutate()}
          >
            {wallpaper.data ? 'Trocar imagem…' : 'Escolher imagem…'}
          </Button>
          {wallpaper.data && (
            <Button
              appearance="subtle"
              disabled={remove.isPending}
              onClick={() => remove.mutate()}
            >
              Remover
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
