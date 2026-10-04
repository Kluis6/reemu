import {
  Body1,
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Radio,
  RadioGroup,
  Subtitle2,
  makeStyles,
  mergeClasses,
  tokens,
  type RadioGroupOnChangeData,
} from '@fluentui/react-components'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useState } from 'react'
import { Navigate, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { AnimatedBackground } from '../components/AnimatedBackground'
import { AppLogo } from '../components/AppLogo'
import { LoadingState } from '../components/EmptyState'
import { ProfileAvatar } from '../components/ProfileAvatar'
import { ProfileForm } from '../components/ProfileForm'
import {
  getLanguagePreference,
  LANGUAGES,
  setLanguagePreference,
  type LanguagePreference,
} from '../i18n'
import { getProfile, setProfile, type Profile } from '../lib/tauri'
import { errorToast } from '../lib/toast'
import { useToastStore } from '../stores/useToastStore'

/** Tempo entre fechar um passo e abrir o próximo (a saída do Dialog). */
const SWAP_MS = 320

/** "Bem-vindo" em vários idiomas, na ordem em que aparecem. */
const WELCOMES = [
  'Bem-vindo',
  'Welcome',
  'Bienvenido',
  'Bienvenue',
  'Willkommen',
  'Benvenuto',
  'ようこそ',
  'Добро пожаловать',
  '환영합니다',
  '欢迎',
  'Welkom',
  'Witamy',
  'Välkommen',
  'Hoş geldiniz',
  'Καλώς ήρθατε',
  'Tervetuloa',
]

const wordIn = {
  from: { opacity: 0, transform: 'translateY(10px)' },
  to: { opacity: 1, transform: 'none' },
}
const bgIn = { from: { opacity: 0 }, to: { opacity: 1 } }
const noMotion = { '@media (prefers-reduced-motion: reduce)': { animationName: 'none' } }

const useStyles = makeStyles({
  bg: {
    position: 'fixed',
    inset: 0,
    animationName: bgIn,
    animationDuration: '1100ms',
    animationTimingFunction: tokens.curveEasyEase,
    animationFillMode: 'backwards',
    ...noMotion,
  },
  // Fundo do tema visível atrás do modal (sem escurecer).
  backdrop: { backgroundColor: 'transparent' },
  // Modal grande, quase a tela toda — o mesmo tamanho nos três passos.
  surface: {
    width: 'min(960px, 92vw)',
    maxWidth: 'none',
    height: 'min(620px, 86vh)',
    boxSizing: 'border-box',
    display: 'flex',
  },
  body: { flexGrow: 1, gridTemplateRows: 'auto 1fr auto' },
  content: {
    display: 'flex',
    flexDirection: 'column',
    gap: tokens.spacingVerticalXL,
    overflowY: 'auto',
  },
  // nuvem de "bem-vindo": cada palavra entra com fade, uma depois da outra
  words: {
    display: 'flex',
    flexWrap: 'wrap',
    justifyContent: 'center',
    alignItems: 'baseline',
    columnGap: tokens.spacingHorizontalXXL,
    rowGap: tokens.spacingVerticalM,
    padding: `${tokens.spacingVerticalL} 0`,
  },
  word: {
    fontWeight: tokens.fontWeightSemibold,
    color: tokens.colorNeutralForeground1,
    lineHeight: 1.2,
    animationName: wordIn,
    animationDuration: '700ms',
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: 'backwards',
    ...noMotion,
  },
  wordMain: { color: tokens.colorBrandForeground1 },
  center: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    gap: tokens.spacingVerticalM,
    textAlign: 'center',
  },
  ready: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: tokens.spacingVerticalL,
    textAlign: 'center',
    flexGrow: 1,
  },
  muted: { color: tokens.colorNeutralForeground3, maxWidth: '520px' },
})

type Step = 'welcome' | 'profile' | 'ready'
type Draft = Pick<Profile, 'name' | 'bio' | 'avatar'>

/**
 * Primeira execução, em três modais do mesmo tamanho sobre o fundo do tema:
 * boas-vindas (várias línguas + escolha do idioma), nome e avatar, e "tudo
 * pronto". Seguindo o Dialog do Fluent 2: título em cada passo, ações no
 * rodapé, um modal fecha antes do próximo abrir (sem aninhar) e `alert`
 * (não fecha com Esc nem clique fora — é um fluxo obrigatório). O perfil só
 * é salvo no último passo: salvar marca `onboarded` e o app abre.
 */
export function Onboarding() {
  const { t } = useTranslation()
  const s = useStyles()
  const qc = useQueryClient()
  const navigate = useNavigate()
  const push = useToastStore((st) => st.push)
  const profile = useQuery({ queryKey: ['profile'], queryFn: getProfile, retry: false })

  const [step, setStep] = useState<Step>('welcome')
  const [open, setOpen] = useState(true)
  const [lang, setLang] = useState<LanguagePreference>(getLanguagePreference)
  const [draft, setDraft] = useState<Draft | null>(null)
  const onDraft = useCallback((v: Draft) => setDraft(v), [])

  // Fecha o passo atual e abre o próximo depois da animação de saída.
  const go = (next: Step) => {
    setOpen(false)
    window.setTimeout(() => {
      setStep(next)
      setOpen(true)
    }, SWAP_MS)
  }

  const finish = useMutation({
    mutationFn: (d: Draft) => setProfile(d.name, d.bio, d.avatar),
    onSuccess: (_, d) => {
      setOpen(false)
      window.setTimeout(() => {
        qc.setQueryData(['profile'], { ...d, onboarded: true })
        qc.invalidateQueries({ queryKey: ['profile'] })
        navigate('/', { replace: true })
      }, SWAP_MS)
    },
    onError: (e) => push(errorToast(e, 'saveProfile')),
  })

  if (profile.isLoading) return <LoadingState />
  // já passou pelo onboarding, ou sem backend pra persistir → manda pra Home
  if (profile.data?.onboarded || profile.isError) return <Navigate to="/" replace />

  const initial: Draft = draft ?? {
    name: profile.data?.name ?? '',
    bio: profile.data?.bio ?? null,
    avatar: profile.data?.avatar ?? 'preset:1',
  }
  const nameOk = (draft?.name ?? initial.name).trim().length > 0

  return (
    <>
      <div className={s.bg}>
        <AnimatedBackground />
      </div>
      <Dialog open={open} modalType="alert">
        <DialogSurface className={s.surface} backdrop={{ className: s.backdrop }}>
          <DialogBody className={s.body}>
            {step === 'welcome' && (
              <>
                <DialogTitle>{t('onboarding.welcomeTitle')}</DialogTitle>
                <DialogContent className={s.content}>
                  <div className={s.center}>
                    <AppLogo height={64} />
                  </div>
                  <div className={s.words} aria-hidden>
                    {WELCOMES.map((w, i) => (
                      <span
                        key={w}
                        className={mergeClasses(s.word, i === 0 && s.wordMain)}
                        style={{
                          animationDelay: `${250 + i * 220}ms`,
                          fontSize: `${i === 0 ? 40 : [28, 22, 26, 20, 24][i % 5]}px`,
                        }}
                      >
                        {w}
                      </span>
                    ))}
                  </div>
                  <div className={s.center}>
                    <Subtitle2>{t('language.title')}</Subtitle2>
                    <RadioGroup
                      layout="horizontal"
                      aria-label={t('language.title')}
                      value={lang}
                      onChange={(_, data: RadioGroupOnChangeData) => {
                        const v = data.value as LanguagePreference
                        setLang(v)
                        void setLanguagePreference(v)
                      }}
                    >
                      <Radio value="auto" label={t('language.auto')} />
                      {LANGUAGES.map((l) => (
                        // cada idioma no próprio nome (quem não lê o atual acha o seu)
                        <Radio key={l} value={l} label={t(`language.${l}`)} />
                      ))}
                    </RadioGroup>
                  </div>
                </DialogContent>
                <DialogActions>
                  <Button appearance="primary" onClick={() => go('profile')}>
                    {t('onboarding.continue')}
                  </Button>
                </DialogActions>
              </>
            )}

            {step === 'profile' && (
              <>
                <DialogTitle>{t('onboarding.profileTitle')}</DialogTitle>
                <DialogContent className={s.content}>
                  <Body1 className={s.muted}>{t('onboarding.profileText')}</Body1>
                  <ProfileForm initial={initial} showBio={false} onChange={onDraft} />
                </DialogContent>
                <DialogActions>
                  <Button appearance="secondary" onClick={() => go('welcome')}>
                    {t('common.back')}
                  </Button>
                  <Button appearance="primary" disabled={!nameOk} onClick={() => go('ready')}>
                    {t('onboarding.continue')}
                  </Button>
                </DialogActions>
              </>
            )}

            {step === 'ready' && (
              <>
                <DialogTitle>
                  {t('onboarding.readyTitle', { name: draft?.name ?? initial.name })}
                </DialogTitle>
                <DialogContent className={s.content}>
                  <div className={s.ready}>
                    <ProfileAvatar
                      profile={{ name: draft?.name ?? initial.name, avatar: draft?.avatar ?? initial.avatar }}
                      size={96}
                    />
                    <Body1 className={s.muted}>{t('onboarding.readyText')}</Body1>
                  </div>
                </DialogContent>
                <DialogActions>
                  <Button appearance="secondary" onClick={() => go('profile')}>
                    {t('common.back')}
                  </Button>
                  <Button
                    appearance="primary"
                    disabled={finish.isPending}
                    onClick={() => finish.mutate(draft ?? initial)}
                  >
                    {t('shell.start')}
                  </Button>
                </DialogActions>
              </>
            )}
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </>
  )
}
