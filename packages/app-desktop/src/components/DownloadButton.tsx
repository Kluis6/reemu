import {
  Button,
  Spinner,
  Tooltip,
  makeStyles,
  mergeClasses,
  type ButtonProps,
} from '@fluentui/react-components'
import { CloudArrowDownRegular } from '@fluentui/react-icons'

const useStyles = makeStyles({
  // Mais largo que o quadrado do botão só-ícone, como o "baixar" da
  // Biblioteca da Microsoft Store: alvo maior pro mouse e pro controle.
  btn: { minWidth: '96px' },
})

/**
 * Botão de baixar: só o ícone da nuvem com seta (como na Microsoft Store).
 * Botão só com ícone precisa de nome acessível e de tooltip (Fluent 2,
 * Button › "icon-only"): o `label` vira os dois. Ocupado, o ícone vira um
 * `Spinner` e o botão fica desabilitado.
 */
export function DownloadButton({
  label,
  busy = false,
  disabled,
  appearance = 'secondary',
  onClick,
  className,
}: {
  label: string
  busy?: boolean
  disabled?: boolean
  appearance?: ButtonProps['appearance']
  onClick: () => void
  className?: string
}) {
  const s = useStyles()
  return (
    <Tooltip content={label} relationship="label">
      <Button
        className={mergeClasses(s.btn, className)}
        appearance={appearance}
        icon={busy ? <Spinner size="tiny" /> : <CloudArrowDownRegular />}
        disabled={disabled || busy}
        onClick={onClick}
      />
    </Tooltip>
  )
}
