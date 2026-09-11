import {
  Caption1,
  Text,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkFilled } from '@fluentui/react-icons'
import { useThemeStore } from '../../stores/useThemeStore'
import { THEMES, type ThemeId } from '../../styles/themes'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL, maxWidth: '520px' },
  grid: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))',
    gap: tokens.spacingHorizontalM,
  },
  card: {
    position: 'relative',
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXS,
    padding: tokens.spacingHorizontalS,
    borderRadius: tokens.borderRadiusLarge,
    border: `2px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: tokens.colorNeutralBackground2,
    cursor: 'pointer',
    textAlign: 'left',
  },
  cardOn: { border: `2px solid ${tokens.colorBrandStroke1}` },
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
    color: tokens.colorBrandForeground1,
  },
})

const IDS = Object.keys(THEMES) as ThemeId[]

/** Configurações › Aparência — tema de cor + fundo animado. */
export function SettingsAppearance() {
  const s = useStyles()
  const { themeId, setTheme } = useThemeStore()

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
              className={mergeClasses(s.card, on && s.cardOn)}
              onClick={() => setTheme(id)}
            >
              {on && <CheckmarkFilled className={s.check} />}
              <div className={s.swatch}>
                <span className={s.seg} style={{ background: t.reemuBg1 }} />
                <span className={s.seg} style={{ background: t.reemuBrandSolid }} />
                <span className={s.seg} style={{ background: t.reemuBg2 }} />
              </div>
              <Text weight={on ? 'semibold' : 'regular'}>{THEMES[id].label}</Text>
            </button>
          )
        })}
      </div>
    </div>
  )
}
