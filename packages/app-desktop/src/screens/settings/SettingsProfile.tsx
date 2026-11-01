import {
  Avatar,
  Badge,
  Body1Strong,
  Button,
  Caption1,
  MessageBar,
  MessageBarBody,
  Spinner,
  makeStyles,
  tokens,
} from '@fluentui/react-components'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { AlvanistaButton } from '../../components/AlvanistaButton'
import { LoadingState } from '../../components/EmptyState'
import { ProfileAvatar } from '../../components/ProfileAvatar'
import { ProfileForm } from '../../components/ProfileForm'
import { ALVANISTA_CACHED_KEY } from '../../hooks/useEffectiveProfile'
import {
  ALVANISTA_STATUS_KEY,
  DISCONNECTED,
  useAlvanistaStatusSync,
} from '../../hooks/useAlvanistaStatusSync'
import { sysToast } from '../../lib/toast'
import { useAlvanistaConnect, useAlvanistaDisconnect } from '../../hooks/useAlvanistaConnect'
import {
  alvanistaCachedStatus,
  alvanistaOpenSite,
  alvanistaStatus,
  getProfile,
} from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
  block: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-start',
    gap: tokens.spacingVerticalS,
  },
  waiting: { display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalM },
  identity: { display: 'flex', alignItems: 'center', gap: tokens.spacingHorizontalL },
  actions: { display: 'flex', flexWrap: 'wrap', gap: tokens.spacingHorizontalS },
})

/**
 * Configurações › Perfil. Sem conexão, edita o perfil local (o mesmo do
 * onboarding) e oferece "Entrar com Alvanista". Conectado, mostra nome e
 * avatar do Alvanista, só leitura; o perfil local não é tocado e volta ao
 * desconectar.
 */
export function SettingsProfile() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })
  // Primeiro o que está no aparelho (instantâneo); o status completo fala
  // com o Alvanista (até ~15 s) e substitui a tela quando chega.
  const cached = useQuery({
    queryKey: ALVANISTA_CACHED_KEY,
    queryFn: alvanistaCachedStatus,
    retry: false,
  })
  const alva = useQuery({ queryKey: ALVANISTA_STATUS_KEY, queryFn: alvanistaStatus, retry: false })
  const { connect, cancel, connecting, outcome } = useAlvanistaConnect()
  const { disconnect, disconnecting } = useAlvanistaDisconnect()

  const status = alva.data ?? cached.data ?? DISCONNECTED

  useAlvanistaStatusSync(status, alva.data, cached.data)

  if (profile.isLoading || (cached.isLoading && alva.isLoading)) return <LoadingState />

  if (status.connected) {
    return (
      <div className={s.root}>
        <div className={s.identity}>
          {status.avatar_url ? (
            <Avatar
              size={96}
              name={status.username ?? t('alvanista.unknownUser')}
              image={{ src: status.avatar_url }}
            />
          ) : (
            <ProfileAvatar
              profile={{ name: status.username ?? t('alvanista.unknownUser'), avatar: 'preset:1' }}
              size={96}
            />
          )}
          <div className={s.block}>
            <Body1Strong>{status.username ?? t('alvanista.unknownUser')}</Body1Strong>
            {status.offline && (
              <Badge appearance="tint" color="warning">
                {t('alvanista.offlineBadge')}
              </Badge>
            )}
          </div>
        </div>
        <Caption1>{t('alvanista.connectedHint')}</Caption1>
        <div className={s.actions}>
          {/* O editor do perfil no site, como no "Editar no Alvanista" de /perfil. */}
          <Button appearance="secondary" onClick={() => void alvanistaOpenSite('edit').catch(() => {})}>
            {t('alvanista.editOnAlvanista')}
          </Button>
          <Button
            appearance="outline"
            onClick={() => void disconnect()}
            // `disabledFocusable`: o foco (do controle, inclusive) fica no
            // botão enquanto desconecta.
            disabledFocusable={disconnecting}
            icon={disconnecting ? <Spinner size="tiny" /> : undefined}
          >
            {t('alvanista.disconnect')}
          </Button>
        </div>
      </div>
    )
  }

  // Sem banco o Alvanista não tem onde guardar a sessão (o estado falha):
  // só o perfil local.
  const available = !cached.isError

  return (
    <div className={s.root}>
      <Caption1>{t('profile.intro')}</Caption1>
      {available && (
        <div className={s.block}>
          {connecting ? (
            <>
              <div className={s.waiting}>
                <Spinner size="tiny" />
                <Body1Strong>{t('alvanista.waiting')}</Body1Strong>
                <Button appearance="secondary" onClick={cancel}>
                  {t('alvanista.cancel')}
                </Button>
              </div>
              <Caption1>{t('alvanista.waitingHint')}</Caption1>
            </>
          ) : (
            <>
              <AlvanistaButton onClick={connect} />
              <Caption1>{t('alvanista.explain')}</Caption1>
            </>
          )}
          {outcome && (
            <MessageBar intent="warning">
              <MessageBarBody>{t(`alvanista.${outcome}`)}</MessageBarBody>
            </MessageBar>
          )}
        </div>
      )}
      <ProfileForm
        initial={{
          name: profile.data?.name ?? '',
          bio: profile.data?.bio ?? null,
          avatar: profile.data?.avatar ?? 'preset:1',
        }}
        submitLabel={t('common.save')}
        // campos de um lado, avatares do outro (pedido do usuário)
        layout="columns"
        onDone={() => {
          qc.invalidateQueries({ queryKey: ['profile'] })
          push(sysToast(t('profile.updated'), 'Success'))
        }}
      />
    </div>
  )
}
