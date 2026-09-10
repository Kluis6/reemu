import {
  Button,
  Field,
  Input,
  Textarea,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { ImageAddRegular } from '@fluentui/react-icons'
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { PRESET_IDS } from '../lib/avatars'
import { PresetAvatar } from './PresetAvatar'
import { sysToast } from '../lib/toast'
import {
  pickImage,
  setProfile,
  setProfileAvatarFile,
  type Profile,
} from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'
import { ProfileAvatar } from './ProfileAvatar'

const useStyles = makeStyles({
  root: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalL,
    maxWidth: '440px',
  },
  avatarRow: {
    display: 'flex',
    alignItems: 'center',
    gap: tokens.spacingHorizontalL,
  },
  choices: {
    display: 'flex',
    flexWrap: 'wrap',
    gap: tokens.spacingHorizontalS,
  },
  choice: {
    padding: 0,
    minWidth: 'auto',
    width: '52px',
    height: '52px',
    borderRadius: tokens.borderRadiusCircular,
    overflow: 'hidden',
    border: `2px solid transparent`,
  },
  choiceOn: {
    border: `2px solid ${tokens.colorBrandStroke1}`,
  },
  choiceSvg: { width: '100%', height: '100%', borderRadius: '50%' },
})

/**
 * Formulário do perfil, reusado pelo onboarding e por Configurações › Perfil.
 * `onDone` roda depois de salvar (navegar / fechar). `nonceOnAvatarChange`
 * incrementa quando o usuário troca a imagem — pra `ProfileAvatar` refurar o
 * cache do `blob:`.
 */
export function ProfileForm({
  initial,
  submitLabel,
  onDone,
}: {
  initial: Pick<Profile, 'name' | 'bio' | 'avatar'>
  submitLabel: string
  onDone: () => void
}) {
  const s = useStyles()
  const push = useToastStore((t) => t.push)
  const [name, setName] = useState(initial.name)
  const [bio, setBio] = useState(initial.bio ?? '')
  const [avatar, setAvatar] = useState(initial.avatar)
  const [nonce, setNonce] = useState(0)

  const upload = useMutation({
    mutationFn: async () => {
      const path = await pickImage()
      if (!path) return false
      await setProfileAvatarFile(path)
      return true
    },
    onSuccess: (ok) => {
      if (!ok) return
      setAvatar('file')
      setNonce((n) => n + 1)
    },
    onError: (e) => push(sysToast(`Falha ao carregar imagem: ${e}`, 'Error')),
  })

  const save = useMutation({
    mutationFn: () => setProfile(name.trim(), bio.trim() || null, avatar),
    onSuccess: onDone,
    onError: (e) => push(sysToast(`Falha ao salvar: ${e}`, 'Error')),
  })

  const nameError = name.trim().length === 0 ? 'Escolha um nome.' : undefined

  return (
    <div className={s.root}>
      <div className={s.avatarRow}>
        <ProfileAvatar profile={{ name, avatar }} size={72} nonce={nonce} />
        <Field label="Avatar" hint="5 opções ou uma imagem sua (PNG/JPG/WEBP).">
          <div className={s.choices}>
            {PRESET_IDS.map((id) => (
              <Button
                key={id}
                appearance="subtle"
                className={mergeClasses(
                  s.choice,
                  avatar === `preset:${id}` && s.choiceOn,
                )}
                onClick={() => setAvatar(`preset:${id}`)}
                aria-label={`Avatar ${id}`}
                aria-pressed={avatar === `preset:${id}`}
              >
                <PresetAvatar id={id} size={48} style={{ borderRadius: '50%' }} />
              </Button>
            ))}
            <Button
              appearance="subtle"
              className={s.choice}
              icon={<ImageAddRegular />}
              disabled={upload.isPending}
              onClick={() => upload.mutate()}
              aria-label="Escolher imagem"
            />
          </div>
        </Field>
      </div>

      <Field label="Nome" required validationMessage={nameError}>
        <Input
          value={name}
          maxLength={40}
          onChange={(_, d) => setName(d.value)}
          placeholder="Como você quer aparecer"
        />
      </Field>

      <Field label="Bio" hint="Opcional — até 280 caracteres.">
        <Textarea
          value={bio}
          maxLength={280}
          resize="vertical"
          onChange={(_, d) => setBio(d.value)}
          placeholder="Uma linha sobre você"
        />
      </Field>

      <div>
        <Button
          appearance="primary"
          disabled={!!nameError || save.isPending}
          onClick={() => save.mutate()}
        >
          {submitLabel}
        </Button>
      </div>
    </div>
  )
}
