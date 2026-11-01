import { Avatar, makeStyles, mergeClasses } from '@fluentui/react-components'
import { useQuery } from '@tanstack/react-query'
import { useEffect, type CSSProperties } from 'react'
import { presetDataUri, presetOf } from '../lib/avatars'
import { profileAvatarUrl, type AlvanistaTierColors, type Profile } from '../lib/tauri'
import { tierRing } from '../lib/tierRing'

// Anel do plano como o do Alvanista: um disco com o degradê atrás do avatar,
// 7% maior de cada lado, com brilho; no topo do perfil o brilho pulsa.
const useStyles = makeStyles({
  ringed: {
    position: 'relative',
    display: 'inline-flex',
    flexShrink: 0,
    lineHeight: 0,
    verticalAlign: 'middle',
  },
  layer: {
    position: 'absolute',
    borderRadius: '50%',
    pointerEvents: 'none',
  },
  glow: { filter: 'drop-shadow(0 0 6px var(--tier-glow))' },
  pulse: {
    animationName: {
      '0%': { filter: 'drop-shadow(0 0 3px var(--tier-glow))' },
      '50%': { filter: 'drop-shadow(0 0 11px var(--tier-glow))' },
      '100%': { filter: 'drop-shadow(0 0 3px var(--tier-glow))' },
    },
    animationDuration: '2.4s',
    animationTimingFunction: 'ease-in-out',
    animationIterationCount: 'infinite',
    '@media (prefers-reduced-motion: reduce)': { animationName: 'none' },
  },
  // por cima do disco do anel
  above: { position: 'relative' },
})

/**
 * Avatar do perfil — resolve `preset:N` (SVG embutido) ou `file` (imagem do
 * usuário, servida por IPC → `blob:` URL). Envolve o `<Avatar>` do Fluent.
 * `nonce` fura o cache do blob depois de trocar a imagem.
 *
 * `badge` e `ring` ficam desligados por padrão — entram quando tivermos
 * presença/status (rede social).
 *
 * `avatarUrl` (o avatar da conta do Alvanista conectada) passa na frente do
 * `avatar`; se a imagem não carregar, o Fluent mostra as iniciais.
 *
 * `tier` são as cores do plano que o `/me` manda: com elas o avatar ganha o
 * anel do plano. Margem não vai no `className` de um avatar com anel: ela
 * entraria no invólucro e tiraria o anel do lugar.
 */
export function ProfileAvatar({
  profile,
  size = 40,
  nonce = 0,
  badge,
  ring = false,
  tier,
  pulse = false,
  className,
}: {
  profile: Pick<Profile, 'name' | 'avatar'> & { avatarUrl?: string | null }
  size?: number
  nonce?: number
  badge?: 'available' | 'away' | 'busy' | 'offline'
  /** Anel colorido em volta do avatar (Fluent `color="colorful"`). */
  ring?: boolean
  /** Cores do plano do Alvanista; sem cores válidas, sem anel. */
  tier?: AlvanistaTierColors | null
  /** O brilho do anel do plano pulsa (só no topo do perfil). */
  pulse?: boolean
  className?: string
}) {
  const s = useStyles()
  const tierStyle = tierRing(tier)
  const preset = presetOf(profile.avatar)
  const remote = profile.avatarUrl || null
  const isFile = !remote && profile.avatar === 'file'

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

  const avatar = (
    <Avatar
      className={tierStyle ? mergeClasses(className, s.above) : className}
      name={profile.name || 'Jogador'}
      size={size as 20 | 24 | 28 | 32 | 36 | 40 | 48 | 56 | 64 | 72 | 96 | 120 | 128}
      color={ring ? 'colorful' : 'neutral'}
      badge={badge ? { status: badge } : undefined}
      image={
        remote
          ? { src: remote }
          : isFile && file.data
          ? { src: file.data }
          : preset
            ? { src: presetDataUri(preset) }
            : undefined
      }
      aria-label={`Avatar de ${profile.name || 'Jogador'}`}
    />
  )
  if (!tierStyle) return avatar

  const layerStyle = {
    inset: `-${size * 0.07}px`,
    background: tierStyle.background,
    ...(tierStyle.glow ? { '--tier-glow': tierStyle.glow } : {}),
  } as CSSProperties
  return (
    <span className={s.ringed} data-testid="tier-ring" data-pulse={pulse ? '' : undefined}>
      <span
        className={mergeClasses(s.layer, tierStyle.glow ? s.glow : undefined, tierStyle.glow && pulse ? s.pulse : undefined)}
        style={layerStyle}
        data-tier-ring-layer=""
        aria-hidden
      />
      {avatar}
    </span>
  )
}
