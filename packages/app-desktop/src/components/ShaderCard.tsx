import {
  Button,
  Caption1,
  Card,
  CardHeader,
  Text,
  Tooltip,
  makeStyles,
  mergeClasses,
  shorthands,
  tokens,
} from '@fluentui/react-components'
import { OptionsRegular } from '@fluentui/react-icons'
import { useTranslation } from 'react-i18next'

const useStyles = makeStyles({
  // Card selecionável do Fluent: a superfície inteira escolhe o shader
  // (hover/foco mostram que dá pra clicar — "Card › Selectable" no Fluent 2).
  // Borda na cor de marca no escolhido, como os cards de tema.
  card: {
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    border: `1px solid transparent`,
    // Na lista com rolagem (coluna flex com altura máxima) o card não pode
    // encolher pra caber: com muitos shaders eles eram espremidos e o texto
    // saía cortado. Com isso a lista rola.
    flexShrink: 0,
  },
  selected: { border: `1px solid ${tokens.colorBrandStroke1}` },
  disabled: { opacity: 0.5, cursor: 'default' },
  title: { display: 'inline-flex', alignItems: 'center', gap: tokens.spacingHorizontalXS },
  desc: { color: tokens.colorNeutralForeground3 },
  // `outline` com a borda visível sobre o fundo do card: o `Stroke1` do
  // tema tem quase a cor do card; o `StrokeAccessible` é o token do Fluent
  // feito pra ficar visível (contraste de 3:1).
  settingsBtn: {
    ...shorthands.borderColor(`${tokens.colorNeutralStrokeAccessible} !important`),
  },
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
            {title}
          </Text>
        }
        description={description ? <Caption1 className={s.desc}>{description}</Caption1> : undefined}
        action={
          onSettings ? (
            <Tooltip content={t('video.shaderSettings')} relationship="label">
              <Button
                className={s.settingsBtn}
                appearance="outline"
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
