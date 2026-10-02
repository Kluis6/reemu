import { makeStyles, tokens } from '@fluentui/react-components'

/** Símbolo do ReEmu, o mesmo do site (`site/logo.webp`), copiado para
 *  `packages/app-desktop/public/`. */
const MARK_SRC = '/reemu-logo.webp'

const useStyles = makeStyles({
  wordmark: {
    display: 'inline-flex',
    alignItems: 'center',
    columnGap: '0.25em',
    fontWeight: tokens.fontWeightBold,
    letterSpacing: '0.02em',
    lineHeight: 1,
  },
  mark: { height: '1.5em', width: 'auto' },
  // `--reemuBrandText`, não `colorBrandForeground1` — é texto, e o tom
  // padrão de marca falha contraste AA no tema claro (ver
  // `styles/themes.ts::ReEmuTokens.reemuBrandText`).
  green: { color: "var(--reemuBrandText)" },
})

/** Logo do ReEmu: o símbolo do site ao lado do nome, como no topo do site. */
export function AppLogo({ height = 40 }: { height?: number }) {
  const s = useStyles()
  return (
    <span className={s.wordmark} style={{ fontSize: height * 0.7 }}>
      <img className={s.mark} src={MARK_SRC} alt="" />
      <span>
        Re<span className={s.green}>Emu</span>
      </span>
    </span>
  )
}
