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
import { useEffect, useState } from 'react'
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
    // Brilho do foco (`::before` do index.css, regra 4) no círculo: lá o raio
    // é o dos botões retangulares e ficava um quadrado em volta do anel
    // redondo (mesmo caso dos botões-pílula da início).
    '::before': { borderRadius: `${tokens.borderRadiusCircular} !important` },
  },
  choiceOn: {
    border: `2px solid ${tokens.colorBrandStroke1}`,
  },
  // Adicionar foto: um círculo como os avatares (raio total — o hover do
  // Fluent desenhava um quadrado; pedido do usuário), com fundo e o ícone
  // maior pra se ler como botão.
  addPhoto: {
    borderRadius: `${tokens.borderRadiusCircular} !important`,
    // o Fluent trava botão só-ícone em 32 de largura (`max-width`): sem
    // isto o círculo virava uma pílula estreita
    maxWidth: 'none !important',
    minWidth: '0 !important',
    backgroundColor: tokens.colorNeutralBackground3,
    ':hover': { backgroundColor: tokens.colorNeutralBackground3Hover },
    ':active': { backgroundColor: tokens.colorNeutralBackground3Pressed },
    '& .fui-Button__icon': { fontSize: '24px', width: '24px', height: '24px' },
  },
  // `layout="columns"` (Configurações › Perfil): os avatares à esquerda e
  // nome, bio e salvar à direita; uma coluna só em janela estreita.
  columns: {
    display: 'grid',
    // duas colunas iguais ocupando a largura toda do card (pedido do
    // usuário)
    gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
    alignItems: 'start',
    columnGap: tokens.spacingHorizontalXXXL,
    rowGap: tokens.spacingVerticalL,
    '@media (max-width: 860px)': { gridTemplateColumns: 'minmax(0, 1fr)' },
  },
  col: { display: 'flex', flexDirection: 'column', gap: tokens.spacingVerticalL, minWidth: 0 },
  // avatar ativo grande e centralizado na coluna, com as opções embaixo,
  // também centralizadas (pedido do usuário)
  avatarCol: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    gap: tokens.spacingVerticalL,
    minWidth: 0,
    textAlign: 'center',
    '& .fui-Field': { justifyItems: 'center' },
    // dica ("5 opções ou uma imagem sua…") mais longe das opções (pedido do
    // usuário)
    '& .fui-Field__hint': { marginTop: tokens.spacingVerticalM },
  },
  choicesCentered: { justifyContent: 'center' },
  // Prévia de 240 (200 em tela baixa, pra coluna caber no card): o `Avatar`
  // do Fluent só aceita tamanhos fixos até 128 (com outro valor cai no
  // padrão), então vai no 128 e a raiz estica — a imagem ocupa a raiz
  // inteira.
  previewLg: {
    width: '240px !important',
    height: '240px !important',
    '& img': { width: '100%', height: '100%' },
    '@media (max-height: 640px)': {
      width: '200px !important',
      height: '200px !important',
    },
  },
  // Bio mais alta nas Configurações (pedido do usuário); continua podendo
  // esticar na vertical
  bioLg: { '& textarea': { minHeight: '140px' } },
  // Opção de avatar maior nas Configurações: 72 (botão de 76); na coluna
  // estreita (interface Maior, ~400 px) 56, pra as 6 caberem numa linha.
  // A imagem acompanha o botão (`& img`), descontada a borda de 2 + folga.
  choiceLg: {
    width: '76px',
    height: '76px',
    '& img': { width: '72px', height: '72px' },
    '@media (max-width: 1200px)': {
      width: '60px',
      height: '60px',
      '& img': { width: '56px', height: '56px' },
    },
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
  showBio = true,
  onChange,
  layout = 'stack',
  inputSize = 'medium',
  className,
}: {
  initial: Pick<Profile, 'name' | 'bio' | 'avatar'>
  /** Sem `submitLabel`, o formulário não tem botão nem salva: só avisa os
   *  valores por `onChange` (o onboarding salva no último passo). */
  submitLabel?: string
  onDone?: () => void
  showBio?: boolean
  onChange?: (v: Pick<Profile, 'name' | 'bio' | 'avatar'>) => void
  /** `stack`: tudo numa coluna. `columns`: avatares de um lado e campos do
   *  outro (Configurações › Perfil e onboarding). */
  layout?: 'stack' | 'columns'
  /** Tamanho do campo de nome (`large` no onboarding, como os botões dele). */
  inputSize?: 'medium' | 'large'
  /** Classe extra na raiz (o onboarding estica o formulário na altura). */
  className?: string
}) {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const push = useToastStore((st) => st.push)
  const [name, setName] = useState(initial.name)
  const [bio, setBio] = useState(initial.bio ?? '')
  const [avatar, setAvatar] = useState(initial.avatar)
  const [nonce, setNonce] = useState(0)
  // o erro de nome só aparece depois que o campo foi mexido (não de cara)
  const [touched, setTouched] = useState(false)
  useEffect(() => {
    onChange?.({ name: name.trim(), bio: bio.trim() || null, avatar })
  }, [name, bio, avatar, onChange])

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
      onDone?.()
    },
    onError: (e) => push(errorToast(e, 'saveProfile')),
  })

  const nameEmpty = name.trim().length === 0
  const nameError = nameEmpty && touched ? t('profileForm.nameRequired') : undefined

  const big = layout === 'columns'
  const choices = (
    <div className={mergeClasses(s.choices, big && s.choicesCentered)}>
      {/* adicionar foto primeiro na lista (pedido do usuário) */}
      <Button
        appearance="subtle"
        className={mergeClasses(s.choice, big && s.choiceLg, s.addPhoto)}
        icon={<ImageAddRegular />}
        disabled={upload.isPending}
        onClick={() => upload.mutate()}
        aria-label={t('profileForm.chooseImage')}
      />
      {PRESET_IDS.map((id) => (
        <Button
          key={id}
          appearance="subtle"
          className={mergeClasses(
            s.choice,
            big && s.choiceLg,
            avatar === `preset:${id}` && s.choiceOn,
          )}
          onClick={() => setAvatar(`preset:${id}`)}
          aria-label={t('profileForm.avatarLabel', { name: t(`profileForm.presets.${id}`) })}
          title={t(`profileForm.presets.${id}`)}
          aria-pressed={avatar === `preset:${id}`}
        >
          <PresetAvatar id={id} size={big ? 72 : 48} style={{ borderRadius: '50%' }} />
        </Button>
      ))}
    </div>
  )

  const fields = (
    <>
      <Field label={t('profileForm.name')} required validationMessage={nameError}>
        <Input
          appearance="filled-darker"
          size={inputSize}
          value={name}
          maxLength={40}
          onChange={(_, d) => {
            setName(d.value)
            setTouched(true)
          }}
          onBlur={() => setTouched(true)}
          placeholder={t('profileForm.namePlaceholder')}
        />
      </Field>

      {showBio && (
        <Field label={t('profileForm.bio')} hint={t('profileForm.bioHint')}>
          <Textarea
            className={big ? s.bioLg : undefined}
            appearance="filled-darker"
            value={bio}
            maxLength={280}
            resize="vertical"
            onChange={(_, d) => setBio(d.value)}
            placeholder={t('profileForm.bioPlaceholder')}
          />
        </Field>
      )}

      {submitLabel && (
        <div>
          <Button
            appearance="primary"
            disabled={nameEmpty || save.isPending}
            onClick={() => save.mutate()}
          >
            {submitLabel}
          </Button>
        </div>
      )}
    </>
  )

  if (layout === 'columns')
    return (
      <div className={mergeClasses(s.columns, className)}>
        <div className={s.avatarCol}>
          <ProfileAvatar
            profile={{ name, avatar }}
            size={128}
            nonce={nonce}
            className={s.previewLg}
          />
          {/* sem o rótulo "Avatar" em cima (pedido do usuário): a lista é
              um grupo com nome acessível, e a dica fica embaixo */}
          <Field hint={t('profileForm.avatarHint')}>
            <div role="group" aria-label={t('profileForm.avatar')}>
              {choices}
            </div>
          </Field>
        </div>
        <div className={s.col}>{fields}</div>
      </div>
    )

  return (
    <div className={mergeClasses(s.root, className)}>
      <div className={s.avatarRow}>
        <ProfileAvatar profile={{ name, avatar }} size={72} nonce={nonce} />
        <Field label={t('profileForm.avatar')} hint={t('profileForm.avatarHint')}>
          {choices}
        </Field>
      </div>
      {fields}
    </div>
  )
}
