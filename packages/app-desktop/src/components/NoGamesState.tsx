import { Button } from '@fluentui/react-components'
import { AddRegular, JoystickRegular } from '@fluentui/react-icons'
import { useTranslation } from 'react-i18next'
import { useEffectiveProfile } from '../hooks/useEffectiveProfile'
import { EmptyState } from './EmptyState'

/**
 * Página sem nenhum jogo — a mesma na Início e na Biblioteca (pedido do
 * usuário): joystick de fliperama do Fluent (lembra mais um controle retrô
 * que o gamepad de linha), "Bem-vindo, fulano!" com o nome do perfil (sem
 * nome, "Bem-vindo ao ReEmu") e o botão de adicionar ROMs, tudo grande e no
 * meio da página (`EmptyState size="large"`). Conectado ao Alvanista, o nome
 * é o de lá (`useEffectiveProfile`).
 */
export function NoGamesState({ onAdd }: { onAdd: () => void }) {
  const { t } = useTranslation()
  const name = useEffectiveProfile()?.name?.trim()
  return (
    <EmptyState
      size="large"
      art={<JoystickRegular />}
      title={name ? t('home.welcomeName', { name }) : t('home.welcome')}
      action={
        <Button appearance="primary" size="large" icon={<AddRegular />} onClick={onAdd}>
          {t('library.addRoms')}
        </Button>
      }
    >
      {t('home.welcomeHint')}
    </EmptyState>
  )
}
