import { makeStyles, tokens } from '@fluentui/react-components'
import { useImageExists } from '../hooks/useImageExists'

/** `reemu-logo.png` fica em `packages/app-desktop/public/`. */
const LOGO_SRC = '/reemu-logo.png'

const useStyles = makeStyles({
  img: { display: 'block', width: 'auto', objectFit: 'contain' },
  wordmark: { fontWeight: tokens.fontWeightBold, letterSpacing: '0.02em', lineHeight: 1 },
  // `--reemuBrandText`, não `colorBrandForeground1` — este é texto (o
  // fallback do wordmark), e o tom padrão de marca falha contraste AA no
  // tema claro (ver `styles/themes.ts::ReEmuTokens.reemuBrandText`).
  green: { color: "var(--reemuBrandText)" },
})

/** Logo do ReEmu — imagem quando disponível, senão o wordmark "ReEmu". */
export function AppLogo({ height = 40 }: { height?: number }) {
  const s = useStyles()
  // Pré-carrega fora do DOM — nunca monta um <img> quebrado (o ícone de
  // imagem ausente do navegador piscava na tela até o onError reagir).
  const imgOk = useImageExists(LOGO_SRC)
  if (!imgOk)
    return (
      <span className={s.wordmark} style={{ fontSize: height * 0.7 }}>
        Re<span className={s.green}>Emu</span>
      </span>
    )
  return (
    <img className={s.img} src={LOGO_SRC} alt="ReEmu" style={{ height }} />
  )
}
