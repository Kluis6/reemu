import {
  Button,
  Caption1,
  Card,
  CardHeader,
  Text,
  Tooltip,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkCircleFilled, OptionsRegular } from '@fluentui/react-icons'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  // Card selecionável do Fluent: a superfície inteira escolhe o shader
  // (hover/foco mostram que dá pra clicar — "Card › Selectable" no Fluent 2).
  // Borda na cor de marca no escolhido, como os cards de tema.
  card: {
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    border: `1px solid transparent`,
  },
  selected: { border: `1px solid ${tokens.colorBrandStroke1}` },
  disabled: { opacity: 0.5, cursor: 'default' },
  title: { display: 'inline-flex', alignItems: 'center', gap: tokens.spacingHorizontalXS },
  check: { color: tokens.colorBrandForeground1, flexShrink: 0 },
  desc: { color: tokens.colorNeutralForeground3 },
})

/**
 * Um shader da lista: card selecionável (escolher = aplicar) com título e
 * descrição no `CardHeader`. Com `onSettings`, mostra no slot `action` do
 * cabeçalho o botão que abre os ajustes do shader (ação secundária, separada
 * da seleção, como a documentação do Card orienta).
 */
export function ShaderCard({
  title,
  description,
  selected,
  disabled,
  onSelect,
  onSettings,
}: {
  title: string
  description?: string
  selected: boolean
  disabled?: boolean
  onSelect: () => void
  onSettings?: () => void
}) {
  const { t } = useTranslation()
  const s = useStyles()
  return (
    <Card
      className={mergeClasses(s.card, selected && s.selected, disabled && s.disabled)}
      selected={selected}
      onSelectionChange={() => {
        if (!disabled && !selected) onSelect()
      }}
      aria-disabled={disabled || undefined}
      aria-label={title}
    >
      <CardHeader
        header={
          <Text weight="semibold" className={s.title}>
            {selected && <CheckmarkCircleFilled className={s.check} />}
            {title}
          </Text>
        }
        description={description ? <Caption1 className={s.desc}>{description}</Caption1> : undefined}
        action={
          onSettings ? (
            <Tooltip content={t('video.shaderSettings')} relationship="label">
              <Button
                appearance="subtle"
                icon={<OptionsRegular />}
                // o clique/tecla no botão não pode virar seleção do card
                onClick={(e) => {
                  e.stopPropagation()
                  onSettings()
                }}
                onKeyDown={(e) => e.stopPropagation()}
              />
            </Tooltip>
          ) : undefined
        }
      />
    </Card>
  )
}
