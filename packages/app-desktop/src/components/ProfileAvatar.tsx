import { Avatar } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useEffect } from 'react'
import { presetDataUri, presetOf } from '../lib/avatars'
import { profileAvatarUrl, type Profile } from '../lib/tauri'

/**
 * Avatar do perfil — resolve `preset:N` (SVG embutido) ou `file` (imagem do
 * usuário, servida por IPC → `blob:` URL). Envolve o `<Avatar>` do Fluent
 * (badge, borda circular, tamanho). `nonce` fura o cache do blob depois de
 * trocar a imagem.
 */
export function ProfileAvatar({
  profile,
  size = 40,
  nonce = 0,
  badge,
  className,
}: {
  profile: Pick<Profile, 'name' | 'avatar'>
  size?: number
  nonce?: number
  badge?: 'available' | 'away' | 'busy' | 'offline'
  className?: string
}) {
  const preset = presetOf(profile.avatar)
  const isFile = profile.avatar === 'file'

  const file = useQuery({
    queryKey: ['profile-avatar', nonce],
    queryFn: profileAvatarUrl,
    enabled: isFile,
    staleTime: Infinity,
  })

  // libera o object URL quando trocar / desmontar
  useEffect(() => {
    const url = file.data
    return () => {
      if (url) URL.revokeObjectURL(url)
    }
  }, [file.data])

  return (
    <Avatar
      className={className}
      name={profile.name || 'Jogador'}
      size={size as 20 | 24 | 28 | 32 | 36 | 40 | 48 | 56 | 64 | 72 | 96 | 120 | 128}
      color="colorful"
      badge={badge ? { status: badge } : undefined}
      image={
        isFile && file.data
          ? { src: file.data }
          : preset
            ? { src: presetDataUri(preset) }
            : undefined
      }
      aria-label={`Avatar de ${profile.name || 'Jogador'}`}
    />
  )
}
