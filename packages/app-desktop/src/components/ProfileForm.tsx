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
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { PRESET_IDS } from '../lib/avatars'
import { PresetAvatar } from './PresetAvatar'
import { errorToast } from '../lib/toast'
import {
  pickImage,
  setProfile,
  setProfileAvatarFile,
  type Profile,
} from '../lib/tauri'
import { useToastStore } from '../stores/useToastStore'
import { ProfileAvatar } from './ProfileAvatar'
import { useTranslation } from 'react-i18next'

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
    // SEM overflow:hidden aqui — cortava o próprio anel de foco no
    // WebKitGTK (mesmo bug do GameCard). O SVG já se arredonda sozinho
    // (border-radius inline no <PresetAvatar>).
    border: `2px solid transparent`,
  },
  choiceOn: {
    border: `2px solid ${tokens.colorBrandStroke1}`,
  },
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
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const [name, setName] = useState(initial.name)
  const [bio, setBio] = useState(initial.bio ?? '')
  const [avatar, setAvatar] = useState(initial.avatar)
  const [nonce, setNonce] = useState(0)

  const upload = useMutation({
    mutationFn: async () => {
      const path = await pickImage(t('profileForm.pickAvatar'))
      if (!path) return false
      await setProfileAvatarFile(path)
      return true
    },
    onSuccess: (ok) => {
      if (!ok) return
      setAvatar('file')
      setNonce((n) => n + 1)
    },
    onError: (e) => push(errorToast(e, 'loadImage')),
  })

  const save = useMutation({
    mutationFn: () => setProfile(name.trim(), bio.trim() || null, avatar),
    onSuccess: () => {
      // atualiza o cache na hora (o RootLayout usa ['profile'] pro gate de
      // onboarding — sem isto ele redireciona de volta pra cá).
      const next: Profile = {
        name: name.trim(),
        bio: bio.trim() || null,
        avatar,
        onboarded: true,
      }
      qc.setQueryData(['profile'], next)
      qc.invalidateQueries({ queryKey: ['profile'] })
      onDone()
    },
    onError: (e) => push(errorToast(e, 'saveProfile')),
  })

  const nameError = name.trim().length === 0 ? t('profileForm.nameRequired') : undefined

  return (
    <div className={s.root}>
      <div className={s.avatarRow}>
        <ProfileAvatar profile={{ name, avatar }} size={72} nonce={nonce} />
        <Field label={t('profileForm.avatar')} hint={t('profileForm.avatarHint')}>
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
                aria-label={t('profileForm.avatarLabel', { name: t(`profileForm.presets.${id}`) })}
                title={t(`profileForm.presets.${id}`)}
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
              aria-label={t('profileForm.chooseImage')}
            />
          </div>
        </Field>
      </div>

      <Field label={t('profileForm.name')} required validationMessage={nameError}>
        <Input
          value={name}
          maxLength={40}
          onChange={(_, d) => setName(d.value)}
          placeholder={t('profileForm.namePlaceholder')}
        />
      </Field>

      <Field label={t('profileForm.bio')} hint={t('profileForm.bioHint')}>
        <Textarea
          value={bio}
          maxLength={280}
          resize="vertical"
          onChange={(_, d) => setBio(d.value)}
          placeholder={t('profileForm.bioPlaceholder')}
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
