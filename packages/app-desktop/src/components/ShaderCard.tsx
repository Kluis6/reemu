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
import { ChevronRightRegular, OptionsRegular } from '@fluentui/react-icons'
import type { Ref } from 'react'
import { useTranslation } from 'react-i18next'
import { TOOLTIP_POSITION } from '../lib/tooltip'

const settingsButtonColors = {
  backgroundColor: 'var(--reemuShaderOptionsBg)',
  color: 'var(--reemuShaderOptionsIcon)',
}

const useStyles = makeStyles({
  // Card selecionável do Fluent: a superfície inteira escolhe o shader
  // (hover/foco mostram que dá pra clicar — "Card › Selectable" no Fluent 2).
  // Borda na cor de marca no escolhido, como os cards de tema.
  card: {
    padding: `${tokens.spacingVerticalS} ${tokens.spacingHorizontalM}`,
    border: `1px solid transparent`,
    // sem a sombra do <Card> do Fluent (pedido do usuário): destaque só pela
    // cor, como os outros cards
    boxShadow: 'none !important',
    // Na lista com rolagem (coluna flex com altura máxima) o card não pode
    // encolher pra caber: com muitos shaders eles eram espremidos e o texto
    // saía cortado. Com isso a lista rola.
    flexShrink: 0,
  },
  // O fundo usa a cor de marca com contraste para texto branco.
  selected: {
    border: '1px solid transparent',
    backgroundColor: 'var(--reemuSelectedShaderBg)',
    color: '#fff',
    '::after': { ...shorthands.borderColor('transparent') },
    ':hover': {
      backgroundColor: 'var(--reemuSelectedShaderBg)',
      '::after': { ...shorthands.borderColor('transparent') },
    },
    ':active': {
      backgroundColor: 'var(--reemuSelectedShaderBg)',
      '::after': { ...shorthands.borderColor('transparent') },
    },
    '& .fui-Text, & .fui-Caption1, & .fui-CardHeader__description': {
      color: '#fff',
    },
  },
  disabled: { opacity: 0.5, cursor: 'default' },
  title: { display: 'inline-flex', alignItems: 'center', gap: tokens.spacingHorizontalXS },
  desc: { color: tokens.colorNeutralForeground3 },
  settingsBtn: {
    ...settingsButtonColors,
    ...shorthands.borderColor('transparent'),
    ':is(:hover, :focus, :active)': settingsButtonColors,
    ':focus-visible': { outlineColor: `${tokens.colorBrandStroke1} !important` },
  },
  settingsBtnSelected: {
    // O anel da marca pode coincidir com a cor do card selecionado.
    ['--colorBrandStroke1' as string]: 'var(--reemuShaderOptionsFocus)',
  },
})

export function ShaderCategoryCard({ title, count, onOpen, ref }: {
  title: string
  count: number
  onOpen: () => void
  ref?: Ref<HTMLDivElement>
}) {
  const { t } = useTranslation()
  const s = useStyles()
  return (
    <Card
      ref={ref}
      appearance="filled-alternative"
      className={s.card}
      role="button"
      tabIndex={0}
      focusMode="off"
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault()
          onOpen()
        }
      }}
    >
      <CardHeader
        header={<Text weight="semibold" className={s.title}>{title}</Text>}
        description={<Caption1 className={s.desc}>{t('shaders.presets', { count })}</Caption1>}
        action={<ChevronRightRegular aria-hidden />}
      />
    </Card>
  )
}

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
      // `filled-alternative`: fundo `colorNeutralBackground2` (e Hover/
      // Pressed/Selected), o tom dos outros cards das Configurações. O
      // padrão (`filled`) usa o `Background1` — o mesmo do card que envolve
      // a página, então a lista ficava sem contraste.
      appearance="filled-alternative"
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
            <Tooltip positioning={TOOLTIP_POSITION} content={t('video.shaderSettings')} relationship="label">
              <Button
                className={mergeClasses(s.settingsBtn, selected && s.settingsBtnSelected)}
                appearance="secondary"
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
