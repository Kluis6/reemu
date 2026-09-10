import { Caption1, makeStyles, tokens } from '@fluentui/react-components'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { LoadingState } from '../../components/EmptyState'
import { ProfileForm } from '../../components/ProfileForm'
import { sysToast } from '../../lib/toast'
import { getProfile } from '../../lib/tauri'
import { useToastStore } from '../../stores/useToastStore'

const useStyles = makeStyles({
  root: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalM },
})

/** Configurações › Perfil — edita o mesmo perfil local do onboarding. */
export function SettingsProfile() {
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((t) => t.push)
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })

  if (profile.isLoading) return <LoadingState />

  return (
    <div className={s.root}>
      <Caption1>
        Seu perfil local — um por instalação. No futuro dá pra ligar a uma rede
        social.
      </Caption1>
      <ProfileForm
        initial={{
          name: profile.data?.name ?? '',
          bio: profile.data?.bio ?? null,
          avatar: profile.data?.avatar ?? 'preset:1',
        }}
        submitLabel="Salvar"
        onDone={() => {
          qc.invalidateQueries({ queryKey: ['profile'] })
          push(sysToast('Perfil atualizado.', 'Success'))
        }}
      />
    </div>
  )
}
