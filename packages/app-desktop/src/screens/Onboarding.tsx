import { Body1, makeStyles, tokens } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { Navigate, useNavigate } from 'react-router-dom'
import { AnimatedBackground } from '../components/AnimatedBackground'
import { AppLogo } from '../components/AppLogo'
import { LoadingState } from '../components/EmptyState'
import { ProfileForm } from '../components/ProfileForm'
import { getProfile } from '../lib/tauri'

const useStyles = makeStyles({
  root: {
    position: 'fixed',
    inset: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: tokens.spacingHorizontalXXL,
    overflowY: 'auto',
  },
  card: {
    position: 'relative',
    zIndex: 1,
    width: '100%',
    maxWidth: '480px',
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
    backgroundColor: tokens.colorNeutralBackground1,
    borderRadius: tokens.borderRadiusXLarge,
    padding: tokens.spacingHorizontalXXL,
    boxShadow: tokens.shadow28,
  },
  head: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalXS },
})

/**
 * Fluxo de 1ª execução — define nome, bio e avatar do perfil local (um por
 * instalação). Fica FORA da `AppShell` (sem rail/topbar). Ao concluir,
 * `setProfile` marca `onboarded` e a `/` passa a abrir normalmente.
 */
export function Onboarding() {
  const s = useStyles()
  const navigate = useNavigate()
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })

  if (profile.isLoading) return <LoadingState />
  // já passou pelo onboarding, ou sem backend pra persistir → manda pra Home
  if (profile.data?.onboarded || profile.isError) return <Navigate to="/" replace />

  return (
    <div className={s.root}>
      <AnimatedBackground />
      <div className={s.card}>
        <div className={s.head}>
          <AppLogo height={132} />
          <Body1>
            Bem-vindo — vamos criar seu perfil. Ele fica só neste computador;
            depois dá pra ligar a uma rede social.
          </Body1>
        </div>
        <ProfileForm
          initial={{
            name: profile.data?.name ?? '',
            bio: profile.data?.bio ?? null,
            avatar: profile.data?.avatar ?? 'preset:1',
          }}
          submitLabel="Começar"
          onDone={() => navigate('/', { replace: true })}
        />
      </div>
    </div>
  )
}
