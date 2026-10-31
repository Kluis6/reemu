import {
  Body1,
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Portal,
  Subtitle2,
  makeStyles,
  mergeClasses,
  tokens,
} from '@fluentui/react-components'
import { CheckmarkRegular } from '@fluentui/react-icons'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Navigate, useNavigate } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { AnimatedBackground } from '../components/AnimatedBackground'
import { ButtonHints, type Hint } from '../components/ButtonHints'
import { REEMU_ONBOARDING_BACK } from '../hooks/useMenuNav'
import { useSearchStore } from '../stores/useSearchStore'
import { useModalButtonStyles } from '../styles/buttons'
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
    width: 'min(1200px, 92vw)',
    maxWidth: 'none',
    // 82vh: a barra de dicas do controle (centro, embaixo) cabe abaixo do
    // modal sem encostar nele
    height: 'min(760px, 82vh)',
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
  // Boas-vindas em duas colunas (pedido do usuário): idiomas à esquerda,
  // logo + nuvem de "bem-vindo" à direita (mais larga, pras palavras).
  welcome: {
    display: 'grid',
    gridTemplateColumns: 'minmax(240px, 2fr) 3fr',
    // o dobro do maior token (XXXL = 32): as colunas respiram (pedido do
    // usuário)
    columnGap: `calc(${tokens.spacingHorizontalXXXL} * 2)`,
    flexGrow: 1,
    minHeight: 0,
  },
  langs: {
    display: 'flex',
    flexDirection: 'column',
    justifyContent: 'center',
    rowGap: tokens.spacingVerticalM,
  },
  // "Escolha o idioma" mais afastado dos botões que da lista entre si
  langTitle: { marginBottom: tokens.spacingVerticalL },
  // botões grandes na largura da coluna, texto à esquerda como numa lista
  langBtn: { width: '100%', justifyContent: 'flex-start' },
  // "Bem-vindo" no idioma escolhido, em destaque no lugar do logo; entra de
  // novo (fade) a cada troca de idioma
  helloWord: {
    fontFamily: 'var(--reemuFontDisplay)',
    fontSize: tokens.fontSizeHero1000,
    lineHeight: tokens.lineHeightHero1000,
    fontWeight: tokens.fontWeightSemibold,
    color: tokens.colorBrandForeground1,
    textAlign: 'center',
    animationName: wordIn,
    animationDuration: '500ms',
    animationTimingFunction: tokens.curveDecelerateMid,
    ...noMotion,
  },
  hello: {
    display: 'flex',
    flexDirection: 'column',
    justifyContent: 'center',
    alignItems: 'center',
    rowGap: tokens.spacingVerticalL,
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
  // Final, no modal: o avatar escolhido cresce e aparece, depois "Bem-vindo,
  // fulano!" grande e a frase de baixo, uma depois da outra; o botão
  // "Tudo pronto para começar" fica no centro, embaixo (pedido do usuário).
  done: {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    rowGap: tokens.spacingVerticalXXL,
    flexGrow: 1,
    textAlign: 'center',
  },
  // o `DialogActions` se alinha ao fim da grade do `DialogBody`
  // (`justify-self`): aqui ocupa a linha toda e centraliza o botão
  doneActions: { gridColumn: '1 / -1', justifySelf: 'center' },
  // o título do Dialog (nome acessível) fica só pro leitor de tela: o
  // "Bem-vindo, fulano!" visível é o grande do meio
  srOnly: {
    position: 'absolute',
    width: '1px',
    height: '1px',
    overflow: 'hidden',
    clipPath: 'inset(50%)',
    whiteSpace: 'nowrap',
  },
  // anel da cor primária com o brilho do foco do app em volta do avatar
  doneAvatar: {
    width: '220px !important',
    height: '220px !important',
    boxShadow: `0 0 0 4px ${tokens.colorBrandStroke1}, 0 0 48px color-mix(in srgb, ${tokens.colorBrandStroke1} 55%, transparent)`,
    '& img': { width: '100%', height: '100%' },
    animationName: {
      from: { opacity: 0, transform: 'scale(0.6)' },
      to: { opacity: 1, transform: 'none' },
    },
    animationDuration: '800ms',
    animationTimingFunction: tokens.curveDecelerateMax,
    animationFillMode: 'backwards',
    ...noMotion,
  },
  doneTitle: {
    fontFamily: 'var(--reemuFontDisplay)',
    fontSize: tokens.fontSizeHero1000,
    lineHeight: tokens.lineHeightHero1000,
    fontWeight: tokens.fontWeightSemibold,
    color: tokens.colorBrandForeground1,
    overflowWrap: 'anywhere',
    animationName: wordIn,
    animationDuration: '700ms',
    animationDelay: '450ms',
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: 'backwards',
    ...noMotion,
  },
  doneText: {
    color: tokens.colorNeutralForeground2,
    fontSize: tokens.fontSizeBase500,
    lineHeight: tokens.lineHeightBase500,
    animationName: wordIn,
    animationDuration: '700ms',
    animationDelay: '850ms',
    animationTimingFunction: tokens.curveDecelerateMid,
    animationFillMode: 'backwards',
    ...noMotion,
  },
  muted: { color: tokens.colorNeutralForeground3, maxWidth: '520px' },
  hintsLayer: { zIndex: '1000001 !important' },
  // Perfil: o formulário ocupa a altura livre e a coluna dos avatares fica
  // no meio dela (a linha do grid estica até a altura; o nome continua no
  // topo da coluna dele) — pedido do usuário.
  profileForm: {
    flexGrow: 1,
    '& > :first-child': { alignSelf: 'center' },
  },
})

type Step = 'welcome' | 'profile' | 'done'
type Draft = Pick<Profile, 'name' | 'bio' | 'avatar'>

/**
 * Primeira execução, em três modais do mesmo tamanho sobre o fundo do tema:
 * boas-vindas (várias línguas + escolha do idioma), nome e avatar, e a
 * animação de boas-vindas com o avatar e o nome, com o botão "Tudo pronto
 * para começar". Seguindo o Dialog do Fluent 2: título em cada passo, ações
 * no rodapé, um modal fecha antes do próximo abrir (sem aninhar) e `alert`
 * (não fecha com Esc nem clique fora — é um fluxo obrigatório). O perfil é
 * salvo ao continuar no passo do nome (marca `onboarded`); o cache só é
 * atualizado no botão final — antes disso o `RootLayout` já tiraria do
 * onboarding.
 */
export function Onboarding() {
  const { t } = useTranslation()
  const s = useStyles()
  const btn = useModalButtonStyles()
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

  // B do controle (`useMenuNav`): volta um passo — o mesmo que o botão Voltar.
  const goRef = useRef(go)
  const stepRef = useRef(step)
  useEffect(() => {
    goRef.current = go
    stepRef.current = step
  })
  useEffect(() => {
    const back = () => {
      const prev: Record<Step, Step | null> = { welcome: null, profile: 'welcome', done: null }
      const to = prev[stepRef.current]
      if (to) goRef.current(to)
    }
    window.addEventListener(REEMU_ONBOARDING_BACK, back)
    return () => window.removeEventListener(REEMU_ONBOARDING_BACK, back)
  }, [])

  // Dicas do controle: A em todos os passos, B onde dá pra voltar. Somem com
  // o teclado na tela aberto (ele mostra os glifos dele).
  const keyboard = useSearchStore((st) => st.keyboard)
  const hints: Hint[] = [
    { glyph: 'A', label: t('hints.select') },
    ...(step === 'profile' ? [{ glyph: 'B' as const, label: t('hints.back') }] : []),
  ]

  const finish = useMutation({
    mutationFn: (d: Draft) => setProfile(d.name, d.bio, d.avatar),
    onSuccess: () => go('done'),
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

  // "Tudo pronto para começar": fecha o modal e abre a página inicial.
  const start = () => {
    const d = draft ?? initial
    setOpen(false)
    window.setTimeout(() => {
      qc.setQueryData(['profile'], { ...d, onboarded: true })
      qc.invalidateQueries({ queryKey: ['profile'] })
      navigate('/', { replace: true })
    }, SWAP_MS)
  }

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
                <DialogContent className={mergeClasses(s.content, s.welcome)}>
                  <div className={s.langs}>
                    <Subtitle2 id="onboarding-lang" className={s.langTitle}>
                      {t('onboarding.chooseLanguage')}
                    </Subtitle2>
                    {/* Escolha única em botões grandes: o grupo é um
                        `radiogroup` e cada botão um `radio` (leitor de tela);
                        o escolhido fica em destaque, com o check. */}
                    <div
                      role="radiogroup"
                      aria-labelledby="onboarding-lang"
                      style={{ display: 'contents' }}
                    >
                      {(['auto', ...LANGUAGES] as LanguagePreference[]).map((l) => (
                        <Button
                          key={l}
                          size="large"
                          role="radio"
                          aria-checked={lang === l}
                          appearance={lang === l ? 'primary' : 'secondary'}
                          icon={lang === l ? <CheckmarkRegular /> : undefined}
                          iconPosition="after"
                          // não escolhido: o secundário preenchido dos modais
                          className={mergeClasses(s.langBtn, lang !== l && btn.secondary)}
                          onClick={() => {
                            setLang(l)
                            void setLanguagePreference(l)
                          }}
                        >
                          {/* cada idioma no próprio nome (quem não lê o atual acha o seu) */}
                          {t(l === 'auto' ? 'language.auto' : `language.${l}`)}
                        </Button>
                      ))}
                    </div>
                  </div>
                  <div className={s.hello}>
                    <div key={lang} className={s.helloWord}>
                      {t('onboarding.hello')}
                    </div>
                    <div className={s.words} aria-hidden>
                    {WELCOMES.map((w, i) => (
                      <span
                        key={w}
                        className={s.word}
                        style={{
                          animationDelay: `${250 + i * 220}ms`,
                          fontSize: `${[28, 22, 26, 20, 24][i % 5]}px`,
                        }}
                      >
                        {w}
                      </span>
                    ))}
                    </div>
                  </div>
                </DialogContent>
                <DialogActions>
                  <Button appearance="primary" size="large" onClick={() => go('profile')}>
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
                  {/* como em Configurações › Perfil (avatar grande | campos),
                      sem a bio (pedido do usuário) */}
                  <ProfileForm
                    initial={initial}
                    showBio={false}
                    layout="columns"
                    inputSize="large"
                    className={s.profileForm}
                    onChange={onDraft}
                  />
                </DialogContent>
                <DialogActions>
                  <Button
                    appearance="secondary"
                    size="large"
                    className={btn.secondary}
                    onClick={() => go('welcome')}
                  >
                    {t('common.back')}
                  </Button>
                  <Button
                    appearance="primary"
                    size="large"
                    disabled={!nameOk || finish.isPending}
                    onClick={() => finish.mutate(draft ?? initial)}
                  >
                    {t('onboarding.continue')}
                  </Button>
                </DialogActions>
              </>
            )}

            {step === 'done' && (
              <>
                <DialogTitle className={s.srOnly}>
                  {t('onboarding.welcomeUser', { name: (draft?.name ?? initial.name).trim() })}
                </DialogTitle>
                <DialogContent className={mergeClasses(s.content, s.done)}>
                  <ProfileAvatar
                    profile={{ name: draft?.name ?? initial.name, avatar: draft?.avatar ?? initial.avatar }}
                    size={128}
                    className={s.doneAvatar}
                  />
                  <div className={s.doneTitle} aria-hidden>
                    {t('onboarding.welcomeUser', { name: (draft?.name ?? initial.name).trim() })}
                  </div>
                  <div className={s.doneText}>{t('onboarding.readyText')}</div>
                </DialogContent>
                <DialogActions className={s.doneActions}>
                  <Button appearance="primary" size="large" onClick={start}>
                    {t('onboarding.allSet')}
                  </Button>
                </DialogActions>
              </>
            )}
          </DialogBody>
        </DialogSurface>
      </Dialog>
      {/* Portal próprio, uma camada acima do Dialog (o nó do portal do Fluent
          é `z-index: 1000000`): dentro do DialogSurface o `position: fixed`
          ficava preso ao modal, e fora de um portal o modal cobria a barra. */}
      {!keyboard && (
        <Portal mountNode={{ className: s.hintsLayer }}>
          <ButtonHints hints={hints} centered />
        </Portal>
      )}
    </>
  )
}
