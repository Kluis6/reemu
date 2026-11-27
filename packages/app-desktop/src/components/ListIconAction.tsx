import { Button, Tooltip, makeStyles, mergeClasses, tokens } from '@fluentui/react-components'
import type { MouseEventHandler, ReactElement } from 'react'
import { TOOLTIP_ABOVE } from '../lib/tooltip'
import { useModalButtonStyles } from '../styles/buttons'

// Tamanho das ações de linha em Configurações, como em Gerenciar biblioteca.
export const LIST_ICON_ACTION_WIDTH = '48px'

const useStyles = makeStyles({
  action: {
    width: LIST_ICON_ACTION_WIDTH,
    height: LIST_ICON_ACTION_WIDTH,
    flexShrink: 0,
    padding: '0 !important',
    minWidth: '0 !important',
    maxWidth: 'none !important',
    border: '1px solid transparent !important',
  },
  keepBg: {
    ':hover, :hover:active, :focus, :focus-visible': {
      backgroundColor: 'var(--reemuControlFill) !important',
      color: `${tokens.colorNeutralForeground1} !important`,
    },
  },
})

export function ListIconAction({ label, icon, primary = false, disabled, onClick, 'aria-haspopup': hasPopup }: {
  label: string
  icon: ReactElement
  primary?: boolean
  disabled?: boolean
  onClick: MouseEventHandler<HTMLButtonElement>
  'aria-haspopup'?: 'dialog'
}) {
  const s = useStyles()
  const button = useModalButtonStyles()
  return (
    <Tooltip positioning={TOOLTIP_ABOVE} content={label} relationship="label">
      <Button
        className={mergeClasses(s.action, !primary && s.keepBg, !primary && button.secondaryFlat)}
        appearance={primary ? 'primary' : 'secondary'}
        icon={icon}
        aria-haspopup={hasPopup}
        disabled={disabled}
        onClick={onClick}
      />
    </Tooltip>
  )
}
