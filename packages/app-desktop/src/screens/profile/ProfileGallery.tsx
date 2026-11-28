import {
  Button, Dialog, DialogBody, DialogContent, DialogSurface, DialogTitle, makeStyles, tokens,
} from '@fluentui/react-components'
import { useRef, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { CloseButton } from '../../components/CloseButton'
import { SectionHeader } from '../../components/SectionHeader'
import { DIALOG_FADE_ONLY } from '../../lib/motion'

const PREVIEW_SIZE = 9
const tileCorners = {
  '& .fui-Card, & [data-art]': { borderRadius: '10px' },
}

const useStyles = makeStyles({
  section: { maxWidth: '450px', overflowWrap: 'anywhere' },
  heading: { fontSize: '20px', lineHeight: '24px', fontWeight: tokens.fontWeightRegular },
  button: { whiteSpace: 'nowrap' },
  preview: {
    ...tileCorners,
    display: 'grid',
    gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
    gap: 'clamp(10px, 1.4vw, 16px)',
  },
  dialog: {
    width: 'min(500px, calc(100vw - 32px))',
    maxHeight: 'min(600px, calc(100vh - 36px))',
    padding: 0,
    overflow: 'hidden',
    borderRadius: '16px',
  },
  backdrop: { backgroundColor: 'rgba(0, 0, 0, 0.67)' },
  body: {
    display: 'grid',
    gridTemplateRows: 'auto minmax(0, 1fr)',
    maxHeight: 'min(600px, calc(100vh - 36px))',
    rowGap: 0,
    padding: '10px 22px 0',
    '& .fui-DialogTitle__action .fui-Button': {
      borderRadius: '8px',
      backgroundColor: tokens.colorNeutralBackground3,
    },
  },
  title: { padding: '4px 0 12px', margin: 0, fontSize: '20px', lineHeight: '28px', fontWeight: tokens.fontWeightRegular },
  content: { overflowY: 'auto', scrollbarGutter: 'stable' },
  fullGrid: {
    ...tileCorners,
    display: 'grid',
    gridTemplateColumns: 'repeat(4, minmax(0, 1fr))',
    gap: '14px',
    '@media (max-width: 460px)': { gridTemplateColumns: 'repeat(3, minmax(0, 1fr))' },
  },
  description: { margin: '0 0 12px', color: tokens.colorNeutralForeground2 },
  footer: { marginTop: '14px' },
})

export function ProfileGallery({
  title,
  items,
  testId,
  description,
  footer,
  onOpenChange,
}: {
  title: string
  items: ReactNode[]
  testId: string
  description?: string
  footer?: ReactNode
  onOpenChange?: (open: boolean) => void
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const [open, setOpen] = useState(false)
  const trigger = useRef<HTMLButtonElement>(null)
  const changeOpen = (next: boolean) => {
    setOpen(next)
    onOpenChange?.(next)
    if (!next) requestAnimationFrame(() => trigger.current?.focus({ preventScroll: true }))
  }
  return (
    <section className={s.section} data-testid={testId}>
      <SectionHeader
        title={title}
        titleClassName={s.heading}
        onCard
        right={
          <Button
            ref={trigger}
            appearance="secondary"
            className={s.button}
            aria-haspopup="dialog"
            onClick={() => changeOpen(true)}
          >
            {t('shell2.showAll')}
          </Button>
        }
      />
      {items.length > 0 && <div className={s.preview}>{items.slice(0, PREVIEW_SIZE)}</div>}
      <Dialog open={open} onOpenChange={(_, data) => changeOpen(data.open)} surfaceMotion={DIALOG_FADE_ONLY}>
        <DialogSurface className={s.dialog} backdrop={{ className: s.backdrop }}>
          <DialogBody className={s.body}>
            <DialogTitle className={s.title} action={<CloseButton onClick={() => changeOpen(false)} />}>
              {title}
            </DialogTitle>
            <DialogContent className={s.content}>
              {description && <p className={s.description}>{description}</p>}
              <div className={s.fullGrid}>{items}</div>
              {footer && <div className={s.footer}>{footer}</div>}
            </DialogContent>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </section>
  )
}
