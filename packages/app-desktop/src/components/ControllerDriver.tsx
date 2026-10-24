import {
  Caption1,
  Radio,
  RadioGroup,
  Text,
  makeStyles,
  tokens,
  type RadioGroupOnChangeData,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import {
  CONTROLLER_DRIVERS,
  getControllerDriver,
  setControllerDriver,
  type ControllerDriver as Driver,
} from '../lib/tauri'
import { errorToast, sysToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', rowGap: tokens.spacingVerticalM },
  optionHint: { display: 'block', color: tokens.colorNeutralForeground3 },
})

/** Configurações › Controles: driver do SDL que lê os controles no Windows
 *  (`input_desktop::pad_source::Driver`). Fora do Windows não aparece — lá
 *  não há escolha. A troca reinicia só a leitura dos controles. */
export function ControllerDriver() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const settings = useQuery({
    queryKey: ['controller-driver'],
    queryFn: getControllerDriver,
    retry: false,
  })
  const change = useMutation({
    mutationFn: setControllerDriver,
    onSuccess: (_, driver) => {
      qc.invalidateQueries({ queryKey: ['controller-driver'] })
      qc.invalidateQueries({ queryKey: ['gamepads'] })
      push(sysToast(t('controllers.driver.changed', { name: t(`controllers.driver.${driver}`) }), 'Success'))
    },
    onError: (e) => {
      qc.invalidateQueries({ queryKey: ['controller-driver'] })
      push(errorToast(e, 'setControllerDriver'))
    },
  })

  if (!settings.data?.supported) return null
  // Enquanto troca, o rádio já mostra a escolha nova.
  const value = change.isPending && change.variables ? change.variables : settings.data.driver

  return (
    <section className={s.root}>
      <div>
        <Text as="strong" weight="semibold">
          {t('controllers.driver.title')}
        </Text>
        <Caption1 as="p" block style={{ margin: '2px 0 0' }}>
          {t('controllers.driver.description')}
        </Caption1>
      </div>
      <RadioGroup
        aria-label={t('controllers.driver.title')}
        value={value}
        disabled={change.isPending}
        onChange={(_, data: RadioGroupOnChangeData) => change.mutate(data.value as Driver)}
      >
        {CONTROLLER_DRIVERS.map((d) => (
          <Radio
            key={d}
            value={d}
            label={
              <>
                {t(`controllers.driver.${d}`)}
                <Caption1 className={s.optionHint}>{t(`controllers.driver.${d}Hint`)}</Caption1>
              </>
            }
          />
        ))}
      </RadioGroup>
    </section>
  )
}
