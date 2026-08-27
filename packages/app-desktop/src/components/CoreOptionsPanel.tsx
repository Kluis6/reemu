import {
  Dropdown,
  Field,
  Option,
  Slider,
  Switch,
  makeStyles,
  tokens,
} from '@fluentui/react-components'

// Espelha `domain::core_options::CoreOptionDefinition` (via IPC).
export type CoreOptionType =
  | { kind: 'combo'; choices: string[] }
  | { kind: 'bool' }
  | { kind: 'range'; min: number; max: number; step: number }

export interface CoreOptionDefinition {
  optionKey: string
  displayName: string
  optionType: CoreOptionType
  defaultValue: string
}

const useStyles = makeStyles({
  panel: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalM,
  },
})

/**
 * Tela de opções de core **gerada automaticamente** a partir do schema
 * dinâmico — nunca UI custom por core (decisão da etapa 07).
 */
export function CoreOptionsPanel({
  schema,
  values,
  onChange,
}: {
  schema: CoreOptionDefinition[]
  values: Record<string, string>
  onChange: (key: string, value: string) => void
}) {
  const styles = useStyles()
  const valueOf = (d: CoreOptionDefinition) => values[d.optionKey] ?? d.defaultValue

  return (
    <div className={styles.panel}>
      {schema.map((d) => (
        <Field key={d.optionKey} label={d.displayName}>
          {d.optionType.kind === 'combo' && (
            <Dropdown
              value={valueOf(d)}
              selectedOptions={[valueOf(d)]}
              onOptionSelect={(_, data) => data.optionValue && onChange(d.optionKey, data.optionValue)}
            >
              {d.optionType.choices.map((c) => (
                <Option key={c}>{c}</Option>
              ))}
            </Dropdown>
          )}
          {d.optionType.kind === 'bool' && (
            <Switch
              checked={valueOf(d) === 'true'}
              onChange={(_, data) => onChange(d.optionKey, String(data.checked))}
            />
          )}
          {d.optionType.kind === 'range' && (
            <Slider
              min={d.optionType.min}
              max={d.optionType.max}
              step={d.optionType.step}
              value={Number(valueOf(d))}
              onChange={(_, data) => onChange(d.optionKey, String(data.value))}
            />
          )}
        </Field>
      ))}
    </div>
  )
}
